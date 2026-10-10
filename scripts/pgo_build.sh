#!/usr/bin/env bash
# closure-rs: profile-guided (PGO) release build of the closure-rs CLI.
#
#   scripts/pgo_build.sh --training DIR [--target TRIPLE]
#
#  1. builds closure-rs instrumented (-Cprofile-generate) in <target dir>/pgo-instrumented/;
#  2. runs it, from the repository root, on every DIR/*.args file: one compiler flag per line,
#     paths relative to the repository root (`scripts/run_bench.py --job REGEX --write-args DIR`
#     writes them for the benchmark projects of bench/projects.json); every run must succeed;
#  3. merges the profiles with llvm-profdata: $LLVM_PROFDATA, or the toolchain's own (rustup
#     component llvm-tools);
#  4. builds the release binary with the merged profile (-Cprofile-use), where
#     `cargo build --release --locked --bin closure-rs [--target TRIPLE]` puts it.
#
# The profile changes how fast the compiler runs, never what it writes (DECISIONS.md D-025): on
# the benchmark bundles the profile-guided binary takes about 8% less CPU time.
# Exit code: 0 = built with the profile; non-zero = a step failed (the caller may build without).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TRAINING="" TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --training) TRAINING="$2"; shift 2 ;;
    --target) TARGET="$2"; shift 2 ;;
    *) echo "pgo_build.sh: unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$TRAINING" ] || { echo "pgo_build.sh: --training DIR is required" >&2; exit 2; }
shopt -s nullglob
ARGS_FILES=("$TRAINING"/*.args)
[ ${#ARGS_FILES[@]} -gt 0 ] || { echo "pgo_build.sh: no .args file in $TRAINING" >&2; exit 2; }

HOST="$(rustc -vV | sed -n 's/^host: //p')"
EXE=""
case "${TARGET:-$HOST}" in *windows*) EXE=".exe" ;; esac
# rustc on Windows needs native paths; Git Bash's cygpath converts them (D:/a/... form).
native() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
SYSROOT="$(rustc --print sysroot)"
if command -v cygpath >/dev/null 2>&1; then SYSROOT="$(cygpath -u "$SYSROOT")"; fi
PROFDATA="${LLVM_PROFDATA:-$SYSROOT/lib/rustlib/$HOST/bin/llvm-profdata$EXE}"
[ -x "$PROFDATA" ] || { echo "pgo_build.sh: llvm-profdata not found at $PROFDATA (rustup component add llvm-tools)" >&2; exit 2; }

TARGET_ROOT="${CARGO_TARGET_DIR:-$ROOT/target}"
PGO="$TARGET_ROOT/pgo"
rm -rf "$PGO" && mkdir -p "$PGO/raw" "$PGO/build-scripts" build/bench
INSTRUMENTED_DIR="$TARGET_ROOT/pgo-instrumented"
OUT_SUBDIR="release"
[ -z "$TARGET" ] || OUT_SUBDIR="$TARGET/release"
TARGET_FLAG=()
[ -z "$TARGET" ] || TARGET_FLAG=(--target "$TARGET")
# (expanded as ${TARGET_FLAG[@]+"${TARGET_FLAG[@]}"}: bash 3.2, macOS' /bin/bash, takes an empty array
# for an unset variable under set -u)
BASE_RUSTFLAGS="${RUSTFLAGS:-}"

echo "== pgo_build: instrumented build"
# Build scripts of a native build are instrumented too; their profiles go elsewhere.
RUSTFLAGS="$BASE_RUSTFLAGS -Cprofile-generate=$(native "$PGO/build-scripts")" \
  CARGO_TARGET_DIR="$INSTRUMENTED_DIR" \
  cargo build --release --locked --bin closure-rs ${TARGET_FLAG[@]+"${TARGET_FLAG[@]}"}
INSTRUMENTED="$INSTRUMENTED_DIR/$OUT_SUBDIR/closure-rs$EXE"

echo "== pgo_build: training (${#ARGS_FILES[@]} compiles)"
for f in "${ARGS_FILES[@]}"; do
  # --flagfile: a project's input list can exceed the length of a Windows command line.
  if ! LLVM_PROFILE_FILE="$(native "$PGO/raw")/closure-rs-%p-%m.profraw" \
      "$INSTRUMENTED" --flagfile="$(native "$f")" > /dev/null 2> "$PGO/stderr.txt" < /dev/null; then
    echo "pgo_build.sh: training compile failed: $f" >&2
    tail -n 20 "$PGO/stderr.txt" >&2
    exit 1
  fi
  echo "   $(basename "$f")"
done

echo "== pgo_build: merging the profiles"
"$PROFDATA" merge -o "$(native "$PGO/closure-rs.profdata")" "$(native "$PGO/raw")"

echo "== pgo_build: profile-guided build"
RUSTFLAGS="$BASE_RUSTFLAGS -Cprofile-use=$(native "$PGO/closure-rs.profdata")" \
  cargo build --release --locked --bin closure-rs ${TARGET_FLAG[@]+"${TARGET_FLAG[@]}"}
echo "== pgo_build: done ($TARGET_ROOT/$OUT_SUBDIR/closure-rs$EXE)"
