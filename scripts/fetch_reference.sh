#!/usr/bin/env bash
# Fetches the pinned Java reference (docs/PORTING.md §1) into reference/closure-compiler.
# Idempotent; verifies the checked-out commit.
set -euo pipefail
PIN=bb8c8e7cb8d0b14b27ff5e969d186bb97017eb06
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/reference/closure-compiler"
if [ ! -d "$DEST/.git" ]; then
  mkdir -p "$DEST"
  git -C "$DEST" init -q
  git -C "$DEST" remote add origin https://github.com/google/closure-compiler.git
fi
if [ "$(git -C "$DEST" rev-parse HEAD 2>/dev/null || true)" != "$PIN" ]; then
  git -C "$DEST" fetch -q --depth 1 origin "$PIN"
  git -C "$DEST" checkout -q --detach FETCH_HEAD
fi
test "$(git -C "$DEST" rev-parse HEAD)" = "$PIN"
echo "reference at $PIN: $DEST"
