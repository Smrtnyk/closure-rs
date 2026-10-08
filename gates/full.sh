#!/usr/bin/env bash
# Full gate: gates/ci.sh + D2 differential runner (ratchet) + unit-record runner hook.
#   gates/full.sh [checkout-dir] [--bin PATH] [--baseline RATCHET.json] [-- extra d2_rust.py run args]
# See gates/README_d2_rust.md.
set -uo pipefail
DIR=""
BIN="${CLOSURE_RS_BIN:-}"
BASELINE="${D2_BASELINE:-}"
EXTRA=()
while [ $# -gt 0 ]; do
  case "$1" in
    --bin) BIN="$2"; shift 2 ;;
    --bin=*) BIN="${1#--bin=}"; shift ;;
    --baseline) BASELINE="$2"; shift 2 ;;
    --baseline=*) BASELINE="${1#--baseline=}"; shift ;;
    --) shift; EXTRA=("$@"); break ;;
    *) if [ -z "$DIR" ]; then DIR="$1"; shift; else echo "full.sh: unexpected argument $1" >&2; exit 2; fi ;;
  esac
done
DIR="${DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
DIR="$(cd "$DIR" && pwd)" || exit 2
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
fail=0

T0=$SECONDS
echo "=== full 1/3: CI (gates/ci.sh)"
bash "$DIR/gates/ci.sh" "$DIR" || fail=1

echo "  (CI took $((SECONDS - T0)) s)"; T1=$SECONDS
echo "=== full 2/3: D2 differential runner"
if [ -z "$BASELINE" ] && [ -f "$DIR/gates/d2_ratchet.json" ]; then BASELINE="$DIR/gates/d2_ratchet.json"; fi
if [ -n "$BIN" ]; then
  BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"
else
  # A bin target defined by a package under crates/ (prefer closure-rs, the CLI's binary, then closure-compiler).
  name=$(cd "$DIR" && cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c '
import json, os, sys
try:
    meta = json.load(sys.stdin)
except ValueError:
    sys.exit(0)
crates = os.path.join(sys.argv[1], "crates") + os.sep
bins = sorted(t["name"] for p in meta["packages"] if p["manifest_path"].startswith(crates)
              for t in p["targets"] if "bin" in t["kind"])
pick = [n for n in ("closure-rs", "closure-compiler") if n in bins]
print(pick[0] if pick else (bins[0] if len(bins) == 1 else ""))
' "$DIR")
  if [ -n "$name" ]; then
    echo "building Rust CLI bin target '$name' (cargo build --release)"
    if (cd "$DIR" && cargo build --release --quiet --bin "$name"); then
      BIN="${CARGO_TARGET_DIR:-$DIR/target}/release/$name"
    else
      echo "FAIL: cargo build --release --bin $name"; fail=1
    fi
  fi
fi
# The unit-record runner (step 3) only needs the checkout and cargo, so it runs concurrently with
# the D2 runner (whose binary is built by now); its output is printed under step 3 afterwards.
UNIT_LOG=""
if [ "${FULL_SERIAL:-}" != 1 ] && [ -x "$DIR/gates/unit_rust.sh" ]; then
  UNIT_LOG=$(mktemp); ( "$DIR/gates/unit_rust.sh" "$DIR" >"$UNIT_LOG" 2>&1; echo $? >"$UNIT_LOG.rc" ) &
  UNIT_PID=$!
fi
if [ -n "$BIN" ] && [ -x "$BIN" ]; then
  args=(run --bin "$BIN")
  if [ -n "$BASELINE" ] && [ "${D2_GATE_MODE:-}" = passing ]; then
    # quick regression check: only the previously passing pairs (the ratchet's guarantee); raising
    # the baseline needs a complete run
    echo "D2 gate mode: passing set only ($BASELINE); do not copy this run's ratchet.json as the baseline"
    args+=(--only-passing "$BASELINE")
  elif [ -n "$BASELINE" ]; then args+=(--baseline "$BASELINE"); fi
  python3 "$DIR/gates/d2_rust.py" "${args[@]}" "${EXTRA[@]}" || { echo "FAIL: D2 runner / ratchet"; fail=1; }
elif [ -n "$BIN" ]; then
  echo "FAIL: D2 binary $BIN is not executable"; fail=1
else
  echo "D2: SKIP (no Rust CLI binary yet)"
  if [ -n "$BASELINE" ]; then
    passes=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["total"]["pass"])' "$BASELINE" 2>/dev/null || echo "?")
    if [ "$passes" != "0" ]; then echo "FAIL: baseline $BASELINE has $passes passing pairs but there is no binary (regression)"; fail=1; fi
  fi
fi

echo "=== full 3/3: unit-record runner"
if [ -n "$UNIT_LOG" ]; then
  wait "$UNIT_PID"; cat "$UNIT_LOG"
  [ "$(cat "$UNIT_LOG.rc" 2>/dev/null)" = 0 ] || { echo "FAIL: unit-record runner"; fail=1; }
  rm -f "$UNIT_LOG" "$UNIT_LOG.rc"
elif [ -x "$DIR/gates/unit_rust.sh" ]; then
  "$DIR/gates/unit_rust.sh" "$DIR" || { echo "FAIL: unit-record runner"; fail=1; }
else
  echo "UNIT: SKIP (unit-record runner not added yet)"
fi

echo "  (full gate took $((SECONDS - T0)) s; D2 and unit runners $((SECONDS - T1)) s)"
if [ $fail -eq 0 ]; then echo "FULL PASS"; else echo "FULL FAIL"; fi
exit $fail
