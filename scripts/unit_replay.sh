#!/usr/bin/env bash
# Unit-corpus replay (docs/PORTING.md §4.2, gate 0.2(a)/(c)/(d)).
#   scripts/unit_replay.sh [--build-dir DIR] [--classes A,B] [--mutate-noop PassFQCN] [--coverage FILE]
# --mutate-noop is the class-level no-op mutation of D-015 (d) (HARNESS.md "Class-level no-op
# mutation"): the NoopAgent JVM agent (oracle/replay/agent) rewrites the pass class's entry points
# at load time, wherever the pass is constructed; the agent's report goes to $BUILD/noop-agent.txt.
# Compiles oracle/replay/src + oracle/replay/helpers into its own build dir (default
# build/unit/replay-$$), then replays corpus/unit/records/<A>.jsonl.gz with
# corpus/unit/descriptors/<A>.json. Default classes: every class that has a descriptor.
# Classpath = build/unit/jars/unit_support_deploy.jar (compiler + compiler_tests_lib, no *Test
# class) + the replay classes. The script refuses to run if any closure *Test class is on it.
set -euo pipefail
. "$(dirname "${BASH_SOURCE[0]}")/paths.sh"  # ROOT, WT, SSD_WT
. "$ROOT/tools/env.sh"
BUILD="$ROOT/build/unit/replay-$$"
CLASSES=""
MUTATE=""
COVERAGE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --build-dir) BUILD="$2"; shift 2;;
    --classes) CLASSES="$2"; shift 2;;
    --mutate-noop) MUTATE="$2"; shift 2;;
    --coverage) COVERAGE="$2"; shift 2;;
    *) echo "unknown arg $1" >&2; exit 2;;
  esac
done
BUILD="$(mkdir -p "$BUILD" && cd "$BUILD" && pwd)"
SUPPORT="$ROOT/build/unit/jars/unit_support_deploy.jar"
mkdir -p "$BUILD/classes"
# Rule 6: no original *Test class may be loadable.
if unzip -Z1 "$SUPPORT" | grep -E '^com/google/javascript/.*Test(\$.*)?\.class$' | grep -v -E '/(CompilerTestCase|IntegrationTestCase|CompilerTypeTestCase|TypeCheckTestCase|BaseJSTypeTestCase|SourceMapTestCase|CodePrinterTestBase)' >/dev/null; then
  echo "unit_support_deploy.jar contains a *Test class; refusing" >&2; exit 3
fi
find "$ROOT/oracle/replay/src" "$ROOT/oracle/replay/helpers" -name '*.java' > "$BUILD/sources.txt"
if grep -E 'Test\.java$' "$BUILD/sources.txt" | grep -v -E '/Replay(Compiler|Integration|TypeCheck)Test\.java$' >/dev/null; then
  echo "oracle/replay contains a *Test.java that is not a replay harness; refusing" >&2; exit 3
fi
javac -nowarn -encoding UTF-8 -proc:none -d "$BUILD/classes" -cp "$SUPPORT" @"$BUILD/sources.txt"
# Rule 6, binary-name form (same rule as gate 0.2(a)): no class on the replay classpath
# may have the binary name of an original *Test class (any reference test/**/*Test.java) or be nested
# under one, whatever source file it was compiled from.
python3 - "$ROOT/reference/closure-compiler/test" "$SUPPORT" "$BUILD/classes" <<'PY' || exit 3
import os, sys, zipfile
test_root, jar, classes = sys.argv[1:4]
orig = set()
for d, _, fs in os.walk(test_root):
    for f in fs:
        if f.endswith("Test.java"):
            orig.add(os.path.relpath(os.path.join(d, f), test_root)[:-len(".java")])
entries = [n for n in zipfile.ZipFile(jar).namelist() if n.endswith(".class")]
for d, _, fs in os.walk(classes):
    entries += [os.path.relpath(os.path.join(d, f), classes) for f in fs if f.endswith(".class")]
bad = sorted(e for e in entries if e[:-len(".class")].split("$", 1)[0] in orig)
if bad:
    print("replay classpath holds classes named as (or nested under) original *Test classes; refusing:",
          *bad[:20], sep="\n  ", file=sys.stderr)
    sys.exit(1)
print(f"rule 6 (binary names): {len(entries)} class entries, {len(orig)} original *Test classes, 0 violations")
PY
if [ -z "$CLASSES" ]; then
  CLASSES=$(ls "$ROOT/corpus/unit/descriptors" | sed -n 's/\.json$//p' | paste -sd, -)
fi
JAVA_OPTS=(-Xmx2g -Xss8m -XX:+UseParallelGC -Duser.language=en -Duser.country=US -Duser.timezone=UTC -Dfile.encoding=UTF-8)
if [ -n "$COVERAGE" ]; then
  AGENT=$(ls "${BAZEL_OUTPUT_USER_ROOT:-$HOME/.cache/bazel/_bazel_$(id -un)}"/*/external/rules_java++toolchains+remote_java_tools/java_tools/third_party/java/jacoco/jacocoagent-*.jar 2>/dev/null | head -1)
  [ -n "$AGENT" ] || { echo "no jacoco agent found" >&2; exit 4; }
  JAVA_OPTS+=("-javaagent:$AGENT=destfile=$COVERAGE,includes=com.google.javascript.*,excludes=com.google.javascript.jscomp.Replay*")
fi
ARGS=(--records "$ROOT/corpus/unit/records" --descriptors "$ROOT/corpus/unit/descriptors" --classes "$CLASSES" --out "$BUILD/failures.jsonl")
if [ -n "$MUTATE" ]; then
  bash "$ROOT/scripts/unit_noop_agent_build.sh" "$BUILD/noop-agent.jar" >/dev/null
  rm -f "$BUILD/noop-agent.txt"
  JAVA_OPTS+=(--add-exports=java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED
              "-javaagent:$BUILD/noop-agent.jar=$MUTATE,report=$BUILD/noop-agent.txt")
fi
cd "$ROOT/reference/closure-compiler-recording"
set +e
LC_ALL=C.UTF-8 TZ=UTC java "${JAVA_OPTS[@]}" -cp "$BUILD/classes:$SUPPORT" com.google.javascript.jscomp.ReplayMain "${ARGS[@]}"
rc=$?
echo "failures: $BUILD/failures.jsonl"
exit $rc
