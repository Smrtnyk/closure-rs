# Harness semantics (what a record's `outcome` and `observed` encode)

A `compiler_test_case` record passes when replaying it through the same harness pipeline gives
the same `outcome` and `observed` (FORMAT.md "Replay"). `outcome.status = normal` therefore
stands for **every** assertion the harness makes inside the hooked call, not only the output
comparison. This file states that pipeline as a function of the recorded harness fields, with
line numbers in the pinned reference (commit bb8c8e7):
`CTC` = `reference/closure-compiler/test/com/google/javascript/jscomp/CompilerTestCase.java`,
`ITC` = `.../jscomp/integration/IntegrationTestCase.java`, `TCTC` = `.../jscomp/TypeCheckTestCase.java`,
`NS` = `reference/closure-compiler/src/com/google/javascript/rhino/testing/NodeSubject.java`,
`Node` = `.../src/com/google/javascript/rhino/Node.java`. A non-Java harness that skips a step is
weaker than the corpus and must not be used as D1 ground truth.

Every hooked API is specified.
Related: options defaults and warnings-guard order (FORMAT.md), the derived expected-side flags
(`corpus/unit/derived/expected_pipeline.jsonl.gz`), post-call neutral equality (FORMAT.md), value
decoding and processor identity (FORMAT.md), resolved DSL signatures (REPLAY.md).
Passes named below (Normalize, ProcessClosurePrimitives, TypeCheck, ...) are the ported passes
themselves, run with the arguments given; the printer is the ported CodePrinter.

## Diagnostic matching (used throughout)
- A **DiagnosticType** matches by `key` only (`DiagnosticType.equals`, `DiagnosticType.java:98`).
- An expected **Diagnostic** (`expected.diagnostics[i]`, CTC:2028-2048) matches a JSError when:
  the key matches; if a message predicate is present it holds on the error's formatted
  `description` (`exact_trimmed`: `description.trim() == message`, where `message` is already
  trimmed; `contains`: `description` contains `message`); and each of `line` (1-based), `charno`
  (0-based) and `length` that is recorded (≠ -1) equals the error's. `level` selects which list
  the diagnostic is checked against (errors or warnings); it is not compared per error.
- "**exactly**" below means Truth `containsExactlyElementsIn` with that correspondence: the two
  lists have the same length and there is a one-to-one pairing of actual and expected elements
  (any order; duplicates count). "**in order**" additionally requires pairing position by position.
- A **DiagnosticGroup** matches a JSError when the group contains the error's type (by key).

## Harness field index
Every key of `harness.fields` (21,297/21,297 `compiler_test_case` records carry all of them),
with the step where it acts. Values are taken after `harness.fieldsAfterGetOptions` is applied.
`__currentRec`, `__recCompiler`, `lastCompiler` and `setUpRan` are bookkeeping, never inputs.

| Field | Where it acts |
|---|---|
| `acceptedLanguage` | step 0: `compiler.setAllowableFeatures(acceptedLanguage.toFeatureSet())` (CTC:1739); `getOptions`: `setLanguageIn` (CTC:351). Effect on options is already in `options`. |
| `languageOut`, `browserFeaturesetYear`, `moduleResolutionMode`, `parseJsDocDocumentation`, `assumeStaticInheritanceIsNotUsed` | `getOptions` only (CTC:349-362: `setLanguageOut`, then `setBrowserFeaturesetYear` when non-null, `setModuleResolutionMode`, `setParseJsDocDocumentation`, `setAssumeStaticInheritanceIsNotUsed`). Their whole effect is already in the record's effective `options`; a harness that builds options from `options_defaults.json` + `options` must not apply them again. |
| `ignoredWarnings` | `getOptions` (CTC:372-375): when non-empty, a warnings guard setting a DiagnosticGroup of exactly these types to OFF. Already in `options.warningsGuard` (FORMAT.md "Options"). |
| `debugLoggingEnabled` | `getOptions` (CTC:378-380): debug log directory. Already in `options`; no effect on outcome. |
| `webpackModulesById` | step 0: when non-empty, `compiler.initWebpackMap(webpackModulesById)` (CTC:1740-1742). Not in `options`; the harness must apply it. |
| `defaultExternsInputs` | externs of the expected-side compiler (step 8e, CTC:1607) and the default externs of the `test(js, expected)` convenience overloads (already resolved into `inputs.externs`). |
| `allowSourcelessWarnings` | steps 8a and 9 (`validateSourceLocation`, CTC:1550-1569). |
| `declaredAccessors` | step 7 (`updateAccessorSummary`, CTC:1575-1580). |
| `normalizeExpectedOutputEnabled` | step 8e (expected-side normalization; see `corpus/unit/derived/expected_pipeline.jsonl.gz`). |
| `genericNameReplacements` | step 2. |
| `compareAsTree`, `compareJsDoc` | step 8e and `testExternChanges` (comparison mode). |
| `allowExternsChanges`, `checkAstChangeMarking` | steps 7 and 8c. |
| `expectParseWarningsInThisTest` | step 5. |
| `annotateSourceInfo`, `createModuleMap`, `astValidationEnabled` | step 6. |
| `polymerPass`, `rewriteClosureCode`, `rewriteModulesAfterTypechecking`, `closurePassEnabled`, `processCommonJsModules`, `rewriteEsModulesEnabled`, `transpileEnabled`, `librariesToInject`, `typeCheckEnabled`, `runTypeCheckAfterProcessing`, `inferConsts`, `gatherExternPropertiesEnabled`, `multistageCompilation`, `replaceTypesWithColors`, `rewriteClosureProvides`, `normalizeEnabled`, `computeSideEffects`, `typeInfoValidationEnabled`, `checkAccessControls` | step 7. |
| `closurePassEnabledForExpected` | step 8e. |
| `parseTypeInfo` | step 1 (`setCheckTypes(parseTypeInfo \|\| typeCheckEnabled)`). |

## `testInternal(externs, inputs, expected, diagnostics, postconditions)` (CTC:939-1360)

0. **createCompiler** (CTC:1737-1745, unless the test overrides it; `harness.effective.createCompiler`
   names the class used): a new Compiler; `setAllowableFeatures(acceptedLanguage.toFeatureSet())`;
   `initWebpackMap(webpackModulesById)` when non-empty; `setPreferRegexParser(false)`.
1. **Compiler** (CTC:946-953, 961-972): `options = getOptions()` (the record's `options` are the
   effective result, FORMAT.md "Options capture point"); for flat sources
   `options.setCheckTypes(parseTypeInfo || typeCheckEnabled)` then
   `compiler.init(externs, sources, options)`; for chunks `compiler.initChunks(...)`.
   `lastCompiler = compiler`. If `typeCheckEnabled`, native properties are added to the type
   registry (`BaseJSTypeTestCase.addNativeProperties`).
2. **Expected-name mapping** (CTC:1040-1049, `UnitTestUtils.updateGenericVarNamesInExpectedFiles`):
   only when `genericNameReplacements` is non-empty, an expected output is given and the inputs are
   flat (not chunks). The expected and input source lists must have equal length. For each index
   `i`: `h` = Java `String.hashCode()` of input `i`'s file name (32-bit wrapping
   `s[0]*31^(n-1) + ... + s[n-1]` over UTF-16 code units); `hs` = `"m" + (-h)` when `h < 0`
   (with 32-bit wrapping negation) else decimal `h`. For each entry `(short, prefix)` of
   `genericNameReplacements` in map order, every occurrence of `short` in expected source `i` is
   replaced (plain substring replace, left to right, non-overlapping) by `prefix + hs`. The file
   name of the expected source is kept. The reverse mapping is used only in failure messages; it
   does not change the comparison.
3. **Preconditions** (CTC:1054-1061): expected errors and expected warnings are not both
   non-empty; expected errors and an expected output are not both present; `setUpRan`.
4. **Parse** (CTC:1065-1078): `compiler.parseInputs()`. If the root is null (parse error), the
   compiler errors must match the expected errors **exactly** (Diagnostic matching), the
   postconditions run, and the call ends.
5. **Parse warnings** (CTC:1081-1085): with `expectParseWarningsInThisTest` false, there must be
   no warning after parsing; with it true, at least one.
6. **Pre-passes once** (CTC:1089-1103): `annotateSourceInfo` → SourceInformationAnnotator;
   `createModuleMap` → GatherModuleMetadata + ModuleMapCreator; `astValidationEnabled` →
   `AstValidator(validateScriptFeatures=true).validateRoot(root)`. The root is cloned
   (`rootClone`; `externsRootClone`, `mainRootClone`) for the change-reporting check.
7. **Repetitions** (CTC:1117-1318), `i = 0 .. getNumRepetitions()-1`, each only while the
   compiler has no error. Each repetition installs a fresh BlackHoleErrorManager. On repetition
   0 only, in this order: PolymerPass (`polymerPass`); CheckClosureImports + ScopedAliases
   (`rewriteClosureCode`); ClosureRewriteModule (`rewriteClosureCode` and not
   `rewriteModulesAfterTypechecking`); ProcessClosurePrimitives (`closurePassEnabled`);
   ProcessCommonJSModules (`processCommonJsModules`); ES module rewriting
   (`rewriteEsModulesEnabled || transpileEnabled`); runtime library injection
   (`librariesToInject`, unless injected from TypedASTs); TypeCheck before the processor
   (`typeCheckEnabled` and not `runTypeCheckAfterProcessing`); ClosureRewriteModule after type
   checking; InferConsts (`inferConsts`); GatherExternProperties
   (`gatherExternPropertiesEnabled`); then **multistageCompilation** (CTC:1197-1209: the
   whole AST goes through TypedAST serialization and deserialization into a new compiler,
   which becomes `lastCompiler`) or **replaceTypesWithColors** (RemoveCastNodes +
   ConvertTypesToColors); library injection from TypedASTs; ProcessClosureProvidesAndRequires
   (`rewriteClosureProvides`); transpilation to ES5 (`transpileEnabled`) else normalization
   (`normalizeEnabled`); PureFunctionIdentifier.Driver (`computeSideEffects`). On every
   repetition: **updateAccessorSummary** (CTC:1575-1580: the accessor summary is set to
   `GatherGetterAndSetterProperties.gather(root)` with every entry of `declaredAccessors` put over
   it, insertion order kept); a ChangeVerifier snapshot (`checkAstChangeMarking`);
   `compiler.beforePass(getName())`; **the processor** (`getProcessor(compiler).process`);
   `changeVerifier.checkRecordedChanges` (every changed scope must have been reported);
   **verifyAccessorSummary** (CTC:1582-1593: unless the summary is null, it must contain every
   entry `gather(root)` now returns); TypeCheck after the processor (`runTypeCheckAfterProcessing`,
   repetition 0); AstValidator with type-info validation COLOR/JSTYPE/NONE
   (`astValidationEnabled`, `typeInfoValidationEnabled`, unless halting errors);
   SourceInfoCheck; CheckAccessControls (`checkAccessControls`); warnings are aggregated; with
   `normalizeEnabled`, ValidityCheck.VerifyConstants.
8. **No expected error** (CTC:1319-1342), in this order:
   a. there must be no compiler error; **validateWarnings** (CTC:1416-1446): with no expected
      warning the aggregated warnings are empty; otherwise their count is
      `repetitions × expected` and each repetition's warnings match the expected warnings
      **exactly**; every warning passes `validateSourceLocation` (CTC:1550-1569: unless
      `allowSourcelessWarnings`, the source name is non-empty, a SCRIPT with that name exists in
      `lastCompiler`, and line and charno are not -1).
   b. with `normalizeEnabled`, the clone from step 6 is normalized
      (`Normalize.createNormalizeForOptimizations`, with "type checking has run" cleared for the
      duration).
   c. with `checkAstChangeMarking || !allowExternsChanges`, **validateCodeChangeReporting**
      (CTC:1447-1490). `codeChange` = the main clone from step 6 (normalized in 8b when
      `normalizeEnabled`) is **not** deep-equivalent to the final main root, and `externsChange`
      likewise for the externs, where deep-equivalent is the shallow comparison of mode `ast`
      below without JSDoc **plus** equal side-effect flags and equal `isUnusedParameter`, recursing
      into children but not into closure-unaware shadow roots (`DEEP_NO_SHADOW`, types ignored).
      If `externsChange` and not `allowExternsChanges`, the externs root must tree-equal its clone
      in mode `ast` (no JSDoc). If `checkAstChangeMarking`: when neither changed, no code change
      may have been reported during the call; otherwise a code change must have been reported.
   d. (nothing else runs between c and e.)
   e. if an expected output is given (`expected.output` not null; `"SAME"` means the input
      sources): the **expected side** is built by `parseExpectedJs` (CTC:1599-1646), then compared.
      Expected side, on a fresh compiler from `createCompiler()`, `init(defaultExternsInputs,
      expectedSources, getOptions())` (so `fieldsAfterGetOptions` is applied again), then
      `parseInputs()`, which must not fail ("Unexpected parse error(s)"); then, in this order,
      with `hasErrors` = the expected-side compiler's error state at that moment:
      1. RemoveCastNodes if `removeCastsExpected` (= `replaceTypesWithColors || multistageCompilation`);
      2. GatherModuleMetadata(processCommonJs=false, BROWSER) then ProcessClosurePrimitives, if
         `closurePassForExpected` (= `closurePassEnabled && closurePassEnabledForExpected`) and not `hasErrors`;
      3. ClosureRewriteModule(null, null) then ScopedAliases, if `closureRewriteModuleForExpected` (= `rewriteClosureCode`);
      4. ProcessClosureProvidesAndRequires(preserveGoogProvidesAndRequires=false), if
         `closureProvidesForExpected` (= `rewriteClosureProvides && closurePassEnabledForExpected`) and not `hasErrors`;
      5. if `transpileExpected` (= `transpileEnabled`) and not `hasErrors`: ES module rewriting
         then transpilation to ES5, which itself normalizes when `normalizeEnabled`
         (`transpileNormalizes`) whatever `normalizeExpectedOutputEnabled` says; **else** if
         `normalizeExpected` (= `normalizeEnabled && normalizeExpectedOutputEnabled && !transpileEnabled`)
         and not `hasErrors`: `Normalize.createNormalizeForOptimizations`.
      The static flags are precomputed per record in `corpus/unit/derived/expected_pipeline.jsonl.gz`
      (`scripts/unit_derive_expected_pipeline.py`, keyed by record file and line). **Do not use the
      recorded `comparison.normalizeExpected`/`closurePassForExpected`**: the recorder wrote
      `normalizeEnabled || normalizeExpectedOutputEnabled` and `closurePassEnabledForExpected`
      alone, which differ from Java in 5,060 records with an expected output (FORMAT.md
      "comparison"). Comparison: `compareAsTree` → mode `ast` below, else mode `string` below.
   f. **validateNormalizationInvariants** (CTC:1521-1547): a clone of the final main root must
      tree-equal it (mode `ast`, no JSDoc); with `normalizeEnabled`, normalizing the clone with
      `assertOnChange(true)` must change nothing and the clone must still tree-equal it.
9. **Expected errors** (CTC:1343-1355): the compiler errors must match the expected errors
   **exactly**; every error passes `validateSourceLocation` (8a) and its description has no
   unreplaced `{n}` placeholder (regex `\{\d\}`).
10. **Postconditions** (CTC:1356-1358) run on the final compiler (`lastCompiler`), in order.

Any assertion failure ends the call with `outcome.status = exception` (`assertion = true`).

## Comparison modes

### `ast` (compareExpectedToActualAsTree, CTC:1492-1505; NS:152-160, 628-660)
Actual = the main root (JS root, child 2 of the global root) after step 7; expected = the main root
of the expected side. Equality is `findFirstMismatch(actual, expected, jsDoc = compareJsDoc)`:
a pre-order walk that compares each node pair **shallowly** with `Node.isEquivalentTo(other,
compareType = false, recurse = false, jsDoc, sideEffect = false)` and then recurses into the
children pairwise and into the closure-unaware shadow roots. A shallow node comparison
(Node:2390-2475 plus the subclass overrides at Node:302-460) requires all of:
1. same `token`, same child count, same node representation class (number / bigint / string /
   template-literal-substring / plain node);
2. JSDoc (only when `compareJsDoc`): both absent, or both present and `JSDocInfo.areEquivalent`
   (`JSDocInfo.java:697-726`): the bitset of boolean flags is equal ignoring `INCLUDE_DOCUMENTATION`
   and `INLINE_TYPE`, the set of present properties is equal, and every property value is equal
   by that property's own equality (type expressions compare as type-expression trees, strings and
   lists by value). Documentation text is therefore not compared, and source positions never are;
3. the declared type expression (`getDeclaredTypeExpression`): both absent, or both present and
   equivalent (recursively, same rules);
4. for every property present on either node among: `ARROW_FN`, `ASYNC_FN`, `GENERATOR_FN`,
   `START_OF_OPT_CHAIN`, `STATIC_MEMBER`, `YIELD_ALL` (as booleans), `EXPORT_DEFAULT`,
   `EXPORT_ALL_FROM`, `INCRDECR`, `QUOTED` (as ints), `FREE_CALL`, `DIRECT_EVAL`,
   `COMPUTED_PROP_METHOD`, `COMPUTED_PROP_GETTER`, `COMPUTED_PROP_SETTER` (as booleans), the values
   are equal (Node:2487-2504). **No other property** is compared (types, colors, side-effect flags,
   `isUnusedParameter`, source positions, original names, input ids, length, etc. are ignored);
5. both nodes have a closure-unaware shadow root or neither does;
6. subclass payload: number nodes compare the double value with `==`; bigint nodes the BigInteger
   value; string-carrying nodes (NAME, STRING, STRING_KEY, GETPROP, ... any node with a string) the
   string by value; template-literal substrings both `raw` and `cooked` (null equal to null).
`genericNameReplacements` and the pretty-printer serializer affect only the failure message.

### `string` (compareExpectedToActualAsStrings, CTC:1507-1519; 78 records)
`compiler.toSource(mainRoot)` (Compiler.java:2796-2808: each license collected from the scripts is
emitted first as `"/*\n" + license + "*/\n"`, then the CodePrinter output for the node under the
compiler's effective options, without source map) with every match of the regex ` +\n` (one or more
spaces before a newline) replaced by `\n`, must equal the concatenation, in order and without
separators, of the expected sources' code (`expected.output[*].code`, after step 2).

### Integration `ast_assertNode` (ITC:174-309)
`assertNode(root).isEqualTo(expectedRoot)`: mode `ast` without JSDoc, actual = the compiled main
root, expected = `parseExpectedCode` below. Recorded `observed.output` (`compiler.toSource()`, the
compiled program with the compiler's effective options) and `observed.errors/warnings` are
observations: Java replay compares them JSON-equal; they are not the test's own assertion.

## IntegrationTestCase pipelines (`kind = integration`)
Common: **compile** (ITC:313-332): the `originals` become a chain of chunks (chunk `i` depends on
chunk `i-1`), each holding one input named `inputFileNamePrefix + i + inputFileNameSuffix`
(`JSChunkGraphBuilder.forChain`); a new `Compiler(BlackHoleErrorManager)` becomes `lastCompiler`
and runs `compileChunks(inputs.externs, chunks, options)` with the recorded `options`. API `compile`
with `inputs.chunks` compiles those chunks directly. **parseExpectedCode** (ITC:359-381): with
`processCommonJSModules` temporarily false on the same options object, a fresh `Compiler()` is
`init(externs, [input i named prefix+i+suffix for each expected string], options)`; there must be 0
errors+warnings, then `compiler.parse()` (parse only, no passes), again 0 errors+warnings; the
expected root is the main root.
- `test` (`test(options, original[], compiled[])`, also `testSame` with `compiled = original`):
  compile; errors+warnings count must be 0; if `compiled` is non-null compare (ast_assertNode).
- `test_warning` (`test(options, original[], compiled[] or null, DiagnosticGroup)`): compile;
  errors+warnings count must be exactly 1; if `compiled` non-null compare; the single diagnostic
  (the error if there is one, else the warning) must be in the group.
- `test_warnings` (`test(options, original[], compiled[] or null, DiagnosticGroup[])`): compile;
  errors+warnings count must equal the number of groups; if `compiled` non-null compare; then the
  list errors-then-warnings must match the groups **exactly** (group membership correspondence).
- `testParseError` (`original`, optional `compiled`): compile; every error must be in
  `DiagnosticGroups.PARSING`; no warnings; if `compiled` non-null compare.
- `testNoWarnings`: compile; errors empty; warnings empty.
- `compile`: compile only; the test's own follow-up checks are not recorded (always
  `notCaptured`, FORMAT.md "Post-call state").
`expected.diagnosticGroups` lists the groups (`[{group, types}]`); `expected.output` the compiled
strings or null.

## TypeCheckTestCase pipelines (`kind = type_check`)
### `TypeTestBuilder.run` (TCTC:98-210)
The builder's compiler: a new `Compiler`, options = `defaultOptions()` with MISSING_OVERRIDE,
STRICT_MISSING_PROPERTIES and STRICT_PRIMITIVE_OPERATORS set to WARNING (the record's `options`
are the effective result), `compiler.initOptions(options)`, `markFeatureNotAllowed(MODULES)`.
`run`:
1. sources must be non-empty (`inputs.sources`; files added without a name are `testcode<i>`);
2. if `includeDefaultExterns`, `defaultExterns` (CTC `DEFAULT_EXTERNS`, recorded verbatim) is put
   first in the externs list; all externs strings are joined with `"\n"` into one string;
3. expected = `expected.diagnosticTypes` (match by key) if non-empty, else
   `expected.diagnosticDescriptions` (match: `JSError.description` equals the trimmed string);
4. each group in `harness.suppress` is set to OFF on the compiler's options (in order);
5. **parseAndTypeCheckWithScope**(compiler, joinedExterns, sources, `reportUnknownTypes`), below;
6. `asserted` = errors if `diagnosticsAreErrors` else warnings; `other` = the other list;
   `asserted` must match the expected list **exactly, in order**; `other` must be empty.
Comparison names: `diagnostic_types_in_order` / `diagnostic_descriptions_in_order`.

### `parseAndTypeCheckWithScope` (TCTC:264-290)
`compiler.init([SourceFile "[externs]" with the externs string], sources, compiler.getOptions())`;
`compiler.parse()`; GatherModuleMetadata(processCommonJs=false, BROWSER); ModuleMapCreator;
InferConsts; the compiler must have **no error** ("Regarding errors:"); then
`TypeCheck(compiler, SemanticReverseAbstractInterpreter(registry), registry)
.reportUnknownTypes(r).processForTesting(externsRoot, jsRoot)`; then
`AstValidator(compiler).setTypeValidationMode(JSTYPE).process(externsRoot, jsRoot)`. Called
directly by a test (api `parseAndTypeCheckWithScope`, source named `"[testcode]" + extension`), the
call's result is used by the test's own assertions, which are not recorded: comparison
`observed_only`, always `notCaptured`.

## `testExternChanges(externs, inputs, expectedExterns, warnings...)` (CTC:1649-1735)
1. `compiler = createCompiler()` (step 0); `options = getOptions()`; flat sources
   `compiler.init(externs, sources, options)` (no `setCheckTypes` line), chunks
   `compiler.initChunks(externs, chunks, getOptions())`.
2. `compiler.parseInputs()`; with `createModuleMap`, GatherModuleMetadata(false, BROWSER) and
   ModuleMapCreator on (externsRoot, jsRoot).
3. The compiler must have no error.
4. The expected externs are parsed by `parseExpectedJs` exactly as step 8e (expected side, including
   the derived flags of this record); the compiler must still have no error.
5. `compiler.beforePass(getName())`; the processor runs on (externsRoot, jsRoot).
6. With `compareAsTree`: if the externs root has more than one child, every childless SCRIPT under
   it is removed; the expected main root is detached and compared with the **externs root** in mode
   `ast` (JSDoc when `compareJsDoc`). Otherwise `compiler.toSource(externsRoot)` must equal
   `compiler.toSource(expectedRoot)` (both printed by the actual compiler).
7. If warnings were passed (the record's `expected.diagnostics`, possibly empty): the warning count
   must equal their number and warning `i`'s type key must equal expected `i`'s key (in order;
   message/line predicates are not checked here). Errors are not checked after step 4.
No postconditions run and there are no repetitions.

## Post-call snapshot step (D-015 a)
After the hooked call returns or throws (after the outcome and `observed` are taken), a harness
takes the post-call snapshot defined in FORMAT.md "Post-call snapshot" and compares it with the
record's `postCall` (neutral equality, FORMAT.md "Post-call state: neutral equality"):
1. `compiler`: read each listed accessor on the compiler whose diagnostics fill `observed`; emit a
   key only when the value is non-null (and, for the keys FORMAT.md marks so, non-empty / non-zero /
   true). The keys include (D-017 item 2) `injectedLibraries` (injection order),
   `allowableFeatures` (Feature names in enum order), `packageJsonMainEntries`, `typedPercent` and
   `moduleMapBoundNames`, and `moduleMetadataByPath.*.nestedModules` holds the nested modules'
   metadata (recursively). A Rust harness emits them from its own compiler
   state: the runtime libraries it injected, the feature set it allows, the module loader's
   package.json main entries, the typed percentage the type checker reported, and the module map.
2. `pass`: search result producers in the processor built for the first repetition (its fields to
   object depth 2, list/array items included), then in the harness's helper objects; the first
   producer per key wins; emit the listed accessor's value.
3. A record with `postCall` passes only when the replayed snapshot has exactly the same keys and
   equal values. A Rust harness emits the same keys from its own pass results (the key, not the Java
   class, is the contract).
The opt-in test-field part is the descriptor's `testFieldsAfter` holder (step 8 / FORMAT.md
"Post-call state"), compared in the referenceable encoding. Guava tables and protobuf messages in
these values use the neutral encodings of FORMAT.md "Neutral encodings": a Rust
harness writes a table as its `[row, column, value]` cells and a protobuf message by descriptor field
names, nested messages in full, `LazyAst.script` decoded to its `AstNode`, string-pool strings as
UTF-8 strings.
4. **Compared field set.** The fields a `testFieldsAfter` check compares
   are exactly the keys of the expected post-call object: the record's `testFieldsAfter` keys, then
   minus each `testFieldsAfterSkip` field, then plus each `testFieldsAfterAlso` field not yet present
   (its value: `testFieldsAfter` if it changed, else its `testFields` entry value). Replay writes this
   set, sorted, as `comparedFields` on the record's `--post-out` line. A record is `checked` only when
   the set is non-empty; an opt-in that compares nothing leaves the record `unclassified` when its
   `testFieldsAfter` changed, and without post-call state otherwise. Gate (a) verifies each
   `testFieldsAfter.<f>` claim of corpus/unit/postcall/ against this set.

## DSL exception fidelity
Code the DSL invokes (a `new` constructor, a `static`/`call` method, a verbatim helper, a DSL-lambda
body, a postcondition built from them) may throw. Replay (`ReplayDsl.invocationFailure`) rethrows
the cause of an `InvocationTargetException` **unchanged** when it is unchecked (`Error`, including
`AssertionError` and Truth's `AssertionErrorWithFacts`, or `RuntimeException`), exactly as a direct
Java call propagates it; only a checked cause, or a reflection failure without a cause, is wrapped
in a `RuntimeException`. So an assertion raised inside a helper or a DSL-built postcondition reaches
`testInternal` with its own class and message, the record's outcome (status, class, message,
`assertion`) compares like the original, and the gate's assertion-type rule sees it in the no-op
checks. A non-Java harness propagates its own assertion failures unchanged likewise.

## Class-level no-op mutation (D-015 d)
Gate 0.2(d) turns one pass class into a no-op **wherever it is constructed** (by the harness, a
descriptor, a helper or a wrapper such as `PhaseOptimizer` or `PeepholeOptimizationsPass`), without
modifying `src/`. The Java implementation is the JVM agent `oracle/replay/agent` (`NoopAgent`, built
by `scripts/unit_noop_agent_build.sh`, using the JDK's internal ASM copy):

    java --add-exports=java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED \
         -javaagent:noop-agent.jar=<pass FQCN>,report=<file> ... ReplayMain ...

`scripts/unit_replay.sh --mutate-noop <FQCN>` and the gate's `mutate` phase pass exactly this. When
the JVM loads the pass class P, or a class nested in P (`P$...`), the agent replaces the bodies of
the entry points that class declares (concrete, non-bridge methods only), before the class is
defined:

| entry point | replacement |
|---|---|
| `process(Node, Node)`, `hotSwapScript(Node, Node)` (CompilerPass) | return |
| `process(Node, Node, OptimizeCalls.ReferenceMap)` (CallGraphCompilerPass) | return |
| `optimizeSubtree(Node)`, `transpileSubtree(Node)` (peephole optimization / transpilation) | return the argument |
| NodeTraversal callback roles (a pass handed to `NodeTraversal` directly): `shouldTraverse(NodeTraversal, Node, Node)` | return true |
| `visit(NodeTraversal, Node, Node)`, `enterScope`/`exitScope`/`enterScopeWithCfg`/`exitScopeWithCfg(NodeTraversal)`, `enterChangedScopeRoot(AbstractCompiler, Node)` | return |

Constructors, fields and every other method keep their code, so a mutated pass is still built,
configured and handed to the same callers as before; only its work is skipped. The agent writes
`TARGET`, `LOADED <class>` and `REWROTE <class>.<method><descriptor>` lines to the report file; the
gate fails a pass whose entry points were never rewritten. The gate deletes a unit's report file
right before that unit runs, and only then; a finished unit keeps the report its own run wrote, so
a resumed gate run reads the same proof. A non-Java harness implements the same contract by replacing the pass's
entry points with these no-ops.

Counting: a record counts as caught only when it passed in (a)
and fails under the mutation with an assertion-type failure (an assertion error or comparison
failure that the original call did not raise, or the reverse, a `postCall`, `testFieldsAfter` or
`observed` difference). Crashes of test-side code (NullPointerException, ClassCastException and
other runtime exceptions, harness errors) never count. The rate is over the pass's change-expecting
records (FORMAT.md "Change-expecting records"); records that expect no change are reported and are
expected to pass.

The descriptor-level stand-in (`ReplayMain --mutate-noop`, DSL.md "No-op mutation") still exists
for debugging descriptors; the gate no longer uses it.

**Harness-pass no-op for the vacuity check (gate (a)).** The same agent,
with the same flags and counting rule, also serves gate (a)'s vacuity check: for a test class whose
descriptor names `vacuityNoopClass` (FORMAT.md "No-op vacuity"), the gate's `noop` phase replays the
class once more per named pass under `-javaagent:noop-agent.jar=<that FQCN>,report=<file>`
(`build/gate02/noop/<Class>.vnc.<Simple>.*`). The class is no longer vacuous when a counted record
that passes in (a) fails there with an assertion-type failure; the agent must report at least one
`REWROTE` line, otherwise gate (a) fails.

**Inherited entry points.** When P or a class nested in P inherits a concrete entry
point from a superclass outside P (`ReplaceMessages$FullReplacementPass extends JsMessageVisitor`,
whose `process`/`visit` are declared in JsMessageVisitor), the agent reads the superclass chain from
the class loader's resources and synthesizes an overriding no-op method in the loaded class, reported
as `REWROTE <class>.<name><desc> (synthesized, inherited from <superclass>)`. An inherited entry point
declared `final` cannot be overridden and is reported as `FINAL-INHERITED` (in practice
`AbstractPostOrderCallback.shouldTraverse`, which already returns true). Gate (d) fails a pass when a
loaded pass-role class under P has no REWROTE line, unless every executable
entry point the class has is inherited from a class whose REWROTE line names that exact method
(`Gate02EntryOwners`, gates/lib/unit_gate02.py `ENTRY_JAVA`, resolves each entry point to its nearest
declaration along the superclass chain, as the agent's `inheritedEntries` does; a class with no
executable entry point is not covered). Example: `TypedScopeCreator$NormalScopeBuilder`,
`$ClassScopeBuilder` and `$FunctionScopeBuilder` declare no entry point and inherit the `final`
`shouldTraverse`/`visit` of `$AbstractScopeBuilder`, which the agent rewrote. The report lists such
classes as `agentLoadedPassClassesCoveredByInheritedRewrite`, and for every unit or group the members
that have no REWROTE line in any class (`membersWithoutRewroteLine`, reported, not a verdict input). `TypeCheck.processForTesting` is not an entry
point: it runs TypeInferencePass (a different pass class) and then `this.process`, which is rewritten.

## Pass trace and group no-op (D-017 items 8 and 11)
`oracle/replay/agent` (NoopAgent + Trace) has three modes:
- `-javaagent:noop-agent.jar=<FQCN>[,report=<file>]`: class-level no-op of one pass.
  Inherited entry points: a concrete entry point that P or P$... inherits from a superclass outside
  P (e.g. `ReplaceMessages$FullReplacementPass` from `JsMessageVisitor`) is overridden by a
  synthesized no-op and reported as `REWROTE ... (synthesized, inherited from ...)` (D-017 item 4).
- `-javaagent:noop-agent.jar=<FQCN>;<FQCN>;...[,report=<file>]`: group no-op; every listed class
  and its nested classes are no-op'd jointly in one JVM (D-017 item 11 groups).
- `-javaagent:noop-agent.jar=trace=<scope file>[,report=<file>]`: TRACE mode; nothing is no-op'd;
  each declared concrete entry point of a class whose top-level class is in the scope file starts
  with `closurers.agent.Trace.hit(id)`. `Trace.begin()`/`Trace.end()` bracket one hooked call
  (FORMAT.md "passTrace"). Trace and no-op are separate runs (one agent instance per JVM).
ReplayMain looks the Trace class up by reflection; with the agent in TRACE mode it compares every
record's `passTrace` and fails a record whose replayed set differs. Gate 0.2(a) replays every class
in TRACE mode (`replay_argv(..., trace_out=...)`, files `build/gate02/replay/<C>.trace.jsonl`).
Mutation runs (d) do not trace.
