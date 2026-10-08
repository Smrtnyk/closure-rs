#!/usr/bin/env python3
"""Per-profile summary of a seam3.py run directory (default build/oracle/seam/v3).

Usage: python3 oracle/test/seam3_report.py [RUN_DIR]

"JS/exit differs" compares exit code, stdout and every output file. Its sub-column "D8" counts
the pairs where the checks run exited non-zero (promoted, non-halting errors), compile wrote no
output file and optimize_from_typedast exited 0 (oracle/SEAM.md, D8); "other" is the rest.
The last lines list the JVM environments (oracle/PROTOCOL.md, "Environment") the results were
produced in, as stored per result in "jvm_env" (absent = an older result, run in the inherited env).
"""
import collections, glob, json, os, sys
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
d = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "build/oracle/seam/v3")
t = collections.defaultdict(collections.Counter)
ex = collections.defaultdict(list)
envs = collections.Counter()


def is_d8(r):
    ck, c, p = r.get("checks") or {}, r.get("compile") or {}, r.get("optimize") or {}
    return (ck.get("exit_code") not in (0, None) and not c.get("files")
            and c.get("exit_code") == ck.get("exit_code") and p.get("exit_code") == 0)


for f in sorted(glob.glob(os.path.join(d, "results", "*.json"))):
    r = json.load(open(f))
    t[r["profile"]][r["class"]] += 1
    e = r.get("jvm_env")
    envs[("inherited (not recorded)",) if not e else
         (e.get("locale.format"), e.get("timezone"), e.get("stdout.encoding"),
          e.get("stderr.encoding"))] += 1
    if r["class"] == "differs":
        d8 = is_d8(r)
        t[r["profile"]]["differs_d8" if d8 else "differs_other"] += 1
        ex[r["profile"]].append((r["key"], r["case"], r.get("differing_files"),
                                 r.get("identical_exit"), r.get("identical_stdout"),
                                 "D8" if d8 else "other"))
print("| profile | pairs | identical | JS identical, stderr differs | JS/exit differs | (D8) | (other) | no TypedAST | oracle failure |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
for p in sorted(t):
    c = t[p]
    n = sum(v for k, v in c.items() if not k.startswith("differs_"))
    print(f"| {p} | {n} | {c['identical']} | {c['identical_js_stderr_differs']} | "
          f"{c['differs']} | {c['differs_d8']} | {c['differs_other']} | "
          f"{c['checks_no_typedast']} | {c['oracle_failure']} |")
for p in sorted(ex):
    for e in ex[p]:
        print("differs:", p, *e)
for k, v in sorted(envs.items(), key=str):
    print("jvm_env (locale.format, timezone, stdout.encoding, stderr.encoding):", k, "results:", v)
