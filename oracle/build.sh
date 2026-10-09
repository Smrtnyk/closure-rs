#!/usr/bin/env bash
# Builds the oracle jar $ORACLE_JAR (build/oracle/oracle.jar for the default reference;
# scripts/paths.sh, docs/PORTING.md §9) from this checkout's oracle/src against the pinned
# reference uberjar $REF_JAR (classpath only; the reference src/ is never modified).
# Refuses a reference whose jar sha256 is not pinned in scripts/references.tsv.
# Usage: oracle/build.sh
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/paths.sh"  # ROOT, REF_TAG, REF_JAR, REF_JAR_SHA256, ORACLE_JAR
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REF="$REF_JAR"
OUT="$(dirname "$ORACLE_JAR")"
want="$REF_JAR_SHA256"
if [ "$want" = - ] || [ -z "$want" ]; then
  echo "reference $REF_TAG has no pinned jar sha256 in scripts/references.tsv; refusing to build $OUT" >&2
  exit 1
fi
. "$ROOT/tools/env.sh"
got=$(sha256sum "$REF" | cut -d' ' -f1)
[ "$got" = "$want" ] || { echo "reference jar sha256 mismatch: $got" >&2; exit 1; }
rm -rf "$OUT/classes"; mkdir -p "$OUT/classes" "$OUT/tmp"
find "$HERE/oracle/src" -name '*.java' | sort > "$OUT/sources.txt"
javac --release 21 -nowarn -encoding UTF-8 -cp "$REF" -d "$OUT/classes" @"$OUT/sources.txt"
( cd "$OUT/classes" && jar --create --file "$ORACLE_JAR.tmp" . )
mv "$ORACLE_JAR.tmp" "$ORACLE_JAR"
echo "built $ORACLE_JAR"
