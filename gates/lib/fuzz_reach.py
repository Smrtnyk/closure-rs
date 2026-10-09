#!/usr/bin/env python3
"""Pass-effectiveness ("reach") measurement for Gate 0.3 (docs/PORTING.md §4.6).

Why not only pass-entry counters: whether a pass is *entered* is set by the options
(DefaultPassConfig decides the pass list from CompilerOptions); the input can only lower it,
when an error halts compilation before the later passes. So an entry-based reach says little
about the input and is bounded by the profile mix. (Gate 0.3 (b) still gates on
entry; fuzz_gate03.py `entry` measures it, and this effect measure is reported next to it.) This module
measures which passes CHANGE the program instead, using the oracle's
`compile_with_pass_dumps` (Compiler.maybePrintSourceAfterEachPass prints a block only when
`toSource()` differs from the last printed block; oracle/PROTOCOL.md).

A pass is *effective* on a (input, argv) run when its name heads at least one printed block,
i.e. the printed source after it differs from the printed source before it. Limitation: a pass
that changes only non-printed state (types, JSDoc-only, node props) is not counted.

Subcommands
  denominator                         every pass name DefaultPassConfig can register
  d2   [--sample 300] [--seed N] [--servers 5]    effective passes on a seeded sample of
                                      visible D2 pairs (resumable; results in build/fuzz/reach/)
  dir  --dir D [--servers 5]          effective passes on a directory of generated programs:
                                      D/<name>.js with D/<name>.json = {"profile": "..."}
  report [--seed N] [--fuzz-results F] summary JSON (denominator, D2-effective set, fuzz reach)
  tracer-check --files F... [--level ADVANCED]  cross-check against --tracer_mode=TIMING_ONLY:
                                      per input, passes entered (runs > 0), passes with
                                      changingRuns > 0, and pass-dump effective passes

All oracle servers run in the golden environment (oracle_client.golden_env(), ready line
checked against GOLDEN_JVM_ENV; D-010).  Heap: --servers x 1536m plus at most one 3g retry.
"""
from __future__ import annotations

import argparse
import json
import os
import random
import re
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "oracle"))
import case_args  # noqa: E402
import oracle_client as oc  # noqa: E402

# Logical names (reported as is); read from the selected reference checkout (ref_src()).
DPC = "reference/closure-compiler/src/com/google/javascript/jscomp/DefaultPassConfig.java"
PASSNAMES = "reference/closure-compiler/src/com/google/javascript/jscomp/PassNames.java"
PROCESS_DEFINES = "reference/closure-compiler/src/com/google/javascript/jscomp/ProcessDefines.java"
WORK = "build/fuzz/reach"
SERVER_FLAGS = ["-Xms128m", "-XX:+UseSerialGC", "-XX:MinHeapFreeRatio=10",
                "-XX:MaxHeapFreeRatio=30", "-XX:-UsePerfData", "-XX:+ExitOnOutOfMemoryError",
                "-XX:+DisplayVMOutputToStderr", "-Xlog:disable", "-Xlog:all=off"]
REQUEST_TIMEOUT_S = 600


def rp(rel: str) -> str:
    return os.path.join(REPO, rel)


def ref_src(logical: str) -> str:
    """A reference/closure-compiler/... path in the selected reference checkout (paths.REF_SRC)."""
    return os.path.join(case_args.paths.REF_SRC, logical.removeprefix("reference/closure-compiler/"))


# ---------------------------------------------------------------------------------------
# Denominator


def denominator() -> dict:
    """Every name a DefaultPassConfig PassFactory can carry (PassFactory.builder().setName)."""
    consts = {}
    with open(ref_src(PASSNAMES), encoding="utf-8") as f:
        for m in re.finditer(r'static final String (\w+)\s*=\s*"([^"]+)"', f.read()):
            consts[m.group(1)] = m.group(2)
    with open(ref_src(PROCESS_DEFINES), encoding="utf-8") as f:
        pd = f.read()
    modes = re.findall(r"^\s+([A-Z_]+)\((?:true|false), (?:true|false)\)", pd, re.M)
    with open(ref_src(DPC), encoding="utf-8") as f:
        src = f.read()
    factories = []
    for m in re.finditer(r"\.setName\(\s*(PassNames\.(\w+)|\"([^\"]+)\"(\s*\+\s*mode\.name\(\))?)",
                         src):
        line = src.count("\n", 0, m.start()) + 1
        if m.group(2):
            factories.append({"line": line, "names": [consts[m.group(2)]]})
        elif m.group(4):
            factories.append({"line": line, "names": [m.group(3) + md for md in modes]})
        else:
            factories.append({"line": line, "names": [m.group(3)]})
    names = sorted({n for fct in factories for n in fct["names"]})
    return {"source": DPC, "factory_count": len(factories), "names": names,
            "name_count": len(names), "factories": factories,
            "not_factories_but_printed": [consts["PARSE_INPUTS"]]}


# ---------------------------------------------------------------------------------------
# Oracle pool


class Server:
    def __init__(self, xmx: str = "1536m"):
        self.xmx = xmx
        self.o = oc.Oracle(xmx=xmx, extra_jvm=SERVER_FLAGS)  # golden env + ready-line check

    def request(self, req: dict, timeout: float = REQUEST_TIMEOUT_S) -> dict:
        timer = threading.Timer(timeout, self.o.proc.kill)
        timer.start()
        try:
            return self.o.request(req)
        finally:
            timer.cancel()

    def close(self):
        self.o.close()


BIG_SLOT = threading.Semaphore(1)  # at most one 3g server at a time (heap budget)


def effective_passes(resp: dict) -> list[str]:
    return [p["pass"] for p in resp.get("passes") or []]


def run_one(server_box: list, req: dict) -> dict:
    """Run one pass-dump request; on server death retry once on a fresh -Xmx3g server."""
    t0 = time.time()
    try:
        resp = server_box[0].request(req)
        attempt = "1536m"
    except Exception as e:  # server died (OOM, timeout kill): retry once at 3g
        err1 = repr(e)
        try:
            server_box[0].close()
        except Exception:
            pass
        with BIG_SLOT:
            big = Server("3g")
            try:
                resp = big.request(req)
            except Exception as e2:
                resp = {"ok": False, "error": f"first: {err1}; retry: {e2!r}"}
            finally:
                big.close()
        server_box[0] = Server()
        attempt = "3g"
    out = {"ok": resp.get("ok"), "exit_code": resp.get("exit_code"), "attempt": attempt,
           "wall_s": round(time.time() - t0, 2)}
    if resp.get("ok") and "--tracer_mode=TIMING_ONLY" in (req.get("args") or []):
        # Pass-entry counters (Gate 0.3 (b)): the --tracer_mode summary CSV.
        import base64
        t = tracer_csv(base64.b64decode(resp.get("stderr_b64") or "").decode("utf-8", "replace"))
        if t is None:
            out["ok"] = False
            out["error"] = "no tracer summary in stderr"
        else:
            out["entered"] = sorted(k for k, (runs, _) in t.items() if runs > 0)
            out["changing"] = sorted(k for k, (_, ch) in t.items() if ch > 0)
    elif resp.get("ok"):
        seq = effective_passes(resp)
        out["sequence"] = seq
        out["effective"] = sorted(set(seq))
        out["stripped_args"] = resp.get("stripped_args")
    else:
        out["error"] = (resp.get("error") or "")[:2000]
    return out


def pool_run(jobs: list[dict], results_path: str, servers: int):
    """jobs: [{"key":..., "req":..., ...meta}] -> appends one JSON line per job (resumable)."""
    done = set()
    if os.path.exists(results_path):
        with open(results_path, encoding="utf-8") as f:
            for line in f:
                try:
                    done.add(json.loads(line)["key"])
                except Exception:
                    pass
    todo = [j for j in jobs if j["key"] not in done]
    print(f"{len(jobs)} jobs, {len(done)} done, {len(todo)} to run, {servers} servers",
          flush=True)
    lock = threading.Lock()
    it = iter(todo)
    fout = open(results_path, "a", encoding="utf-8")
    counter = [len(done)]

    def worker():
        box = [Server()]
        try:
            while True:
                with lock:
                    job = next(it, None)
                if job is None:
                    return
                res = run_one(box, job["req"])
                rec = {k: v for k, v in job.items() if k != "req"}
                rec.update(res)
                with lock:
                    fout.write(json.dumps(rec, sort_keys=True) + "\n")
                    fout.flush()
                    counter[0] += 1
                    print(f"[{counter[0]}/{len(jobs)}] {job['key']} ok={res['ok']} "
                          f"eff={len(res.get('effective', []))} "
                          f"entered={len(res.get('entered', []))} {res['wall_s']}s", flush=True)
        finally:
            box[0].close()

    ts = [threading.Thread(target=worker) for _ in range(servers)]
    for t in ts:
        t.start()
    for t in ts:
        t.join()
    fout.close()


# ---------------------------------------------------------------------------------------
# D2 sample


def d2_pairs() -> list[tuple[dict, str]]:
    cases = case_args.load_cases("corpus/d2/cases.jsonl")
    return [(c, p) for c in cases for p in case_args.case_profiles(c)]


def d2_jobs(sample: int, seed: int) -> list[dict]:
    pairs = d2_pairs()
    rnd = random.Random(seed)
    rnd.shuffle(pairs)
    jobs = []
    for c, prof in pairs[:sample]:
        out_dir = f"{WORK}/out/{c['id']}/{prof}"
        args = case_args.compiler_args(c, prof, out_dir=out_dir)
        jobs.append({"key": f"{c['id']}|{prof}", "case": c["id"], "profile": prof,
                     "req": {"op": "compile_with_pass_dumps", "args": args}})
    return jobs


def dir_jobs(d: str) -> list[dict]:
    profiles = case_args.load_profiles()
    jobs = []
    for fn in sorted(os.listdir(d)):
        if not fn.endswith(".js"):
            continue
        name = fn[:-3]
        meta_p = os.path.join(d, name + ".json")
        meta = json.load(open(meta_p)) if os.path.exists(meta_p) else {}
        prof = meta.get("profile", "simple")
        js = os.path.relpath(os.path.join(d, fn), REPO)
        case = {"id": "fuzz-" + name, "inputs": [js], "externs": [], "shims": [],
                "extra_flags": [], "profiles": [prof]}
        if not case_args.profile_applies(profiles, case, prof):
            prof = "advanced"
        args = case_args.compiler_args(case, prof, out_dir=f"{WORK}/fuzz-out/{name}/{prof}",
                                       profiles=profiles)
        jobs.append({"key": f"{name}|{prof}", "case": name, "profile": prof,
                     "req": {"op": "compile_with_pass_dumps", "args": args}})
    return jobs


# ---------------------------------------------------------------------------------------
# Tracer cross-check


def tracer_csv(stderr: str) -> dict | None:
    """The --tracer_mode summary CSV: {pass: (runs, changingRuns)}; None if absent."""
    i = stderr.find("pass,runtime,allocMem,runs,changingRuns")
    if i < 0:
        return None
    out = {}
    for line in stderr[i:].splitlines()[1:]:
        f = line.split(",")
        if len(f) != 8 or not f[3].isdigit():
            break
        out[f[0]] = (int(f[3]), int(f[4]))
    return out


def tracer_check(files: list[str], level: str) -> dict:
    import base64
    den = set(denominator()["names"])
    rows = []
    srv = Server()
    try:
        for fpath in files:
            a = [f"--compilation_level={level}", f"--js={fpath}",
                 f"--js_output_file={WORK}/tracer-out/out.js"]
            r = srv.request({"op": "compile", "args": a + ["--tracer_mode=TIMING_ONLY"]})
            t = tracer_csv(base64.b64decode(r.get("stderr_b64") or "").decode("utf-8", "replace"))
            d = srv.request({"op": "compile_with_pass_dumps", "args": a})
            eff = set(effective_passes(d)) & den
            row = {"file": fpath, "exit_code": r.get("exit_code"), "effective": sorted(eff)}
            if t is None:
                row["tracer"] = None
            else:
                entered = {k for k, (runs, _) in t.items() if runs > 0}
                changing = {k for k, (_, ch) in t.items() if ch > 0} & den
                row.update({"entered_all": sorted(entered), "entered_in_denominator": len(entered & den),
                            "changing": sorted(changing),
                            "effective_not_changing": sorted(eff - changing),
                            "changing_not_effective": sorted(changing - eff)})
            rows.append(row)
    finally:
        srv.close()
    ents = {tuple(r["entered_all"]) for r in rows if "entered_all" in r}
    return {"level": level, "rows": rows, "distinct_entered_sets": len(ents)}


# ---------------------------------------------------------------------------------------
# Report


def load_results(path: str) -> list[dict]:
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def summarize(results: list[dict], denom: list[str]) -> dict:
    dset = set(denom)
    counts: dict[str, int] = {}
    by_profile: dict[str, set] = {}
    per_run = []
    ok = [r for r in results if r.get("ok")]
    for r in ok:
        eff = [p for p in r.get("effective", []) if p in dset]
        per_run.append(len(eff))
        by_profile.setdefault(r["profile"], set()).update(eff)
        for p in eff:
            counts[p] = counts.get(p, 0) + 1
    unknown = sorted({p for r in ok for p in r.get("effective", []) if p not in dset})
    return {"runs": len(results), "ok_runs": len(ok),
            "failed": [r["key"] for r in results if not r.get("ok")],
            "effective_union": sorted(counts), "effective_union_count": len(counts),
            "pass_hit_counts": dict(sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))),
            "mean_effective_per_run": (sum(per_run) / len(per_run)) if per_run else 0.0,
            "union_by_profile": {k: len(v) for k, v in sorted(by_profile.items())},
            "printed_names_outside_denominator": unknown}


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("denominator")
    a = sub.add_parser("d2")
    a.add_argument("--sample", type=int, default=300)
    a.add_argument("--seed", type=int, default=20261006)
    a.add_argument("--servers", type=int, default=5)
    b = sub.add_parser("dir")
    b.add_argument("--dir", required=True)
    b.add_argument("--servers", type=int, default=5)
    b.add_argument("--results")
    r = sub.add_parser("report")
    r.add_argument("--seed", type=int, default=20261006)
    r.add_argument("--fuzz-results", action="append", default=[])
    tc = sub.add_parser("tracer-check")
    tc.add_argument("--files", nargs="+", required=True)
    tc.add_argument("--level", default="ADVANCED")
    args = ap.parse_args()
    os.chdir(REPO)
    os.makedirs(rp(WORK), exist_ok=True)
    if args.cmd == "denominator":
        print(json.dumps(denominator(), indent=1))
    elif args.cmd == "d2":
        pool_run(d2_jobs(args.sample, args.seed), rp(f"{WORK}/d2-{args.seed}.jsonl"),
                 args.servers)
    elif args.cmd == "dir":
        res = args.results or rp(f"{WORK}/dir-{os.path.basename(args.dir.rstrip('/'))}.jsonl")
        pool_run(dir_jobs(args.dir), res, args.servers)
    elif args.cmd == "tracer-check":
        print(json.dumps(tracer_check(args.files, args.level), indent=1))
    elif args.cmd == "report":
        den = denominator()
        d2 = summarize(load_results(rp(f"{WORK}/d2-{args.seed}.jsonl")), den["names"])
        out = {"denominator": {"factories": den["factory_count"], "names": den["name_count"]},
               "d2": d2,
               "never_effective_on_d2": sorted(set(den["names"]) - set(d2["effective_union"]))}
        e = set(d2["effective_union"])
        for fr in args.fuzz_results:
            fz = summarize(load_results(fr), den["names"])
            hit = set(fz["effective_union"]) & e
            fz["reach_vs_d2_effective"] = (len(hit) / len(e)) if e else 0.0
            fz["reach_vs_all_names"] = len(fz["effective_union"]) / den["name_count"]
            fz["d2_effective_not_reached"] = sorted(e - hit)
            out.setdefault("fuzz", {})[fr] = fz
        print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
