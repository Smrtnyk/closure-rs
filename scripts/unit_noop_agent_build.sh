#!/usr/bin/env bash
# Builds the class-level no-op mutation agent (oracle/replay/agent, HARNESS.md "Class-level no-op
# mutation", D-015 d) into a jar:  scripts/unit_noop_agent_build.sh [OUT_JAR]
# Default OUT_JAR: build/unit/noop-agent/noop-agent.jar. Uses the JDK's internal ASM copy
# (jdk.internal.org.objectweb.asm, java.base of JDK 21); the manifest exports it to the agent.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, WT, SSD_WT
. "$ROOT/tools/env.sh"
OUT="${1:-$ROOT/build/unit/noop-agent/noop-agent.jar}"
TMP="$(mktemp -d "${OUT%.jar}.build.XXXXXX" 2>/dev/null || (mkdir -p "$(dirname "$OUT")" && mktemp -d "${OUT%.jar}.build.XXXXXX"))"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/classes" "$(dirname "$OUT")"
javac -nowarn -encoding UTF-8 -proc:none \
  --add-exports java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED \
  -d "$TMP/classes" "$ROOT/oracle/replay/agent/src/closurers/agent/NoopAgent.java" "$ROOT/oracle/replay/agent/src/closurers/agent/Trace.java" 2>&1 \
  | grep -v -E '^(warning|Note|1 warning)' || true
[ -f "$TMP/classes/closurers/agent/NoopAgent.class" ] || { echo "agent compile failed" >&2; exit 1; }
cat > "$TMP/MANIFEST.MF" <<'MF'
Manifest-Version: 1.0
Premain-Class: closurers.agent.NoopAgent
Add-Exports: java.base/jdk.internal.org.objectweb.asm
Can-Redefine-Classes: false
Can-Retransform-Classes: false
MF
# Deterministic jar: fixed timestamps.
find "$TMP/classes" -exec touch -d '2026-01-01T00:00:00Z' {} +
jar --create --file "$OUT.tmp" --manifest "$TMP/MANIFEST.MF" --date 2026-01-01T00:00:00Z -C "$TMP/classes" .
mv "$OUT.tmp" "$OUT"
echo "$OUT"
