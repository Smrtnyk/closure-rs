#!/usr/bin/env bash
# Rebuilds reference/closure-compiler-recording from scratch: a clone of the pristine
# reference checkout at bb8c8e7 plus oracle/patches/*.patch (test/ and BUILD only).
# The pristine checkout is only read (git clone), never modified.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$ROOT/reference/closure-compiler"
WS="$ROOT/reference/closure-compiler-recording"
COMMIT=bb8c8e7cb8d0b14b27ff5e969d186bb97017eb06
if [ -e "$WS" ]; then
  # A bazel output base may hold read-only files; make them writable before removal.
  chmod -R u+w "$WS" 2>/dev/null || true
  rm -rf "$WS"
fi
git clone --quiet --no-hardlinks --no-checkout "$REF" "$WS"
git -C "$WS" checkout --quiet --detach "$COMMIT"
# MODULE.bazel.lock is untracked in the pristine checkout; copy it so Bazel resolves identically.
[ -f "$REF/MODULE.bazel.lock" ] && cp "$REF/MODULE.bazel.lock" "$WS/MODULE.bazel.lock"
for p in $(ls "$ROOT"/oracle/patches/*.patch 2>/dev/null | sort); do
  # Guard: patches may only touch test/ and BUILD files.
  bad=$(grep -E '^\+\+\+ b/' "$p" | sed 's#^+++ b/##' | grep -Ev '^(test/|BUILD\.bazel$|oracle_recording/)' || true)
  if [ -n "$bad" ]; then echo "patch $p touches forbidden paths: $bad" >&2; exit 1; fi
  git -C "$WS" apply --whitespace=nowarn "$p"
  echo "applied $(basename "$p")"
done
echo "recording workspace ready: $WS"
