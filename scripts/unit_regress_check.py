#!/usr/bin/env python3
"""Regression check after a re-recording: every record that replayed green before must
replay green now. Usage: scripts/unit_regress_check.py PREV_REPLAY_DIR NEW_REPLAY_DIR PREV_RECORDS NEW_RECORDS
Records are matched by (class, method, call). Prints a JSON summary; exit 1 on any regression or on
a previously green record that is missing from the new recording."""
import gzip, json, os, sys

prev_dir, new_dir, prev_rec, new_rec = sys.argv[1:5]


def keys(rec_dir, c):
    out = []
    with gzip.open(f"{rec_dir}/{c}.jsonl.gz", "rt", encoding="utf-8") as f:
        for line in f:
            r = json.loads(line)
            out.append((r["method"], r["call"]))
    return out


def fails(d, c):
    p = f"{d}/{c}.failures.jsonl"
    s = {}
    if os.path.exists(p):
        for line in open(p):
            x = json.loads(line)
            s[x["index"]] = x["why"]
    return s


def finished(d, c, n):
    p = f"{d}/{c}.done.json"
    if not os.path.exists(p):
        return False
    rr = (json.load(open(p)).get("replay") or {}).get(c)
    return rr is not None and rr["records"] == n


classes = sorted(f[:-len(".jsonl.gz")] for f in os.listdir(new_rec) if f.endswith(".jsonl.gz"))
summary = {"classes": 0, "prevGreen": 0, "stillGreen": 0, "regressed": [], "missing": [], "newRecords": 0,
           "unfinishedNew": [], "nowGreenPrevFailing": 0}
for c in classes:
    nk = keys(new_rec, c)
    if not nk:
        continue
    summary["classes"] += 1
    pk = keys(prev_rec, c) if os.path.exists(f"{prev_rec}/{c}.jsonl.gz") else []
    pf, nf = fails(prev_dir, c), fails(new_dir, c)
    if not finished(new_dir, c, len(nk)):
        summary["unfinishedNew"].append(c)
        continue
    prev_ok = finished(prev_dir, c, len(pk))
    nidx = {k: i for i, k in enumerate(nk)}
    for i, k in enumerate(pk):
        if not prev_ok or i in pf:
            if prev_ok and k in nidx and nidx[k] not in nf:
                summary["nowGreenPrevFailing"] += 1
            continue
        summary["prevGreen"] += 1
        j = nidx.get(k)
        if j is None:
            summary["missing"].append({"class": c, "method": k[0], "call": k[1]})
        elif j in nf:
            summary["regressed"].append({"class": c, "method": k[0], "call": k[1], "why": nf[j][:300]})
        else:
            summary["stillGreen"] += 1
    summary["newRecords"] += len([k for k in nk if k not in set(pk)])
summary["regressedCount"] = len(summary["regressed"])
summary["missingCount"] = len(summary["missing"])
summary["regressed"] = summary["regressed"][:50]
summary["missing"] = summary["missing"][:50]
print(json.dumps(summary, indent=1))
sys.exit(1 if summary["regressedCount"] or summary["missingCount"] or summary["unfinishedNew"] else 0)
