#!/usr/bin/env python3
"""Unit-record ratchet across a re-recording (a reference change, docs/PORTING.md §9).

The unit ratchet (gates/unit_ratchet.json, gates/unit_rust.sh) names records Class[index]: the
record file stem and the 0-based line in corpus/unit/records/Class.jsonl.gz. A re-recording against
another reference moves those indexes. This script maps every id the old ratchet lists as passing to
its record key (class, method, call) through the OLD record files, finds that key in the NEW record
files (the key scripts/unit_regress_check.py matches records by; equal keys in one file pair up in
file order) and reports:
  lost        old passing records whose key is no longer recorded (test removed or renamed upstream)
  regressed   (with --new-ratchet) old passing records whose new id does not pass in the new run
  new         records of the new recording whose key the old recording did not have
  gained      (with --new-ratchet) new passing ids that were not passing before

  python3 scripts/unit_ratchet_rebase.py --old-ratchet gates/unit_ratchet.json \\
      --old-records OLD_DIR --new-records NEW_DIR [--new-ratchet NEW_RATCHET.json] [--allow-lost]

Prints a JSON summary (lists cut to 50 entries). Exit 0 = nothing regressed and nothing lost
(--allow-lost: lost records are reported only), 1 = otherwise, 2 = bad input.
"""
from __future__ import annotations

import argparse
import collections
import gzip
import json
import os
import re
import sys

ID_RE = re.compile(r"(.*)\[(\d+)\]")


def record_keys(records_dir: str, cls: str) -> list[tuple]:
    """[(cls, method, call, n)] in file order, n = occurrence of (method, call) before it."""
    path = os.path.join(records_dir, f"{cls}.jsonl.gz")
    if not os.path.exists(path):
        return []
    seen: collections.Counter = collections.Counter()
    out = []
    with gzip.open(path, "rt", encoding="utf-8") as f:
        for line in f:
            if not line.strip():
                continue
            r = json.loads(line)
            k = (r["method"], r["call"])
            out.append((cls, r["method"], r["call"], seen[k]))
            seen[k] += 1
    return out


def classes(records_dir: str) -> list[str]:
    return sorted(f[:-len(".jsonl.gz")] for f in os.listdir(records_dir) if f.endswith(".jsonl.gz"))


def parse_id(rid: str) -> tuple[str, int]:
    m = ID_RE.fullmatch(rid)
    if not m:
        raise SystemExit(f"unit_ratchet_rebase: not a Class[index] id: {rid!r}")
    return m.group(1), int(m.group(2))


def sort_key(rid: str):
    c, i = parse_id(rid)
    return c, i


def load_passing(path: str) -> list[str]:
    with open(path, encoding="utf-8") as f:
        d = json.load(f)
    if not isinstance(d.get("passing"), list):
        raise SystemExit(f"unit_ratchet_rebase: {path} has no passing list")
    return d["passing"]


def rebase(old_passing: list[str], old_records: str, new_records: str,
           new_passing: list[str] | None = None) -> dict:
    old_keys = {c: record_keys(old_records, c) for c in classes(old_records)}
    new_keys = {c: record_keys(new_records, c) for c in classes(new_records)}
    new_index = {k: f"{c}[{i}]" for c, ks in new_keys.items() for i, k in enumerate(ks)}
    old_all = {k for ks in old_keys.values() for k in ks}
    mapped, lost = {}, []
    for rid in old_passing:
        c, i = parse_id(rid)
        ks = old_keys.get(c, [])
        if i >= len(ks):
            raise SystemExit(f"unit_ratchet_rebase: {rid} is not in the old records ({len(ks)} records in {c})")
        k = ks[i]
        if k in new_index:
            mapped[rid] = new_index[k]
        else:
            lost.append({"id": rid, "class": c, "method": k[1], "call": k[2]})
    new = sorted((rid for k, rid in new_index.items() if k not in old_all), key=sort_key)
    s = {"oldPassing": len(old_passing), "mapped": len(mapped), "lost": lost, "new": new,
         "moved": sum(1 for a, b in mapped.items() if a != b)}
    if new_passing is not None:
        now = set(new_passing)
        s["regressed"] = sorted(({"id": a, "newId": b} for a, b in mapped.items() if b not in now),
                                key=lambda x: sort_key(x["newId"]))
        before = set(mapped.values())
        s["gained"] = sorted((r for r in now if r not in before), key=sort_key)
        s["stillPassing"] = len(before & now)
    return s


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--old-ratchet", required=True)
    ap.add_argument("--old-records", required=True)
    ap.add_argument("--new-records", required=True)
    ap.add_argument("--new-ratchet")
    ap.add_argument("--allow-lost", action="store_true")
    a = ap.parse_args(argv)
    for d in (a.old_records, a.new_records):
        if not os.path.isdir(d):
            print(f"unit_ratchet_rebase: no records directory {d}", file=sys.stderr)
            return 2
    s = rebase(load_passing(a.old_ratchet), a.old_records, a.new_records,
               load_passing(a.new_ratchet) if a.new_ratchet else None)
    out = dict(s)
    for k in ("lost", "new", "regressed", "gained"):
        if k in out:
            out[k + "Count"] = len(out[k])
            out[k] = out[k][:50]
    print(json.dumps(out, indent=1))
    bad = bool(s.get("regressed")) or (bool(s["lost"]) and not a.allow_lost)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
