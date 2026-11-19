#!/usr/bin/env python3
"""Writes record_counts.tsv: per record file, the number of records of each kind.

Independent of the Rust reader (Python's json module), so tests/records_load.rs can check the
per-class counts it computes against it, and the record tests take each class's record count from
it (closure_testing::corpus_counts::record_count). scripts/unit_corpus_counts.py writes it too,
with the other counts the tests pin. Run from anywhere:
    python3 crates/testing/tests/data/gen_record_counts.py
"""
import argparse
import collections
import glob
import gzip
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
RECORDS = os.path.join(HERE, "../../../../corpus/unit/records")
KINDS = ["compiler_test_case", "integration", "type_check"]


def table(records=RECORDS):
    """The text of record_counts.tsv for the record files in `records`."""
    rows = []
    for f in sorted(glob.glob(os.path.join(records, "*.jsonl.gz"))):
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
    return "".join("\t".join(r) + "\n" for r in [["stem", "total"] + KINDS] + rows)


def main():
    argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter).parse_args()
    with open(os.path.join(HERE, "record_counts.tsv"), "w", encoding="utf-8") as out:
        out.write(table())


if __name__ == "__main__":
    main()
