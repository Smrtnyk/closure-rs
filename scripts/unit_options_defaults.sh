#!/usr/bin/env bash
# Writes the CompilerOptions fixtures of this checkout (the one this script lives in, also when it
# is a git worktree), from the Java programs in scripts/unit_options_defaults/:
#   corpus/unit/options_defaults.json                  OptionsDefaultsDump: every field of
#       `new CompilerOptions()` in the record value encoding (corpus/unit/FORMAT.md)
#   crates/jscomp/tests/data/compiler_options_strings.json   OptionsStringsDump:
#       CompilerOptions#toString after 15 configurations (compiler_options_strings_test.rs)
#   crates/jscomp/tests/data/compiler_options_configs.json   OptionsConfigurationsDump: the fields
#       that 317 option-setting configurations change (compiler_options_configs_test.rs)
# The fields are dumped by the recorder's own encoder (UnitRecorder) from the unit support jar of
# the selected reference (scripts/paths.sh, $CLOSURE_RS_REF):
# $REF_RECORDING_WS/bazel-bin/unit_support_deploy.jar, built by scripts/unit_bazel_build.sh (test/
# support classes only, no *Test class), run with the pinned JDK (tools/jdk-21). Run twice to
# confirm the output is stable.
#   scripts/unit_options_defaults.sh [--dry-run | --help]
# `--dry-run` prints the jar, sources and outputs and writes nothing.
set -euo pipefail
case "${1:-}" in
  -h|--help) sed -n '2,/^set -euo/{/^set -euo/d;s/^# \{0,1\}//;p}' "$0"; exit 0 ;;
  ''|--dry-run) ;;
  *) echo "unit_options_defaults.sh: unknown option $1 (--dry-run, --help)" >&2; exit 2 ;;
esac
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT (tools/), REF_RECORDING_WS
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"  # this checkout: sources and outputs
SUPPORT="$REF_RECORDING_WS/bazel-bin/unit_support_deploy.jar"
SRC_DIR="$HERE/scripts/unit_options_defaults"
# main class (package com.google.javascript.jscomp) -> output, relative to this checkout
DUMPS=(
  "OptionsDefaultsDump corpus/unit/options_defaults.json"
  "OptionsStringsDump crates/jscomp/tests/data/compiler_options_strings.json"
  "OptionsConfigurationsDump crates/jscomp/tests/data/compiler_options_configs.json"
)
if [ "${1:-}" = --dry-run ]; then
  echo "reference=$REF_TAG"; echo "jar=$SUPPORT"
  for d in "${DUMPS[@]}"; do
    set -- $d
    echo "source=$SRC_DIR/$1.java"; echo "output=$HERE/$2"
  done
  exit 0
fi
[ -f "$SUPPORT" ] || { echo "missing $SUPPORT (scripts/unit_bazel_build.sh for $REF_TAG)" >&2; exit 1; }
. "$ROOT/tools/env.sh"
OUT_CLASSES="$(mktemp -d)"
TMP_OUTS=()
trap 'rm -rf "$OUT_CLASSES" "${TMP_OUTS[@]}"' EXIT
SOURCES=()
for d in "${DUMPS[@]}"; do set -- $d; SOURCES+=("$SRC_DIR/$1.java"); done
"$ROOT/tools/jdk-21/bin/javac" -nowarn -encoding UTF-8 -proc:none -d "$OUT_CLASSES" -cp "$SUPPORT" "${SOURCES[@]}"
for d in "${DUMPS[@]}"; do
  set -- $d
  out="$HERE/$2"
  TMP_OUTS+=("$out.tmp")
  env -i PATH=/usr/bin:/bin LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC \
    "$ROOT/tools/jdk-21/bin/java" -Xmx1g -cp "$OUT_CLASSES:$SUPPORT" "com.google.javascript.jscomp.$1" \
    > "$out.tmp"
  mv "$out.tmp" "$out"
  echo "wrote $out ($(sha256sum "$out" | cut -c1-16))"
done
