#!/usr/bin/env bash
# Builds build/oracle/oracle.jar against the pinned reference uberjar (classpath only; the
# reference src/ is never modified). Usage: oracle/build.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$ROOT/tools/env.sh"
REF="$ROOT/build/reference/closure-compiler.jar"
OUT="$ROOT/build/oracle"
want=4ef5a893f30378aad76e53efc20353e06830289a4d835be0a12cd34bab2af821
got=$(sha256sum "$REF" | cut -d' ' -f1)
[ "$got" = "$want" ] || { echo "reference jar sha256 mismatch: $got" >&2; exit 1; }
rm -rf "$OUT/classes"; mkdir -p "$OUT/classes" "$OUT/tmp"
find "$ROOT/oracle/src" -name '*.java' | sort > "$OUT/sources.txt"
javac --release 21 -nowarn -encoding UTF-8 -cp "$REF" -d "$OUT/classes" @"$OUT/sources.txt"
( cd "$OUT/classes" && jar --create --file "$OUT/oracle.jar.tmp" . )
mv "$OUT/oracle.jar.tmp" "$OUT/oracle.jar"
echo "built $OUT/oracle.jar"
