#!/usr/bin/env bash
# Full unit-corpus recording (docs/PORTING.md §4.2): every *Test class from //:unit_all_tests runs
# outside Bazel with plain JUnitCore, cwd = the recording workspace, recording hook on.
# One JVM per class (-Xmx2g), P parallel workers (default 10 => <= 20 GB heap total).
# Resumable: a class whose stats file exists is skipped. Run it detached:
#   setsid nohup scripts/unit_record_all.sh > build/logs/unit_record.log 2>&1 &
#   scripts/unit_record_all.sh [CLASS_LIST]   (default build/unit/all_test_classes.txt)
# The environment can move every output, as scripts/upstream_sync.py does for a new reference:
#   UNIT_JARS_DIR       the recording jars (default build/unit/jars, scripts/unit_bazel_build.sh)
#   UNIT_RECORDS_DIR    the records (default corpus/unit/records of the main checkout)
#   UNIT_RECORDING_DIR  stats/, logs/, the agent jar, the trace scope file and the PID file
#                       (default build/unit/recording; the others then stay where they were)
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, REF_RECORDING_WS
RECDIR="${UNIT_RECORDING_DIR:-$ROOT/build/unit/recording}"
PIDDIR="${UNIT_RECORDING_DIR:-$ROOT/build/logs}"
mkdir -p "$PIDDIR"
echo $$ > "$PIDDIR/unit_record.pid"
. "$ROOT/tools/env.sh"
P="${UNIT_RECORD_PARALLEL:-10}"
WS="$REF_RECORDING_WS"
JARS="${UNIT_JARS_DIR:-$ROOT/build/unit/jars}"
REC="${UNIT_RECORDS_DIR:-$ROOT/corpus/unit/records}"
STAT="$RECDIR/stats"
LOGD="$RECDIR/logs"
mkdir -p "$REC" "$STAT" "$LOGD"
CLASSES="${1:-$ROOT/build/unit/all_test_classes.txt}"
# RECORDING.md "Pass trace": the NoopAgent runs in TRACE mode in every recording JVM,
# so each record stores passTrace (the in-scope src pass classes whose entry points ran during the
# hooked call). Agent jar and scope file are rebuilt here (deterministic) before any class runs.
AGENT_JAR="$ROOT/build/unit/noop-agent/noop-agent.jar"
TRACE_SCOPE="$ROOT/build/unit/trace/scope.txt"
if [ -n "${UNIT_RECORDING_DIR:-}" ]; then
  AGENT_JAR="$RECDIR/noop-agent.jar"; TRACE_SCOPE="$RECDIR/trace-scope.txt"
fi
bash "$(dirname "${BASH_SOURCE[0]}")/unit_noop_agent_build.sh" "$AGENT_JAR" >/dev/null || { echo "agent build failed" >&2; exit 1; }
python3 "$(dirname "${BASH_SOURCE[0]}")/unit_trace_scope.py" "$TRACE_SCOPE" || { echo "trace scope failed" >&2; exit 1; }
export WS JARS REC STAT LOGD AGENT_JAR TRACE_SCOPE
run_one() {
  cls="$1"; name="${cls##*.}"
  [ -f "$STAT/$name.json" ] && { echo "SKIP $name"; return 0; }
  cd "$WS" || return 1
  timeout 2400 java -Xmx2g -Xss8m -XX:+UseParallelGC -Dclosurers.unit.record=true \
    --add-exports=java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED \
    "-javaagent:$AGENT_JAR=trace=$TRACE_SCOPE" \
    -Duser.language=en -Duser.country=US -Duser.timezone=UTC -Dfile.encoding=UTF-8 \
    -cp "$JARS/unit_support_deploy.jar:$JARS/unit_all_tests.jar" \
    com.google.javascript.jscomp.UnitRecordingMain "$REC" "$STAT" "$name" "$cls" \
    > "$LOGD/$name.log" 2>&1
  rc=$?
  echo "DONE $name rc=$rc $(tail -1 "$LOGD/$name.log" | cut -c1-200)"
  if [ $rc -ne 0 ]; then echo "{\"class\":\"$cls\",\"record\":\"$name\",\"jvmExit\":$rc}" > "$STAT/$name.crash.json"; fi
}
export -f run_one
# Biggest test sources first, so the long tail is short.
while read -r cls; do
  f="$WS/test/$(echo "$cls" | tr . /).java"; sz=$(stat -c %s "$f" 2>/dev/null || echo 0); echo "$sz $cls"
done < "$CLASSES" | sort -rn | awk '{print $2}' | \
  LC_ALL=C.UTF-8 TZ=UTC xargs -P "$P" -I{} bash -c 'run_one "$@"' _ {}
echo "UNIT_RECORD_ALL_DONE"
