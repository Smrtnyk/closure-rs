#!/usr/bin/env bash
# Builds the recording jars in $REF_RECORDING_WS (scripts/paths.sh) with Bazel and copies
# them to build/unit/jars/. Run it detached: it writes its PID first.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, REF_RECORDING_WS
echo $$ > "$ROOT/build/logs/unit_bazel.pid"
. "$ROOT/tools/env.sh"
cd "$REF_RECORDING_WS"
bazelisk build --local_resources=memory=HOST_RAM*0.3 --local_resources=cpu=HOST_CPUS*0.75 \
  //:unit_support_deploy.jar //:unit_all_tests //:compiler_tests_lib
mkdir -p "$ROOT/build/unit/jars"
cp -f bazel-bin/unit_support_deploy.jar "$ROOT/build/unit/jars/unit_support_deploy.jar"
cp -f bazel-bin/libunit_all_tests.jar "$ROOT/build/unit/jars/unit_all_tests.jar"
cp -f bazel-bin/libcompiler_tests_lib.jar "$ROOT/build/unit/jars/compiler_tests_lib.jar"
chmod u+w "$ROOT"/build/unit/jars/*.jar
echo "BAZEL_UNIT_BUILD_DONE"
