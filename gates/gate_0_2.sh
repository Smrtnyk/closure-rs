#!/usr/bin/env bash
# Gate 0.2 (docs/PORTING.md §4.2; criteria (a)-(e) as defined by D-015 and D-017): unit corpus validity.
#   (a) every counted record replays green (rule 6 proven on the replay classpath); every flagged
#       post-call-assertion method classified (captured / rust_unit_test) from corpus/unit/postcall/<Class>.json;
#       exclusions (descriptor-marked + post-call not captured) <= 3% of all records; phase noop replays every
#       compiler_test_case class with a whole-processor no-op and, for a class whose descriptor names
#       vacuityNoopClass (only read when the whole-processor no-op catches nothing), with the class-level
#       NoopAgent for each named harness-run pass; in a class still vacuous, records without replay-checked
#       post-call state or postconditions become notCaptured (FORMAT.md "No-op vacuity"); processor identity
#       and postcondition counts hold
#   (b) record count within 2% of the hook's call count, per class and in total
#   (c) JaCoCo line coverage of in-scope src/ (docs/PORTING.md §2 dirs and J2cl*/Polymer*/Chrome*/Debugger* files
#       dropped) from the replay >= 98% of the original suite's harness-based (record-bearing)
#       classes; whole-suite ratio reported; every in-scope non-harness class listed in RUST_UNIT_TESTS.md
#   (d) class-level no-op mutation (oracle/replay/agent NoopAgent) of the 30 in-scope passes with the most
#       change-expecting records fails >= 90% of those records (assertion-type failures only); D-017 item 10:
#       change-expecting = input and expected output not isEquivalentTo (parsed in the record's language mode), a
#       diagnostic, or a postcondition whose expected value differs from the whole-processor no-op value; D-017
#       item 11: records go to the ranked passes their passTrace shows (named test class -> that pass only), two or
#       more -> a group no-op'd jointly (agent form A;B;C) that must also reach 90%; TypeCheck is the unit
#       {TypedScopeCreator, TypeInferencePass, TypeCheck}; TypeCheck alone is reported for information
#   (e) records with any unrepresentable value (listed, placeholder in any section, descriptor-marked,
#       or post-call assertions not captured) <= 3%
# Phases (each resumable; finished units are skipped): prep, replay, cov-orig, cov-replay, mutate,
# noop, report.  Usage: gates/gate_0_2.sh [phase ...]   (default: all phases in order)
# Results are stamped with the sha256 of the records, descriptors, hook stats, jars, JaCoCo jars,
# replay and helper sources, JVM flags and the gate's own phase code (PHASE_CODE); prep archives stale
# results to build/gate02/stale-<time>/, and report refuses ok=true if the stamp changed since prep.
# At most 8 JVMs at -Xmx2g at a time (GATE02_PARALLEL; 16 GB heap). Run it detached:
#   setsid nohup gates/gate_0_2.sh > build/logs/gate_0_2.log 2>&1 &
# The script writes its own PID to build/logs/gate_0_2.pid.
# Exit: 0 = all of (a)-(e) pass; 1 = a criterion fails; 2 = setup error.
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/paths.sh"  # ROOT, WT, SSD_WT
mkdir -p "$ROOT/build/logs" "$ROOT/build/gate02" "$ROOT/gates/reports"
echo $$ > "$ROOT/build/logs/gate_0_2.pid"
set -uo pipefail
. "$ROOT/tools/env.sh"
PHASES=("$@")
[ ${#PHASES[@]} -eq 0 ] && PHASES=(prep replay cov-orig cov-replay mutate noop report)
for p in "${PHASES[@]}"; do
  echo "=== phase $p $(date -Is)"
  python3 "$ROOT/gates/lib/unit_gate02.py" "$p"
  rc=$?
  if [ "$p" = report ]; then echo "GATE_0_2_DONE rc=$rc $(date -Is)"; exit $rc; fi
  [ $rc -ne 0 ] && { echo "phase $p failed rc=$rc"; exit 2; }
done
