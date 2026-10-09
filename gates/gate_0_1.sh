#!/usr/bin/env bash
# Gate 0.1 (docs/PORTING.md §4.1): the oracle server's compile result must be byte-identical to the
# golden uberjar result for every (case, profile) in corpus/d2/cases.jsonl.
#
# Usage: gates/gate_0_1.sh [run options...]   e.g. --servers 8 --xmx 1536m --seed 20261006
#   Defaults: 8 long-lived oracle servers at -Xmx1536m, one OutOfMemoryError retry at -Xmx3g
#   (the golden heap); worst-case heap 8 x 1.5 + 1.5 = 13.5 GB <= --heap-budget-gb 14 (host
#   limit; the run refuses to start above it).  See `python3 gates/lib/gate01.py --help`.
#   Re-runnable and resumable: pairs with a current result in build/gate01/results/ (same gate
#   version, oracle jar sha256, argv and golden file) are skipped; the seed is fixed in
#   build/gate01/run.json on the first run.  If build/oracle/oracle.jar changed, the old run is
#   archived under build/gate01/archive/ and a new one starts automatically.  --fresh archives
#   and starts over; --redo-failed re-runs server failures (never compiler-output mismatches).
#   Always (re)writes gates/reports/gate_0_1.{md,json}.  Only one run at a time (all runs write
#   the golden out_dirs build/golden-tmp/<case>/<profile>; enforced by a lock); a server at the
#   OOM-retry heap is limited to one at a time, so the heap budget is a hard bound.
#
# Independent confirmation run (another seed, own work dir, report
# gates/reports/gate_0_1.confirm-s<seed>.{md,json}; the main report lists it and fails on any
# mismatch in it):
#   GATE01_WORK=build/gate01/confirm-s20261007 gates/gate_0_1.sh --seed 20261007
#   Exit: 0 = all pairs compared, current and identical; 1 = mismatches; 2 = incomplete,
#   stale or setup error.
#
# The full run takes ~20-30 min; launch it detached:
#   setsid nohup gates/gate_0_1.sh > build/logs/gate_0_1.log 2>&1 &
#   echo $! > build/logs/gate_0_1.pid
set -uo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/paths.sh"  # ROOT, ORACLE_JAR (the reference's oracle jar)
ORACLE_REL="${ORACLE_JAR#"$ROOT"/}"  # repo-relative, as gates/lib/gate01.py resolves it
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
[ -x tools/jdk-21/bin/java ] || { echo "missing tools/jdk-21 (scripts/setup_tools.sh)" >&2; exit 2; }
[ -f "$ORACLE_REL" ] || { echo "missing $ORACLE_REL (oracle/build.sh)" >&2; exit 2; }
mkdir -p build/gate01 gates/reports
python3 gates/lib/gate01.py run "$@"
rc=$?
if [ $rc -ne 0 ]; then
  echo "gate01 run exited with $rc" >&2
fi
python3 gates/lib/gate01.py report
rep=$?
[ $rc -ne 0 ] && [ $rep -eq 0 ] && rep=2
echo "gate_0_1.sh exit $rep"
exit $rep
