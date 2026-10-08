"""Select a small D2 fixture without changing any recorded source text."""
import collections
import json
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[3] / "scripts"))
import paths  # noqa: E402  scripts/paths.py: ROOT is the main checkout

rows = [json.loads(line) for line in pathlib.Path(sys.argv[1]).open()]
valid = [r for r in rows if r["stderr"] == r["report_stderr"] == r["golden_stderr"]]

def encode(row):
    return json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n"

def size(row):
    return len(encode(row).encode())

def key(row):
    return row["case_id"], row["profile"]

chosen = {}
features = {
    "mapped": lambda r: "Originally at:" in r["stderr"],
    "truncated": lambda r: "\n...\n" in r["stderr"],
    "tabs": lambda r: "\t" in r["stderr"],
    "unicode": lambda r: any(ord(c) > 127 for c in r["stderr"]),
    "negative_location": lambda r: any(e["lineno"] == -1 or e["charno"] == -1 for e in r["diagnostics"]),
    "no_node": lambda r: any(not e["has_node"] for e in r["diagnostics"]),
}
for feature, test in features.items():
    row = min((r for r in valid if test(r)), key=size)
    chosen[key(row)] = row
buckets = collections.defaultdict(list)
for row in valid:
    buckets[row["source"], row["profile"]].append(row)
for bucket in sorted(buckets):
    row = min(buckets[bucket], key=size)
    if len(chosen) == 40:
        break
    if size(row) < 10000:
        chosen[key(row)] = row
for row in sorted(valid, key=lambda r: (size(r), key(r))):
    if len(chosen) == 40:
        break
    chosen[key(row)] = row
def repo_relative(row):
    """golden_path as D2Errors.java records it, relative to the repository."""
    if os.path.isabs(row["golden_path"]):
        return {**row, "golden_path": os.path.relpath(row["golden_path"], paths.ROOT)}
    return row

text = "".join(encode(repo_relative(chosen[k])) for k in sorted(chosen))
assert len(chosen) == 40
assert len(text.encode()) < 500000
pathlib.Path(sys.argv[2]).write_text(text)
print("fixture rows=40 bytes=" + str(len(text.encode())))

excluded = [r for r in rows if r["stderr"] != r["report_stderr"]]
md = "# D2 rows excluded from report-generator parity\n\n"
md += "These rows contain Java compiler stack traces emitted outside the report generator. Generator output is still checked for every row.\n\n"
md += "| Case | Profile | Reason |\n|---|---|---|\n"
for row in excluded:
    reason = "Java compiler stack trace" if "\n\tat " in row["stderr"] else "CLI text"
    md += f"| `{row['case_id']}` | `{row['profile']}` | {reason} |\n"
pathlib.Path(sys.argv[2]).with_name("D2_EXCLUSIONS.md").write_text(md)
