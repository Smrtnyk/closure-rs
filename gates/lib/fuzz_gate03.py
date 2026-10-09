#!/usr/bin/env python3
"""Gate 0.3 (docs/PORTING.md §4.6): parse acceptance, pass reach, and the 1-hour driver run.

Subcommands
  reach  --manifest M [--results R] [--servers 3]
         compile_with_pass_dumps for every parse-accepted program in an export manifest
         (target/release/fuzz-driver export), under the profile the driver picks for it, with
         argv from case_args.compiler_args (multi-file programs: all files as --js, chunk
         profiles apply; sloppy programs carry --strict_mode_input=false as extra_flags).
         Resumable (fuzz_reach.pool_run); golden env + ready-line check (D-010).
  entry  --manifest M [--results E] [--servers 3]
         the Gate 0.3 definition: pass-entry counters. The same argv as `reach` plus
         --tracer_mode=TIMING_ONLY (oracle `compile`); records the DefaultPassConfig names with
         runs > 0. Resumable; golden env + ready-line check (D-010).
  report --export-dir D --reach R --entry E --run-report J --run-exit F [--d2 ...]
         writes gates/reports/gate_0_3.json and gates/reports/gate_0_3.md.

Verdict: (b) is decided only on the metric the Gate 0.3 definition
states, the average per program of pass-entry counters over all 138 DefaultPassConfig names.
D-016 adds (b2), an extra criterion and not a substitute for (b): the union over the fuzz programs
of the passes they make effective (printed source changed after the pass) must cover at least 85% of
the D2-effective set. (b2) counts only runs where Java did not crash (exit 254, D-009 drops those
pairs from every comparison); the union with crash runs is reported next to it. The overall gate is
(a) and (b) and (b2) and (c). The other effect-measure aggregates stay diagnostics.

Reach definition (fuzz/DESIGN.md "Reach measurement"): a pass is *effective* on
a run when the source printed after it differs from the source before it
(Compiler.maybePrintSourceAfterEachPass via the oracle's compile_with_pass_dumps). Pass-entry
counters measure an input poorly: entry is set mainly by the options, so the report also gives
the ceiling the profile mix imposes on the as-written metric.

How a non-zero exit relates to entry (Java: SortingErrorManager.hasHaltingErrors,
PhaseOptimizer, Result.success, AbstractCommandLineRunner): only an error whose diagnostic type is
ERROR by default halts the pass loop. A warning promoted to an error (advanced_strict's
--jscomp_error=*) does not halt it: the compile runs every optimization pass, and only then is the
output suppressed (Result.success is false when any error was reported, so the CLI writes no output
and exits with the error count). On the gate data most failing runs are of this second kind
(advanced_strict), so failing runs enter *more* names on average than exit-0 runs, and their
optimized output is never compared (the differential driver sees only diagnostics and the exit
code). The report gives the (b) mean over exit-0 runs and the share of measured runs without
comparable output as diagnostics; they do not change the (b) definition.
"""
from __future__ import annotations

import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import case_args  # noqa: E402
import fuzz_reach  # noqa: E402

GATE = "build/fuzz/gate"
THRESH_PARSE = 0.95
THRESH_REACH = 0.60
THRESH_EFFECT_UNION = 0.85  # (b2), D-016 item 3
MIN_RUN_S = 3600
# D-016 item 1: the driver's documented profile weights (fuzz/driver/src/main.rs PROFILE_WEIGHTS_PCT).
D016_WEIGHTS = {"advanced_family": 0.65, "ws": 0.03, "other": 0.32}


def rp(rel: str) -> str:
    return os.path.join(REPO, rel)


def load_manifest(path: str) -> list[dict]:
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def reach_jobs(manifest: list[dict], entry: bool = False, out_base: str = GATE) -> list[dict]:
    """Jobs for `reach`/`entry`. Compiler outputs go under `out_base` (the results file's
    directory), so a sample run elsewhere never overwrites the gate's own output trees."""
    profiles = case_args.load_profiles()
    jobs = []
    for m in manifest:
        if not m.get("accepted"):
            continue
        prof = m["profile"]
        case = {"id": f"fz-{m['index']}", "inputs": m["files"], "externs": [], "shims": [],
                "extra_flags": m.get("extra_flags", []), "profiles": [prof]}
        if not case_args.profile_applies(profiles, case, prof):
            raise SystemExit(f"profile {prof} does not apply to program {m['index']}")
        sub = "entry-out" if entry else "reach-out"
        args = case_args.compiler_args(case, prof, out_dir=f"{out_base}/{sub}/p{m['index']}",
                                       profiles=profiles)
        req = ({"op": "compile", "args": args + ["--tracer_mode=TIMING_ONLY"]} if entry
               else {"op": "compile_with_pass_dumps", "args": args})
        jobs.append({"key": f"p{m['index']}|{prof}", "index": m["index"], "case": case["id"],
                     "category": m["category"], "profile": prof, "req": req})
    return jobs


CRASH_DIR = "build/fuzz/java-crashes"
CRASH_CLASSES = f"{GATE}/java-crash-classes.jsonl"


def _program_sha(p: str) -> str:
    """Content hash of a kept crash program (its .js files and origin.txt). Origins and
    directory names repeat across fuzzer versions for the same seed, so the classification cache
    is keyed on this, never on the origin string alone."""
    import hashlib
    h = hashlib.sha256()
    for f in sorted(os.listdir(p)):
        if f.endswith(".js") or f == "origin.txt":
            h.update(f.encode() + b"\0")
            with open(os.path.join(p, f), "rb") as fh:
                h.update(fh.read())
            h.update(b"\0")
    return h.hexdigest()


def _exception_class(exc_lines: list[str]) -> str:
    """The JVM exception class of a crash: the first "Caused by:" line among the exception lines,
    else the first line; the fully qualified class name in it when there is one. Closure can print
    its JSON diagnostics summary on the same stderr line, before the exception (a naive split labels
    such crashes '[{"level"'), so the label is not simply the text before the first colon."""
    import re
    cause = next((ln for ln in exc_lines if ln.startswith("Caused by:")), exc_lines[0] if exc_lines else "")
    m = re.search(r"\b((?:[a-z_$][\w$]*\.)+[A-Z][\w$]*(?:Exception|Error))\b", cause)
    if m:
        return m.group(1)
    return re.sub(r"^Caused by:\s*", "", cause).split(":")[0].strip()


def _rerun_crash(d: str, origin: str, prof: str, extra: list[str], profiles: dict) -> dict:
    """Re-run one kept Java crash with the reference CLI (not the oracle), golden env."""
    import re
    import subprocess
    import run_reference as rr
    p = os.path.join(CRASH_DIR, d)
    files = sorted(os.path.join(p, f) for f in os.listdir(p) if f.endswith(".js"))
    case = {"id": "crash-" + d, "inputs": files, "externs": [], "shims": [],
            "extra_flags": extra, "profiles": [prof]}
    od = f"{GATE}/crash-diag/{d}"
    os.makedirs(od, exist_ok=True)
    args = case_args.compiler_args(case, prof, out_dir=od, profiles=profiles)
    cmd = [rr.JAVA] + rr.jvm_flags("1g", use_cds=False) + ["-jar", rr.JAR] + args
    rec = {"dir": d, "origin": origin, "profile": prof, "program_sha256": _program_sha(p)}
    try:
        r = subprocess.run(cmd, env=rr.child_env(), capture_output=True, timeout=180)
    except subprocess.TimeoutExpired:
        return {**rec, "exit": None, "timeout": True, "reproduced": False}
    err = r.stderr.decode("utf-8", "replace")
    exc = [ln.strip() for ln in err.splitlines()
           if re.search(r"(Exception|Error)(:|$)", ln.strip()) and not ln.strip().startswith("at ")
           and "JSC_" not in ln][:3]
    cls = _exception_class(exc)
    # Match the JVM class names only: Closure's AST dumps on stderr list extern type names such as
    # GPUOutOfMemoryError, which a bare substring test mistook for a JVM OutOfMemoryError.
    return {**rec, "exit": r.returncode, "internal_compiler_error": "INTERNAL COMPILER ERROR" in err,
            "stack_overflow": re.search(r"\bjava\.lang\.StackOverflowError\b", err) is not None,
            "oom": re.search(r"\bjava\.lang\.OutOfMemoryError\b", err) is not None,
            "reproduced": r.returncode == 254 or "INTERNAL COMPILER ERROR" in err,
            "exception_class": cls, "exception_lines": exc}


def classify_java_crashes(run: dict, since: float | None = None) -> dict:
    """Reproduce every Java crash the run dropped (D-009) with the reference CLI, so a harness
    fault cannot hide among them. Cached in CRASH_CLASSES, keyed on (origin, profile, content hash
    of the kept program); at most 4 JVMs at -Xmx1g, run only after the driver's oracle servers are
    gone. `since` (epoch s): only kept-program directories written at or after it belong to this
    run (directory names repeat across runs of the same seed). Never fails the report."""
    from concurrent.futures import ThreadPoolExecutor
    import run_reference as rr
    dropped = run.get("java_crashes_dropped") or []
    seed = run.get("seed")
    out = {"dropped": len(dropped), "method": f"reference CLI ({rr.JAR}) "
           "re-run of each kept program under its profile, golden env, -Xmx1g"}
    try:
        cache = {}
        if os.path.exists(CRASH_CLASSES):
            with open(CRASH_CLASSES, encoding="utf-8") as f:
                for line in f:
                    if line.strip():
                        r = json.loads(line)
                        if r.get("program_sha256"):  # older rows lack it: never trusted
                            cache[(r["origin"], r["profile"], r["program_sha256"])] = r
        dirs: dict[tuple, list] = {}
        older_dirs = 0
        for d in sorted(os.listdir(CRASH_DIR)) if os.path.isdir(CRASH_DIR) else []:
            p = os.path.join(CRASH_DIR, d)
            if not (os.path.isdir(p) and d.startswith(f"{seed}-")):
                continue
            try:
                if since is not None and os.path.getmtime(os.path.join(p, "origin.txt")) < since:
                    older_dirs += 1
                    continue
                lines = open(os.path.join(p, "origin.txt"), encoding="utf-8").read().splitlines()
                extra = json.loads(lines[1]) if len(lines) > 1 else []
            except (OSError, ValueError):
                continue
            prof = d.split("-", 2)[2]
            dirs.setdefault((lines[0], prof), []).append((d, extra, _program_sha(p)))
        todo, missing, picked = [], [], []
        for e in dropped:
            origin, _, prof = e.rpartition(" ")
            cand = dirs.get((origin, prof))
            if not cand:
                missing.append(e)
                continue
            d, extra, sha = cand.pop(0)  # one directory per dropped entry (origins can repeat)
            picked.append((origin, prof, sha))
            if (origin, prof, sha) not in cache:
                todo.append((d, origin, prof, extra))
        profiles = case_args.load_profiles()
        workers = int(os.environ.get("GATE03_CLASSIFY_WORKERS", "4"))
        if todo and workers > 0:
            with ThreadPoolExecutor(workers) as ex:
                futs = [ex.submit(_rerun_crash, d, o, p, x, profiles) for d, o, p, x in todo]
                with open(CRASH_CLASSES, "a", encoding="utf-8") as f:
                    for fu in futs:
                        try:
                            r = fu.result()
                        except Exception:  # noqa: BLE001
                            continue
                        cache[(r["origin"], r["profile"], r["program_sha256"])] = r
                        f.write(json.dumps(r) + "\n")
                        f.flush()
        rows = [cache[k] for k in picked if k in cache]
        out["kept_dirs_from_older_runs_ignored"] = older_dirs
        out["not_classified"] = len(dropped) - len(rows) - len(missing)
        by_class: dict[str, int] = {}
        for r in rows:
            # Re-derived from the kept exception lines, so cached rows get the current parser too.
            k = (_exception_class(r.get("exception_lines") or []) or r.get("exception_class")
                 or ("timeout" if r.get("timeout") else "no exception line on stderr"))
            by_class[k] = by_class.get(k, 0) + 1
        out.update({"classified": len(rows), "reproduced": sum(1 for r in rows if r.get("reproduced")),
                    "internal_compiler_error": sum(1 for r in rows if r.get("internal_compiler_error")),
                    "stack_overflow": sum(1 for r in rows if r.get("stack_overflow")),
                    "oom": sum(1 for r in rows if r.get("oom")),
                    "not_reproduced": [f"{r['origin']} {r['profile']} (exit {r.get('exit')})"
                                       for r in rows if not r.get("reproduced")],
                    "missing_program_dir": missing,
                    "by_exception_class": dict(sorted(by_class.items(), key=lambda kv: -kv[1])),
                    "rows": os.path.relpath(rp(CRASH_CLASSES), REPO)})
    except Exception as e:  # noqa: BLE001
        out["error"] = repr(e)
    return out


def pct(x: float, nd: int = 1) -> str:
    return f"{100 * x:.{nd}f}%"


# ---- fuzzer quality diagnostics (reported, not gated) ----
JSGEN_CATS = ("lang", "closure", "sloppy")
LOW_PROFILES = ("lang_es5", "lang_es2015")
ADV_FAMILY = ("advanced", "advanced_strict", "chunks2", "chunks3")
LOW_TARGET_OUTPUT = 0.80  # target for jsgen runs under lang_es5/lang_es2015
DATAFLOW_PASSES = ("flowSensitiveInlineVariables", "coalesceVariableNames",
                   "collapseAnonymousFunctions", "collapseVariableDeclarations", "denormalize",
                   "invertContextualRenaming", "inlineVariables", "deadAssignmentsElimination",
                   "inlineFunctions", "removeUnusedCode")
# Seeded, mutually disjoint samples of D2 pairs (fuzz_reach.d2_jobs): the gate's original
# sample, a second sample (copied from build/review-fuzz-gate-honesty/), and a
# third sample (build/fuzz/gate/d2x/d2_third.py). Missing files are skipped.
D2_SAMPLES = ("build/fuzz/reach/d2-20261006.jsonl", f"{GATE}/d2x/d2-977.jsonl",
              f"{GATE}/d2x/d2-20261008.jsonl")
BASELINE_DIR = "build/fuzz/gate-stale-20261006T220248"  # the pre-fix programs


def meaningful_len(js: str) -> int:
    """Length without /* */ comments, line comments at line start, and whitespace (the same
    rough measure as fuzz/mutate advanced_viable)."""
    import re
    js = re.sub(r"/\*.*?\*/", "", js, flags=re.S)
    js = re.sub(r"(?m)^//.*$", "", js)
    return len(re.sub(r"\s", "", js))


def strip_console_log(js: str) -> str:
    """Remove `console.log(...)` calls (balanced parentheses, string literals skipped)."""
    out, i, n = [], 0, len(js)
    while i < n:
        if js.startswith("console.log(", i):
            j, depth, q = i + len("console.log("), 1, None
            while j < n and depth:
                ch = js[j]
                if q:
                    if ch == "\\":
                        j += 1
                    elif ch == q:
                        q = None
                elif ch in "'\"`":
                    q = ch
                elif ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
                j += 1
            i = j
            while i < n and js[i] in ";, \n":
                i += 1
            continue
        out.append(js[i])
        i += 1
    return "".join(out)


def trivial_output(js: str) -> bool:
    """The review's measure: no function, loop or branch left, or under 40 bytes once
    console.log calls are removed (approximated on the printed text)."""
    import re
    rest = re.sub(r"\s", "", strip_console_log(js))
    if len(rest) < 40:
        return True
    # A function is `function`, an arrow, a class, or a method (`name(params){`).
    return not re.search(r"function|=>|\bclass\b|[\w$]\([^()]*\)\{|\bfor\(|\bwhile\(|\bdo\{|\bif\(|"
                         r"\bswitch\(|\?|&&|\|\|", rest)


def _out_js(gate_dir: str, idx: int) -> str | None:
    p = os.path.join(gate_dir, "reach-out", f"p{idx}", "out.js")
    if not os.path.exists(p):
        return None
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def quality(gate_dir: str, d2_rows: list[dict]) -> dict | None:
    """Per-review-issue diagnostics for one gate directory (reach.jsonl + reach-out/)."""
    rpath = os.path.join(gate_dir, "reach.jsonl")
    mpath = os.path.join(gate_dir, "export", "manifest.jsonl")
    if not (os.path.exists(rpath) and os.path.exists(mpath)):
        return None
    man = {m["index"]: m for m in load_manifest(mpath)}
    rows = [r for r in fuzz_reach.load_results(rpath) if r.get("ok")]
    q: dict = {"dir": os.path.relpath(gate_dir, REPO) if os.path.isabs(gate_dir) else gate_dir,
               "ok_runs": len(rows)}
    # Issue 1: jsgen output under lang_es5 / lang_es2015.
    low = {}
    for cat in JSGEN_CATS + ("all_jsgen",):
        for prof in LOW_PROFILES:
            sel = [r for r in rows if r["profile"] == prof
                   and (r["category"] == cat or (cat == "all_jsgen" and r["category"] in JSGEN_CATS))]
            outn = sum(1 for r in sel if r.get("exit_code") == 0
                       and (_out_js(gate_dir, r["index"]) or "") != "")
            low[f"{cat}|{prof}"] = {"runs": len(sel),
                                    "exit0": sum(1 for r in sel if r.get("exit_code") == 0),
                                    "output": outn, "output_rate": outn / len(sel) if sel else None}
    allsel = [r for r in rows if r["profile"] in LOW_PROFILES and r["category"] in JSGEN_CATS]
    alln = sum(1 for r in allsel if r.get("exit_code") == 0
               and (_out_js(gate_dir, r["index"]) or "") != "")
    q["low_target"] = {"cells": low, "runs": len(allsel), "output": alln,
                       "output_rate": alln / len(allsel) if allsel else None,
                       "target": LOW_TARGET_OUTPUT,
                       "meets_target": bool(allsel) and alln / len(allsel) >= LOW_TARGET_OUTPUT}
    # Issue 2: mutants under the ADVANCED family.
    mut = {}
    for prof in ADV_FAMILY:
        sel = [r for r in rows if r["profile"] == prof and r["category"] == "mutate"]
        e0 = [r for r in sel if r.get("exit_code") == 0]
        if prof.startswith("chunks"):
            meaningful = None
        else:
            meaningful = sum(1 for r in e0 if meaningful_len(_out_js(gate_dir, r["index"]) or "") >= 40)
        mut[prof] = {"runs": len(sel), "exit0": len(e0), "meaningful_output": meaningful}
    q["mutate_advanced"] = mut
    # Issue 3: how often the data-flow passes change the output, per profile, vs D2.
    eff = {}
    for prof in ("advanced", "simple"):
        sel = [r for r in rows if r["profile"] == prof]
        d2sel = [r for r in d2_rows if r.get("profile") == prof]
        eff[prof] = {"runs": len(sel), "d2_runs": len(d2sel), "passes": {
            p: {"fuzz": sum(1 for r in sel if p in r.get("effective", [])),
                "d2": sum(1 for r in d2sel if p in r.get("effective", []))}
            for p in DATAFLOW_PASSES}}
    q["dataflow_effect"] = eff
    triv = {}
    for cat in ("lang", "closure", "multi", "sloppy", "mutate", "all"):
        sel = [r for r in rows if r["profile"] == "advanced" and r.get("exit_code") == 0
               and (cat == "all" or r["category"] == cat)]
        outs = [(_out_js(gate_dir, r["index"]), man.get(r["index"], {}).get("bytes")) for r in sel]
        outs = [(o, b) for o, b in outs if o is not None]
        ratios = sorted(len(o.encode()) / b for o, b in outs if b)
        triv[cat] = {"exit0_with_output_file": len(outs),
                     "trivial": sum(1 for o, _ in outs if trivial_output(o)),
                     "median_size_ratio": ratios[len(ratios) // 2] if ratios else None}
    q["advanced_trivial"] = triv
    return q


def fuzz_src_sha() -> str:
    """The same hash gates/gate_0_3.sh keys the gate results on (sha256 of `sha256sum` lines of
    every fuzz/ *.rs and Cargo.toml outside target/, in C-locale path order)."""
    import hashlib
    paths = []
    for root, dirs, files in os.walk(os.path.join(REPO, "fuzz")):
        dirs[:] = [d for d in dirs if d != "target"]
        for fn in files:
            if fn.endswith(".rs") or fn == "Cargo.toml":
                paths.append(os.path.relpath(os.path.join(root, fn), REPO))
    lines = []
    for rel in sorted(paths, key=lambda x: x.encode()):
        with open(os.path.join(REPO, rel), "rb") as f:
            lines.append(f"{hashlib.sha256(f.read()).hexdigest()}  {rel}\n")
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def report(a) -> dict:
    den = fuzz_reach.denominator()
    all_names = den["names"]
    # The D2-effective denominator is the union over several disjoint seeded samples of visible
    # D2 pairs (a single 300-pair sample was a thin basis; later samples show whether it
    # is saturated).
    d2_files = [f for f in a.d2 if os.path.exists(rp(f))]
    d2_all_rows = [r for f in d2_files for r in fuzz_reach.load_results(rp(f))]
    d2 = fuzz_reach.summarize(d2_all_rows, all_names)
    d2_eff = set(d2["effective_union"])
    d2_samples, seen = [], set()
    for f in d2_files:
        sm = fuzz_reach.summarize(fuzz_reach.load_results(rp(f)), all_names)
        eff = set(sm["effective_union"])
        d2_samples.append({"file": f, "runs": sm["runs"], "ok_runs": sm["ok_runs"],
                           "effective_names": len(eff), "new_names_vs_earlier_samples": sorted(eff - seen)})
        seen |= eff

    # (a) parse acceptance
    with open(os.path.join(a.export_dir, "summary.json"), encoding="utf-8") as f:
        exp = json.load(f)
    manifest = load_manifest(os.path.join(a.export_dir, "manifest.jsonl"))
    rejected = [{"index": m["index"], "category": m["category"], "origin": m["origin"],
                 "errors": m["errors"][:3], "oracle_error": m.get("oracle_error")}
                for m in manifest if not m.get("accepted")]
    gate_a = {"programs": exp["count"], "accepted": exp["accepted"], "rate": exp["rate"],
              "threshold": THRESH_PARSE, "pass": exp["count"] >= 5000 and exp["rate"] >= THRESH_PARSE,
              "by_category": exp["by_category"], "seed": exp["seed"], "source": exp["source"],
              "method": exp["method"], "rejected": rejected}

    # (b) reach
    res = fuzz_reach.load_results(a.reach)
    ok = [r for r in res if r.get("ok")]
    per_d2, per_all, union = [], [], {}
    by_cat_union: dict[str, set] = {}
    by_prof_union: dict[str, set] = {}
    for r in ok:
        eff = set(r.get("effective", [])) & set(all_names)
        per_all.append(len(eff))
        per_d2.append(len(eff & d2_eff))
        for p in eff:
            union[p] = union.get(p, 0) + 1
        by_cat_union.setdefault(r.get("category", "?"), set()).update(eff & d2_eff)
        by_prof_union.setdefault(r["profile"], set()).update(eff & d2_eff)
    union_d2 = set(union) & d2_eff
    n_ok = len(ok)
    # Runs where Java itself crashed (exit 254, D-009) still count the passes that changed the
    # source before the crash; the union without them is reported too, as a robustness check.
    crash_runs = [r for r in ok if r.get("exit_code") == 254]
    union_no_crash = set()
    for r in ok:
        if r.get("exit_code") != 254:
            union_no_crash |= set(r.get("effective", [])) & d2_eff
    mean_d2 = sum(per_d2) / n_ok if n_ok else 0.0
    mean_all = sum(per_all) / n_ok if n_ok else 0.0
    # D2 calibration: the same per-run averages for the real-world sample itself.
    d2_rows = [r for r in d2_all_rows if r.get("ok")]
    d2_mean_all = (sum(len(set(r.get("effective", [])) & set(all_names)) for r in d2_rows)
                   / len(d2_rows)) if d2_rows else 0.0
    union_rate = len(union_d2) / len(d2_eff) if d2_eff else 0.0
    gate_b = {
        "definition": "effective passes (printed source changed after the pass; oracle compile_with_pass_dumps), "
                      "each accepted program compiled once under the profile the driver picks for it",
        "gated_metric": "(b) is gated on spec_literal (pass-entry counters, average per program, all "
                        "DefaultPassConfig names); the effect-measure union without Java-crash runs is gated "
                        "separately as (b2) (D-016); the other effect aggregates are diagnostics",
        "denominator_used": {"name": "D2-effective set", "size": len(d2_eff),
                             "source": ", ".join(d2_files) + f" ({d2['ok_runs']} ok runs of {d2['runs']} seeded D2 pairs in "
                                       f"{len(d2_files)} disjoint samples, all profiles)",
                             "samples": d2_samples,
                             "why": "the DefaultPassConfig factories that change printed output on real-world "
                                    "inputs under the D2 profiles; the other names are checks (never change "
                                    "source), out-of-scope J2CL/Polymer/Chrome passes, or passes the D2 "
                                    "profiles never enable, so no sampled D2 input reached them"},
        "all_names_denominator": len(all_names), "factories": den["factory_count"],
        "programs_measured": len(res), "ok_runs": n_ok,
        "failed_runs": [r["key"] for r in res if not r.get("ok")],
        "union_d2_effective_reached": len(union_d2), "union_rate_vs_d2_effective": union_rate,
        "java_crash_runs": len(crash_runs),
        "union_d2_effective_reached_excluding_java_crash_runs": len(union_no_crash),
        "union_rate_vs_d2_effective_excluding_java_crash_runs":
            len(union_no_crash) / len(d2_eff) if d2_eff else 0.0,
        "union_all_names_reached": len(union), "union_rate_vs_all_names": len(union) / len(all_names),
        "mean_per_program_vs_d2_effective": mean_d2 / len(d2_eff) if d2_eff else 0.0,
        "mean_per_program_vs_all_names": mean_all / len(all_names),
        "mean_effective_passes_per_program": mean_all,
        "d2_calibration": {"mean_effective_passes_per_pair": d2_mean_all,
                           "mean_per_pair_vs_d2_effective": d2_mean_all / len(d2_eff) if d2_eff else 0.0,
                           "mean_per_pair_vs_all_names": d2_mean_all / len(all_names),
                           "union_vs_all_names": len(d2_eff) / len(all_names)},
        "d2_effective_not_reached": sorted(d2_eff - union_d2),
        "reached_outside_d2_effective": sorted(set(union) - d2_eff),
        "pass_hit_counts": dict(sorted(union.items(), key=lambda kv: (-kv[1], kv[0]))),
        "union_by_category_vs_d2_effective": {k: len(v) for k, v in sorted(by_cat_union.items())},
        "union_by_profile_vs_d2_effective": {k: len(v) for k, v in sorted(by_prof_union.items())},
        "threshold": THRESH_REACH,
        "pass_union_vs_all_names": len(union) / len(all_names) >= THRESH_REACH,
        "pass_literal_average_vs_all_names": mean_all / len(all_names) >= THRESH_REACH,
        "pass_union_definition": union_rate >= THRESH_REACH,
        "pass_literal_average": (mean_d2 / len(d2_eff) if d2_eff else 0.0) >= THRESH_REACH,
    }
    # the Gate 0.3 definition: "on average it reaches >= 60% of the passes in
    # DefaultPassConfig, measured by pass-entry counters in the oracle".
    ent = [r for r in fuzz_reach.load_results(a.entry) if r.get("ok") and "entered" in r]
    ent_failed = [r["key"] for r in fuzz_reach.load_results(a.entry) if not r.get("ok")]
    ent_counts = [len(set(r["entered"]) & set(all_names)) for r in ent]
    by_prof_ent: dict[str, list[int]] = {}
    for r, c in zip(ent, ent_counts):
        by_prof_ent.setdefault(r["profile"], []).append(c)
    ent_mean = sum(ent_counts) / len(ent_counts) if ent_counts else 0.0
    ent_rate = ent_mean / len(all_names)
    # The union over the programs of the names entered (reported, not gated: the definition
    # says "on average"), overall and per profile, plus the names no program entered.
    ent_union: dict[str, int] = {}
    ent_union_by_prof: dict[str, set] = {}
    for r in ent:
        names_r = set(r["entered"]) & set(all_names)
        for n in names_r:
            ent_union[n] = ent_union.get(n, 0) + 1
        ent_union_by_prof.setdefault(r["profile"], set()).update(names_r)
    ent_cover = len(ent) >= 0.95 * gate_a["accepted"] if gate_a["accepted"] else False
    # Entry is set mainly by the options (the profile plus any drawn option flags). The input moves
    # it mostly through errors, in two different ways (see the module docstring): an error whose
    # diagnostic type is ERROR by default halts the pass loop early and lowers the count; a warning
    # promoted to an error (advanced_strict) does not halt it, so the run enters every pass and only
    # its output is suppressed. The ceiling is the average if every measured program entered as many
    # names as the most any run of its profile entered (an empirical bound for this profile mix).
    ent_rows = fuzz_reach.load_results(a.entry)
    ent_fail_crash = sum(1 for r in ent_rows if not r.get("ok") and r.get("exit_code") == 254)
    prof_max = {k: max(v) for k, v in by_prof_ent.items()}
    ent_ceiling = (sum(prof_max[r["profile"]] for r in ent) / len(ent)) if ent else 0.0
    ent_short = [prof_max[r["profile"]] - c for r, c in zip(ent, ent_counts) if c < prof_max[r["profile"]]]
    by_prof_e0: dict[str, list[int]] = {}
    for r, c in zip(ent, ent_counts):
        if r.get("exit_code") == 0:
            by_prof_e0.setdefault(r["profile"], []).append(c)
    # Which measured runs have output the differential comparison can see. A run
    # with a non-zero exit writes no output (Result.success is false), so only its diagnostics and
    # exit code are compared. Failing runs that reached the end of their profile's pipeline ran the
    # optimization passes (the promoted-error case); the others halted early (an error of default
    # level ERROR). The end marker is per profile: latePeepholeOptimizations when any exit-0 run of
    # the profile entered it (every exit-0 non-ws run of the D-016 gate data did, including runs with
    # --renaming=false, which skip renameVars, an alternative marker; on that data both
    # markers give the same 1326 runs); otherwise (ws, whose pipeline has no optimization passes)
    # the names every exit-0 run of that profile entered. A profile with no exit-0 run falls back to
    # latePeepholeOptimizations.
    # Diagnostics only: (b) is defined over every measured run and is not changed here.
    ent_dir = os.path.join(os.path.dirname(os.path.abspath(a.entry)), "entry-out")

    def _has_output(r: dict) -> bool | None:
        d = os.path.join(ent_dir, f"p{r['index']}")
        if not os.path.isdir(ent_dir):
            return None
        return os.path.isdir(d) and any(os.path.isfile(os.path.join(d, f)) for f in os.listdir(d))
    e0_counts = [c for r, c in zip(ent, ent_counts) if r.get("exit_code") == 0]
    nz = [(r, c) for r, c in zip(ent, ent_counts) if r.get("exit_code") != 0]
    full_marker = "latePeepholeOptimizations"
    e0_entered_by_prof: dict[str, list[set]] = {}
    for r in ent:
        if r.get("exit_code") == 0:
            e0_entered_by_prof.setdefault(r["profile"], []).append(set(r["entered"]))

    def _end_marker(prof: str) -> frozenset:
        sets = e0_entered_by_prof.get(prof)
        if not sets or any(full_marker in s for s in sets):
            return frozenset({full_marker})
        return frozenset(set.intersection(*sets))
    end_markers = {p: _end_marker(p) for p in sorted({r["profile"] for r in ent})}
    nz_full = [(r, c) for r, c in nz if end_markers[r["profile"]] <= set(r["entered"])]
    nz_halt = [(r, c) for r, c in nz if not end_markers[r["profile"]] <= set(r["entered"])]
    out_flags = [_has_output(r) for r in ent]
    out_known = all(f is not None for f in out_flags)
    _m = lambda xs: (sum(xs) / len(xs)) if xs else None  # noqa: E731
    _byp = lambda xs: {k: sum(1 for r, _ in xs if r["profile"] == k)  # noqa: E731
                       for k in sorted({r["profile"] for r, _ in xs})}
    comparability = {
        "note": "diagnostic only; (b) is the mean over every measured run and is unchanged",
        "exit0_runs": len(e0_counts),
        "exit0_mean_entered": _m(e0_counts),
        "exit0_rate": (_m(e0_counts) / len(all_names)) if e0_counts else None,
        "exit0_rate_at_or_above_60": ((_m(e0_counts) / len(all_names)) >= THRESH_REACH) if e0_counts else None,
        "nonzero_runs": len(nz), "nonzero_mean_entered": _m([c for _, c in nz]),
        "nonzero_share_of_measured": len(nz) / len(ent) if ent else None,
        "nonzero_by_profile": _byp(nz),
        "nonzero_full_pipeline_runs": len(nz_full),
        "nonzero_full_pipeline_mean_entered": _m([c for _, c in nz_full]),
        "nonzero_full_pipeline_by_profile": _byp(nz_full),
        "nonzero_halted_early_runs": len(nz_halt),
        "nonzero_halted_early_mean_entered": _m([c for _, c in nz_halt]),
        "nonzero_halted_early_by_profile": _byp(nz_halt),
        "full_pipeline_marker": (f"{full_marker} entered" if all(m == {full_marker} for m in end_markers.values())
                                 else f"{full_marker} entered; " + "; ".join(
                                     f"for {p}: {', '.join(sorted(m))} entered" for p, m in end_markers.items()
                                     if m != {full_marker})),
        "end_marker_by_profile": {p: sorted(m) for p, m in end_markers.items()},
        "output_files_checked": out_known,
        "runs_with_output_files": sum(1 for f in out_flags if f) if out_known else None,
        "nonzero_runs_with_output_files": (sum(1 for (r, _), f in zip(zip(ent, ent_counts), out_flags)
                                               if f and r.get("exit_code") != 0) if out_known else None),
        "exit0_runs_without_output_files": (sum(1 for (r, _), f in zip(zip(ent, ent_counts), out_flags)
                                                if not f and r.get("exit_code") == 0) if out_known else None),
    }
    # Estimate of the same ceiling for the real-world D2 corpus under its own profile matrix (every
    # pair weighted by the largest entered count measured for its profile in these runs).
    try:
        d2_prof_counts: dict[str, int] = {}
        for _c, _p in fuzz_reach.d2_pairs():
            d2_prof_counts[_p] = d2_prof_counts.get(_p, 0) + 1
        _w = sum(n for k, n in d2_prof_counts.items() if k in prof_max)
        d2_entry_ceiling = (sum(prof_max[k] * n for k, n in d2_prof_counts.items() if k in prof_max) / _w
                            if _w else None)
    except Exception as _e:  # informational only
        d2_prof_counts, d2_entry_ceiling = {"error": repr(_e)}, None
    gate_b["spec_literal"] = {
        "metric": "mean over accepted programs of |DefaultPassConfig names entered (tracer runs > 0)| / "
                  f"{len(all_names)}, each program compiled once under the profile the driver picks "
                  "(oracle compile with --tracer_mode=TIMING_ONLY, golden env)",
        "results": os.path.relpath(a.entry, REPO) if os.path.isabs(a.entry) else a.entry,
        "measured_runs": len(ent), "failed_runs": ent_failed[:50], "n_failed": len(ent_failed),
        "failed_java_crash_exit_254": ent_fail_crash,
        "failed_other": len(ent_failed) - ent_fail_crash,
        "coverage_ok": ent_cover, "mean_entered": ent_mean, "rate": ent_rate,
        "union_entered": len(ent_union), "union_rate": len(ent_union) / len(all_names),
        "union_at_or_above_60": len(ent_union) / len(all_names) >= THRESH_REACH,
        "union_by_profile": {k: len(v) for k, v in sorted(ent_union_by_prof.items())},
        "never_entered": sorted(set(all_names) - set(ent_union)),
        "entered_program_counts": dict(sorted(ent_union.items(), key=lambda kv: (-kv[1], kv[0]))),
        "output_comparability": comparability,
        "runs_below_profile_max": len(ent_short),
        "mean_shortfall_below_profile_max": sum(ent_short) / len(ent_short) if ent_short else None,
        "ceiling_mean_entered": ent_ceiling, "ceiling_rate": ent_ceiling / len(all_names),
        "d2_profile_matrix_pairs": d2_prof_counts,
        "d2_ceiling_estimate_mean_entered": d2_entry_ceiling,
        "d2_ceiling_estimate_rate": (d2_entry_ceiling / len(all_names)) if d2_entry_ceiling else None,
        "ceiling_definition": "mean over the measured programs of the largest entered count any run of "
                              "the same profile reached (every program compiled as far as its profile "
                              "allows); the profile mix bounds the as-written metric by this",
        "by_profile": {k: {"runs": len(v), "mean_entered": sum(v) / len(v),
                           "min": min(v), "max": max(v),
                           "exit0_runs": len(by_prof_e0.get(k, [])),
                           "mean_entered_exit0": (sum(by_prof_e0[k]) / len(by_prof_e0[k])
                                                  if by_prof_e0.get(k) else None)}
                       for k, v in sorted(by_prof_ent.items())},
        "pass": ent_cover and ent_rate >= THRESH_REACH,
    }
    # Diagnostic only (not a proposed reading): union of D2-effective passes >= 60%.
    gate_b["pass_under_proposed_decision"] = None
    gate_b["diag_union_d2_effective_ge_60"] = (gate_b["pass_union_definition"]
                                               and n_ok >= 0.95 * gate_a["accepted"])
    gate_b["pass_as_written"] = gate_b["spec_literal"]["pass"]
    # (b) is also measured with the effect definition (effective passes,
    # fuzz_reach.py) and reported under both aggregates. The definition's aggregate is "on average";
    # the union is reported as a diagnostic only. Per-run calibration: how many D2-effective passes a
    # single (program, profile) run changes at most, for the fuzz programs and for real-world D2.
    fz_per_run = sorted(per_d2)
    d2_per_run = sorted(len(set(r.get("effective", [])) & d2_eff) for r in d2_rows)
    need = THRESH_REACH * len(d2_eff)
    prof_d2: dict[str, set] = {}
    for r in d2_rows:
        prof_d2.setdefault(r["profile"], set()).update(set(r.get("effective", [])) & d2_eff)
    gate_b["design_phase"] = {
        "definition": gate_b["definition"],
        "denominator": {"name": "D2-effective set", "size": len(d2_eff), "samples": d2_samples,
                        "alternative_reported": f"all {len(all_names)} DefaultPassConfig names (fuzz/DESIGN.md)"},
        "literal_average_per_program": {"mean_passes": mean_all,
                                        "rate_vs_d2_effective": gate_b["mean_per_program_vs_d2_effective"],
                                        "rate_vs_all_names": gate_b["mean_per_program_vs_all_names"],
                                        "at_or_above_60": gate_b["pass_literal_average"]},
        "union_over_programs": {"reached": len(union_d2), "rate_vs_d2_effective": union_rate,
                                "reached_all_names": len(union),
                                "rate_vs_all_names": len(union) / len(all_names),
                                "at_or_above_60": gate_b["diag_union_d2_effective_ge_60"]},
        "per_run_calibration": {
            "passes_needed_per_run_for_60pct": need,
            "fuzz_max_per_run": fz_per_run[-1] if fz_per_run else 0,
            "fuzz_p99_per_run": fz_per_run[int(0.99 * len(fz_per_run))] if fz_per_run else 0,
            "fuzz_runs_at_or_above_need": sum(1 for x in fz_per_run if x >= need),
            "d2_max_per_run": d2_per_run[-1] if d2_per_run else 0,
            "d2_runs_at_or_above_need": sum(1 for x in d2_per_run if x >= need),
            "d2_runs": len(d2_per_run),
            "d2_mean_per_run_vs_d2_effective": (sum(d2_per_run) / len(d2_per_run) / len(d2_eff))
                                               if d2_per_run and d2_eff else 0.0},
        "union_by_profile_vs_profile_d2_effective": {
            k: {"fuzz_reached": len(by_prof_union.get(k, set()) & v), "d2_effective_in_profile": len(v)}
            for k, v in sorted(prof_d2.items())},
    }
    # The gate verdict: (b) is decided on the measure the Gate 0.3 definition states and
    # nothing else: pass-entry counters in the oracle, averaged per program, over
    # all DefaultPassConfig names. The effect measure is a diagnostic of what the
    # generated inputs do once a pass is entered; it does not decide (b) (D-016 adds (b2) on its union,
    # as an extra criterion), and no relaxed reading
    # (union, reduced denominator) is proposed.
    gate_b["pass"] = gate_b["pass_as_written"]
    sl = gate_b["spec_literal"]
    fam = ("advanced", "advanced_strict", "chunks2", "chunks3")
    adv_fam = [c for r, c in zip(ent, ent_counts) if r["profile"] in fam]
    rest = [c for r, c in zip(ent, ent_counts) if r["profile"] not in fam + ("ws",)]
    a_mean = sum(adv_fam) / len(adv_fam) if adv_fam else 0.0
    r_mean = sum(rest) / len(rest) if rest else 0.0
    need_n = THRESH_REACH * len(all_names)
    # Share p of ADVANCED-family draws (ws dropped) for which p*a_mean + (1-p)*r_mean >= need_n.
    p_need = ((need_n - r_mean) / (a_mean - r_mean)) if a_mean > r_mean else None
    sl["attainability"] = {
        "advanced_family_runs": len(adv_fam), "advanced_family_mean_entered": a_mean,
        "advanced_family_rate": a_mean / len(all_names),
        "other_non_ws_runs": len(rest), "other_non_ws_mean_entered": r_mean,
        "advanced_family_share_needed_without_ws": p_need,
        "note": "the as-written metric depends mostly on the fuzzer's profile distribution, which is a "
                "fuzzer design choice and not fixed by the definition; ADVANCED-family runs alone average above "
                "60%, so the as-written threshold is attainable by changing the distribution or by "
                "randomising in-scope option flags beyond the D2 profiles"}
    gate_b["verdict"] = (("PASS" if gate_b["pass"] else "FAIL")
                         + " as specified (the Gate 0.3 definition: pass-entry counters in the oracle, average per "
                         f"program, all {len(all_names)} DefaultPassConfig names)")
    gate_b["verdict_basis"] = (
        "(b) is gated only on the measure the Gate 0.3 definition states. The effect measure is "
        "reported next to it (what the inputs make the entered passes do) and does not decide (b); D-016 adds "
        "(b2), a separate criterion on its union, which the gate needs in addition to (b); no "
        "relaxed reading (union over programs, D2-effective denominator) is proposed. The as-written "
        "threshold is attainable: entry depends mainly on the fuzzer's profile distribution, and "
        f"ADVANCED-family runs alone average {pct(a_mean / len(all_names))} of the names"
        + (f" (dropping ws, the average reaches 60% once at least {pct(p_need)} of draws are ADVANCED-family)"
           if p_need is not None and 0 <= p_need <= 1 else "")
        + ". Changing the distribution is a fuzzer design decision, not a change to the gate definition.")

    # (b2), D-016 item 3: the effect-measure union over the fuzz programs (runs where Java did not
    # crash) must cover >= 85% of the D2-effective set. Added to (b), never a substitute for it.
    b2_rate = len(union_no_crash) / len(d2_eff) if d2_eff else 0.0
    b2_cover = n_ok >= 0.95 * gate_a["accepted"] if gate_a["accepted"] else False
    gate_b2 = {
        "decision": "D-016 item 3",
        "metric": "|union over the fuzz programs (runs with exit != 254) of the passes effective on the run "
                  "(printed source changed after the pass; oracle compile_with_pass_dumps) & D2-effective set| "
                  "/ |D2-effective set|",
        "denominator": {"name": "D2-effective set", "size": len(d2_eff), "source": gate_b["denominator_used"]["source"]},
        "reached": len(union_no_crash), "rate": b2_rate, "threshold": THRESH_EFFECT_UNION,
        "reached_including_java_crash_runs": len(union_d2),
        "rate_including_java_crash_runs": union_rate,
        "not_reached": sorted(d2_eff - union_no_crash),
        "ok_runs": n_ok, "accepted_programs": gate_a["accepted"], "coverage_ok": b2_cover,
        "pass": bool(d2_eff) and b2_cover and b2_rate >= THRESH_EFFECT_UNION,
    }
    # D-016 item 1: realised profile distribution of the measured programs vs the documented weights.
    prof_n: dict[str, int] = {}
    for m in manifest:
        prof_n[m["profile"]] = prof_n.get(m["profile"], 0) + 1
    grp = lambda p: "advanced_family" if p in ADV_FAMILY else ("ws" if p == "ws" else "other")  # noqa: E731
    grp_n: dict[str, int] = {}
    for k, v in prof_n.items():
        grp_n[grp(k)] = grp_n.get(grp(k), 0) + v
    n_man = len(manifest) or 1
    gate_b2["profile_distribution"] = {
        "weights": D016_WEIGHTS, "driver_weights": exp.get("profile_weights"),
        "programs": len(manifest),
        "group_share": {g: grp_n.get(g, 0) / n_man for g in D016_WEIGHTS},
        "by_profile": dict(sorted(prof_n.items())),
    }
    gate_b2["option_pool"] = exp.get("option_pool")

    # (c) the 1-hour run
    run = {}
    if os.path.exists(a.run_report):
        with open(a.run_report, encoding="utf-8") as f:
            run = json.load(f)
    exit_code = None
    if os.path.exists(a.run_exit):
        with open(a.run_exit, encoding="utf-8") as f:
            t = f.read().strip()
            exit_code = int(t) if t.lstrip("-").isdigit() else None
    gate_c = {"run_report": os.path.relpath(a.run_report, REPO), "exit_code": exit_code,
              "wall_s": run.get("wall_s"), "harness_crashes": run.get("harness_crashes"),
              "finished": run.get("finished"),
              **{k: run.get(k) for k in ("engine_a", "engine_b", "source", "seed", "servers",
                                         "programs", "parse_rejected", "compared", "mismatches",
                                         "oracle_errors", "oracle_error_samples",
                                         "java_crashes_dropped", "filed", "by_category",
                                         "by_profile")}}
    # The run's start: run-1h.json is written when the run ends, wall_s seconds after it began.
    since = (os.path.getmtime(a.run_report) - float(run["wall_s"]) - 300
             if run and run.get("wall_s") and os.path.exists(a.run_report) else None)
    gate_c["java_crash_classification"] = classify_java_crashes(run, since) if run else {}
    gate_c["pass"] = bool(run) and exit_code == 0 and run.get("finished") is True \
        and run.get("harness_crashes") == 0 and (run.get("wall_s") or 0) >= MIN_RUN_S \
        and (run.get("compared") or 0) > 0
    overall = gate_a["pass"] and gate_b["pass"] and gate_b2["pass"] and gate_c["pass"]
    gate_dir = os.path.dirname(os.path.abspath(a.reach))
    qual = {"note": "diagnostics of the generator and mutator; reported, not gated",
            "current": quality(gate_dir, d2_rows),
            "baseline": quality(rp(a.baseline), d2_rows) if a.baseline else None}
    integ = None
    if os.path.exists(rp(f"{GATE}/integration.json")):
        with open(rp(f"{GATE}/integration.json"), encoding="utf-8") as f:
            integ = json.load(f)
    # Staleness: the measurements are only those of the current generator when the fuzz/ source
    # hash they were keyed on equals the hash of the sources now on disk.
    measured_sha = None
    if os.path.exists(os.path.join(gate_dir, "fuzz-src.sha256")):
        with open(os.path.join(gate_dir, "fuzz-src.sha256"), encoding="utf-8") as f:
            measured_sha = f.read().strip()
    current_sha = fuzz_src_sha()
    stale = measured_sha != current_sha
    out = {"gate": "0.3", "spec": "docs/PORTING.md §4.6", "integration": integ,
           "fuzz_src": {"measured_sha256": measured_sha, "current_sha256": current_sha, "stale": stale},
           "scope_flags": scope_flags_check(),
           "a_parse": gate_a, "b_reach": gate_b, "b2_effect_union": gate_b2,
           "c_run": gate_c, "quality": qual, "pass": overall,
           "status": ("STALE (measurements are of an older generator; re-run gates/gate_0_3.sh) - "
                      if stale else "") + ("PASS" if overall else "FAIL")}
    if stale:
        out["pass"] = False
    if not out["scope_flags"]["up_to_date"]:
        out["pass"] = False
        out["status"] = "FAIL (scope/flags.txt is not what scope/gen_flags.py generates)"
    return out


def scope_flags_check() -> dict:
    """D-016 item 4: scope/flags.txt must be what scope/gen_flags.py generates from the pinned
    CommandLineRunner (the driver checks its option pool against that file at start-up)."""
    import subprocess
    r = subprocess.run([sys.executable, rp("scope/gen_flags.py"), "--check"], capture_output=True, text=True)
    rows = []
    try:
        sys.path.insert(0, rp("scope"))
        import gen_flags  # noqa: E402
        rows = gen_flags.load()
    except Exception:  # noqa: BLE001
        pass
    return {"up_to_date": r.returncode == 0, "message": (r.stdout + r.stderr).strip(),
            "flags": len(rows), "in_scope": sum(1 for x in rows if x.get("scope") == "in"),
            "out_of_scope": [f"{x['flag']}: {x['reason']}" for x in rows if x.get("scope") == "out"]}


def write_quality_md(qual: dict) -> list[str]:
    cur, base = qual.get("current"), qual.get("baseline")
    if not cur:
        return []
    fr = lambda n, d: f"{n}/{d} ({pct(n / d, 0)})" if d else "n/a"  # noqa: E731
    lines = ["## Fuzzer quality (diagnostics; reported, not gated)", "",
             f"Current programs: `{cur['dir']}` ({cur['ok_runs']} reach runs). "
             + (f"Baseline: the pre-fix programs of the same seed, `{base['dir']}` ({base['ok_runs']} reach runs)."
                if base else "No baseline."), "",
             "### jsgen output under lang_es5 / lang_es2015 (target ≥ 80% of runs write a non-empty out.js)", "",
             "| Category | Profile | Output now | Output before |", "|---|---|---:|---:|"]
    for k, v in cur["low_target"]["cells"].items():
        b = (base or {}).get("low_target", {}).get("cells", {}).get(k)
        cat, prof = k.split("|")
        lines.append(f"| {cat} | {prof} | {fr(v['output'], v['runs'])} | {fr(b['output'], b['runs']) if b else 'n/a'} |")
    lt = cur["low_target"]
    lines += ["", f"All jsgen runs under lang_es5/lang_es2015: {fr(lt['output'], lt['runs'])}; "
              f"target {pct(lt['target'], 0)}: {'met' if lt['meets_target'] else 'NOT met'}.", "",
              "### Mutants under the ADVANCED family", "",
              "Meaningful output = out.js of at least 40 bytes without comments and whitespace "
              "(chunk profiles: not measured).", "",
              "| Profile | Exit 0 now | Meaningful output now | Exit 0 before | Meaningful output before |",
              "|---|---:|---:|---:|---:|"]
    for prof, v in cur["mutate_advanced"].items():
        b = (base or {}).get("mutate_advanced", {}).get(prof)
        mo = lambda x: fr(x["meaningful_output"], x["runs"]) if x and x["meaningful_output"] is not None else "n/a"  # noqa: E731
        lines.append(f"| {prof} | {fr(v['exit0'], v['runs'])} | {mo(v)} | "
                     f"{fr(b['exit0'], b['runs']) if b else 'n/a'} | {mo(b)} |")
    lines += ["", "### How often data-flow passes change the output (runs where the pass is effective / runs)", ""]
    for prof, v in cur["dataflow_effect"].items():
        b = (base or {}).get("dataflow_effect", {}).get(prof)
        lines += [f"Profile `{prof}`: {v['runs']} fuzz runs now, {b['runs'] if b else 'n/a'} before, "
                  f"{v['d2_runs']} D2 runs (`build/fuzz/reach/d2-20261006.jsonl`).", "",
                  "| Pass | Fuzz now | Fuzz before | D2 |", "|---|---:|---:|---:|"]
        for p, x in v["passes"].items():
            bx = b["passes"][p] if b else None
            lines.append(f"| {p} | {fr(x['fuzz'], v['runs'])} | {fr(bx['fuzz'], b['runs']) if bx else 'n/a'} | "
                         f"{fr(x['d2'], v['d2_runs'])} |")
        lines.append("")
    lines += ["### Trivial ADVANCED outputs (exit 0, profile `advanced`)", "",
              "Trivial = under 40 bytes once console.log calls and whitespace are removed, or no "
              "function, loop or branch (`function`, `=>`, `class`, a method `name(...){`, `for(`, `while(`, "
              "`do{`, `if(`, `switch(`, `?`, `&&`, `||`) left; a text heuristic.", "",
              "| Category | Trivial now | Median out/in size now | Trivial before | Median out/in size before |",
              "|---|---:|---:|---:|---:|"]
    for cat, v in cur["advanced_trivial"].items():
        b = (base or {}).get("advanced_trivial", {}).get(cat)
        med = lambda x: f"{x['median_size_ratio']:.2f}x" if x and x["median_size_ratio"] is not None else "n/a"  # noqa: E731
        lines.append(f"| {cat} | {fr(v['trivial'], v['exit0_with_output_file'])} | {med(v)} | "
                     f"{fr(b['trivial'], b['exit0_with_output_file']) if b else 'n/a'} | {med(b)} |")
    lines.append("")
    return lines


def write_design_phase_md(b: dict) -> list[str]:
    dp = b.get("design_phase") or {}
    if not dp:
        return []
    den, cal = dp["denominator"], dp["per_run_calibration"]
    la, un = dp["literal_average_per_program"], dp["union_over_programs"]
    out = ["### (b) Diagnostic: effect measure (does not decide (b); its union is gated separately as (b2))", "",
           f"- Diagnostic denominator: the D2-effective set, {den['size']} names. It is the union of the passes that "
           "change printed source on seeded samples of D2 pairs under the D2 profiles "
           f"({b['denominator_used']['source']}). Why: the effect measure counts a pass only when the source "
           "printed after it changes. The other names never changed printed source on any sampled real-world pair "
           "under any D2 profile: they are checks, passes the D2 profiles never enable, or out-of-scope "
           "J2CL/Polymer/Chrome passes. Counting them would measure the profile matrix and the measure's blind "
           "spots, not the generator. It is used only for this diagnostic, never as the gate's denominator; the gate "
           f"uses {den['alternative_reported']}. The set is bounded by the D2 profile list, so passes that only "
           "other in-scope flags enable are missing from it by construction.",
           "- Saturation of the denominator, sample by sample:"]
    for n_s, sm in enumerate(den["samples"]):
        new_names = ("(first sample)" if n_s == 0
                     else ", ".join(sm["new_names_vs_earlier_samples"]) or "none")
        out.append(f"  - `{sm['file']}`: {sm['ok_runs']}/{sm['runs']} runs ok, {sm['effective_names']} effective names, "
                   f"new versus the earlier samples: {new_names}.")
    out += [f"- Average per program (diagnostic): {la['mean_passes']:.2f} effective "
            f"passes per program = {pct(la['rate_vs_d2_effective'])} of {den['size']} and "
            f"{pct(la['rate_vs_all_names'])} of {b['all_names_denominator']}.",
            f"- Union over the programs, Java-crash runs included (reported; (b2) gates the union without them): {un['reached']}/{den['size']} = "
            f"{pct(un['rate_vs_d2_effective'])}; against all {b['all_names_denominator']} names "
            f"{un['reached_all_names']}/{b['all_names_denominator']} = {pct(un['rate_vs_all_names'])}.",
            f"- Per-run calibration: fuzz runs change at most {cal['fuzz_max_per_run']} D2-effective passes "
            f"(p99 {cal['fuzz_p99_per_run']}); real-world D2 runs at most {cal['d2_max_per_run']}, average "
            f"{pct(cal['d2_mean_per_run_vs_d2_effective'])} of the set. This compares generated with real-world "
            "input under the effect measure; it says nothing about the as-written metric, which counts entry.",
            "", "| Profile | D2-effective passes in this profile (D2 samples) | Reached by the fuzz programs (union) |",
            "|---|---:|---:|"]
    for k, v in dp["union_by_profile_vs_profile_d2_effective"].items():
        out.append(f"| {k} | {v['d2_effective_in_profile']} | {v['fuzz_reached']} |")
    out.append("")
    return out


def write_md(r: dict, path: str):
    a, b, c = r["a_parse"], r["b_reach"], r["c_run"]
    ok = lambda x: "PASS" if x else "FAIL"  # noqa: E731
    lines = [
        "# Gate 0.3: fuzzer (docs/PORTING.md §4.6)", "",
        f"**Overall: {r.get('status', ok(r['pass']))}**", "",
        *([f"> **Stale:** these measurements were taken with fuzz/ sources {(r['fuzz_src']['measured_sha256'] or 'unknown')[:12]}; "
           f"the sources on disk are {r['fuzz_src']['current_sha256'][:12]}. The verdict logic is current, the numbers "
           "are not; the gate does not pass until `gates/gate_0_3.sh` re-measures the current generator.", ""]
          if (r.get("fuzz_src") or {}).get("stale") else []),
        "| Check | Result | Threshold | Verdict |", "|---|---|---|---|",
        f"| (a) Java-parseable programs | {a['accepted']}/{a['programs']} ({pct(a['rate'], 2)}) | ≥ 95% of 5,000 | {ok(a['pass'])} |",
        f"| **(b) gated:** pass-entry counters in the oracle (the Gate 0.3 definition), average per program, vs all {b['all_names_denominator']} DefaultPassConfig names | {(f"{b['spec_literal']['mean_entered']:.1f}/{b['all_names_denominator']} ({pct(b['spec_literal']['rate'])}), {b['spec_literal']['measured_runs']} runs") if b['spec_literal']['measured_runs'] else 'not measured'} | ≥ 60% | {ok(b['pass_as_written'])} |",
        f"| (b) reported, not gated (the definition says \"on average\"): pass-entry counters, union over the {b['spec_literal']['measured_runs']} programs, vs all {b['all_names_denominator']} names | {b['spec_literal'].get('union_entered', 'n/a')}/{b['all_names_denominator']} ({pct(b['spec_literal'].get('union_rate', 0.0))}) | n/a | n/a |",
        f"| (b) diagnostic, not gated: effect measure, average per program, vs the D2-effective set ({b['denominator_used']['size']} names) / all names | {b['mean_effective_passes_per_program']:.2f} passes = {pct(b['mean_per_program_vs_d2_effective'])} / {pct(b['mean_per_program_vs_all_names'])} | n/a | n/a |",
        f"| (b) diagnostic, not gated: effect measure, union over the {b['ok_runs']} programs (Java-crash runs included; see (b2)), vs the D2-effective set / all names | {b['union_d2_effective_reached']}/{b['denominator_used']['size']} ({pct(b['union_rate_vs_d2_effective'])}) / {b['union_all_names_reached']}/{b['all_names_denominator']} ({pct(b['union_rate_vs_all_names'])}) | n/a | n/a |",
        f"| **(b2) gated (D-016):** effect measure, union over the programs (runs where Java did not crash), vs the D2-effective set | {r['b2_effect_union']['reached']}/{r['b2_effect_union']['denominator']['size']} ({pct(r['b2_effect_union']['rate'])}) | ≥ 85% | {ok(r['b2_effect_union']['pass'])} |",
        f"| (c) Driver against the Java oracle for 1 hour | {c.get('wall_s') and round(c['wall_s'])} s, {c.get('harness_crashes')} harness crashes, exit {c.get('exit_code')} | ≥ 3,600 s, 0 crashes | {ok(c['pass'])} |",
        "",
    ]
    integ = r.get("integration") or {}
    if integ:
        stale_note = ([f"_These integration notes were written for fuzz/ sources "
                       f"{(r['fuzz_src']['measured_sha256'] or '?')[:12]}, which are no longer the sources on disk "
                       "(see the stale notice above)._", ""] if (r.get("fuzz_src") or {}).get("stale") else [])
        lines += [f"## Integration ({integ.get('round', 'gate run')})", ""] + stale_note + [f"- {x}" for x in integ.get("lines", [])] + [""]
    lines += [
        "## (a) Parse acceptance", "",
        f"Programs 0..{a['programs'] - 1} of `fuzz-driver export --seed {a['seed']} --source {a['source']}`, "
        "the same programs `fuzz-driver run` draws for that seed. Method: " + a["method"] + ".", "",
        "| Category | Programs | Accepted | Rate |", "|---|---:|---:|---:|",
    ]
    for k, v in a["by_category"].items():
        lines.append(f"| {k} | {v['programs']} | {v['accepted']} | {pct(v['rate'])} |")
    lines += ["", "Mix (`--source mixed`, per program index from a seeded fork): 30% lang (jsgen, "
              "closure off), 25% closure (jsgen single file, closure on), 15% multi (jsgen "
              "multi-file goog.module/goog.provide/ES module/CommonJS programs; chunk profiles "
              "apply), 5% sloppy (jsgen sloppy dialect with `--strict_mode_input=false`), 25% "
              "mutate (mutator over visible single-input D2 files; mutants compiled under an "
              "ADVANCED-family profile draw only from files whose D2 golden ADVANCED run exits 0 "
              "with non-trivial output; donor splices whose free names are neither known globals "
              "nor bound in the host are rejected). Single-file jsgen programs whose profile has "
              "--language_out below ES2018 are generated without BigInt, ES2018+ regexp features "
              "or new.target (origin tag `:low_target`). The "
              "`unsupported` dialect (private class elements) is excluded: every CLI mode "
              "reports JSC_UNSUPPORTED_LANGUAGE_FEATURE for it.", "",
              f"Rejected programs: {len(a['rejected'])}."]
    for x in a["rejected"][:20]:
        lines.append(f"- {x['index']} ({x['category']}, `{x['origin']}`): {x['errors'][:1] or x['oracle_error']}")
    sl = b["spec_literal"]
    lines += ["", "## (b) Pass reach", "",
              "Two measures, each reported both as the average per program and as the union over the programs:",
              f"- gated: pass-entry counters in the oracle (`--tracer_mode=TIMING_ONLY`), the measure the Gate 0.3 definition names, "
              f"against all {b['all_names_denominator']} unique DefaultPassConfig names, because the definition says \"the passes in "
              "`DefaultPassConfig`\" and names no subset;",
              "- diagnostic: the effect measure (`fuzz/DESIGN.md`, `gates/lib/fuzz_reach.py`): "
              + b["definition"] + f", against the D2-effective set ({b['denominator_used']['size']} names) and against all names.",
              "",
              *([f"The per-program average is below 60% under both measures (entry: {pct(sl['rate'])} of "
                 f"{b['all_names_denominator']} names; effect: {pct(b['mean_per_program_vs_d2_effective'])} of the "
                 f"D2-effective set, {pct(b['mean_per_program_vs_all_names'])} of all names), so the FAIL of (b) does not "
                 "depend on which definition is chosen. Only the union readings exceed 60% "
                 f"(entry {pct(sl.get('union_rate', 0.0))}, effect {pct(b['union_rate_vs_d2_effective'])} of the D2-effective "
                 "set), and the definition says \"on average\".", ""]
                if sl.get("measured_runs") and not b["pass_as_written"]
                and b["mean_per_program_vs_d2_effective"] < 0.6 else []),
              f"**Verdict for (b): {b['verdict']}.** {b['verdict_basis']}", "",
              ] + write_design_phase_md(b) + [
              "### (b) Gated: pass-entry counters (the Gate 0.3 definition)", "",
              f"- Metric: {b['spec_literal']['metric']}.",
              f"- Measured: {b['spec_literal']['measured_runs']} runs ok, {b['spec_literal']['n_failed']} failed; results in `{b['spec_literal']['results']}`.",
              f"- Average entered per program: {b['spec_literal']['mean_entered']:.2f} of {b['all_names_denominator']} = {pct(b['spec_literal']['rate'])} (threshold 60%): {ok(b['pass_as_written'])}.",
              *([f"- Union over the programs (reported, not gated; the definition says \"on average\"): "
                 f"{b['spec_literal']['union_entered']} of {b['all_names_denominator']} names entered by at least one program "
                 f"= {pct(b['spec_literal']['union_rate'])}. Per profile: "
                 + ", ".join(f"{k} {v}" for k, v in b['spec_literal']['union_by_profile'].items()) + ".",
                 f"- Names no program entered ({len(b['spec_literal']['never_entered'])}): "
                 + ", ".join(f"`{n}`" for n in b['spec_literal']['never_entered']) + "."]
                if 'union_entered' in b['spec_literal'] else []),
              f"- Failed runs, left out of the average: {b['spec_literal'].get('failed_java_crash_exit_254', 0)} where Java crashed "
              f"(exit 254, D-009; Java prints no tracer summary when it crashes) and {b['spec_literal'].get('failed_other', 0)} other.",
              "- Which passes are entered is set mainly by the options (the profile plus any drawn option flags). "
              "The input moves the count mostly through errors, in two different ways. An error whose diagnostic type "
              "is ERROR by default halts the pass loop early and lowers the count. A warning promoted to an error "
              "(advanced_strict's `--jscomp_error=*`) does not halt it: the compile runs every optimization pass, "
              "then writes no output and exits with the error count (Java: SortingErrorManager.hasHaltingErrors, "
              "Result.success). See \"Runs without comparable output\" below. "
              + (f"{b['spec_literal']['runs_below_profile_max']} of {b['spec_literal']['measured_runs']} runs entered fewer names "
                 f"than the most their profile reached, on average {b['spec_literal']['mean_shortfall_below_profile_max']:.1f} fewer. "
                 if b['spec_literal'].get('mean_shortfall_below_profile_max') is not None else "")
              + (f"Ceiling under the fuzzer's current profile mix ({b['spec_literal']['ceiling_definition']}): "
                 f"{b['spec_literal']['ceiling_mean_entered']:.1f}/{b['all_names_denominator']} = {pct(b['spec_literal']['ceiling_rate'])}, "
                 f"{'above' if b['spec_literal']['ceiling_rate'] >= THRESH_REACH else 'below'} the 60% threshold. This ceiling "
                 "belongs to the profile distribution the fuzzer chooses, not to the definition. "
                 if 'ceiling_rate' in b['spec_literal'] else "")
              + ((lambda at: f"**Attainability:** ADVANCED-family runs ({at['advanced_family_runs']}) average "
                 f"{at['advanced_family_mean_entered']:.1f} names ({pct(at['advanced_family_rate'])}); the other non-ws runs "
                 f"({at['other_non_ws_runs']}) average {at['other_non_ws_mean_entered']:.1f}. "
                 + (f"Without ws, the average reaches 60% once at least {pct(at['advanced_family_share_needed_without_ws'])} "
                    "of draws are ADVANCED-family. "
                    if at.get('advanced_family_share_needed_without_ws') is not None else "")
                 + "So the as-written threshold can be met by a different profile distribution or by randomising "
                 "in-scope option flags; ratifying a relaxed reading is not the only path, and none is proposed. ")
                 (b['spec_literal']['attainability']) if b['spec_literal'].get('attainability') else "")
              + (f"Estimated ceiling for the real-world D2 corpus under its own profile matrix (each pair "
                 f"weighted by the largest count measured for its profile here): "
                 f"{b['spec_literal']['d2_ceiling_estimate_mean_entered']:.1f}/{b['all_names_denominator']} = "
                 f"{pct(b['spec_literal']['d2_ceiling_estimate_rate'])}. "
                 if b['spec_literal'].get('d2_ceiling_estimate_mean_entered') else "")
              + "Profiles whose mean entered count is at least 60% of the names "
              "in these runs: " + (", ".join(k for k, v in b["spec_literal"]["by_profile"].items()
                                             if v["mean_entered"] >= THRESH_REACH * b["all_names_denominator"])
                                   or "none") + ".", "",
              "| Profile | Runs | Mean entered | Min | Max | Share of all names | Exit 0 runs | Mean entered (exit 0) |",
              "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for k, v in b["spec_literal"]["by_profile"].items():
        e0 = v.get("mean_entered_exit0")
        lines.append(f"| {k} | {v['runs']} | {v['mean_entered']:.1f} | {v['min']} | {v['max']} | "
                     f"{pct(v['mean_entered'] / b['all_names_denominator'])} | {v.get('exit0_runs', 'n/a')} | "
                     f"{f'{e0:.1f}' if e0 is not None else 'n/a'} |")
    oc = b["spec_literal"].get("output_comparability")
    if oc and oc.get("exit0_runs"):
        f1 = lambda x: f"{x:.2f}" if x is not None else "n/a"  # noqa: E731
        bp = lambda d: ", ".join(f"{k} {v}" for k, v in d.items()) or "none"  # noqa: E731
        nms = b["all_names_denominator"]
        lines += ["", "### (b) Diagnostic: runs without comparable output (does not change (b))", "",
                  "A run that exits non-zero writes no output, so the differential comparison sees only its "
                  "diagnostics and exit code, never its optimized code. (b) counts every measured run, as "
                  "the Gate 0.3 definition and D-016 define it; the figures below show how much of the (b) result rests on "
                  "runs whose output is never compared.", "",
                  f"- Runs with a non-zero exit: {oc['nonzero_runs']} of {b['spec_literal']['measured_runs']} measured "
                  f"({pct(oc['nonzero_share_of_measured'])}), mean entered {f1(oc['nonzero_mean_entered'])} names. "
                  f"By profile: {bp(oc['nonzero_by_profile'])}.",
                  f"  - Reached the end of their profile's pass pipeline ({oc['full_pipeline_marker']}), then had their output "
                  f"suppressed: {oc['nonzero_full_pipeline_runs']}, mean entered {f1(oc['nonzero_full_pipeline_mean_entered'])}. "
                  f"By profile: {bp(oc['nonzero_full_pipeline_by_profile'])}. These are errors promoted from warnings, "
                  "which do not halt the pass loop.",
                  f"  - Halted early: {oc['nonzero_halted_early_runs']}, mean entered "
                  f"{f1(oc['nonzero_halted_early_mean_entered'])}. By profile: {bp(oc['nonzero_halted_early_by_profile'])}.",
                  f"- Runs with exit 0 (output written and compared): {oc['exit0_runs']}, mean entered "
                  f"{f1(oc['exit0_mean_entered'])} of {nms} = {pct(oc['exit0_rate'])}, "
                  f"{'at or above' if oc['exit0_rate_at_or_above_60'] else 'below'} the 60% threshold. This is the (b) "
                  "mean restricted to runs whose output is compared; it is a diagnostic, not the gated metric.",
                  (f"- Output files under entry-out/: {oc['runs_with_output_files']} runs have them; non-zero-exit runs "
                   f"with output files: {oc['nonzero_runs_with_output_files']}; exit-0 runs without: "
                   f"{oc['exit0_runs_without_output_files']}."
                   if oc.get("output_files_checked") else
                   "- Output files: not checked (entry-out/ is missing)."),
                  "- Failing runs enter "
                  + ("more" if (oc["nonzero_mean_entered"] or 0) > (oc["exit0_mean_entered"] or 0) else "fewer")
                  + " names on average than exit-0 runs. "
                  + ("The (b) PASS therefore depends on runs whose optimized output the differential comparison "
                     "never sees: over the exit-0 runs alone the mean is below 60%. "
                     if b["pass_as_written"] and not oc["exit0_rate_at_or_above_60"] else "")
                  + "A strict-aware generation mode (not built yet) would target the advanced_strict share."]
    lines += ["", "### (b) Diagnostic: effect-measure details (the union is gated as (b2); the rest is diagnostic)", "",
              "The effect measure (`fuzz/DESIGN.md`) counts *effect* (the printed source changes after the pass). "
              "It shows what the generated inputs make the entered passes do, and it is blind to passes that "
              "report diagnostics instead of changing source (checks). It is not the measure the Gate 0.3 definition names, "
              "so it does not decide (b), and no reading built on it (union over the programs, a reduced denominator) is "
              "proposed as a substitute for the as-written metric.", "",
              f"- Measured: {b['ok_runs']} of {b['programs_measured']} runs ok (failed: {len(b['failed_runs'])}).",
              f"- Union of D2-effective passes reached (diagnostic): {b['union_d2_effective_reached']}/{b['denominator_used']['size']} = {pct(b['union_rate_vs_d2_effective'])}.",
              f"- Runs where Java crashed (exit 254, D-009): {b['java_crash_runs']}. They still count the passes that changed the source before the crash; without them the union is {b['union_d2_effective_reached_excluding_java_crash_runs']}/{b['denominator_used']['size']} = {pct(b['union_rate_vs_d2_effective_excluding_java_crash_runs'])}.",
              f"- Denominator: {b['denominator_used']['name']} ({b['denominator_used']['size']} names), from {b['denominator_used']['source']}. Why: {b['denominator_used']['why']}.",
              f"- Union against all {b['all_names_denominator']} DefaultPassConfig names ({b['factories']} factories): {b['union_all_names_reached']}/{b['all_names_denominator']} = {pct(b['union_rate_vs_all_names'])}.",
              f"- Average per program (diagnostic): {b['mean_effective_passes_per_program']:.2f} effective passes per program, {pct(b['mean_per_program_vs_d2_effective'])} of the D2-effective set, {pct(b['mean_per_program_vs_all_names'])} of all names.",
              f"- Calibration: the real-world D2 sample itself averages {b['d2_calibration']['mean_effective_passes_per_pair']:.2f} effective passes per pair = {pct(b['d2_calibration']['mean_per_pair_vs_d2_effective'])} of its own effective set and {pct(b['d2_calibration']['mean_per_pair_vs_all_names'])} of all names; its union over all profiles reaches {len(b['d2_effective_not_reached']) + b['union_d2_effective_reached']}/{b['all_names_denominator']} = {pct(b['d2_calibration']['union_vs_all_names'])} of all names.",
              f"- D2-effective passes not reached: {', '.join(b['d2_effective_not_reached']) or 'none'}.",
              f"- Reached outside the D2-effective set: {', '.join(b['reached_outside_d2_effective']) or 'none'}.",
              "", "| Category | D2-effective passes reached (union) |", "|---|---:|"]
    for k, v in b["union_by_category_vs_d2_effective"].items():
        lines.append(f"| {k} | {v} |")
    lines += ["", "| Profile | D2-effective passes reached (union) |", "|---|---:|"]
    for k, v in b["union_by_profile_vs_d2_effective"].items():
        lines.append(f"| {k} | {v} |")
    lines += ["", "Pass hit counts (programs on which the pass was effective):", ""]
    lines.append(", ".join(f"`{k}` {v}" for k, v in b["pass_hit_counts"].items()))
    b2 = r["b2_effect_union"]
    pdist = b2.get("profile_distribution") or {}
    lines += ["", "## (b2) Effect-measure union (D-016, gated in addition to (b))", "",
              f"- Metric: {b2['metric']}.",
              f"- Reached: {b2['reached']}/{b2['denominator']['size']} = {pct(b2['rate'])} (threshold 85%): {ok(b2['pass'])}. "
              f"With Java-crash runs counted: {b2['reached_including_java_crash_runs']}/{b2['denominator']['size']} = "
              f"{pct(b2['rate_including_java_crash_runs'])} (reported, not gated).",
              f"- Not reached: {', '.join(b2['not_reached']) or 'none'}.",
              f"- Runs: {b2['ok_runs']} ok of {b2['accepted_programs']} accepted programs (coverage ≥ 95%: {ok(b2['coverage_ok'])}).",
              f"- Denominator: {b2['denominator']['name']} ({b2['denominator']['size']} names), {b2['denominator']['source']}.",
              "- (b) above is unchanged (the Gate 0.3 definition); (b2) is an added criterion, so the gate needs both.",
              ""]
    if pdist:
        lines += ["Profile distribution of the measured programs (D-016 item 1; driver weights: ADVANCED family "
                  "65%, ws 3%, other 32%, uniform within a group over the applicable profiles):", "",
                  "| Group | D-016 weight | Realised share |", "|---|---:|---:|"]
        for g, w in pdist["weights"].items():
            lines.append(f"| {g} | {pct(w)} | {pct(pdist['group_share'].get(g, 0.0))} |")
        lines += ["", "By profile: " + ", ".join(f"{k} {v}" for k, v in pdist["by_profile"].items()) + ".", ""]
    sf = r.get("scope_flags") or {}
    if sf:
        lines += [f"scope/flags.txt (D-016 item 4): {sf.get('flags')} CommandLineRunner flags, {sf.get('in_scope')} in scope; "
                  f"generator check: {'up to date' if sf.get('up_to_date') else 'OUT OF DATE'}. Out of scope: "
                  + "; ".join(sf.get("out_of_scope") or []) + ".", ""]
    lines += ["", "## (c) One-hour driver run", "",
              f"`fuzz-driver run --duration 3600 --source {c.get('source')} --engine-b {c.get('engine_b')} --seed {c.get('seed')} --servers {c.get('servers')}`: "
              f"engine A = Java oracle, engine B = a second independent Java oracle JVM, all servers in the golden environment with the ready line checked (D-010).", "",
              f"- Wall time: {c.get('wall_s')} s; exit code {c.get('exit_code')}; harness crashes (panicked worker threads): {c.get('harness_crashes')}.",
              f"- Programs: {c.get('programs')}; parse-rejected: {c.get('parse_rejected')}; compared: {c.get('compared')}; mismatches: {c.get('mismatches')}; oracle errors: {c.get('oracle_errors')}; Java crashes dropped (D-009): {len(c.get('java_crashes_dropped') or [])}.",
              f"- By profile: {', '.join(f'{k} {v}' for k, v in (c.get('by_profile') or {}).items()) or 'n/a'}.",
              f"- Findings filed: {c.get('filed')}.",
              ""]
    if c.get("oracle_error_samples"):
        lines += ["Oracle error samples:", ""] + [f"- {s}" for s in c["oracle_error_samples"][:10]] + [""]
    jc = c.get("java_crash_classification") or {}
    if jc:
        lines += ["Java crashes, re-run with the reference CLI to rule out harness faults "
                  f"({jc.get('method')}):", "",
                  f"- Dropped by the run: {jc.get('dropped')}; re-run: {jc.get('classified')}; "
                  f"reproduced (exit 254 / INTERNAL COMPILER ERROR): {jc.get('reproduced')}; "
                  f"StackOverflowError: {jc.get('stack_overflow')}; OutOfMemoryError: {jc.get('oom')}.",
                  f"- By exception class: {', '.join(f'{k} {v}' for k, v in (jc.get('by_exception_class') or {}).items()) or 'n/a'}.",
                  f"- Not reproduced (needs a look; possible oracle-side behaviour): {jc.get('not_reproduced') or 'none'}.",
                  f"- Kept program missing: {jc.get('missing_program_dir') or 'none'}; not classified: "
                  f"{jc.get('not_classified', 'n/a')}; kept directories from older runs of the same seed, ignored: "
                  f"{jc.get('kept_dirs_from_older_runs_ignored', 'n/a')} (the cache is keyed on the program's content hash)."]
        if jc.get("error"):
            lines.append(f"- Classification error: {jc['error']}.")
        lines.append("")
    if c.get("java_crashes_dropped"):
        lines += ["Java crashes (kept under build/fuzz/java-crashes/, not porting defects):", ""] + \
                 [f"- {s}" for s in c["java_crashes_dropped"][:20]] + [""]
    lines += write_quality_md(r.get("quality") or {})
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")


def out_base(results: str) -> str:
    """Repository-relative directory of a results file (the gate's own: build/fuzz/gate)."""
    d = os.path.dirname(os.path.abspath(results))
    return os.path.relpath(d, REPO)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("reach")
    r.add_argument("--manifest", required=True)
    r.add_argument("--results", default=rp(f"{GATE}/reach.jsonl"))
    r.add_argument("--servers", type=int, default=3)
    e = sub.add_parser("entry")
    e.add_argument("--manifest", required=True)
    e.add_argument("--results", default=rp(f"{GATE}/entry.jsonl"))
    e.add_argument("--servers", type=int, default=3)
    p = sub.add_parser("report")
    p.add_argument("--export-dir", default=rp(f"{GATE}/export"))
    p.add_argument("--reach", default=rp(f"{GATE}/reach.jsonl"))
    p.add_argument("--entry", default=rp(f"{GATE}/entry.jsonl"))
    p.add_argument("--run-report", default=rp(f"{GATE}/run-1h.json"))
    p.add_argument("--run-exit", default=rp(f"{GATE}/run-1h.exit"))
    p.add_argument("--d2", action="append", default=None,
                   help="D2 pass-dump sample(s); the D2-effective set is their union (repeatable)")
    p.add_argument("--baseline", default=BASELINE_DIR,
                   help="gate directory of the pre-fix programs, for the quality comparison ('' = none)")
    p.add_argument("--out", default=rp("gates/reports/gate_0_3"))
    args = ap.parse_args()
    if getattr(args, "d2", "x") is None:
        args.d2 = list(D2_SAMPLES)
    os.chdir(REPO)
    if args.cmd == "reach":
        fuzz_reach.pool_run(reach_jobs(load_manifest(args.manifest), out_base=out_base(args.results)),
                            args.results, args.servers)
    elif args.cmd == "entry":
        fuzz_reach.pool_run(reach_jobs(load_manifest(args.manifest), entry=True,
                                       out_base=out_base(args.results)), args.results, args.servers)
    else:
        rep = report(args)
        with open(args.out + ".json", "w", encoding="utf-8") as f:
            json.dump(rep, f, indent=1)
        write_md(rep, args.out + ".md")
        print(json.dumps({**{k: rep[k]["pass"] if isinstance(rep[k], dict) else rep[k]
                             for k in ("a_parse", "b_reach", "b2_effect_union", "c_run", "pass")},
                          "status": rep["status"]}))


if __name__ == "__main__":
    main()
