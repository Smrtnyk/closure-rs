#!/usr/bin/env python3
"""closure-rs: compare the speed of two closure-rs binaries on the same machine.

  python3 scripts/bench_ab.py --base BIN --head BIN [--rounds N] [--out DIR]
                              [--project P]... [--job REGEX] [--level L]... [--variants]
  python3 scripts/bench_ab.py --report DIR

Runs the benchmarks of bench/projects.json (scripts/run_bench.py --impl rust --reps 1) with the
base and the head binary in turns: each round runs every selected job with both binaries, the
order alternating from round to round (base, head; head, base; ...), so a machine that gets
faster or slower during the run affects both alike. The results of every pass go to DIR
(default build/bench/ab/) as base-<round>.json and head-<round>.json.

Then prints a Markdown report, head relative to base: for wall time, CPU time (user + sys) and
peak RSS the geometric mean over the jobs of head/base, from the medians over the rounds, and
the same ratio computed within each round alone. Differences between two runs on different
machines, as between two CI runs, are often 10-30%; within one run the per-round ratios show how
large the noise is, so a change that is smaller than their spread is not distinguishable from
it. --report DIR only reprints the report of an earlier run.

The two binaries' outputs are not compared (they may differ on purpose, for example across an
upstream sync); scripts/run_bench.py and the Java comparison check them against Java.
"""

from __future__ import annotations

import argparse
import glob
import json
import math
import os
import re
import statistics
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN_BENCH = os.path.join(ROOT, "scripts/run_bench.py")
METRICS = (("wall_s", "wall time"), ("cpu_s", "CPU time"), ("rss_kb", "peak RSS"))


def run_rounds(a) -> None:
    os.makedirs(a.out, exist_ok=True)
    for old in glob.glob(os.path.join(a.out, "*.json")):
        os.remove(old)
    select = [f"--project={p}" for p in a.project] + [f"--level={lv}" for lv in a.level]
    if a.job:
        select.append(f"--job={a.job}")
    if a.variants:
        select.append("--variants")
    bins = {"base": a.base, "head": a.head}
    for r in range(1, a.rounds + 1):
        order = ("base", "head") if r % 2 else ("head", "base")
        for side in order:
            out = os.path.join(a.out, f"{side}-{r}.json")
            cmd = [sys.executable, RUN_BENCH, "--impl=rust", "--reps=1", f"--bin={bins[side]}",
                   f"--out={out}", *select]
            p = subprocess.run(cmd, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                               text=True)
            if p.returncode != 0:
                sys.stderr.write(p.stderr)
                sys.exit(f"{side} round {r}: scripts/run_bench.py exited {p.returncode}")
            with open(out, encoding="utf-8") as f:
                total = sum(j["rust"]["wall_s"] for j in json.load(f)["jobs"])
            print(f"round {r}/{a.rounds} {side}: {total:.2f} s", file=sys.stderr, flush=True)


def load(out_dir: str) -> dict[str, list[dict]]:
    """{side: [one {job id: {metric: value}} per round, in round order]}"""
    sides = {}
    for side in ("base", "head"):
        files = glob.glob(os.path.join(out_dir, f"{side}-*.json"))
        files.sort(key=lambda p: int(re.search(r"-(\d+)\.json$", p)[1]))
        rounds = []
        for path in files:
            with open(path, encoding="utf-8") as f:
                doc = json.load(f)
            rounds.append({j["id"]: {**{m: j["rust"][m] for m, _ in METRICS},
                                     "group": f"{j['project']}/{j['level']}",
                                     "exit": j["rust"]["exit"]} for j in doc["jobs"]})
        sides[side] = rounds
    if not sides["base"] or len(sides["base"]) != len(sides["head"]):
        sys.exit(f"{out_dir}: need the same number (>= 1) of base-N.json and head-N.json")
    return sides


def geomean(xs: list[float]) -> float:
    return math.exp(sum(math.log(x) for x in xs) / len(xs))


def pct(x: float) -> str:
    return f"{(x - 1) * 100:+.1f}%"


def report(out_dir: str) -> str:
    sides = load(out_dir)
    base, head = sides["base"], sides["head"]
    ids = sorted(set.intersection(*(set(r) for r in base + head)))

    def usable(jid, m):
        return all(r[jid][m] for r in base + head)

    def med(rounds, jid, m):
        return statistics.median(r[jid][m] for r in rounds)

    lines = [f"Head relative to base: {len(ids)} jobs, {len(base)} rounds "
             "(geometric mean of head/base over the jobs; negative is faster).", "",
             "| | all rounds (medians) | each round alone | rounds |", "|---|---:|---:|---|"]
    ratios = {}
    for m, name in METRICS:
        keys = [j for j in ids if usable(j, m)]
        per_job = {j: med(head, j, m) / med(base, j, m) for j in keys}
        ratios[m] = per_job
        rounds = [geomean([h[j][m] / b[j][m] for j in keys]) for b, h in zip(base, head)]
        spread = f"{pct(min(rounds))} to {pct(max(rounds))}" if len(rounds) > 1 else "-"
        lines.append(f"| {name} | {pct(geomean(list(per_job.values())))} | {spread} | "
                     + " ".join(pct(x) for x in rounds) + " |")

    groups: dict[str, list[str]] = {}
    for j in ids:
        groups.setdefault(base[0][j]["group"], []).append(j)
    lines += ["", "| Group | jobs | wall time | peak RSS |", "|---|---:|---:|---:|"]
    for g in sorted(groups):
        w = [ratios["wall_s"][j] for j in groups[g] if j in ratios["wall_s"]]
        r = [ratios["rss_kb"][j] for j in groups[g] if j in ratios["rss_kb"]]
        lines.append(f"| {g} | {len(groups[g])} | {pct(geomean(w)) if w else '-'} | "
                     f"{pct(geomean(r)) if r else '-'} |")

    by_wall = sorted(ratios["wall_s"].items(), key=lambda kv: kv[1])
    slower = [f"{j} {pct(x)}" for j, x in reversed(by_wall[-5:]) if x > 1]
    faster = [f"{j} {pct(x)}" for j, x in by_wall[:5] if x < 1]
    lines += ["", f"Largest wall-time changes: slower {', '.join(slower) or '-'}; "
              f"faster {', '.join(faster) or '-'}."]
    exits = [j for j in ids if base[0][j]["exit"] != head[0][j]["exit"]]
    if exits:
        lines.append(f"Exit codes differ between base and head on: {', '.join(exits)}.")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[1],
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base")
    ap.add_argument("--head")
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--out", default=os.path.join(ROOT, "build/bench/ab"))
    ap.add_argument("--project", action="append", default=[])
    ap.add_argument("--job", help="regex over job ids (project/job/LEVEL)")
    ap.add_argument("--level", action="append", default=[])
    ap.add_argument("--variants", action="store_true")
    ap.add_argument("--report", metavar="DIR", help="only print the report of an earlier run")
    a = ap.parse_args()
    if a.report:
        print(report(a.report), end="")
        return 0
    if not (a.base and a.head) or a.rounds < 1:
        ap.error("--base and --head are required, and --rounds must be at least 1")
    for b in (a.base, a.head):
        if not os.access(b, os.X_OK):
            ap.error(f"not an executable: {b}")
    a.base, a.head = os.path.abspath(a.base), os.path.abspath(a.head)
    run_rounds(a)
    print(report(a.out), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
