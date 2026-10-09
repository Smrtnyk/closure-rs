#!/usr/bin/env bash
# Rebuilds the recording workspace $REF_RECORDING_WS (reference/closure-compiler-recording for the
# default reference; docs/PORTING.md §9) from scratch: a clone of the pristine reference checkout
# $REF_SRC at $REF_COMMIT plus this checkout's oracle/patches/*.patch (test/ and BUILD only).
# The pristine checkout is only read (git clone), never modified.
#   scripts/unit_make_recording_ws.sh [--dry-run]   (--dry-run: print the targets, change nothing)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, REF_TAG, REF_COMMIT, REF_SRC, REF_RECORDING_WS
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"  # this checkout: oracle/patches come from here
REF="$REF_SRC"
WS="$REF_RECORDING_WS"
COMMIT="$REF_COMMIT"
DRY=0; [ "${1:-}" = --dry-run ] && DRY=1
# Guard before the rm -rf: WS must be exactly the registry's recording_ws of this reference, a
# non-empty path strictly under $ROOT/reference/, and no reference's pristine checkout.
ws_rel="$(ref_row "$REF_TAG" | cut -f4)"
case "$ws_rel" in ''|/*|*..*) echo "recording_ws '$ws_rel' of $REF_TAG is not a plain relative path; refusing" >&2; exit 1;; esac
if [ "$WS" != "$ROOT/$ws_rel" ]; then
  echo "recording workspace $WS is not the registry's recording_ws of $REF_TAG; refusing" >&2; exit 1
fi
if [ "${WS#"$ROOT"/reference/}" = "$WS" ] || [ "$WS" = "$ROOT/reference/" ]; then
  echo "recording workspace $WS is not under $ROOT/reference/; refusing" >&2; exit 1
fi
if awk -F'\t' -v w="$ws_rel" '!/^#/ && NF >= 7 && $4 == w { f = 1 } END { exit !f }' "$REF_REGISTRY"; then
  echo "recording workspace $WS is a pristine reference checkout in the registry; refusing" >&2; exit 1
fi
if [ $DRY = 1 ]; then
  echo "dry run: would rebuild $WS (reference $REF_TAG) as a clone of $REF at $COMMIT"
  exit 0
fi
if [ -e "$WS" ]; then
  # A bazel output base may hold read-only files; make them writable before removal.
  chmod -R u+w "$WS" 2>/dev/null || true
  rm -rf "$WS"
fi
git clone --quiet --no-hardlinks --no-checkout "$REF" "$WS"
git -C "$WS" checkout --quiet --detach "$COMMIT"
# MODULE.bazel.lock is untracked in the pristine checkout; copy it so Bazel resolves identically.
[ -f "$REF/MODULE.bazel.lock" ] && cp "$REF/MODULE.bazel.lock" "$WS/MODULE.bazel.lock"
for p in $(ls "$HERE"/oracle/patches/*.patch 2>/dev/null | sort); do
  # Guard: patches may only touch test/ and BUILD files.
  bad=$(grep -E '^\+\+\+ b/' "$p" | sed 's#^+++ b/##' | grep -Ev '^(test/|BUILD\.bazel$|oracle_recording/)' || true)
  if [ -n "$bad" ]; then echo "patch $p touches forbidden paths: $bad" >&2; exit 1; fi
  git -C "$WS" apply --whitespace=nowarn "$p"
  echo "applied $(basename "$p")"
done
echo "recording workspace ready: $WS"
