#!/usr/bin/env python3
"""Summarise seam results: python3 oracle/test/seam_report.py [build/oracle/seam/v2]"""
import collections
import glob
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "build/oracle/seam/v2")
recs = [json.load(open(f)) for f in glob.glob(os.path.join(d, "results", "*.json"))]
recs.sort(key=lambda r: r["idx"])
by = collections.Counter(r["class"] for r in recs)
print(f"dir={d} pairs={len(recs)} classes={dict(by)}")
prof = collections.Counter((r["profile"], r["class"]) for r in recs)
print("by profile:", dict(prof))


def dep(r):
    a = r.get("args", [])
    return "PRUNE" if any(x.startswith("--dependency_mode=PRUNE") for x in a) else "none"


src = collections.Counter((r["case"].split("-")[0], dep(r), r["class"]) for r in recs)
print("by source/dep_mode:")
for k, v in sorted(src.items()):
    print("  ", k, v)
# stderr relationship for identical-stdout pairs: is compile.stderr == checks.stderr?
same_as_checks = other = 0
for r in recs:
    if r["class"] == "identical_stdout_stderr_differs":
        dd = os.path.join(d, "diffs", str(r["idx"]))
        try:
            c = open(os.path.join(dd, "compile.stderr"), "rb").read()
            k = open(os.path.join(dd, "checks.stderr"), "rb").read()
            o = open(os.path.join(dd, "optimize.stderr"), "rb").read()
        except OSError:
            other += 1
            continue
        if c == k and o == b"":
            same_as_checks += 1
        else:
            other += 1
print(f"stderr-differs pairs where compile.stderr == checks.stderr and optimize.stderr empty: "
      f"{same_as_checks}; other: {other}")
for r in recs:
    if r["class"] in ("differs", "checks_no_typedast", "oracle_failure") or (
            r["class"] == "identical_stdout_stderr_differs" and False):
        print(" ", r["idx"], r["class"], r["case"], r["profile"], dep(r),
              "rc", r.get("compile", {}).get("exit_code"), r.get("checks", {}).get("exit_code"),
              r.get("optimize", {}).get("exit_code"))
