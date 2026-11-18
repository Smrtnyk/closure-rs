#!/usr/bin/env python3
"""Keeps the unit corpus counts that the docs quote equal to corpus/unit/records.

  python3 scripts/unit_corpus_counts.py           rewrite the counts in the docs
  python3 scripts/unit_corpus_counts.py --check   exit 1, naming each stale count, when one differs

The counts come from the record files of this checkout (the one this script lives in, also a git
worktree), one per *Test.java class of the reference (empty for a class without a recorded call;
scripts/upstream_sync.py's unit-install step installs them):
  classes   the record files, i.e. the reference's test classes
  records   the records (the non-empty lines of all files)
  recorded  the record files with at least one record (one descriptor each, descriptors_load.rs)
  ctc       the records of kind compiler_test_case (one expected_pipeline line each, cross_checks.rs)
and the docs quote them in the sentences of PHRASES: corpus/unit/RECORDING.md (the classes the
recording runs, the corpus size) and crates/testing/README.md (what its tests load). Every phrase
must occur at least once, so a reworded sentence fails --check instead of going stale.
unit-install runs the rewrite after installing a recording; gates/ci.sh runs --check.
"""
from __future__ import annotations

import argparse
import glob
import gzip
import json
import os
import re
import sys

CHECKOUT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RECORDS = "corpus/unit/records"
N = r"(\d[\d,]*)"
# file -> [(regex with one group per count, the counts' names)]; whitespace may break lines.
PHRASES = {
    "corpus/unit/RECORDING.md": [
        (rf"\ball {N} `\*Test\.java` files", ("classes",)),
        (rf"\bevery one of the {N} classes", ("classes",)),
        (rf"\b{N} records from {N} of the {N} test classes", ("records", "recorded", "classes")),
    ],
    "crates/testing/README.md": [
        (rf"\bholds for all {N} records", ("records",)),
        (rf"\ball {N} records in {N} files \({N} non-empty\)", ("records", "classes", "recorded")),
        (rf"\bevery one of the {N} records", ("records",)),
        (rf"\ball {N} descriptors", ("recorded",)),
        (rf"\bAll {N} `compiler_test_case` records select a case", ("ctc",)),
        (rf"`expected_pipeline` \({N} lines\)", ("ctc",)),
    ],
}
NOUN = {"classes": "test classes (record files)", "records": "records",
        "recorded": "classes with records (non-empty record files)", "ctc": "compiler_test_case records"}


def corpus_counts(root=CHECKOUT):
    files = sorted(glob.glob(os.path.join(root, RECORDS, "*.jsonl.gz")))
    per_file, ctc = [], 0
    for f in files:
        n = 0
        with gzip.open(f, "rt", encoding="utf-8", errors="surrogatepass") as fh:
            for line in fh:
                if line.strip():
                    n += 1
                    ctc += json.loads(line)["kind"] == "compiler_test_case"
        per_file.append(n)
    return {"classes": len(files), "records": sum(per_file), "recorded": sum(1 for n in per_file if n), "ctc": ctc}


def rewrite(path, text, counts):
    """(new text, [(line, name, quoted)], [missing phrase])."""
    found, missing = [], []
    for pattern, names in PHRASES[path]:
        rx = re.compile(pattern.replace(" ", r"\s+"))

        def sub(m, names=names):
            s, pos, out = m.group(0), 0, []
            for i, name in enumerate(names, 1):
                a, b = m.start(i) - m.start(), m.end(i) - m.start()
                out += [s[pos:a], f"{counts[name]:,}"]
                pos = b
                found.append((text.count("\n", 0, m.start(i)) + 1, name, m.group(i)))
            return "".join(out) + s[pos:]

        text, n = rx.subn(sub, text)
        if not n:
            missing.append(pattern.replace(N, "N").replace(r"\b", "").replace("\\", ""))
    return text, found, missing


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="only compare; exit 1 when a count is stale")
    a = ap.parse_args()
    counts = corpus_counts()
    if not counts["classes"]:
        print(f"unit_corpus_counts.py: no record files in {RECORDS}", file=sys.stderr)
        return 2
    stale, broken, changed = [], [], []
    for path in PHRASES:
        full = os.path.join(CHECKOUT, path)
        text = open(full, encoding="utf-8").read()
        new, found, missing = rewrite(path, text, counts)
        broken += [f"{path}: no sentence matches {p!r}" for p in missing]
        stale += [f"{path}:{line}: {quoted} {NOUN[name]}, the corpus has {counts[name]:,}"
                  for line, name, quoted in found if quoted != f"{counts[name]:,}"]
        if new != text and not a.check:
            with open(full, "w", encoding="utf-8") as f:
                f.write(new)
            changed.append(path)
    summary = (f"{RECORDS}: {counts['records']:,} records ({counts['ctc']:,} compiler_test_case) in "
               f"{counts['classes']} files ({counts['recorded']} non-empty)")
    if broken:
        print("\n".join(broken + ["the sentence was reworded: update PHRASES in scripts/unit_corpus_counts.py"]),
              file=sys.stderr)
        return 1
    if a.check:
        if stale:
            print("\n".join(stale + [f"({summary}; python3 scripts/unit_corpus_counts.py rewrites them)"]),
                  file=sys.stderr)
            return 1
        print(f"{summary}: the docs quote these counts")
        return 0
    print(f"{summary}: " + (f"rewrote {', '.join(changed)}" if changed else "the docs quote these counts"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
