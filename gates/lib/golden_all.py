#!/usr/bin/env python3
"""Resumable, parallel golden reference run over every D2 (case, profile).

Usage:
  python3 gates/lib/golden_all.py [--jobs N|auto] [--mem-budget-gb 14] [--source S ...]
         [--profile P ...] [--case-list FILE] [--limit N] [--force] [--summary-only]

* Pairs = every case in corpus/d2/candidates/*.jsonl x case_args.case_profiles(case).
* A pair whose result file already exists is skipped (resumable); --force re-runs it.
* --jobs auto: concurrency = floor(mem budget / p99 peak RSS of existing results, +25%
  margin), capped at 3/4 of the CPUs.  Additionally a job is only started while
  /proc/meminfo MemAvailable stays above --reserve-gb (host protection).
* Long jobs first (inputs ordered by total input bytes, descending) for load balancing.
* At the end (and with --summary-only) writes <golden>/_summary.json and prints it.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures as cf
import json
import os
import re
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import case_args  # noqa: E402
import run_reference as rr  # noqa: E402

REPO = case_args.REPO
ICE_PATTERNS = [
    ("internal_compiler_error", re.compile(r"INTERNAL COMPILER ERROR")),
    ("java_exception", re.compile(r"(?m)^(?:Exception in thread|Caused by: |\s+at (?:com|java|jdk)\.)"
                                  r"|java\.lang\.\w+(?:Error|Exception)")),
    ("out_of_memory", re.compile(r"OutOfMemoryError")),
    ("stack_overflow", re.compile(r"StackOverflowError")),
]


def mem_available_kb() -> int:
    with open("/proc/meminfo") as f:
        for line in f:
            if line.startswith("MemAvailable:"):
                return int(line.split()[1])
    return 0


def input_bytes(case: dict) -> int:
    n = 0
    for p in case.get("inputs", []) + case.get("shims", []) + case.get("externs", []):
        try:
            n += os.path.getsize(os.path.join(REPO, p))
        except OSError:
            pass
    return n


def all_pairs(cases, profiles):
    for c in cases:
        for p in case_args.case_profiles(c, profiles):
            yield c, p


def existing_rss_kb() -> list[int]:
    vals = []
    root = os.path.join(REPO, rr.GOLDEN_ROOT)
    if not os.path.isdir(root):
        return vals
    for d in os.listdir(root):
        dp = os.path.join(root, d)
        if not os.path.isdir(dp):
            continue
        for fn in os.listdir(dp):
            if fn.endswith(".json"):
                try:
                    with open(os.path.join(dp, fn), encoding="utf-8") as f:
                        r = json.load(f)
                    if r.get("runner_version", 1) >= 2 and r.get("peak_rss_kb"):
                        vals.append(r["peak_rss_kb"])  # v1 values are polluted by driver RSS
                except (OSError, ValueError, KeyError):
                    pass
    return vals


def choose_jobs(budget_gb: float) -> tuple[int, str]:
    rss = sorted(existing_rss_kb())
    cpu_cap = max(1, (os.cpu_count() or 4) * 3 // 4)
    if len(rss) < 20:
        per = 1_500_000  # conservative 1.5 GB per JVM until we have measurements
        why = "no measurements; assumed 1.5 GB/JVM"
    else:
        p99 = rss[min(len(rss) - 1, int(len(rss) * 0.99))]
        per = int(p99 * 1.25)
        why = f"p99 peak RSS {p99} kB over {len(rss)} runs (+25%)"
    jobs = max(1, min(cpu_cap, int(budget_gb * 1024 * 1024 // per)))
    return jobs, why


def stderr_text(r: dict) -> str:
    s = r.get("stderr", "")
    return s if isinstance(s, str) else ""


def classify(r: dict) -> list[str]:
    s = stderr_text(r)
    return [name for name, rx in ICE_PATTERNS if rx.search(s)]


def summarize(cases, profiles) -> dict:
    by_id = {c["id"]: c for c in cases}
    total = 0
    missing = []
    exit_codes = collections.Counter()
    timeouts = []
    ice = collections.defaultdict(list)
    per_source = collections.defaultdict(lambda: collections.Counter())
    per_profile = collections.defaultdict(lambda: collections.Counter())
    wall_sum = 0
    rss = []
    rss_unreliable = 0
    for c, p in all_pairs(cases, profiles):
        total += 1
        path = os.path.join(REPO, rr.result_path(c["id"], p))
        if not os.path.exists(path):
            missing.append(f"{c['id']}/{p}")
            continue
        with open(path, encoding="utf-8") as f:
            r = json.load(f)
        ec = "timeout" if r["timed_out"] else str(r["exit_code"])
        exit_codes[ec] += 1
        if r["timed_out"]:
            timeouts.append(f"{c['id']}/{p}")
        for k in classify(r):
            ice[k].append(f"{c['id']}/{p}")
        per_source[c["source"]]["runs"] += 1
        per_source[c["source"]][f"exit_{ec}"] += 1
        per_profile[p]["runs"] += 1
        per_profile[p][f"exit_{ec}"] += 1
        wall_sum += r["wall_ms"]
        if r.get("runner_version", 1) >= 2 and r.get("peak_rss_kb"):
            rss.append(r["peak_rss_kb"])  # v1 peak_rss_kb is max(driver RSS, JVM RSS)
        else:
            rss_unreliable += 1
    rss.sort()
    q = lambda f: rss[min(len(rss) - 1, int(len(rss) * f))] if rss else None  # noqa: E731
    return {
        "reference_jar_sha256": rr.JAR_SHA256,
        "pairs_total": total,
        "results_present": total - len(missing),
        "missing": len(missing),
        "missing_examples": missing[:50],
        "exit_codes": dict(sorted(exit_codes.items())),
        "timeouts": timeouts,
        "stderr_flags": {k: {"count": len(v), "examples": v[:50]} for k, v in sorted(ice.items())},
        "per_source": {k: dict(sorted(v.items())) for k, v in sorted(per_source.items())},
        "per_profile": {k: dict(sorted(v.items())) for k, v in sorted(per_profile.items())},
        "sum_run_wall_ms": wall_sum,
        "peak_rss_unreliable_v1_results": rss_unreliable,
        "peak_rss_kb": {"p50": q(0.5), "p90": q(0.9), "p99": q(0.99), "max": rss[-1] if rss else None},
        "cases": len(by_id),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--jobs", default="auto")
    ap.add_argument("--mem-budget-gb", type=float, default=14.0)
    ap.add_argument("--reserve-gb", type=float, default=2.5,
                    help="do not start a JVM while MemAvailable is below this")
    ap.add_argument("--source", action="append")
    ap.add_argument("--profile", action="append")
    ap.add_argument("--case-list", help="file with 'case_id profile' lines to run instead")
    ap.add_argument("--limit", type=int)
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--summary-only", action="store_true")
    ap.add_argument("--timeout", type=int, default=rr.DEFAULT_TIMEOUT_S)
    ap.add_argument("--xmx", default=rr.DEFAULT_XMX)
    a = ap.parse_args()

    profiles = case_args.load_profiles()
    cases = case_args.load_cases()
    if a.summary_only:
        print(json.dumps(summarize(cases, profiles), indent=1))
        return 0

    problems = case_args.verify(profiles, cases)
    if problems:
        for p in problems:
            print("PROBLEM:", p)
        return 2
    rr.check_jar()
    rr.ensure_cds_archive(a.xmx)

    if a.case_list:
        by_id = {c["id"]: c for c in cases}
        pairs = []
        with open(a.case_list) as f:
            for line in f:
                if line.strip():
                    cid, prof = line.split()
                    pairs.append((by_id[cid], prof))
    else:
        pairs = [(c, p) for c, p in all_pairs(cases, profiles)
                 if (not a.source or c["source"] in a.source)
                 and (not a.profile or p in a.profile)]
    todo = [(c, p) for c, p in pairs
            if a.force or not os.path.exists(os.path.join(REPO, rr.result_path(c["id"], p)))]
    sizes = {c["id"]: input_bytes(c) for c, _ in todo}
    todo.sort(key=lambda cp: -sizes[cp[0]["id"]])
    if a.limit:
        todo = todo[: a.limit]

    if a.jobs == "auto":
        jobs, why = choose_jobs(a.mem_budget_gb)
    else:
        jobs, why = int(a.jobs), "explicit"
    print(f"[golden] pairs={len(pairs)} todo={len(todo)} jobs={jobs} ({why}) "
          f"pid={os.getpid()}", flush=True)

    lock = threading.Lock()
    done = 0
    fails = collections.Counter()
    t0 = time.monotonic()
    reserve_kb = int(a.reserve_gb * 1024 * 1024)
    start_gate = threading.Lock()

    def work(cp):
        nonlocal done
        c, p = cp
        with start_gate:  # serialize starts so the memory check sees earlier JVMs grow
            waited = 0
            while mem_available_kb() < reserve_kb and waited < 600:
                time.sleep(2)
                waited += 2
        try:
            r = rr.run_one(c, p, timeout_s=a.timeout, xmx=a.xmx, profiles=profiles)
            key = "timeout" if r["timed_out"] else str(r["exit_code"])
        except Exception as e:  # noqa: BLE001 - keep the driver alive, report at the end
            key = f"runner_error:{type(e).__name__}"
            print(f"[golden] RUNNER ERROR {c['id']}/{p}: {e!r}", flush=True)
        with lock:
            done += 1
            fails[key] += 1
            if done % 200 == 0 or done == len(todo):
                el = time.monotonic() - t0
                eta = el / done * (len(todo) - done)
                print(f"[golden] {done}/{len(todo)} elapsed={el:.0f}s eta={eta:.0f}s "
                      f"memavail={mem_available_kb() // 1024}MB results={dict(fails)}",
                      flush=True)

    with cf.ThreadPoolExecutor(max_workers=jobs) as ex:
        list(ex.map(work, todo))
    el = time.monotonic() - t0
    summ = summarize(cases, profiles)
    summ["last_driver_run"] = {"todo": len(todo), "jobs": jobs, "jobs_reason": why,
                               "wall_s": round(el, 1), "results": dict(fails)}
    out = os.path.join(REPO, rr.GOLDEN_ROOT, "_summary.json")
    tmp = out + ".tmp"
    with open(tmp, "w") as f:
        json.dump(summ, f, indent=1)
    os.replace(tmp, out)
    print(json.dumps(summ, indent=1), flush=True)
    print(f"[golden] finished in {el:.0f}s; missing={summ['missing']}", flush=True)
    return 0 if summ["missing"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
