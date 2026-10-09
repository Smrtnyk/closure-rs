# Unit corpus recording (docs/PORTING.md §4.2)

## Method
- **Workspace:** `scripts/unit_make_recording_ws.sh` clones the pristine reference checkout
  (`$REF_SRC`, `reference/closure-compiler-v20261006` at `48f4107ca` for the default reference
  `v20261006`; docs/PORTING.md §9) into `$REF_RECORDING_WS`,
  copies the untracked `MODULE.bazel.lock`, and applies `oracle/patches/*.patch`. The script
  refuses any patch that touches `src/`.
- **Jars:** `scripts/unit_bazel_build.sh` builds `build/unit/jars/unit_support_deploy.jar`
  (`//:unit_support`: the compiler, its runtime deps and compiler_tests_lib, with no closure
  `*Test` class) and `unit_all_tests.jar` (all 432 `*Test.java` files).
- **Recording:** `scripts/unit_record_all.sh` runs every one of the 432 classes outside Bazel:
  plain JUnitCore (`UnitRecordingMain`), cwd = the recording workspace,
  `-Dclosurers.unit.record=true`, one JVM per class at `-Xmx2g -Xss8m`, 10 JVMs in parallel
  (`UNIT_RECORD_PARALLEL`). The run is resumable through the per-class stats files; run it
  detached.
- **Hooks:** listed in FORMAT.md. Only the outermost hooked call makes a record. Every hooked
  entry, at any depth, is counted in `build/unit/recording/stats/<Name>.json`.
- **Summary:** `scripts/unit_report.sh` writes `build/unit/report/{classes.json,table.md}`
  (per-class entries and records, kinds, failures, unrepresentable values).

The corpus holds 24,782 records from 255 of the 432 test classes (kinds compiler_test_case,
type_check and integration). Hooked entries below the outermost one are nested calls, such as
`compile` inside `test` or `parseAndTypeCheckWithScope` inside `TypeTestBuilder.run`; they are
counted but, by design, not recorded.

## What the recorder writes
The recorder is `UnitRecorder` in patch 0002 (FORMAT.md has the schema):
- **Processor dump:** the pass under test, 2 object levels deep, so a wrapper's wrapped pass dumps
  its configuration; unrepresentable values are listed under `processor.` paths.
- **Test fields:** `testFields` and `testFieldsAfter` (and `harness.fieldsAfterGetOptions`) list
  their unrepresentable values (`testAfter.` / `harnessAfterGetOptions.` paths, only for the
  fields they keep). A `TypedScope` value is dumped as `{"typedScope": ...}` (every variable's
  name, type string and isTypeInferred), so TypedScopeCreatorTest's
  globalScope/lastLocalScope/lastFunctionScope are in testFieldsAfter.
- **Referenceable encoding:** in `testFields`, `testFieldsAfter` and the processor dump, lambdas,
  cycles, objects beyond the depth limit and unreadable fields become `{"ref": "<FQCN>"}` and are
  not listed as unrepresentable (FORMAT.md "Referenceable values").
- **Post-call snapshot:** every compiler_test_case record carries `postCall`, taken right after
  the hooked call (`UnitRecorder.postCallSnapshot`): compiler accessors (`accessorSummary`,
  `externProperties`, `moduleMetadataByPath` with the nested modules' metadata, the variable,
  property and string maps, the source map when set, `typeMismatches`, `runJ2clPasses`,
  `injectedLibraries`, `allowableFeatures`, `packageJsonMainEntries`, `typedPercent`,
  `moduleMapBoundNames`) and pass producers (`pass.ambiguatedPropertyMap`, `pass.variableMap`,
  `pass.idGeneratorMappings`, `pass.propertyMap`, `pass.globalRegExpPropertiesUsed`,
  `pass.exportedVariableNames`, `pass.stringMap`, `pass.referenceMap`,
  `pass.crossChunkReferences`). The replay compares `postCall` exactly ("postCall differs" fails
  a record). See FORMAT.md "Post-call snapshot".
- **Neutral encodings:** protobuf messages and builders are written as
  `{"proto": full name, "fields": {...}}` (set fields by descriptor name, nested messages in full
  with no depth limit, no memoized/unknown fields; `StringPoolProto.strings` as UTF-8 strings,
  `LazyAst.script` decoded to its `AstNode`); Guava `Table` values as
  `{"table": [[row, column, value], ...], "impl": ...}`. Replay decodes the protobuf encoding back
  into messages where a recorded value is rebuilt (`ReplayValues`, for example
  ClosureUnawareCodeIntegrationTest's `ConformanceConfig` options).
- **Postcondition data** (FORMAT.md "Postcondition data") is captured by `CompilerTestCase`
  (`__notePostconditionCompiler` right before each postcondition loop of `testInternal`,
  `__recDepth` to recognise the outermost call).

## Pass trace (D-017 item 8)
The recording does not run through Bazel targets; it runs `UnitRecordingMain` with plain `java`
(`scripts/unit_record_all.sh`). That script builds the NoopAgent jar
(`scripts/unit_noop_agent_build.sh`) and the trace scope file (`scripts/unit_trace_scope.py`,
the in-scope `src/` top-level classes of the reference) and adds
`--add-exports=java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED
-javaagent:build/unit/noop-agent/noop-agent.jar=trace=build/unit/trace/scope.txt` to every
recording JVM, so each record stores `passTrace`. `UnitRecorder.enter` (outermost call) calls
`Trace.begin()` and `UnitRecorder.exit` calls `Trace.end()`, by reflection; without the agent the
recorder writes no `passTrace`. No `src/` file is touched; the patch (0002) changes `test/` only.

## Changing the recorder
1. Edit the recording workspace (`$REF_RECORDING_WS`, `reference/closure-compiler-v20261006-recording`
   for the default reference, `test/` only).
2. Regenerate `oracle/patches/0002-recording-hooks.patch` from it (`git diff -- test/`) and check
   that a fresh clone of the pristine reference checkout plus patches 0001 and 0002 is
   byte-identical to the workspace in `test/` and `BUILD.bazel`, and that its `src/` is identical
   to the pristine `src/`.
3. Rebuild `build/unit/jars` (`scripts/unit_bazel_build.sh`; the oracle jar is not touched,
   D-010), archive the previous records and stats, and re-record all classes
   (`scripts/unit_record_all.sh`).
4. Check the new records against the old ones: the record count per class, JVM crashes
   (`*.crash.json`), records with `postCallError`, and `scripts/unit_regress_check.py` (every
   record that replayed green before replays green now), then replay the corpus
   (`scripts/unit_replay.sh`, `gates/gate_0_2.sh`).

## Failures outside Bazel
Two Java tests fail outside Bazel:
- `integration.IntegrationTest#testIssue63SourceMap`
- `parsing.ParserTest#testParseDeep4`

The second depends on stack depth (the recording runs with `-Xss8m`).

## Replay through Java (gate 0.2(a))
`scripts/unit_replay.sh [--build-dir DIR] [--classes A,B] [--mutate-noop PassClass] [--coverage FILE]`
compiles `oracle/replay/src` and `oracle/replay/helpers` into its own build dir and runs on
`unit_support_deploy.jar`. It refuses to run if a closure `*Test` class is on the classpath.

A record passes when the replayed outcome and the observed diagnostics are identical:
- the outcome: status, exception class and message;
- the observed errors and warnings: key, level, description and location;
- for integration records, also the output source;
- the `postCall` snapshot.

Class-level no-op mutation: `oracle/replay/agent` (NoopAgent, HARNESS.md "Class-level no-op
mutation"). With `-javaagent:noop-agent.jar=com.google.javascript.jscomp.RenameVars`, the agent
reports `REWROTE com.google.javascript.jscomp.RenameVars.process(Node,Node)V` and RenameVarsTest
records fail with assertion-type failures, while classes that do not use RenameVars are
unaffected.

## Caveats
- **Classes with no records:** most of the 177 classes without records do not extend a hooked
  harness (Node, JSType and graph unit tests, which docs/PORTING.md §4.2 excludes). Some
  CompilerTestCase subclasses also never call the harness test API, for example
  ExpressionDecomposerTest. They are listed in RUST_UNIT_TESTS.md.
- **Unhooked harnesses:** BaseJSTypeTestCase, SourceMapTestCase, CodePrinterTestBase, and
  CompilerTypeTestCase subclasses that do not go through TypeCheckTestCase.
- **Postcondition lambdas** are not replayed as code; their data is recorded
  (`expected.postconditions`, FORMAT.md "Postcondition data").
