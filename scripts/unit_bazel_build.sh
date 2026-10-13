#!/usr/bin/env bash
# Builds the recording jars in $REF_RECORDING_WS (scripts/paths.sh) with Bazel and copies
# them to build/unit/jars/ ($UNIT_JARS_DIR overrides it). $UNIT_BAZEL_JOBS caps Bazel's parallelism.
# Run it detached: it writes its PID first.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, REF_RECORDING_WS
JARS="${UNIT_JARS_DIR:-$ROOT/build/unit/jars}"
mkdir -p "$ROOT/build/logs"
echo $$ > "$ROOT/build/logs/unit_bazel.pid"
. "$ROOT/tools/env.sh"
cd "$REF_RECORDING_WS"
if [ -n "${UNIT_BAZEL_JOBS:-}" ]; then CPUS=(--jobs="$UNIT_BAZEL_JOBS" --local_resources=cpu="$UNIT_BAZEL_JOBS")
else CPUS=(--local_resources=cpu=HOST_CPUS*0.75); fi
bazelisk build --local_resources=memory=HOST_RAM*0.3 "${CPUS[@]}" \
  //:unit_support_deploy.jar //:unit_all_tests //:compiler_tests_lib
mkdir -p "$JARS"
cp -f bazel-bin/unit_support_deploy.jar "$JARS/unit_support_deploy.jar"
cp -f bazel-bin/libunit_all_tests.jar "$JARS/unit_all_tests.jar"
cp -f bazel-bin/libcompiler_tests_lib.jar "$JARS/compiler_tests_lib.jar"
chmod u+w "$JARS"/*.jar
echo "BAZEL_UNIT_BUILD_DONE"
