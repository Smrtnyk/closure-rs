#!/usr/bin/env bash
# Fetches the pinned Java reference (docs/PORTING.md §1) into its checkout: the
# scripts/references.tsv row $CLOSURE_RS_REF (default: the registry's default), commit
# $REF_COMMIT into $REF_SRC (reference/closure-compiler-v20261006 for the default; docs/PORTING.md §9).
# Idempotent; verifies the checked-out commit. Refuses to touch a checkout that holds a different
# commit (references live alongside each other; an existing checkout is never moved).
#   scripts/fetch_reference.sh [--dry-run]   (--dry-run: print the target and the guard verdict)
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, REF_TAG, REF_COMMIT, REF_SRC
PIN="$REF_COMMIT"
DEST="$REF_SRC"
DRY=0; [ "${1:-}" = --dry-run ] && DRY=1
if [ -d "$DEST/.git" ]; then
  have="$(git -C "$DEST" rev-parse -q --verify HEAD 2>/dev/null || true)"
  if [ -n "$have" ] && [ "$have" != "$PIN" ]; then
    echo "fetch_reference: $DEST holds $have, not $PIN (reference $REF_TAG); refusing to move it" >&2
    exit 1
  fi
elif [ -e "$DEST" ] && [ -n "$(ls -A "$DEST" 2>/dev/null)" ]; then
  echo "fetch_reference: $DEST exists and is not a git checkout; refusing" >&2
  exit 1
fi
if [ $DRY = 1 ]; then
  echo "dry run: would fetch $PIN (reference $REF_TAG) into $DEST"
  exit 0
fi
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
