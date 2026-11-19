#!/usr/bin/env python3
"""Keeps the unit corpus counts that the tests pin and the docs quote equal to the corpus.

  python3 scripts/unit_corpus_counts.py           rewrite the counts files and the docs
  python3 scripts/unit_corpus_counts.py --check   exit 1, naming each stale count, when one differs

The counts come from the files of this checkout (the one this script lives in, also a git
worktree), read with Python's json module, independently of the Rust reader:
  corpus/unit/records/*.jsonl.gz   one per *Test.java class of the reference (empty for a class
                                   without a recorded call; the unit-install step of
                                   scripts/upstream_sync.py installs them)
  corpus/unit/descriptors/*.json, corpus/unit/derived/expected_pipeline.jsonl.gz,
  corpus/unit/options_defaults.json and crates/jscomp/tests/data/compiler_options_configs.json
                                   (the CompilerOptions fixtures, scripts/unit_options_defaults.sh)

It writes the two files the Rust tests take their counts from (closure_testing::corpus_counts):
  crates/testing/tests/data/record_counts.tsv   per record file, its records of each kind
                                                (crates/testing/tests/data/gen_record_counts.py)
  crates/testing/tests/data/corpus_counts.tsv   the totals: record files, records per kind and
                                                per API, descriptors and their cases,
                                                expected_pipeline lines, CompilerOptions fields
                                                and Java configurations
A test compares the count it computes through the Rust reader (or the Rust CompilerOptions) with
the committed one: a reader that drops or invents an entry, or a field the Rust side forgets, fails
the test, and a corpus change shows as a diff of these files.

The docs quote some of the counts in the sentences of PHRASES: corpus/unit/RECORDING.md (the
classes the recording runs, the corpus size) and crates/testing/README.md (what its tests load).
Every phrase must occur at least once, so a reworded sentence fails --check instead of going stale.

unit-install runs the rewrite after installing a recording and the CompilerOptions fixtures;
gates/ci.sh runs --check.
"""
from __future__ import annotations

import argparse
import collections
import glob
import gzip
import importlib.util
import json
import os
import re
import sys

CHECKOUT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
UNIT = "corpus/unit"
RECORDS = f"{UNIT}/records"
DATA = "crates/testing/tests/data"
RECORD_COUNTS = f"{DATA}/record_counts.tsv"
CORPUS_COUNTS = f"{DATA}/corpus_counts.tsv"
OPTIONS_CONFIGS = "crates/jscomp/tests/data/compiler_options_configs.json"
# The case keys tests/descriptors_load.rs counts (DSL.md "Case keys").
CASE_KEYS = ("when", "processor", "compilerSetup", "options", "postconditions", "testFieldsAfter",
             "testFieldsAfterAlso", "testFieldsAfterSkip", "testFieldsAfterUnchecked")
HEADER = """\
# The counts the tests pin (closure_testing::corpus_counts), from the corpus read with Python's json
# module. Written by scripts/unit_corpus_counts.py; do not edit, run it after the corpus changes.
"""
N = r"(\d[\d,]*)"
# file -> [(regex with one group per count, the counts' names)]; whitespace may break lines.
PHRASES = {
    "corpus/unit/RECORDING.md": [
        (rf"\ball {N} `\*Test\.java` files", ("record_files",)),
        (rf"\bevery one of the {N} classes", ("record_files",)),
        (rf"\b{N} records from {N} of the {N} test classes", ("records", "recorded_files", "record_files")),
    ],
    "crates/testing/README.md": [
        (rf"\bholds for all {N} records", ("records",)),
        (rf"\ball {N} records in {N} files \({N} non-empty\)", ("records", "record_files", "recorded_files")),
        (rf"\bevery one of the {N} records", ("records",)),
        (rf"\ball {N} descriptors", ("descriptors",)),
        (rf"\ball {N} descriptors \({N} cases\)", ("descriptors", "descriptor_cases")),
        (rf"\bAll {N} `compiler_test_case` records select a case", ("kind.compiler_test_case",)),
        (rf"`options_defaults\.json` \({N} fields\)", ("options_fields",)),
        (rf"`expected_pipeline` \({N} lines\)", ("expected_pipeline",)),
        (rf"\bReplay assigns all {N} fields", ("options_fields",)),
    ],
}
NOUN = {"record_files": "test classes (record files)", "records": "records",
        "recorded_files": "classes with records (non-empty record files)", "descriptors": "descriptors",
        "descriptor_cases": "descriptor cases", "kind.compiler_test_case": "compiler_test_case records",
        "options_fields": "CompilerOptions fields", "expected_pipeline": "expected_pipeline lines"}


def load_gen_record_counts():
    path = os.path.join(CHECKOUT, DATA, "gen_record_counts.py")
    spec = importlib.util.spec_from_file_location("gen_record_counts", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def jsonl_gz(path):
    with gzip.open(path, "rt", encoding="utf-8", errors="surrogatepass") as fh:
        for line in fh:
            if line.strip():
                yield json.loads(line)


def corpus_counts(root=CHECKOUT):
    """{name: count}, in the order of corpus_counts.tsv."""
    files = sorted(glob.glob(os.path.join(root, RECORDS, "*.jsonl.gz")))
    kinds, apis, records, recorded = collections.Counter(), collections.Counter(), 0, 0
    for f in files:
        n = 0
        for r in jsonl_gz(f):
            n += 1
            kinds[r["kind"]] += 1
            apis[r["api"]] += 1
        records += n
        recorded += n > 0
    descriptors = sorted(glob.glob(os.path.join(root, UNIT, "descriptors", "*.json")))
    cases, case_keys = 0, collections.Counter()
    for f in descriptors:
        with open(f, encoding="utf-8") as fh:
            for c in json.load(fh)["cases"]:
                cases += 1
                case_keys.update(k for k in CASE_KEYS if c.get(k) is not None)
    pipeline = sum(1 for _ in jsonl_gz(os.path.join(root, UNIT, "derived", "expected_pipeline.jsonl.gz")))
    with open(os.path.join(root, UNIT, "options_defaults.json"), encoding="utf-8") as fh:
        defaults = json.load(fh)
    if list(defaults["fields"]) != list(defaults["fieldTypes"]):
        raise SystemExit(f"{UNIT}/options_defaults.json: fields and fieldTypes name different fields")
    with open(os.path.join(root, OPTIONS_CONFIGS), encoding="utf-8") as fh:
        configs = json.load(fh)
    out = {"record_files": len(files), "records": records, "recorded_files": recorded}
    out.update({f"kind.{k}": kinds[k] for k in sorted(kinds)})
    out.update({f"api.{a}": apis[a] for a in sorted(apis)})
    out["expected_pipeline"] = pipeline
    out["descriptors"] = len(descriptors)
    out["descriptor_cases"] = cases
    out.update({f"descriptor_cases.{k}": case_keys[k] for k in sorted(case_keys)})
    out["options_fields"] = len(defaults["fields"])
    out["options_configs"] = len(configs)
    out["options_configs.dependency"] = sum(1 for c in configs if c["config"]["kind"] == "dependency")
    return out


def counts_tsv(counts):
    return HEADER + "name\tcount\n" + "".join(f"{k}\t{v}\n" for k, v in counts.items())


def parse_tsv(text):
    """{first column: the other columns} of the rows after the column names."""
    lines = [line for line in text.splitlines() if line and not line.startswith("#")]
    return {cols[0]: cols[1:] for cols in (line.split("\t") for line in lines[1:])}


def tsv_stale(path, old, new):
    """The rows of a counts file that differ (`old` the committed text, `new` the computed one)."""
    if not old:
        return [f"{path}: missing"]
    a, b = parse_tsv(old), parse_tsv(new)
    out = []
    for k in list(a) + [k for k in b if k not in a]:
        if a.get(k) != b.get(k):
            got = "\t".join(a[k]) if k in a else "missing"
            want = "\t".join(b[k]) if k in b else "gone"
            out.append(f"{path}: {k} {got}, the corpus has {want}")
    return out or [f"{path}: differs from the corpus (layout)"]


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
    if not counts["record_files"]:
        print(f"unit_corpus_counts.py: no record files in {RECORDS}", file=sys.stderr)
        return 2
    files = {RECORD_COUNTS: load_gen_record_counts().table(os.path.join(CHECKOUT, RECORDS)),
             CORPUS_COUNTS: counts_tsv(counts)}
    stale, broken, changed = [], [], []
    for path, new in files.items():
        full = os.path.join(CHECKOUT, path)
        old = open(full, encoding="utf-8").read() if os.path.exists(full) else ""
        if old == new:
            continue
        stale += tsv_stale(path, old, new)
        if not a.check:
            with open(full, "w", encoding="utf-8") as f:
                f.write(new)
            changed.append(path)
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
    summary = (f"{RECORDS}: {counts['records']:,} records ({counts['kind.compiler_test_case']:,} "
               f"compiler_test_case) in {counts['record_files']} files ({counts['recorded_files']} non-empty); "
               f"{counts['descriptors']} descriptors ({counts['descriptor_cases']} cases); "
               f"{counts['options_fields']} CompilerOptions fields")
    if broken:
        print("\n".join(broken + ["the sentence was reworded: update PHRASES in scripts/unit_corpus_counts.py"]),
              file=sys.stderr)
        return 1
    if a.check:
        if stale:
            print("\n".join(stale + [f"({summary}; python3 scripts/unit_corpus_counts.py rewrites them)"]),
                  file=sys.stderr)
            return 1
        print(f"{summary}: the counts files and the docs quote these counts")
        return 0
    print(f"{summary}: " + (f"rewrote {', '.join(changed)}" if changed else "the counts files and the docs quote these counts"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
