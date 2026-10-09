#!/usr/bin/env python3
"""closure-rs: run the D7 benchmarks (bench/projects.json) on Java and on the Rust CLI.

  python3 scripts/run_bench.py [--reps N] [--project P]... [--job REGEX] [--level L]...
                               [--bin PATH] [--java PATH] [--jar PATH] [--impl both|java|rust]
                               [--timeout S] [--out FILE] [--keep-failing] [--no-save] [--print-args]

docs/PORTING.md §3, D7: on every benchmark, the Rust CLI's wall-clock time <= Java's (a cold
`java -jar closure-compiler.jar`, as users run it) and its peak RSS <= Java's. See bench/README.md.

For every job x compilation level the compiler argv is built from bench/projects.json (globs are
expanded here, sorted, into one --js flag per file, so both compilers see the same input list)
and run with cwd = the repository root, a fixed minimal environment and stdin /dev/null. Each
repetition runs Java, then Rust, under /usr/bin/time (a small process, so its child's ru_maxrss
is the compiler's own peak RSS); wall time is measured around it. The medians of the
repetitions are reported. Every run's exit code, stdout, stderr, output file and the other
files it writes next to it (a source map) are compared byte for byte with Java's first run;
a difference is a porting defect to report, not a benchmark failure.

Writes bench/results/<UTC date>-<commit>.json (unless --no-save) and prints a Markdown table.
With --keep-failing, the outputs of mismatching jobs go to build/bench/failing/<job id>/.
Exit code: 0 = all runs completed (mismatches or not), 2 = setup error (missing input/binary).
"""

from __future__ import annotations

import argparse
import datetime
import glob
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import statistics
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import paths  # noqa: E402  the reference (scripts/paths.py, scripts/references.tsv)
# The reference uberjar of the D2 golden pipeline (oracle/REFERENCE.md, gates/lib/run_reference.py)
JAVA_REL = "tools/jdk-21/bin/java"
JAR_REL = paths.REF.jar  # build/reference-v20261006/closure-compiler.jar for the default reference
JAR_SHA256 = paths.REF.jar_sha256  # "-" while not pinned: only --jar runs then
TIME = "/usr/bin/time"


def child_env() -> dict[str, str]:
    """Fixed, minimal: nothing inherited (JAVA_TOOL_OPTIONS would change Java and print to
    stderr; RUST_* would change Rust), as for the D2 golden runs."""
    return {"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/tmp"),
            "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TZ": "UTC"}


def data_root(explicit: str | None) -> str:
    """Where tools/ and build/reference/ live: --data-root, $CLOSURE_RS_DATA_ROOT, this checkout
    if it has the jar, else the main checkout of the git repository (for worktrees)."""
    for d in (explicit, os.environ.get("CLOSURE_RS_DATA_ROOT")):
        if d:
            return os.path.abspath(d)
    if os.path.isfile(os.path.join(ROOT, JAR_REL)):
        return ROOT
    try:
        common = subprocess.run(["git", "-C", ROOT, "rev-parse", "--path-format=absolute",
                                 "--git-common-dir"], capture_output=True, text=True,
                                check=True).stdout.strip()
        return os.path.dirname(common)
    except (OSError, subprocess.CalledProcessError):
        return ROOT


def sha256_file(p: str) -> str:
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for blk in iter(lambda: f.read(1 << 20), b""):
            h.update(blk)
    return h.hexdigest()


def expand(patterns: list[str]) -> list[str]:
    files: list[str] = []
    for pat in patterns:
        hits = sorted(glob.glob(pat, root_dir=ROOT, recursive=True))
        hits = [h for h in hits if os.path.isfile(os.path.join(ROOT, h))]
        if not hits:
            raise SystemExit(f"no input matches {pat!r}; run scripts/fetch_bench.sh")
        files += [h for h in hits if h not in files]
    return files


def load_jobs(spec: dict) -> list[dict]:
    jobs = []
    for proj in spec["projects"]:
        for job in proj["jobs"]:
            for level in spec["levels"]:
                entry = job["entry_point"]
                js = expand(job["js"])
                if level == "ADVANCED" and job.get("advanced_entry_point"):
                    entry = job["advanced_entry_point"]
                    js = [entry] + js
                jobs.append({"project": proj["name"], "job": job["name"], "level": level,
                             "id": f"{proj['name']}/{job['name']}/{level}"
                                   if proj["name"] != job["name"] else f"{job['name']}/{level}",
                             "entry_point": entry, "js": js,
                             "flags": [f"--compilation_level={level}", *proj["flags"],
                                       f"--entry_point={entry}"]})
    return jobs


def compiler_args(job: dict, out_file: str) -> list[str]:
    return [*job["flags"], *(f"--js={f}" for f in job["js"]), f"--js_output_file={out_file}"]


def run_once(cmd: list[str], out_rel: str, timeout: float) -> dict:
    """One measured run. Returns wall_s, rss_kb, user_s, sys_s, exit, stdout, stderr, output."""
    out_abs = os.path.join(ROOT, out_rel)
    shutil.rmtree(os.path.dirname(out_abs), ignore_errors=True)
    os.makedirs(os.path.dirname(out_abs))
    tfile = os.path.join(os.path.dirname(out_abs), "..", os.path.basename(out_abs) + ".time")
    argv = [TIME, "-q", "-o", tfile, "-f", "%x %M %U %S", *cmd]
    t0 = time.perf_counter()
    p = subprocess.Popen(argv, cwd=ROOT, env=child_env(), stdin=subprocess.DEVNULL,
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = p.communicate(timeout=timeout)
        timed_out = False
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)  # the compiler too, not only /usr/bin/time
        stdout, stderr = p.communicate()
        timed_out = True
    wall = time.perf_counter() - t0
    r = {"wall_s": round(wall, 4), "timed_out": timed_out, "stdout": stdout, "stderr": stderr}
    try:
        with open(tfile) as f:
            line = f.read().strip().splitlines()[-1].split()
        os.remove(tfile)
        r.update(exit=int(line[0]), rss_kb=int(line[1]), user_s=float(line[2]),
                 sys_s=float(line[3]))
    except (OSError, IndexError, ValueError):
        r.update(exit=None if timed_out else p.returncode, rss_kb=None, user_s=None, sys_s=None)
    try:
        with open(out_abs, "rb") as f:
            r["output"] = f.read()
    except OSError:
        r["output"] = None
    # other files the compile wrote next to the output (a --create_source_map=%outname%.map
    # source map): name -> contents, compared like the output
    extra = {}
    for name in sorted(set(os.listdir(os.path.dirname(out_abs))) - {os.path.basename(out_abs)}):
        path = os.path.join(os.path.dirname(out_abs), name)
        if os.path.isfile(path):
            with open(path, "rb") as f:
                extra[name] = f.read()
        else:
            extra[name] = None
    r["extra_files"] = extra
    shutil.rmtree(os.path.dirname(out_abs), ignore_errors=True)
    return r


def first_diff(a: bytes | None, b: bytes | None) -> dict | None:
    if a == b:
        return None
    if a is None or b is None:
        return {"java_len": None if a is None else len(a), "rust_len": None if b is None else len(b)}
    n = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
    line = a[:n].count(b"\n") + 1
    col = n - (a.rfind(b"\n", 0, n) + 1)
    return {"offset": n, "line": line, "col": col, "java_len": len(a), "rust_len": len(b),
            "java_ctx": a[max(0, n - 60):n + 60].decode("utf-8", "replace"),
            "rust_ctx": b[max(0, n - 60):n + 60].decode("utf-8", "replace")}


def compare(ref: dict, r: dict) -> dict:
    d = {}
    if ref["exit"] != r["exit"] or r["timed_out"]:
        d["exit"] = {"java": ref["exit"], "rust": r["exit"], "timed_out": r["timed_out"]}
    for k in ("stdout", "stderr", "output"):
        fd = first_diff(ref[k], r[k])
        if fd:
            d[k] = fd
    if sorted(ref["extra_files"]) != sorted(r["extra_files"]):
        d["extra_files"] = {"java": sorted(ref["extra_files"]), "rust": sorted(r["extra_files"])}
    else:
        for name in ref["extra_files"]:
            fd = first_diff(ref["extra_files"][name], r["extra_files"][name])
            if fd:
                d["file:" + name] = fd
    return d


def median(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def git_head() -> str:
    try:
        return subprocess.run(["git", "-C", ROOT, "rev-parse", "--short=10", "HEAD"],
                              capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def fmt_s(x):
    return "-" if x is None else f"{x:.2f}"


def fmt_mb(kb):
    return "-" if kb is None else f"{kb / 1024:.0f}"


def ratio(r, j):
    return "-" if r is None or not j else f"{r / j:.2f}"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[1],
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--project", action="append", default=[])
    ap.add_argument("--job", help="regex over job ids (project/job/LEVEL)")
    ap.add_argument("--level", action="append", default=[])
    ap.add_argument("--impl", choices=("both", "java", "rust"), default="both")
    ap.add_argument("--bin", default=os.path.join(ROOT, "target/release/closure-rs"))
    ap.add_argument("--data-root")
    ap.add_argument("--java")
    ap.add_argument("--jar")
    ap.add_argument("--timeout", type=float, default=900)
    ap.add_argument("--out", help="result JSON (default bench/results/<date>-<commit>.json)")
    ap.add_argument("--no-save", action="store_true")
    ap.add_argument("--keep-failing", action="store_true")
    ap.add_argument("--print-args", action="store_true",
                    help="print each selected job's compiler argv (one per line) and exit")
    a = ap.parse_args()

    with open(os.path.join(ROOT, "bench/projects.json"), encoding="utf-8") as f:
        spec = json.load(f)
    jobs = [j for j in load_jobs(spec)
            if (not a.project or j["project"] in a.project)
            and (not a.level or j["level"] in a.level)
            and (not a.job or re.search(a.job, j["id"]))]
    if not jobs:
        print("no job selected", file=sys.stderr)
        return 2
    if a.print_args:
        for job in jobs:
            print("\n".join(compiler_args(job, f"build/bench/{job['id'].replace('/', '-')}.js")))
        return 0
    if not os.access(TIME, os.X_OK):
        print(f"{TIME} (GNU time) is required to measure peak RSS", file=sys.stderr)
        return 2
    droot = data_root(a.data_root)
    java = a.java or os.path.join(droot, JAVA_REL)
    jar = a.jar or os.path.join(droot, JAR_REL)
    impls = ["java", "rust"] if a.impl == "both" else [a.impl]
    meta = {"schema": 1, "date_utc": datetime.datetime.now(datetime.timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"), "commit": git_head(), "reps": a.reps,
            "host": {"machine": platform.machine(), "kernel": platform.release(),
                     "cpus": os.cpu_count(), "loadavg_start": os.getloadavg(),
                     "nice": os.nice(0)}}
    if "java" in impls:
        if not (os.access(java, os.X_OK) and os.path.isfile(jar)):
            print(f"Java or the jar is missing: {java} {jar}", file=sys.stderr)
            return 2
        jsha = sha256_file(jar)
        if jsha != JAR_SHA256 and not a.jar:
            print(f"{jar}: sha256 {jsha}, expected {JAR_SHA256} (oracle/REFERENCE.md)",
                  file=sys.stderr)
            return 2
        jv = subprocess.run([java, "-version"], capture_output=True, text=True, env=child_env())
        meta["java"] = {"java": java, "jar": jar, "jar_sha256": jsha,
                        "version": jv.stderr.strip().splitlines()[0] if jv.stderr else ""}
    if "rust" in impls:
        if not os.access(a.bin, os.X_OK):
            print(f"no Rust binary at {a.bin}: cargo build --release -p closure-cli",
                  file=sys.stderr)
            return 2
        meta["rust"] = {"bin": os.path.abspath(a.bin), "bin_sha256": sha256_file(a.bin)}

    cmds = {"java": [java, "-jar", jar], "rust": [a.bin]}
    results = []
    for n, job in enumerate(jobs, 1):
        out_rel = f"build/bench/run/{job['project']}-{job['job']}-{job['level']}/out.js"
        args = compiler_args(job, out_rel)
        runs = {i: [] for i in impls}
        for rep in range(a.reps):
            for impl in impls:
                runs[impl].append(run_once(cmds[impl] + args, out_rel, a.timeout))
        res = {"id": job["id"], "project": job["project"], "job": job["job"],
               "level": job["level"], "inputs": len(job["js"]),
               "input_bytes": sum(os.path.getsize(os.path.join(ROOT, f)) for f in job["js"]),
               "args": args}
        for impl in impls:
            rs = runs[impl]
            res[impl] = {"wall_s": median(r["wall_s"] for r in rs),
                         "rss_kb": median(r["rss_kb"] for r in rs),
                         "cpu_s": median(None if r["user_s"] is None else r["user_s"] + r["sys_s"]
                                         for r in rs),
                         "exit": rs[0]["exit"], "timed_out": any(r["timed_out"] for r in rs),
                         "output_bytes": None if rs[0]["output"] is None else len(rs[0]["output"]),
                         "stderr_bytes": len(rs[0]["stderr"]),
                         "reps": [{k: r[k] for k in ("wall_s", "rss_kb", "user_s", "sys_s",
                                                     "exit")} for r in rs]}
            # each implementation must be deterministic across repetitions
            nd = [i for i, r in enumerate(rs[1:], 1) if compare(rs[0], r)]
            if nd:
                res[impl]["nondeterministic_reps"] = nd
        if "java" in impls and "rust" in impls:
            diffs = [compare(runs["java"][0], r) for r in runs["rust"]]
            res["identical"] = not any(diffs)
            res["diff"] = next((d for d in diffs if d), None)
            if not res["identical"] and a.keep_failing:
                keep = os.path.join(ROOT, "build/bench/failing", job["id"])
                shutil.rmtree(keep, ignore_errors=True)
                for impl in impls:
                    d = os.path.join(keep, impl)
                    os.makedirs(d)
                    r = runs[impl][0]
                    for k in ("stdout", "stderr", "output"):
                        if r[k] is not None:
                            with open(os.path.join(d, k if k != "output" else "out.js"), "wb") as f:
                                f.write(r[k])
                    for name, data in r["extra_files"].items():
                        if data is not None:
                            with open(os.path.join(d, name), "wb") as f:
                                f.write(data)
                with open(os.path.join(keep, "argv.json"), "w") as f:
                    json.dump({"cwd": ROOT, "java": cmds["java"] + args,
                               "rust": cmds["rust"] + args}, f, indent=1)
        results.append(res)
        line = f"[{n}/{len(jobs)}] {job['id']}:"
        for impl in impls:
            line += (f" {impl} {fmt_s(res[impl]['wall_s'])}s {fmt_mb(res[impl]['rss_kb'])}MB"
                     f" exit={res[impl]['exit']}")
        if "identical" in res:
            line += " identical" if res["identical"] else " MISMATCH " + ",".join(res["diff"])
        print(line, file=sys.stderr, flush=True)

    # totals: per project x level (d3-12 = the 12 compiles summed), and per level overall
    groups: dict[str, list[dict]] = {}
    for r in results:
        groups.setdefault(f"{r['project']}/{r['level']}", []).append(r)
        groups.setdefault(f"ALL/{r['level']}", []).append(r)
    totals = {}
    for g, rs in groups.items():
        t = {"jobs": len(rs)}
        for impl in impls:
            ws = [r[impl]["wall_s"] for r in rs]
            ms = [r[impl]["rss_kb"] for r in rs]
            t[impl] = {"wall_s": None if None in ws else round(sum(ws), 4),
                       "max_rss_kb": None if None in ms else max(ms)}
        if "identical" in rs[0]:
            t["identical"] = sum(r["identical"] for r in rs)
        totals[g] = t
    meta["host"]["loadavg_end"] = os.getloadavg()
    doc = {**meta, "jobs": results, "totals": totals}
    if not a.no_save:
        out = a.out or os.path.join(ROOT, "bench/results",
                                    f"{meta['date_utc'][:10]}-{meta['commit']}.json")
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "w") as f:
            json.dump(doc, f, indent=1)
            f.write("\n")
        print(f"results: {os.path.relpath(out, ROOT)}", file=sys.stderr)

    # Markdown table
    both = impls == ["java", "rust"]
    hdr = ["job", "inputs"]
    for impl in impls:
        hdr += [f"{impl} s", f"{impl} MB"]
    if both:
        hdr += ["time R/J", "RSS R/J", "identical"]
    print("| " + " | ".join(hdr) + " |")
    print("|" + "|".join("---" for _ in hdr) + "|")

    def row(name, inputs, vals, ident):
        cells = [name, inputs]
        for impl in impls:
            cells += [fmt_s(vals[impl][0]), fmt_mb(vals[impl][1])]
        if both:
            cells += [ratio(vals["rust"][0], vals["java"][0]),
                      ratio(vals["rust"][1], vals["java"][1]), ident]
        print("| " + " | ".join(cells) + " |")

    for r in results:
        row(r["id"], str(r["inputs"]), {i: (r[i]["wall_s"], r[i]["rss_kb"]) for i in impls},
            ("yes" if r["identical"] else "NO") if both else "")
    for g, t in totals.items():
        row(f"**total {g}** (time sum, max RSS)", str(t["jobs"]),
            {i: (t[i]["wall_s"], t[i]["max_rss_kb"]) for i in impls},
            f"{t['identical']}/{t['jobs']}" if both else "")
    mism = [r for r in results if r.get("identical") is False]
    if mism:
        print(f"\nMismatches ({len(mism)}; porting defects, see the result JSON for the first "
              "differing byte):")
        for r in mism:
            print(f"- {r['id']}: {', '.join(r['diff'])}; repro: python3 scripts/run_bench.py "
                  f"--job '^{re.escape(r['id'])}$' --reps 1 --keep-failing --no-save")
    return 0


if __name__ == "__main__":
    sys.exit(main())
