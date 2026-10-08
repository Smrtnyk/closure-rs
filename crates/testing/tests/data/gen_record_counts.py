#!/usr/bin/env python3
"""Writes record_counts.tsv: per record file, the number of records of each kind.

Independent of the Rust reader (Python's json module), so tests/records_load.rs can check the
per-class counts it computes against it. Run from anywhere:
    python3 crates/testing/tests/data/gen_record_counts.py
"""
import collections
import glob
import gzip
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
RECORDS = os.path.join(HERE, "../../../../corpus/unit/records")
KINDS = ["compiler_test_case", "integration", "type_check"]


def main():
    rows = []
    for f in sorted(glob.glob(os.path.join(RECORDS, "*.jsonl.gz"))):
        stem = os.path.basename(f).split(".")[0]
        counts = collections.Counter()
        total = 0
        with gzip.open(f, "rt", encoding="utf-8", errors="surrogatepass") as fh:
            for line in fh:
                if not line.strip():
                    continue
                rec = json.loads(line)
                if rec["kind"] not in KINDS:
                    raise SystemExit(f"{f}: unknown kind {rec['kind']!r}")
                counts[rec["kind"]] += 1
                total += 1
        rows.append([stem, str(total)] + [str(counts[k]) for k in KINDS])
    with open(os.path.join(HERE, "record_counts.tsv"), "w", encoding="utf-8") as out:
        out.write("\t".join(["stem", "total"] + KINDS) + "\n")
        for r in rows:
            out.write("\t".join(r) + "\n")


if __name__ == "__main__":
    main()
