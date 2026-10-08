#!/usr/bin/env bash
# Unit-record ratchet (docs/PORTING.md §7, D-019): runs the Rust unit-record runner
# (crates/testing, bin unit_replay, as crates/testing/README.md documents) on the complete unit
# corpus and compares the records that pass with the baseline gates/unit_ratchet.json.
#   gates/unit_rust.sh [checkout-dir] [--update]
# Exit: 0 = the run completed and no record that passes in the baseline fails now (or --update
#       rewrote the baseline); 1 = regression (every such record is listed); 2 = harness error
#       (build failed, the runner did not complete, unreadable baseline).
# Report: <checkout>/build/unit-rust/<UTC time>/. See gates/README_d2_rust.md "Unit-record ratchet".
set -uo pipefail
DIR=""
UPDATE=0
for a in "$@"; do
  case "$a" in
    --update) UPDATE=1 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    -*) echo "unit_rust.sh: unknown option $a" >&2; exit 2 ;;
    *) if [ -z "$DIR" ]; then DIR="$a"; else echo "unit_rust.sh: unexpected argument $a" >&2; exit 2; fi ;;
  esac
done
DIR="${DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
DIR="$(cd "$DIR" && pwd)" || exit 2
# crates/testing/README.md runs it with CARGO_BUILD_JOBS=2; an inherited value wins.
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
BASELINE="$DIR/gates/unit_ratchet.json"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
OUT="$DIR/build/unit-rust/$STAMP"
n=1
while [ -e "$OUT" ]; do OUT="$DIR/build/unit-rust/$STAMP-$n"; n=$((n + 1)); done
mkdir -p "$OUT" || exit 2

echo "unit-record runner: cargo run --release -p closure-testing --bin unit_replay -- --all"
echo "  checkout $DIR, CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-<checkout>/target}, CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"
echo "  report   $OUT/"
start=$SECONDS
(cd "$DIR" && cargo run --release -p closure-testing --bin unit_replay -- --all \
  --report "$OUT/report.json" --records-out "$OUT/records.jsonl") >"$OUT/stdout.txt" 2>"$OUT/stderr.txt"
rc=$?
echo "  runner exit $rc after $((SECONDS - start)) s (stdout.txt / stderr.txt in the report directory)"

python3 - "$DIR" "$OUT" "$BASELINE" "$UPDATE" "$rc" <<'PY'
import hashlib, json, os, re, sys

checkout, out, baseline_path, update, rc = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4] == "1", int(sys.argv[5])
SCHEMA = "closure-rs/unit-ratchet/1"
RECORD_ID = ("Class[index]: Class = record file stem under corpus/unit/records/, index = 0-based line "
             "in Class.jsonl.gz (crates/testing/README.md, the runner's stable record id)")


def harness_error(msg):
    print(f"UNIT HARNESS ERROR: {msg}")
    try:
        with open(os.path.join(out, "stderr.txt"), errors="replace") as f:
            tail = f.read()[-4000:]
        if tail.strip():
            print("--- last lines of the runner's stderr:")
            print(tail.rstrip())
    except OSError:
        pass
    sys.exit(2)


def rid(cls, index):
    return f"{cls}[{index}]"


def sort_key(record_id):
    m = re.fullmatch(r"(.*)\[(\d+)\]", record_id)
    return (m.group(1), int(m.group(2))) if m else (record_id, -1)


def corpus_sha256():
    """Fingerprint of corpus/unit/records: sha256 over (file name, sha256 of its bytes), sorted."""
    root = os.path.join(checkout, "corpus", "unit", "records")
    h = hashlib.sha256()
    for name in sorted(os.listdir(root)):
        if not name.endswith(".jsonl.gz"):
            continue
        with open(os.path.join(root, name), "rb") as f:
            h.update(f"{name}\0{hashlib.sha256(f.read()).hexdigest()}\n".encode())
    return h.hexdigest()


def write_ratchet(path, totals, passing, sha):
    lines = ["{",
             f'  "schema": {json.dumps(SCHEMA)},',
             f'  "record_id": {json.dumps(RECORD_ID)},',
             f'  "corpus_records_sha256": {json.dumps(sha)},',
             f'  "totals": {json.dumps(totals)},',
             '  "passing": [']
    lines += [f"    {json.dumps(r)}{',' if i + 1 < len(passing) else ''}" for i, r in enumerate(passing)]
    lines += ["  ]", "}"]
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(tmp, path)


if rc not in (0, 1):
    harness_error(f"runner exited {rc} (build failure or crash outside a record)")
try:
    with open(os.path.join(out, "stdout.txt"), errors="replace") as f:
        stdout = f.read()
except OSError as e:
    harness_error(str(e))
m = re.search(r"^REPLAY_TOTAL records=(\d+) pass=(\d+) fail=(\d+) unported=(\d+)$", stdout, re.M)
if not m:
    harness_error(f"runner exited {rc} without a REPLAY_TOTAL line (loader or CLI error)")
line_totals = dict(zip(("records", "pass", "fail", "unported"), map(int, m.groups())))
try:
    with open(os.path.join(out, "report.json")) as f:
        report = json.load(f)
    statuses = {}
    harness_errors = []
    with open(os.path.join(out, "records.jsonl")) as f:
        for line in f:
            if line.strip():
                r = json.loads(line)
                key = rid(r["class"], r["index"])
                if key in statuses:
                    harness_error(f"record {key} reported twice")
                statuses[key] = r["status"]
                if str(r.get("why") or "").startswith("harness error:"):
                    harness_errors.append(f"{key}: {r['why'][:200]}")
except (OSError, ValueError, KeyError) as e:
    harness_error(f"unreadable runner output: {e}")
totals = {k: report["totals"][k] for k in ("records", "pass", "fail", "unported")}
counted = {"records": len(statuses), "pass": 0, "fail": 0, "unported": 0}
for s in statuses.values():
    counted[s if s in ("pass", "unported") else "fail"] += 1
if not (totals == line_totals == counted):
    harness_error(f"inconsistent counts: report {totals}, REPLAY_TOTAL {line_totals}, records.jsonl {counted}")
# every record must replay without a harness error (the debug-built unit_replay_runner test only
# samples records, so this full release run is where harness errors are caught)
if harness_errors:
    harness_error(f"{len(harness_errors)} records hit a harness error, e.g.\n  " + "\n  ".join(harness_errors[:20]))
if (rc == 1) != (totals["fail"] > 0):
    harness_error(f"runner exit {rc} does not match fail={totals['fail']}")

classes = report.get("classes", {})
nonempty = [(c, v) for c, v in classes.items() if v["records"]]
width = max([len(c) for c, _ in nonempty] + [5])
print(f"  {'class':<{width}} {'records':>7} {'pass':>6} {'fail':>6} {'unported':>8}")
for c, v in nonempty:
    print(f"  {c:<{width}} {v['records']:>7} {v['pass']:>6} {v['fail']:>6} {v['unported']:>8}")
print(f"  ({len(classes) - len(nonempty)} empty record files omitted)")
print(f"UNIT_TOTAL records={totals['records']} pass={totals['pass']} fail={totals['fail']} unported={totals['unported']}")

passing = sorted((k for k, s in statuses.items() if s == "pass"), key=sort_key)
sha = corpus_sha256()
write_ratchet(os.path.join(out, "ratchet.json"), totals, passing, sha)

regressed, gone, new = [], [], []
base = None
if os.path.exists(baseline_path):
    try:
        with open(baseline_path) as f:
            base = json.load(f)
        if base.get("schema") != SCHEMA or not isinstance(base.get("passing"), list):
            raise ValueError(f"schema is not {SCHEMA}")
    except (OSError, ValueError) as e:
        if not update:
            harness_error(f"baseline {baseline_path}: {e}")
        print(f"note: ignoring the unreadable baseline ({e})")
        base = None
if base is not None:
    old = set(base["passing"])
    for k in sorted(old, key=sort_key):
        if k not in statuses:
            gone.append(k)
        elif statuses[k] != "pass":
            regressed.append(k)
    new = [k for k in passing if k not in old]
    print(f"unit ratchet: baseline {len(old)} passing -> now {len(passing)} passing "
          f"({len(new)} newly passing, {len(regressed)} regressed, {len(gone)} no longer in the corpus)")
    if base.get("corpus_records_sha256") != sha:
        print("note: corpus/unit/records differs from the baseline's corpus; record ids are compared as they are")
    for k in gone:
        print(f"  not in the corpus any more (not counted): {k}")
    for k in regressed:
        print(f"  REGRESSION: {k} passed in the baseline, now {statuses[k]}")
else:
    print(f"unit ratchet: no baseline ({os.path.relpath(baseline_path, checkout)}); {len(passing)} records pass")

if update:
    write_ratchet(baseline_path, totals, passing, sha)
    print(f"unit ratchet: wrote {baseline_path} ({len(passing)} passing of {totals['records']})")
    sys.exit(0)
if regressed:
    print(f"UNIT RATCHET FAIL: {len(regressed)} previously passing records do not pass")
    sys.exit(1)
if new:
    print("note: run 'gates/unit_rust.sh <checkout> --update' after the merge to raise the baseline")
print("UNIT RATCHET OK")
sys.exit(0)
PY
