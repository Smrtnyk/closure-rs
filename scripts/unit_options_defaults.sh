#!/usr/bin/env bash
# Writes corpus/unit/options_defaults.json of this checkout (the one this script lives in, also when
# it is a git worktree): every field of `new CompilerOptions()` in the record value encoding
# (FORMAT.md), produced by the recorder's own encoder from the unit support jar of the selected
# reference (scripts/paths.sh, $CLOSURE_RS_REF): $REF_RECORDING_WS/bazel-bin/unit_support_deploy.jar,
# built by scripts/unit_bazel_build.sh (test/ support classes only, no *Test class). Run twice to
# confirm it is stable. `--dry-run` prints the jar, source and output paths and writes nothing.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT (tools/), REF_RECORDING_WS
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"  # this checkout: source and output
SUPPORT="$REF_RECORDING_WS/bazel-bin/unit_support_deploy.jar"
SRC="$HERE/scripts/unit_options_defaults/OptionsDefaultsDump.java"
OUT="$HERE/corpus/unit/options_defaults.json"
if [ "${1:-}" = --dry-run ]; then
  echo "reference=$REF_TAG"; echo "jar=$SUPPORT"; echo "source=$SRC"; echo "output=$OUT"; exit 0
fi
[ -f "$SUPPORT" ] || { echo "missing $SUPPORT (scripts/unit_bazel_build.sh for $REF_TAG)" >&2; exit 1; }
. "$ROOT/tools/env.sh"
OUT_CLASSES="$(mktemp -d)"
trap 'rm -rf "$OUT_CLASSES" "$OUT.tmp"' EXIT
"$ROOT/tools/jdk-21/bin/javac" -nowarn -encoding UTF-8 -proc:none -d "$OUT_CLASSES" -cp "$SUPPORT" "$SRC"
env -i PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
  "$ROOT/tools/jdk-21/bin/java" -Xmx1g -cp "$OUT_CLASSES:$SUPPORT" com.google.javascript.jscomp.OptionsDefaultsDump \
  > "$OUT.tmp"
mv "$OUT.tmp" "$OUT"
echo "wrote $OUT ($(sha256sum "$OUT" | cut -c1-16))"
