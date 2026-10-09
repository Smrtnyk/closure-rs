#!/bin/bash
# Gate 0.3 (docs/PORTING.md §4.6): fuzzer parse acceptance, pass reach, 1-hour driver run.
#
#   gates/gate_0_3.sh            all steps (resumable; run it detached)
#   gates/gate_0_3.sh report     only rewrite gates/reports/gate_0_3.{md,json}
#
# Steps (artifacts under build/fuzz/gate/):
#   0. Integration (D-016): scope/gen_flags.py --check (scope/flags.txt is what the pinned
#      CommandLineRunner gives; the driver checks its option pool against it), cargo build
#      --workspace, cargo test --workspace, cargo clippy --workspace --all-targets -- -D warnings,
#      cargo build --release --workspace; logs cargo-{build,test,clippy,release}.log and
#      integration.json (the report quotes it). The gate stops if the flags check, build, test,
#      clippy or release fails.
#   1. cargo build --release (fuzz-driver).
#   2. In the background: fuzz-driver run --duration 3600 --source mixed --engine-b java
#      (2 workers = 4 oracle JVMs at -Xmx1536m) -> run-1h.json, run-1h.exit, run-1h.log.
#      Skipped when run-1h.exit already exists.
#   3. fuzz-driver export: programs 0..4999 of seed 20261006, --source mixed, with the parse
#      filter verdict and the profile the driver picks -> export/ (skipped if complete).
#   4. gates/lib/fuzz_gate03.py reach: compile_with_pass_dumps per accepted program
#      (3 servers at -Xmx1536m, one 3g retry slot; resumable) -> reach.jsonl.
#   5. gates/lib/fuzz_gate03.py entry: the same argv plus --tracer_mode=TIMING_ONLY (pass-entry
#      counters, the Gate 0.3 definition; 3 servers; resumable) -> entry.jsonl.
#   6. Wait for step 2, then gates/lib/fuzz_gate03.py report.
# Results from an older fuzzer are never mixed in: the sha256 of fuzz/ sources is kept in
# build/fuzz/gate/fuzz-src.sha256, and on a mismatch the old directory is moved aside.
# (The hash was moved once, by hand, for clippy-only source fixes after a full
# byte-identity check of the generated programs; see build/fuzz/gate/fuzz-src.verified.json.)
# (b) is gated only on the Gate 0.3 definition: pass-entry counters (entry.jsonl), average
# per program, over all 138 DefaultPassConfig names, >= 60%. D-016 adds (b2): the union over the
# programs (runs where Java did not crash) of the passes the effect measure (fuzz/DESIGN.md) finds
# effective (reach.jsonl) must cover >= 85% of the D2-effective set (fuzz_gate03.D2_SAMPLES). The
# gate needs (a), (b), (b2) and (c); the other effect aggregates are diagnostics. The driver draws
# profiles with the D-016 weights (ADVANCED family 65%, ws 3%, other 32%) and its random option
# flags from scope/flags.txt. The report marks itself STALE when the fuzz/ sources on disk differ
# from the ones the results were measured with.
# Heap: at most 4 + 3 JVMs x 1536m (+1.5g during a 3g retry) = 12 GB.
# All oracle JVMs run in the golden environment with the ready line checked (D-010).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/paths.sh"  # ROOT, WT, SSD_WT
G=$ROOT/build/fuzz/gate
SEED=20261006
COUNT=5000
mkdir -p "$ROOT/build/logs"
cd "$ROOT" || exit 1
# A detached job records its own PID first.
[ "${1:-all}" = "report" ] || echo $$ > "$ROOT/build/logs/gate_0_3.pid"
. tools/env.sh

SRC_SHA=$(find fuzz -type f \( -name '*.rs' -o -name 'Cargo.toml' \) -not -path '*/target/*' | LC_ALL=C sort \
  | xargs sha256sum | sha256sum | cut -d' ' -f1)
if [ "${1:-all}" != "report" ] && [ -d "$G" ] && [ "$(cat "$G/fuzz-src.sha256" 2>/dev/null)" != "$SRC_SHA" ]; then
  STALE="$G-stale-$(date -u +%Y%m%dT%H%M%S)"
  echo "[gate03 $(date -u +%T)] fuzz sources changed; moving old results to $STALE"
  mv "$G" "$STALE"
  mkdir -p "$G"
  # The D2 pass-dump samples (the D2-effective diagnostic denominator, fuzz_gate03.D2_SAMPLES) are
  # measured on D2 pairs and do not depend on the fuzz/ sources: carry them over.
  [ -d "$STALE/d2x" ] && cp -a "$STALE/d2x" "$G/d2x"
fi
mkdir -p "$G"
[ -f "$G/fuzz-src.sha256" ] || echo "$SRC_SHA" > "$G/fuzz-src.sha256"

report() {
  python3 gates/lib/fuzz_gate03.py report --export-dir "$G/export" --reach "$G/reach.jsonl" \
    --entry "$G/entry.jsonl" --run-report "$G/run-1h.json" --run-exit "$G/run-1h.exit"
}

if [ "${1:-all}" = "report" ]; then
  report
  exit $?
fi

echo "[gate03 $(date -u +%T)] integration: scope/flags.txt check, cargo build/test/clippy/release (workspace, -j 8)"
python3 scope/gen_flags.py --check > "$G/scope-flags.log" 2>&1; F_EXIT=$?
cat "$G/scope-flags.log"
[ $F_EXIT -eq 0 ] || { echo "scope/flags.txt is out of date (python3 scope/gen_flags.py)"; exit 1; }
cargo build --workspace -j 8 > "$G/cargo-build.log" 2>&1; B_EXIT=$?
cargo test --workspace -j 8 > "$G/cargo-test.log" 2>&1; T_EXIT=$?
cargo clippy --workspace --all-targets -j 8 -- -D warnings > "$G/cargo-clippy.log" 2>&1; C_EXIT=$?
cargo build --release --workspace -j 8 > "$G/cargo-release.log" 2>&1; R_EXIT=$?
python3 - "$G" "$B_EXIT" "$T_EXIT" "$C_EXIT" "$R_EXIT" "$SRC_SHA" <<'PY'
import json, re, sys
g, b, t, c, r, sha = sys.argv[1], *map(int, sys.argv[2:6]), sys.argv[6]
log = open(f"{g}/cargo-test.log", encoding="utf-8", errors="replace").read()
res = [tuple(map(int, m)) for m in re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored", log)]
tp, tf, ti = (sum(x[i] for x in res) for i in range(3))
tests = f"{tp} passed, {tf} failed, {ti} ignored"
json.dump({"round": "D-016", "fuzz_src_sha256": sha, "build_exit": b, "test_exit": t, "clippy_exit": c,
           "release_exit": r, "tests": tests, "lines": [
    f"`cargo build --workspace`: exit {b}. `cargo test --workspace`: exit {t} ({tests}; log "
    "build/fuzz/gate/cargo-test.log). `cargo clippy --workspace --all-targets -- -D warnings`: exit "
    f"{c} (log build/fuzz/gate/cargo-clippy.log). `cargo build --release --workspace`: exit {r}. "
    "Toolchain: host cargo 1.95, -j 8. Run by gates/gate_0_3.sh step 0 on the fuzz/ sources "
    f"{sha[:12]} that every measurement below uses.",
    "Components: the core-ECMAScript generator (fuzz/jsgen/src/lang/, dialects "
    "default/sloppy/unsupported), the Closure generator (fuzz/jsgen/src/closure/, single-file "
    "idioms plus the multi-file goog.module/goog.provide/ES module/CommonJS API) and the mutator/minimizer, "
    "all driven by fuzz-driver `--source mixed` (lang, closure, multi, sloppy, mutate).",
    "D-016 item 5 generator coverage: fuzz/jsgen/src/closure/passes.rs (wide prototype classes, "
    "dead property writes in constructors, trivial and forwarding ES class constructors, @nosideeffects, "
    "@dict) and the multi-file program changes in items.rs/program.rs.",
    "Module forms (fuzz/jsgen/src/closure/program.rs, items.rs): destructuring goog.require "
    "and require, goog.module.get inside functions, whole-object and whole-value exports (exports = {..} / "
    "exports = X, module.exports = {..} / module.exports = X), export let/var, export default function/class, "
    "import {a as b}, export {x} from, and side-effect imports.",
    "D-016 driver changes (fuzz/driver): "
    "profiles are drawn with the "
    "D-016 weights (ADVANCED family 65%, ws 3%, other 32%); the random option flags come from "
    "scope/flags.txt (scope/gen_flags.py, checked here and at driver start-up); the report adds (b2). "
    "scope/gen_flags.py --check: exit 0."]}, open(f"{g}/integration.json", "w", encoding="utf-8"), indent=1)
PY
cat "$G/integration.json"
[ $B_EXIT -eq 0 ] && [ $T_EXIT -eq 0 ] && [ $C_EXIT -eq 0 ] && [ $R_EXIT -eq 0 ] || { echo "integration failed"; exit 1; }

echo "[gate03 $(date -u +%T)] build"
cargo build --release -j 8 -p fuzz-driver || exit 1
DRIVER=$ROOT/target/release/fuzz-driver

RUN_PID=
if [ ! -f "$G/run-1h.exit" ]; then
  echo "[gate03 $(date -u +%T)] starting 1-hour driver run"
  (
    "$DRIVER" run --seed $SEED --duration 3600 --servers 2 --engine-b java --source mixed \
      --work "$G/work-1h" --report "$G/run-1h.json" --findings "$G/findings-1h" \
      > "$G/run-1h.out" 2> "$G/run-1h.log"
    echo $? > "$G/run-1h.exit"
  ) &
  RUN_PID=$!
  echo "$RUN_PID" > "$G/run-1h.subshell.pid"
fi

if ! python3 -c "import json,sys; s=json.load(open('$G/export/summary.json')); sys.exit(0 if s['count']==$COUNT and s['seed']==$SEED else 1)" 2>/dev/null; then
  echo "[gate03 $(date -u +%T)] export $COUNT programs"
  rm -rf "$G/export"
  "$DRIVER" export --seed $SEED --count $COUNT --source mixed --servers 3 --out "$G/export" \
    > "$G/export.out" 2> "$G/export.log" || { echo "export failed"; exit 1; }
fi
cat "$G/export/summary.json"

echo "[gate03 $(date -u +%T)] reach"
python3 gates/lib/fuzz_gate03.py reach --manifest "$G/export/manifest.jsonl" \
  --results "$G/reach.jsonl" --servers 3 > "$G/reach.log" 2>&1 || { echo "reach failed"; exit 1; }

echo "[gate03 $(date -u +%T)] entry counters"
python3 gates/lib/fuzz_gate03.py entry --manifest "$G/export/manifest.jsonl" \
  --results "$G/entry.jsonl" --servers 3 > "$G/entry.log" 2>&1 || { echo "entry failed"; exit 1; }

if [ -n "$RUN_PID" ]; then
  echo "[gate03 $(date -u +%T)] waiting for the 1-hour run"
  wait "$RUN_PID"
fi
echo "[gate03 $(date -u +%T)] report"
report
