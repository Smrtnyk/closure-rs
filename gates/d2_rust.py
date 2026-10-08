#!/usr/bin/env python3
"""D2 differential runner and ratchet for the Rust CLI (docs/PORTING.md §3 D2/D5, §4.4).

  python3 gates/d2_rust.py run --bin PATH [--profile P]... [--source S]... [--case-regex R]
                               [--sample N [--seed K]] [--limit N] [--jobs N] [--timeout S]
                               [--out DIR] [--data-root DIR] [--keep-failing]
                               [--baseline RATCHET.json]
  python3 gates/d2_rust.py check --baseline OLD.json --current NEW.json
  python3 gates/d2_rust.py selftest [--java-sample N] [--seed K] [--jobs N] [--out DIR]

Exit codes: 0 ok; 1 ratchet regression / self-test failure; 2 harness error or ratchets not
comparable.  See gates/README_d2_rust.md.
"""

from __future__ import annotations

import argparse
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib"))
import d2_rust_core as core  # noqa: E402

LIB = os.path.join(os.path.dirname(os.path.abspath(__file__)), "lib")
FAKE_JAVA = os.path.join(LIB, "d2_rust_fake_java.py")
FAKE_EMPTY = os.path.join(LIB, "d2_rust_fake_empty.py")


def _filter_from(a) -> dict:
    only = core.load_json(a.only_passing)["passing"] if getattr(a, "only_passing", None) else None
    return {"only_passing": only, "profiles": a.profile, "sources": a.source, "case_regex": a.case_regex,
            "sample": a.sample, "seed": a.seed, "limit": a.limit}


def _print_summary(rep: dict) -> None:
    print("\n".join(core.summary_lines(rep)))
    print(f"report: {os.path.join(rep['meta']['out'], 'report.md')}")
    print(f"ratchet: {os.path.join(rep['meta']['out'], 'ratchet.json')}")


def cmd_run(a) -> int:
    try:
        rep = core.run(a.bin, flt=_filter_from(a), jobs=a.jobs, timeout_s=a.timeout,
                       out=a.out, data_root=a.data_root, keep_failing=a.keep_failing)
    except core.HarnessError as e:
        print(f"d2_rust: harness error: {e}", file=sys.stderr)
        return 2
    _print_summary(rep)
    if "harness_error" in rep["causes"]:
        print("d2_rust: harness errors (see report.md)", file=sys.stderr)
        return 2
    if a.only_passing:
        code, msgs = core.check_passing_set(core.load_json(a.only_passing),
                                            core.load_json(os.path.join(rep["meta"]["out"], "ratchet.json")))
        print("\n".join(msgs))
        return code
    if a.baseline:
        code, msgs = core.check_ratchet(core.load_json(a.baseline),
                                        core.load_json(os.path.join(rep["meta"]["out"], "ratchet.json")))
        print("\n".join(msgs))
        return code
    return 0


def cmd_check(a) -> int:
    code, msgs = core.check_ratchet(core.load_json(a.baseline), core.load_json(a.current))
    print("\n".join(msgs))
    return code


def cmd_selftest(a) -> int:
    out = os.path.abspath(a.out or core.default_out("selftest-"))
    failures: list[str] = []

    def expect(cond: bool, what: str) -> None:
        print(f"selftest: {'ok  ' if cond else 'FAIL'} {what}")
        if not cond:
            failures.append(what)

    all_pairs, _profiles = core.load_pairs()
    n_all = len(all_pairs)

    # (b) the Java jar behind a fake CLI: must reproduce every golden result.  --jobs 1: at
    # most one -Xmx3g JVM at a time.
    java_flt = {"sample": a.java_sample, "seed": a.seed, "min_per_group": 2}
    sample = core.apply_filter(all_pairs, java_flt)
    strata = {(p["profile"], p["source"]) for p in all_pairs}
    per = {}
    for p in sample:
        per[(p["profile"], p["source"])] = per.get((p["profile"], p["source"]), 0) + 1
    expect(all(per.get(k, 0) >= 2 for k in strata),
           f"java sample covers every (profile, source) with >= 2 pairs ({len(sample)} pairs, "
           f"{len(strata)} combinations)")
    expect(any(core.has_module_flags(p["case"]) for p in sample),
           "java sample includes module-resolution / CommonJS cases")
    java = core.run(FAKE_JAVA, flt=java_flt, jobs=1, timeout_s=a.timeout,
                    out=os.path.join(out, "java"), keep_failing=True)
    print("\n".join(core.summary_lines(java)))
    expect(java["total"]["pairs"] == len(sample) and java["total"]["pass"] == len(sample),
           f"java fake: {java['total']['pass']} / {java['total']['pairs']} pass (want 100%)")

    # (a) a binary that prints nothing: 0% on the complete corpus.
    empty = core.run(FAKE_EMPTY, jobs=a.jobs, timeout_s=a.timeout, out=os.path.join(out, "empty"))
    print("\n".join(core.summary_lines(empty)))
    expect(empty["total"]["pairs"] == n_all and empty["total"]["pass"] == 0,
           f"empty fake: {empty['total']['pass']} / {empty['total']['pairs']} pass on the complete "
           f"corpus of {n_all} (want 0)")
    expect("harness_error" not in empty["causes"] and "harness_error" not in java["causes"],
           "no harness errors")

    # (c) ratchet semantics.
    empty_s = core.run(FAKE_EMPTY, flt=java_flt, jobs=a.jobs, timeout_s=a.timeout,
                       out=os.path.join(out, "empty-sample"), quiet=True)
    jr = core.load_json(os.path.join(out, "java", "ratchet.json"))
    er = core.load_json(os.path.join(out, "empty-sample", "ratchet.json"))
    ec = core.load_json(os.path.join(out, "empty", "ratchet.json"))
    expect(empty_s["total"]["pass"] == 0, "empty fake on the java sample: 0 pass")
    expect(core.check_ratchet(jr, jr)[0] == 0, "ratchet: java sample vs itself = no regression")
    expect(core.check_ratchet(er, jr)[0] == 0, "ratchet: empty -> java on the sample = improvement")
    expect(core.check_ratchet(jr, er)[0] == 1, "ratchet: java -> empty on the sample = regression")
    expect(core.check_ratchet(jr, ec)[0] == 2, "ratchet: sample vs complete = not comparable")
    expect(core.check_ratchet(ec, ec)[0] == 0, "ratchet: complete empty vs itself = no regression")

    print(f"selftest: outputs in {out}")
    print("SELFTEST PASS" if not failures else f"SELFTEST FAIL ({len(failures)})")
    return 0 if not failures else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("run", help="run a binary on the D2 corpus and compare with golden Java")
    r.add_argument("--bin", required=True)
    r.add_argument("--profile", action="append")
    r.add_argument("--source", action="append")
    r.add_argument("--case-regex")
    r.add_argument("--sample", type=int)
    r.add_argument("--seed", type=int, default=0)
    r.add_argument("--limit", type=int)
    r.add_argument("--jobs", type=int)
    r.add_argument("--timeout", type=float, default=core.DEFAULT_TIMEOUT_S)
    r.add_argument("--out")
    r.add_argument("--data-root")
    r.add_argument("--keep-failing", action="store_true")
    r.add_argument("--baseline")
    r.add_argument("--only-passing", metavar="RATCHET_JSON",
                   help="run only the pairs this ratchet lists as passing and check they still pass (the gate)")
    r.set_defaults(func=cmd_run)

    c = sub.add_parser("check", help="compare two ratchet.json files (only goes up)")
    c.add_argument("--baseline", required=True)
    c.add_argument("--current", required=True)
    c.set_defaults(func=cmd_check)

    s = sub.add_parser("selftest", help="Java-backed fake = 100%%, empty fake = 0%%")
    s.add_argument("--java-sample", type=int, default=110)
    s.add_argument("--seed", type=int, default=1)
    s.add_argument("--jobs", type=int)
    s.add_argument("--timeout", type=float, default=core.DEFAULT_TIMEOUT_S)
    s.add_argument("--out")
    s.set_defaults(func=cmd_selftest)

    a = ap.parse_args(argv)
    return a.func(a)


if __name__ == "__main__":
    sys.exit(main())
