#!/usr/bin/env bash
# Gate 0.2 (docs/PORTING.md §4.2; criteria (a)-(e) below, record rules in corpus/unit/FORMAT.md):
# unit corpus validity.
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
#       change-expecting records fails >= 90% of those records (assertion-type failures only);
#       change-expecting (corpus/unit/FORMAT.md) = input and expected output not isEquivalentTo (parsed
#       in the record's language mode), a diagnostic, or a postcondition whose expected value differs
#       from the whole-processor no-op value; attribution (corpus/unit/HARNESS.md "Pass trace and group
#       no-op"): records go to the ranked passes their passTrace shows (named test class -> that pass
#       only), two or more -> a group no-op'd jointly (agent form A;B;C) that must also reach 90%; TypeCheck is the unit
#       {TypedScopeCreator, TypeInferencePass, TypeCheck}; TypeCheck alone is reported for information
#   (e) records with any unrepresentable value (listed, placeholder in any section, descriptor-marked,
#       or post-call assertions not captured) <= 3%
# The corpus does not meet all of (a)-(e) yet (the baseline's strictPassing lists the criteria it
# meets). The ratchet guards it from getting worse: the report phase also compares the state behind
# (a)-(e) with the committed baseline gates/gate_0_2_ratchet.json (gates/lib/unit_gate02_ratchet.py)
# and fails on
#   - a replay failure, or another check of (a) or (d) that must always hold (rule 6, postcall schema,
#     no-op and trace completeness, postcondition counts, processor identity, 30 ranked passes);
#   - a criterion that passes in the baseline (strictPassing) and fails now;
#   - an unclassified post-call-assertion method, a record with unclassified post state, or an
#     excluded ((a)) or unrepresentable ((e)) record that the baseline does not list (by method and
#     record identity, so corpus growth cannot hide one);
#   - a (d) pass or group that fails and is not in the baseline, or fails in a new way, or a listed one
#     below 90% whose rate drops by more than max(0.5 percentage points, one change-expecting record).
# Baseline entries that no longer fail are listed as resolved (still recorded) or gone (no longer
# recorded or measured, e.g. a test deleted upstream); they are not failures. Both verdicts are written
# to gates/reports/gate_0_2.{md,json}.
# Phases (each resumable; finished units are skipped): prep, replay, cov-orig, cov-replay, mutate,
# noop, report.
#   gates/gate_0_2.sh [--ratchet] [phase ...]                  (default: all phases in order)
#   gates/gate_0_2.sh --update-baseline [--grow] [phase ...]   (default: the report phase only)
# --update-baseline rewrites the baseline from the report: it drops resolved and gone entries, raises
# (d) rates that improved and adds criteria that now pass, and refuses while the ratchet fails. With
# --grow the baseline becomes the current state and every addition is listed; use it only for an
# accepted loss (a new upstream test is classified, not baselined). Stale results and failing
# always-required checks are never baselined.
# Inputs: the records, descriptors and corpus/unit/postcall/ of the checkout, and the recording they
# come from: $UNIT_JARS_DIR (default build/unit/jars), $UNIT_RECORDING_STATS (default
# build/unit/recording/stats) and $UNIT_TEST_CLASSES (default build/unit/all_test_classes.txt). An
# upstream sync's recording is in build/upstream-sync/<tag>/unit/ (jars/, recording/stats/,
# all_test_classes.txt). The checkout is the main one, also from a worktree ($CLOSURE_RS_ROOT
# overrides it, scripts/paths.sh).
# Results are stamped with the sha256 of the records, descriptors, hook stats, jars, JaCoCo jars,
# replay and helper sources, JVM flags and the gate's own phase code (PHASE_CODE); prep archives stale
# results to build/gate02/stale-<time>/, and report refuses ok=true if the stamp changed since prep.
# At most 8 JVMs at -Xmx2g at a time (GATE02_PARALLEL; 16 GB heap). Run it detached:
#   setsid nohup gates/gate_0_2.sh > build/logs/gate_0_2.log 2>&1 &
# The script writes its own PID to build/logs/gate_0_2.pid.
# Exit: 0 = all of (a)-(e) pass (--ratchet: the ratchet holds; --update-baseline: the baseline was
#       rewritten); 1 = a criterion fails (--ratchet: the ratchet fails; --update-baseline: refused,
#       the reasons are listed); 2 = setup error (a phase failed, the report could not be computed,
#       a bad option, or with --ratchet / --update-baseline no readable baseline).
OPTS=()
PHASES=()
UPDATE=0
GROW=0
for a in "$@"; do
  case "$a" in
    --ratchet) OPTS+=("$a") ;;
    --grow) OPTS+=("$a"); GROW=1 ;;
    --update-baseline) OPTS+=("$a"); UPDATE=1 ;;
    -h|--help) sed -n '2,70p' "$0" | sed -n '/^#/!q;p'; exit 0 ;;
    -*) echo "gate_0_2.sh: unknown option $a" >&2; exit 2 ;;
    *) PHASES+=("$a") ;;
  esac
done
[ $GROW -eq 1 ] && [ $UPDATE -eq 0 ] && { echo "gate_0_2.sh: --grow needs --update-baseline" >&2; exit 2; }
if [ ${#PHASES[@]} -eq 0 ]; then
  if [ $UPDATE -eq 1 ]; then PHASES=(report); else PHASES=(prep replay cov-orig cov-replay mutate noop report); fi
fi
. "$(dirname "${BASH_SOURCE[0]}")/../scripts/paths.sh"  # ROOT, WT, SSD_WT
mkdir -p "$ROOT/build/logs" "$ROOT/build/gate02" "$ROOT/gates/reports"
echo $$ > "$ROOT/build/logs/gate_0_2.pid"
set -uo pipefail
. "$ROOT/tools/env.sh"
for p in "${PHASES[@]}"; do
  echo "=== phase $p $(date -Is)"
  if [ "$p" = report ]; then
    python3 "$ROOT/gates/lib/unit_gate02.py" report ${OPTS[@]+"${OPTS[@]}"}
    rc=$?
    echo "GATE_0_2_DONE rc=$rc $(date -Is)"; exit $rc
  fi
  python3 "$ROOT/gates/lib/unit_gate02.py" "$p"
  rc=$?
  [ $rc -ne 0 ] && { echo "phase $p failed rc=$rc"; exit 2; }
done
