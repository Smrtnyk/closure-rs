#!/usr/bin/env bash
# Writes corpus/unit/options_defaults.json: every field of `new CompilerOptions()` in the record value
# encoding (FORMAT.md), produced by the recorder's own encoder from the unit support jar (built from
# the recording workspace; test/ support classes only, no *Test class). Run twice to confirm it is stable.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, WT, SSD_WT
. "$ROOT/tools/env.sh"
SUPPORT="$ROOT/build/unit/jars/unit_support_deploy.jar"
OUT_CLASSES="$ROOT/build/unit/options_defaults_classes"
rm -rf "$OUT_CLASSES"; mkdir -p "$OUT_CLASSES"
"$ROOT/tools/jdk-21/bin/javac" -nowarn -encoding UTF-8 -proc:none -d "$OUT_CLASSES" -cp "$SUPPORT" \
  "$ROOT/scripts/unit_options_defaults/OptionsDefaultsDump.java"
env -i PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
  "$ROOT/tools/jdk-21/bin/java" -Xmx1g -cp "$OUT_CLASSES:$SUPPORT" com.google.javascript.jscomp.OptionsDefaultsDump \
  > "$ROOT/corpus/unit/options_defaults.json.tmp"
mv "$ROOT/corpus/unit/options_defaults.json.tmp" "$ROOT/corpus/unit/options_defaults.json"
echo "wrote $ROOT/corpus/unit/options_defaults.json ($(sha256sum "$ROOT/corpus/unit/options_defaults.json" | cut -c1-16))"
