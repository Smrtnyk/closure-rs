# Unit corpus record format (v2)

Records come from runtime hooks in the Java test harness (`oracle/patches/0002-recording-hooks.patch`,
`test/com/google/javascript/jscomp/UnitRecorder.java`). One file per test class:
`corpus/unit/records/<TestClassSimpleName>.jsonl.gz` — **gzip-compressed JSONL** (one JSON object per
line). Every file is gzipped unconditionally: uncompressed records are ~30 KB each because externs
are repeated in every record, which would put the corpus far above 150 MB. Read with `zcat`.
Each record is self-contained (no cross-record references).

**Strings.** JSON strings hold UTF-16 strings (JS and Java semantics). A paired surrogate is
written as its 4-byte UTF-8 character. A lone surrogate, which UTF-8 cannot encode, is written as a
`\uXXXX` escape (lowercase hex). Readers must decode JSON strings into WTF-16 (for example a
`Vec<u16>`), not into UTF-8 strings, or lone surrogates are lost (docs/PORTING.md §8).

**Determinism.** Records are written in test execution order. `harness.overrides` is sorted by
`method` then `declaredIn`, and lambda class names drop their run-dependent address
(`Foo$$Lambda/0x…` is written `Foo$$Lambda`). Known residue: generic `object` dumps of hash-based
internals (for example a `LinkedHashMultimap` in `options.customPasses`, or JSType maps in
`testFields`) can differ between runs of the recording.

## Top-level fields

| Field | Meaning |
|---|---|
| `v` | format version (2) |
| `class`, `method` | JUnit test class (FQCN) and test method (parameterized: `name[i]`) |
| `call` | 0-based index of this record among records of the same `class#method` |
| `kind` | `compiler_test_case`, `integration`, `type_check` |
| `api` | hooked entry point: `testInternal`, `testExternChanges` (CompilerTestCase); `test`, `test_warning`, `test_warnings`, `testParseError`, `testNoWarnings`, `compile` (IntegrationTestCase); `TypeTestBuilder.run`, `parseAndTypeCheckWithScope` (TypeCheckTestCase) |
| `instanceClass` | runtime class of the test instance (absent for static APIs) |
| `inputs` | see below |
| `expected` | see below |
| `comparison` | comparison mode, see below |
| `harness` | harness state, see below |
| `testFields` | reflective dump of the test instance's own fields (declared below the harness base), taken at entry to the hooked call |
| `testFieldsAfter` | CompilerTestCase only: the `testFields` entries whose dump changed during the hooked call (field name → value after the call); `{}` when none changed. Same encoding and depth as `testFields`; its unrepresentable values are listed with path prefix `testAfter.`. Replay must check it or classify it (see "Post-call state"). |
| `options` | `CompilerOptions` diff against `new CompilerOptions()` (field name → value); `@class` if a subclass. When captured: see "Options capture point". |
| `processor` | CompilerTestCase only: `{class, fields}` of what `getProcessor` returned (first repetition), fields dumped to object depth 2 (so a wrapper's wrapped pass dumps its own fields); `null` if the processor never ran (e.g. parse error). Its unrepresentable values are listed with path prefix `processor.`. |
| `observed` | what actually happened: `errors`, `warnings` (lists of JSError dumps); integration also `output` (`compiler.toSource()`) and `compilerClass` |
| `outcome` | `{status: "normal"}` or `{status: "exception", exceptionClass, message, assertion}` — the exception that left the hooked call. Ground truth for replay (tests wrapped in `assertThrows` show up here). See "Outcome contract". |
| `unrepresentable` | list of `{path, reason}` for **every** `{"unrepresentable":…}` placeholder in the record, in every section (`test`, `testAfter`, `harness`, `harnessAfterGetOptions`, `processor`, `options`, `expected`), except values nested inside a lambda's `captures` (the lambda itself is listed). Path grammar: see "Unrepresentable paths". The processor and `testFieldsAfter`/`fieldsAfterGetOptions` dumps are listed too. |
| `testFieldsAfterError` | only when dumping the post-call fields threw: the exception's `toString()` |

Only the **outermost** hooked call on a thread produces a record; nested hooked calls (e.g. `compile`
inside `test`) are counted but not recorded. Per-class counts of all entries and outermost entries
are in `build/unit/recording/stats/<Name>.json` (`entries`, `outermost`, `records`, per-test results).

## observed
- `errors`, `warnings`: lists of JSError dumps, in the order the compiler reported them
  (`Compiler.getErrors()` / `getWarnings()`). Each dump is
  `{key, defaultLevel, description, source, line, charno, length}`:
  `key` = `DiagnosticType.key`; `defaultLevel` = the DiagnosticType's **default** level
  (`ERROR`/`WARNING`/`OFF`), not the effective level (the list it is in gives the effective
  level); `description` = the formatted message; `source` = source file name or null;
  `line` = 1-based line, `charno` = 0-based column, `length` = source length of the node;
  each is -1 when unknown.
- integration only: `output` (`compiler.toSource()`), `compilerClass` (informational, never compared).

## Options capture point
`options` is `compiler.getOptions()` dumped at the **first entry of the processor** (right before
`getProcessor(compiler).process(...)`), so after the harness's `getOptions()` and its mutations
(for example `setCheckTypes(parseTypeInfo || typeCheckEnabled)`), after `Compiler.init` (which
runs `initOptions` and `reconcileOptionsWithGuards`; for example `checkTypes` becomes true when a
type-based diagnostic group such as `MISSING_PROPERTIES` is enabled by a warnings guard) and
after any `compilerSetup` effects. When the processor never ran (parse error, exception), the
options of `lastCompiler` are dumped at exit instead. They are therefore the **effective**
options. Replay rebuilds them with `new CompilerOptions()` plus the diff and then passes them
through the same harness path (`getOptions()` → `Compiler.init`), which re-runs
`initOptions`/`reconcileOptionsWithGuards`; both only set fields to values derived from the
guards and options already present, so re-applying them to effective options is idempotent
(gate 0.2(a) replays every record this way).

## Options defaults and warnings-guard order
`options` is a diff against `new CompilerOptions()`; the defaults it is a diff against are committed
in `corpus/unit/options_defaults.json`: `{"v":2, "class", "depth":4, "fields":{name: value},
"unrepresentable":[]}`, every instance field of `new CompilerOptions()` dumped by the recorder's own
encoder (`UnitRecorder.fields`, the same call and depth `optionsDiff` uses for the defaults side), in
the value encoding below. Regenerate with `scripts/unit_options_defaults.sh` (two consecutive runs
give the same sha256). Effective options of a record = `options_defaults.fields` with each key of
`options` replaced by its value (`@class` names a subclass). A Rust harness checks its own
`CompilerOptions` defaults against this file field by field; the field names are the Java names, and
the file is the schema.

**`composeWarningsGuard` order** (`ComposeWarningsGuard.java:47-120`). The guards list is in
*effective order*: the order in which `level(error)` consults them. That order is: ascending
priority value (`MAX`/`FILTER_BY_PATH` = 1, `SUPPRESS_DOC` = 20, `SUPPRESS_BY_ALLOWLIST` = 40,
`DEFAULT` = 50, `MIN`/`STRICT` = 100; `DiagnosticGroupWarningsGuard` has `DEFAULT`), and within
equal priority the **most recently added first**. Adding a guard that is already present moves it to
the most-recent position. Adding a nested compose guard adds its guards from last to first (so they
keep their relative order) and copies its `demoteErrors` when true. `level(error)` returns the level
of the first guard that returns a non-null level, demoted from ERROR to WARNING when `demoteErrors`
is set, else null (the diagnostic keeps its default level). Rebuilding from the recorded list:
start from an empty compose guard (the default, `options_defaults.json` `warningsGuard`) and add the
recorded guards in **reverse** recorded order; this yields exactly the recorded order (replay does
this, `ReplayValues.java` `composeWarningsGuard`). `demoteErrors` is not part of the encoding.

## inputs
- CompilerTestCase: `externs` (list of SourceFile), and either `sources` (list) or `chunks`.
- Integration: `externs`, and `originals` (String[] passed to `test`) with `inputFileNamePrefix`/`inputFileNameSuffix`
  (inputs are built as a chain of chunks, file name `prefix + i + suffix`), or `chunks` for direct `compile`.
- Type check builder: `sources`, `externStrings`, `includeDefaultExterns`, `defaultExterns` (text).
- SourceFile: `{name, code, kind}` (`kind` = SourceKind). Chunk: `{name, deps:[names], inputs:[SourceFile]}`.

## expected
- CompilerTestCase: `output` = list of SourceFile, `"SAME"`, or `null` (no output check);
  `diagnostics` = `[{key, level, messageMode?, message?, line?, charno?, length?}]` where `messageMode` is
  `exact_trimmed` (`withMessage`), `contains` (`withMessageContaining`) or `unknown` (any other
  message predicate; `message` is then the predicate's name verbatim and replay reports a harness
  error, so such a record cannot pass); `postconditions` = count
  (postcondition lambdas are listed in `unrepresentable`); `postconditionValues` = one value per
  postcondition, in order, in the value encoding below. A lambda is
  `{"unrepresentable": "Foo$$Lambda", "captures": {"arg$1": v, ...}}`: its captured values (javac
  names them `arg$1`, `arg$2`, … in capture order; a captured test instance is `{"ref": FQCN}`). A
  descriptor can rebuild the postcondition from them (DSL.md `postconditions`).
- Integration: `output` (String[] or null), `diagnosticGroups` (`[{group, types}]` or null).
- Type check: `diagnosticTypes` (`[{key, level}]`) or `diagnosticDescriptions` (strings, trimmed).

## comparison
- CompilerTestCase: `mode` `ast` (compareAsTree, `isEquivalentTo`) or `string`; `compareJsDoc`;
  `normalizeExpected`; `closurePassForExpected`. **Warning:** the recorded
  `normalizeExpected` is `normalizeEnabled || normalizeExpectedOutputEnabled` and
  `closurePassForExpected` is `closurePassEnabledForExpected` alone; neither is the condition Java
  applies to the expected side (5,060 records with an expected output have `normalizeExpected: true`
  where Java does not normalize). They are kept unchanged so the records stay byte-identical to what
  the committed recording patch produces. The effective expected-side pipeline is in
  `corpus/unit/derived/expected_pipeline.jsonl.gz` (one line per `compiler_test_case` record,
  keyed by `file` = record file base name and `index` = 0-based line; flags `removeCastsExpected`,
  `closurePassForExpected`, `closureRewriteModuleForExpected`, `closureProvidesForExpected`,
  `transpileExpected`, `normalizeExpected`, `transpileNormalizes`), generated from `harness.fields`
  + `harness.fieldsAfterGetOptions` by `scripts/unit_derive_expected_pipeline.py` (`--check`
  verifies it is fresh). Semantics: HARNESS.md step 8e.
- Integration: `ast_assertNode`. Type check: `diagnostic_types_in_order`, `diagnostic_descriptions_in_order`,
  or `observed_only` (direct `parseAndTypeCheckWithScope`: the test's own follow-up assertions are not captured).

## harness
- `base`: harness base class. `fields`: every instance field declared in the base class (reflective).
- `overrides`: `[{method: "name(ParamTypes)", declaredIn}]` harness methods the test class overrides.
- `fieldsAfterGetOptions` (CompilerTestCase): the base-class fields whose dump changed while the test's
  `getOptions()` ran (for example `enableTypeCheck()`, `enableCreateModuleMap()` called from inside
  `getOptions`), as field name → value after it. Dumped right after `createAndInitializeCompiler`
  (`testInternal`) or right after `getOptions()` (`testExternChanges`). `fields` is dumped at entry,
  before `getOptions()` runs.
- `effective`: effective values of overridable hooks: `getNumRepetitions`, `getCodingConvention` (class),
  `getName`, `createCompiler` (class of the compiler actually used). `getOptions()`'s effect is `options`;
  `getProcessor()`'s effect is `processor`.
- Type check builder: `diagnosticsAreErrors`, `reportUnknownTypes`, `suppress` (groups).

## Value encoding
JSON string/bool/int are themselves. Other values are tagged objects:
`{"long":"123"}`, `{"double":"1.5"}` (Java `Double.toString`), `{"char":"x"}`,
`{"enum":FQCN,"name":N}`, `{"diagnosticType":{key,level}}`, `{"diagnosticGroup":{group,types}}`
(`group` = registered name or `DiagnosticGroups.FIELD`, else null), `{"classRef":FQCN}`,
`{"regex":p,"flags":n}`, `{"sourceFile":SourceFile}`, `{"optional":v}`,
`{"list":[...],"impl":FQCN}`, `{"set":[...],"impl":FQCN}`, `{"map":[[k,v],...],"impl":FQCN}`,
`{"arrayOf":FQCN,"items":[...]}`, `{"composeWarningsGuard":{"guards":[...]}}` (guards in effective order;
each `DiagnosticGroupWarningsGuard` dumps as an object with `group` and `level`),
`{"ref":FQCN}` (Compiler/Node or a test instance, not dumped), `{"object":FQCN,"fields":{...}}` (generic, nesting-limited),
`{"unrepresentable":FQCN}` (lambda, synthetic or too deep; also listed in `unrepresentable`);
`{"unrepresentable":"cycle:FQCN"}` (an object already being dumped on the current path; listed
with reason `cycle`), `{"typedScope":{rootToken, depth, typeOfThis, vars}}` (a
`TypedScope`: `rootToken` = token name of the scope root, `depth` = scope depth, `typeOfThis` =
`JSType.toString()` of the scope's this type or null, `vars` = every own variable in the scope's
iteration order as `[name, JSType.toString() or null, isTypeInferred]`; type strings longer than
2,000 characters are cut to 2,000 and `...<truncated N chars>` is appended).
A `java.util.Optional` is `{"optional":v}` (null when empty). A guava
`com.google.common.base.Optional` has no dedicated tag: it is a generic object dump,
`{"object":"com.google.common.base.Present","fields":{"reference":v}}` or
`{"object":"com.google.common.base.Absent","fields":{}}`. `Float` values are encoded like
`Double` (`{"double":Double.toString((double) f)}`); `Short`/`Byte` are JSON numbers.
A lambda additionally carries `"captures"`: its captured values as a field dump (nested
unrepresentable values inside captures are not listed again).

### Neutral encodings (D-017 item 2)
Two library value kinds have a dedicated neutral tag instead of a generic `object` dump of their
private Java fields (which rule 1a of "Post-call state" demotes):
- **Guava `com.google.common.collect.Table`**: `{"table":[[row,column,value],...],"impl":FQCN}`, the
  cells in `cellSet()` order, each element in the same encoding (same depth budget as the table).
  `impl` is classified like the other containers (`scripts/unit_postcall_impls.py`:
  `HashBasedTable` UNORDERED, i.e. cells compared as a multiset of `[row, column, value]` keyed by
  `[row, column]`; `TreeBasedTable` ORDERED).
- **Protobuf message or builder** (`com.google.protobuf.MessageOrBuilder`):
  `{"proto":<descriptor full name>,"fields":{<field name>:value,...}}` with exactly the fields
  `getAllFields()` returns (the set fields; proto3 scalars only when non-default), in field-number
  order, by descriptor field name. Nested messages are encoded in full: **no depth limit** applies
  inside a message. Never present: `memoizedIsInitialized`, `memoizedSize`, `memoizedHashCode`,
  `unknownFields` or any other Java field. Values: repeated fields (map fields included, as their
  `<Name>Entry` messages with `key`/`value`) are JSON arrays in wire order; message -> this encoding;
  enum -> the value's name (string); `int32`/`uint32`/`sint32`/`fixed32` -> JSON number;
  64-bit integers -> `{"long":"<decimal>"}`; `float`/`double` -> `{"double":Double.toString}`;
  `bool` -> JSON bool; `string` -> JSON string; `bytes` -> `{"bytes":"<base64>"}`, with two exceptions:
  `jscomp.StringPoolProto.strings` -> the UTF-8 decoded JSON string, and `jscomp.LazyAst.script` ->
  `{"astNode":<the bytes parsed as jscomp.AstNode, in this encoding>}` (falls back to `bytes` if they
  do not parse). Used everywhere values are dumped (`testFields`, `testFieldsAfter`, the processor
  dump, `postCall`), on the recording side and in replay (same `UnitRecorder` code).
Inherited field names are prefixed with the declaring class's simple name (`Proxy.nextConvention`).
If that name is already taken in the same dump (two superclasses with the same simple name), the
declaring class's FQCN is used instead (`com.example.Proxy.nextConvention`). Static and synthetic
fields are never dumped. `harness.fields` also contains the recorder's own bookkeeping fields
`__currentRec` and `__recCompiler`; they are not harness state and replay skips them (as it skips
`lastCompiler` and `setUpRan`).

### Unrepresentable paths
`path` = section prefix, then segments: `.name` (a field, named as above), `[i]` (list, set or
array item; guard `i` in a `composeWarningsGuard` is `.guards[i]`), `{ki}` / `{vi}` (key / value
of the i-th map entry in iteration order), `.get` (the value inside an `optional`),
`.captures` (a lambda's captured fields). Section prefixes: `test` (testFields), `testAfter`
(testFieldsAfter), `harness` (harness.fields), `harnessAfterGetOptions`
(harness.fieldsAfterGetOptions), `processor` (processor.fields), `options`, and the `expected`
paths of the expected-value dumps (for example postcondition values). DSL paths (`when`,
`field`/`path`, `record`) are different: they are dotted JSON paths with numeric array indexes.

## Outcome contract
`outcome.status` is `normal` or `exception`. For an exception, `exceptionClass` is the Java class
name of the throwable that left the hooked call, `message` its `getMessage()` (null allowed),
cut to 4,000 characters with `...<truncated N chars>` appended when longer, and `assertion` is
true iff the throwable is a `java.lang.AssertionError` (a test-framework assertion failure,
including Truth's `AssertionErrorWithFacts`) rather than a compiler or harness exception.
Java replay compares status, class and (truncated) message exactly. A non-Java harness cannot
reproduce Java exception classes or messages that embed Java internals (Truth diffs,
`Node.toString()` dumps with hashed `Color{id=…}`, `StackOverflowError` with a null message,
which is JIT-depth-sensitive per D-010). For such a harness the contract is: status must match;
for `assertion: true` records the harness must report its own assertion failure at the same
check; `java.lang.StackOverflowError` records are depth-sensitive and reported separately
(D-010); the exact class and message are compared only where the Rust harness defines an
explicit mapping (gate 0.2 lists these exception records).

## Post-call state
Many tests assert on test-instance state after the hooked call (scopes, collected messages, id or
rename maps). That state is in `testFieldsAfter`. For each `compiler_test_case` record with a
non-empty `testFieldsAfter`, replay (`ReplayMain --post-out`) classifies it:
- **checked**: the selected descriptor case has a `testFieldsAfter` Expr (DSL.md) **and the
  compared field set is non-empty**. The holder's fields are dumped and must equal the expected
  post-call values: every key of `testFieldsAfter`, minus each field named in
  `testFieldsAfterSkip` (a map field → non-empty reason, for example a hash-ordered
  compiler-internal graph), plus each field named in `testFieldsAfterAlso` (its post-call value is
  its `testFieldsAfter` value if it changed, else its entry value from `testFields`). These keys
  are the **compared field set**; replay writes it, sorted, as `comparedFields` on the record's
  `--post-out` line, and gate (a) checks `testFieldsAfter.<f>` claims
  against it. An opt-in whose compared set is empty is not `checked`: the record is
  **unclassified** when its `testFieldsAfter` changed, and has no post-call state otherwise.
  Helper class names are mapped back to the recorded names through the descriptor's `classMap`
  before comparing.
- **notCaptured**: the case (or the descriptor top level) has `testFieldsAfterUnchecked`: a
  non-empty reason why the post-call assertions cannot be represented. Such records are never
  counted green in gate (a) and are counted in gate (e). `type_check` records with comparison
  `observed_only` are always notCaptured (the test's assertions on the returned object are not
  recorded). `integration` records with api `compile` are notCaptured with `integrationCompile:
  true` on the post line, and gate (a) resolves them: such a record is
  counted when the static post-call audit flags no assertion after the call in its method (replay
  already compares its outcome and every `observed` key: `errors`, `warnings`, `output`), or when its
  method's corpus/unit/postcall entry is `captured` and every claim holds for every record of the
  method (for example `observed.warnings`); otherwise it stays notCaptured.
- **unclassified**: neither. Gate (a) fails on any unclassified record.

Gate (a) also runs every `compiler_test_case` class with the whole processor replaced by a
no-op (`--noop-processor`). A class is **vacuous** when every counted record still passes (no
assertion-type failure: a Truth/JUnit `AssertionError`/`ComparisonFailure`, an `observed.*` or
`testFieldsAfter` mismatch, or an expected assertion failure that vanished; NPEs and other harness
exceptions do not count as caught). In a vacuous class a record stays counted in (a) only when replay
checks its post-call state (**checked**) or replays at least one postcondition; every other record
becomes **notCaptured** (excluded from (a), counted in (e)). No free-text reason can override this
(`vacuityAudit` is not read).

**No-op vacuity, harness-run passes.** Some classes have a recorded
processor that does nothing itself (a verbatim no-op `getProcessor`) and assert through a pass the
harness runs, for example `TypeCheck` under `enableTypeCheck`; for them the whole-processor no-op is
identical to the original and can never fail a record. Their descriptor may name that pass with the
top-level key `"vacuityNoopClass": "<FQCN>"` (or a list of FQCNs). Gate (a)'s `noop` phase then also
replays the class with the class-level NoopAgent for each named pass (HARNESS.md "Class-level no-op
mutation"). The class stays vacuous only if every counted record also passes under each such
mutation, where "fails" means: the record passes in (a) and fails under the mutation with an
assertion-type failure (the rule above). The key is mechanical, not an escape: each FQCN must be an
in-scope top-level `src/` class with a pass role (DSL.md), must not be the outer class of the
recorded processor's class, and the agent must report at least one rewritten entry point; any
violation fails gate (a). The key is read only when the whole-processor no-op catches nothing. Gate (a) additionally runs a static audit
of the pristine test sources (`gates/lib/unit_postassert_scan.py`): every record-bearing @Test
method with assertions after a hooked call must be checked (all its records are **checked**) or
classified in its descriptor's `postCallAssertions` list
(`[{"methods": [names] | "*", "status": "out_of_scope", "reason": "..."}]`; such records count in
(e) and are not green in (a)).

## Decoding values into live objects
Replay (`ReplayValues.decode(value, targetType)`) decodes every value against the **declared type of
the slot it is assigned to**: an option field (`options_defaults.json` `fieldTypes` lists them; all
numeric option fields are `int`), a harness or object field (its declared field type), or a DSL
argument (the parameter type of the resolved signature, `replay_signatures.tsv`). Collection items,
map keys and values decode against `Object`.
- JSON number → `int`, converted to `short`/`byte`/`long`/`double` when the slot has that primitive
  or boxed type. `{"double"}` → `double`, or `float` when the slot is `float`/`Float`. `{"long"}` →
  `long`. `{"char"}` → its first UTF-16 unit. Bool and string decode as themselves.
- `{"object":C,"fields":F}`: Guava `Present`/`Absent` → `Optional.of(decode(reference))`/`absent()`.
  A Java record class → its canonical constructor with each component decoded from `F`. Any other
  class → the no-argument constructor when one exists (any visibility), else an allocation **without**
  running a constructor; then each field named in `F` is assigned its decoded value (inherited fields
  by their prefixed names, see "Value encoding"); fields not in `F` keep what the constructor or the
  allocation left (zero/null). `C` is first mapped through the descriptor's `classMap`.
- `{"proto":N,"fields":F}` ("Neutral encodings"): the generated message class of
  full name `N` (the slot's type when it is a generated message, else `N` without its proto package,
  nested names joined by `$`, looked up in the Java packages of the reference `src/` .proto files and
  checked against the class's descriptor); its builder gets every field of `F` by descriptor name, the
  inverse of the encoding (`{"astNode"}` re-encoded to bytes, string-pool strings as UTF-8).
  `{"table":...}` does not decode (no recorded input holds one).
- `{"ref":...}`, `{"unrepresentable":...}` and `{"typedScope":...}` never decode: a record whose
  replay needs one is a harness error unless the descriptor supplies the value by an Expr
  (DSL.md `options.skip`/`then`, `withFields`).

## Processor identity
Replay (`ReplayMain --proc-out`, run by gate 0.2's replay phase) dumps the processor the descriptor
built for each `compiler_test_case` record exactly as the recorder dumped the original
(`UnitRecorder.fields`, depth 2, first repetition, right before it runs) and compares it with the
record's `processor` (`ProcessorIdentity.java`). Leaves are JSON scalars, null, `enum`, `long`,
`double` and `char` values; refs, lambdas and placeholders are not leaves, and protobuf
memo caches (`memoizedHashCode`, `memoizedSize`, `memoizedIsInitialized`) are skipped. A
`list`/`arrayOf` (items in order), `set` (sorted multiset of items) or `map` (sorted set of
key=value entries) whose items, keys and values are all such scalars is itself one leaf (`impl` is
ignored), so pass configuration held in string sets (StripCode's `stripNamePrefixes`, ExternExportsPass's
`exportSymbolFunctionNames`, ...) is compared; other containers are not leaves (object dumps inside
them are still indexed by class). Every `object`
dump in either tree (the processor included) is indexed by class (recorded names mapped through
`classMap`); objects of one class are matched as a multiset and compared on their common field paths,
so a pass wrapped by `PhaseOptimizer`, a helper `Processor` or a `Proxy` is compared wherever it sits.
Status per record: `equal`, `mismatch` (some compared leaf differs: gate (a) fails on any counted
record), `classDiffers` (top classes differ, no compared leaf differs; listed per class with the
count where nothing could be compared), `bothNull`, `noReplayedProcessor`, `dumpError`.
A counted record with status `classDiffers` and nothing compared is unverified unless
its descriptor carries a non-blank `processorIdentityNote` (top level or in a case) that justifies the
substitution (for example "the recorded processor is the test's anonymous wrapper $1 with no fields;
the replayed PeepholeOptimizationsPass is the pass its process() builds"). Unjustified ones fail (a);
the report lists every status count and the unjustified records per class. This leaf comparison is
an additional check: processor identity itself is verified by the pass trace ("passTrace",
D-017 item 8).

## Post-call state: neutral equality
Java replay compares the post-call dumps by exact JSON equality, which includes Java/Guava container
class names (`impl`) and hash-dependent iteration order. A non-Java harness compares them with this
**neutral equality** instead; `scripts/unit_postcall_impls.py` is its reference implementation
(`neutral_equal`) and `--check` fails if a record holds a container the table below does not classify.
1. **Schema.** The recorded dump *is* the schema: field names are the Java field names of the holder
   and of nested `object` dumps (after `classMap` mapping), and `object` class names are compared as
   strings. A non-Java harness emits a projection of its own state under exactly these names (per pass,
   the field set is the set of keys in the records' `testFieldsAfter`, plus `testFieldsAfterAlso`,
   minus `testFieldsAfterSkip`).
2. **`impl` is ignored** except to decide ordering. `list` and `arrayOf` items compare in order.
3. **`set` / `map`** compare in order when `impl` is ORDERED (insertion, sequence, sorted or enum
   order: `ArrayList`, `ArrayDeque`, `LinkedHashSet`, `LinkedHashMap`, `TreeMap`, `TreeSet`,
   `Collections$EmptyList`, `Collections$UnmodifiableRandomAccessList`, Guava `RegularImmutable*`,
   `SingletonImmutable*`, `ImmutableEnumSet`, `StandardTable$Row`, `StandardTable$RowMap`,
   `LinkedHashMultimap$ValueSet`, protobuf `LazyStringArrayList`, `ProtobufArrayList`), and as
   unordered collections when it is UNORDERED (`HashMap`, `HashSet`, `HashBiMap`, `HashBiMap$Inverse`,
   `AbstractMapBasedMultimap$WrappedSet`, `AbstractMapBasedMultimap$AsMap`): a set as a multiset of
   items, a map as a set of key/value pairs (keys unique). Items and keys compare by this same
   equality, recursively.
4. Scalars and every other tag (`long`, `double`, `enum`, `typedScope`, ...) compare exactly.
5. Run-to-run hash residue (see "Determinism") inside an UNORDERED container is absorbed by rule 3.
   Residue anywhere else is not absorbed by this rule: such a field must be classified
   (`testFieldsAfterSkip` or `testFieldsAfterUnchecked`), never compared loosely.

**Rule 1a: library-internal dumps are no capture.** Rule 1 makes the dump the
schema, but an `object` dump of a Java library class holds that library's private state (protobuf
memo caches such as `memoizedIsInitialized`, `unknownFields`, Guava `HashBasedTable` backing maps),
which a non-Java harness cannot reproduce from Closure semantics. Gate 0.2 therefore treats a record
whose replay post state is `checked` as **unclassified** when any compared value holds an `object`
dump whose class is in `com.google.protobuf.*` or `com.google.common.*`, or that is a protobuf message
(its fields include `memoizedIsInitialized`, `memoizedSize`, `memoizedHashCode` or `unknownFields`).
Such a record counts only when a `corpus/unit/postcall` entry classifies it; the fix is a neutral
encoding of the value in the recorder (for example the message's fields by descriptor name, the
table's cells), not looser comparison. Java replay still compares the dumps exactly.

## Replay
`scripts/unit_replay.sh` replays records through `oracle/replay` (ReplayCompilerTest,
ReplayIntegrationTest, ReplayTypeCheckTest). For the processor, see DSL.md.

- **Options:** rebuilt by decoding every diffed field onto `new CompilerOptions()`. Java records
  are rebuilt through their canonical constructor. `composeWarningsGuard` guards are re-added in
  reverse effective order. A descriptor case's `options` override (DSL.md) skips named fields and
  then runs its `then` expressions. A descriptor's `classMap` substitutes class names first.
- **Harness fields:** restored after `setUp()`, except `lastCompiler` and `setUpRan`. Each time the
  replay's `getOptions()` runs, `harness.fieldsAfterGetOptions` is applied to the instance after the
  options are built and before they are returned.
- **compiler_test_case APIs:** `testInternal` (with the case's `postconditions`, default none) or
  `testExternChanges(externs, sources, expected, diagnostics...)`. Observed errors and warnings come
  from `lastCompiler` for `testInternal`, and from the first compiler `createCompiler()` returned
  during the call for `testExternChanges` (the recorder observes that same compiler).
- **Pass rule:** a record passes when the outcome matches (status; for an exception also class and
  message) and every key of the recorded `observed` (except `compilerClass`) is present in the
  replay and JSON-equal. When the descriptor opts in to `testFieldsAfter`, that must match too.
- **Type-check builder records:** rebuilt with the private `TypeTestBuilder(Compiler)`
  constructor, on a Compiler initialised with the recorded options plus
  `markFeatureNotAllowed(MODULES)`.

## Postconditions count rule
For every `compiler_test_case` record, the number of postcondition Exprs in the selected
descriptor case must equal `expected.postconditions` (the number of postconditions the original
call ran). Replay checks this per record (`ReplayMain.postconditionCounts`): on a mismatch the
record's post-call state is `notCaptured` (reason `postconditions: the original call ran N, the
selected case replays M`), so it is excluded from gate (a) and counted in gate (e); it is never
green. Gate (a) re-checks every counted record from the replay's post lines
(`postconditionsRecorded`/`postconditionsReplayed`) and fails on any remaining mismatch.

## Referenceable values (D-015 e)
Test-instance fields (`testFields`, `testFieldsAfter`) and the `processor` dump are written in the
**referenceable** mode (`UnitRecorder.refFields` / `refValue`). It is the value encoding above with
one change: a value the dump cannot represent (a lambda or synthetic class, an object already being
dumped on the current path, an object nested deeper than the dump depth, a field reflection cannot
read) is written as a class-name reference `{"ref": FQCN}` and is **not** listed in
`unrepresentable`. A lambda keeps its `captures` (`{"ref": "Foo$$Lambda", "captures": {...}}`),
because descriptors rebuild helpers from them. Generic `object` dumps within the depth stay, since
replay decodes them (FORMAT.md "Decoding values into live objects") and descriptors read their
paths. `options`, `harness.fields`, `harness.fieldsAfterGetOptions` and the `expected` dumps are
**not** in this mode: the harness decodes every one of their values, so a value there that cannot be
encoded is still `unrepresentable` (and so are postcondition lambdas, whose code no record holds).
Replay dumps the replayed processor and the `testFieldsAfter` holder in the same mode, so both sides
compare like with like. A `ref` compares as its class name (after `classMap` mapping).

## Post-call snapshot (D-015 a)
Every `compiler_test_case` record has a top-level `postCall` object, taken right after the hooked
call returns or throws (in `__recordExit`, after `testFieldsAfter`), or `postCallError` (string) when
taking it threw. It is computed by `UnitRecorder.postCallSnapshot(compiler, processor, roots)`;
replay runs the **same** method on the replayed compiler, the first processor the descriptor built,
and the descriptor's helper objects (`once` cache and DSL variables), maps class names through the
descriptor's `classMap`, and requires the result to be equal to the recorded `postCall` (Java: JSON
equality; a non-Java harness: the neutral equality below). A mismatch fails the record
(`postCall differs`) and counts as an assertion-type failure for the no-op checks.

`postCall = {"compiler": {...}, "pass": {...}}`. A key is present only when its accessor returned a
non-null value (or `<key>Error` = exception class when it threw). Values are in the referenceable
encoding. `compiler` is read from the compiler the record's `observed` comes from:

| key | Java accessor | value |
|---|---|---|
| `externProperties` | `Compiler.getExternProperties()` | set of strings |
| `variableMap` / `propertyMap` / `stringMap` | `Compiler.getVariableMap()` / `getPropertyMap()` / `getStringMap()` → `VariableMap.getOriginalNameToNewNameMap()` | map string → string |
| `accessorSummary` | `Compiler.getAccessorSummary().getAccessors()` | map string → `PropertyAccessKind` enum |
| `moduleMetadataByPath` | `Compiler.getModuleMetadataMap().getModulesByPath()` | object path → `{moduleType, usesClosure, isTestOnly, googNamespaces, stronglyRequiredGoogNamespaces, dynamicallyRequiredGoogNamespaces, maybeRequiredGoogNamespaces, weaklyRequiredGoogNamespaces, es6ImportSpecifiers, readToggles, nestedModules}` (multisets as lists; `nestedModules` = the nested `goog.loadModule` modules' metadata objects of the same shape, recursively, in `nestedModules()` order; D-017 item 2) |
| `sourceMap` | `Compiler.getSourceMap().appendTo(sb, "out.js")` | string (source map V3 JSON) |
| `typeMismatches` | `Compiler.getTypeMismatches()` | list, in recording order, of `{"found": JSType.toString(), "required": JSType.toString()}`; present only when non-empty; no key (and no `Error` key) when the accessor throws because type checking has not run; any other throw gives `typeMismatchesError` = the exception class, as for every key (for example `java.lang.IllegalStateException` in records of classes whose compiler had type checking run and its type information then made unavailable) |
| `runJ2clPasses` | `Compiler.runJ2clPasses()` (set by `J2clSourceFileChecker`) | `true`; present only when true |
| `injectedLibraries` | `Compiler.getRuntimeJsLibManager().getInjectedLibraries()` (D-017 item 2) | list of strings in injection order; present only when non-empty |
| `allowableFeatures` | `Compiler.getAllowableFeatures()` (D-017 item 2) | list of `FeatureSet.Feature` constant names (`Feature.name()`) of the features the set has, in `Feature` enum declaration order; present when non-null |
| `packageJsonMainEntries` | `Compiler.getModuleLoader().getPackageJsonMainEntries()` (D-017 item 2) | map string -> string (the loader's map order); present only when non-empty |
| `typedPercent` | `Compiler.getErrorManager().getTypedPercent()` (D-017 item 2) | `{"double": Double.toString}`; present only when non-zero |
| `moduleMapBoundNames` | `Compiler.getModuleMap().getModulesByPath()` -> `Module.boundNames().keySet()` (D-017 item 2) | object module path -> list of bound local names in `boundNames()` order; present only when the module map is set and non-empty |

`pass` holds the results of **result producers**: pass objects reachable from the processor (its
instance fields to object depth 2, list and array items included) and then from the roots (the
test instance's own fields on the recording side, the helper objects on the replay side). The first
producer found per key wins (processor first, fields in declaration order).

| key | producer class | accessor |
|---|---|---|
| `pass.variableMap` | `RenameVars` | `getVariableMap()` (as a map) |
| `pass.propertyMap` | `RenameProperties` | `getPropertyMap()` (as a map) |
| `pass.stringMap` | `ReplaceStrings` | `getStringMap()` (as a map) |
| `pass.ambiguatedPropertyMap` | `disambiguate.AmbiguateProperties` | field `renamingMap` |
| `pass.idGeneratorMappings` | `ReplaceIdGenerators` | `getSerializedIdMappings()` |
| `pass.exportedVariableNames` | `GatherRawExports` | `getExportedVariableNames()` |
| `pass.globalRegExpPropertiesUsed` | `CheckRegExp` | `isGlobalRegExpPropertiesUsed()` |
| `pass.referenceMap` | `ReferenceCollector` | `getAllSymbols()` in collection order, each `{name, scopeRoot, isAssignedOnceInLifetime, isWellDefined, references: [{node, isDeclaration, isInitializingDeclaration, isLvalue}]}` from `getReferences(var)` |
| `pass.crossChunkReferences` | `CrossChunkReferenceCollector` | `{vars: [...], topLevelStatements: [...]}`: `getGlobalVariableNamesMap()` in order, each `{name, isAssignedOnceInLifetime, isWellDefined, references: [{node, basicBlockRoot}]}` (or `references: null` without a collection); `getTopLevelStatements()`, each `{originalOrder, statement, isDeclarationStatement, isMovableDeclaration, declaredNameReference, nonDeclarationReferences, declaredValueNode, declaredValueNumber?}`, where a reference is named `"<var>#<index in that var's references>"` (`"unlisted <node>"` if it is in no listed collection) |

Node positions in these two producers are strings `"<Token> <source file name>:<line>:<column>"`
(`Node.getToken()`, `getSourceFileName()`, `getLineno()`, `getCharno()`; null for a null node); the
number is `String.valueOf(Node.getDouble())`. Both are computed by `UnitRecorder.postCallSnapshot` on
the recording and the replay side alike.

The keys cover the state that the methods flagged by the static post-call audit assert on (the
assertion arguments after the first hooked call). Post-call targets outside this set are
`TypedScope`s (already in `testFieldsAfter` as `typedScope`), test-field collections (`cssNames`,
`noSideEffectCalls`, `lastCheckViolationMessages`, already in `testFieldsAfter`) and Java objects
of the output AST (`JSType`, `JSDocInfo`, `Color`, `Node` lookups), which are Rust-unit-test
material. The named-anonymous-function map has no producer at the reference release (no
`NameAnonymousFunctionsMapped`); define values are not exposed by `ProcessDefines` through an
accessor (ProcessDefinesTest asserts through `GlobalNamespace`, a Java-internal graph).

**Opt-in test fields.** The test-instance part of the snapshot is the existing opt-in mechanism: a
descriptor case names the holder with `testFieldsAfter` and may add `testFieldsAfterAlso` /
`testFieldsAfterSkip` (see "Post-call state"); values are compared in the referenceable encoding,
with the neutral equality.

**What counts as captured.** A record's post-call assertions are *captured* when replay compares
the asserted state: its `postCall` keys, or its `testFieldsAfter` fields under a descriptor opt-in,
or the postconditions the case replays (count rule). Which one covers a given test method is
declared per class in `corpus/unit/postcall/<Class>.json` (next section) and verified by gate (a).

## corpus/unit/postcall/<Class>.json (post-call classification, D-015 a)
One file per record-bearing test class that has methods flagged by the static post-call audit
(`gates/lib/unit_postassert_scan.py`), maintained by hand. `<Class>` is the record
file stem (`corpus/unit/records/<Class>.jsonl.gz`).

```json
{
  "v": 1,
  "class": "com.google.javascript.jscomp.RenameVarsTest",
  "methods": [
    {"method": "testFoo", "status": "captured",
     "via": ["postCall.pass.variableMap", "testFieldsAfter.renameVars", "postconditions"],
     "note": "optional free text"},
    {"method": "testBar", "status": "rust_unit_test",
     "reason": "asserts on JSType objects of the output AST",
     "fragment": "corpus/unit/rust_unit_tests/RenameVarsTest.md"}
  ]
}
```
- `methods` lists every flagged method of the class exactly once (the JUnit method name without a
  parameter suffix). There is no mechanical classification: a flagged method whose records all have
  post state `checked` is not `captured` without an entry, because that would never check that a
  compared field holds the asserted state (TypedScopeCreatorTest methods that assert JSType
  properties the typedScope dumps do not record, GatherExternPropertiesTest's input field `mode`).
  Every flagged method needs an explicit entry.
  An entry may also name a method that the scan did not flag but that has records with replay post
  state `unclassified` (testFieldsAfter changed, not compared); the entry then classifies those
  records the same way. Any other entry, a duplicate entry, or a file without a record file of the
  same stem fails the gate.
- `class` is the record class FQCN (`class` field of the records). When one record file holds
  records of several classes (nested test classes), an entry may override it with `recordClass`.
- Descriptor `postCallAssertions` entries do not classify anything; the gate lists them so they
  can be moved here.
- `status` is `captured`, `rust_unit_test` or `unclassified`; nothing else is
  accepted. An explicit `unclassified` entry needs a non-empty `pending` text saying what would capture
  the method (for example a D-017 item 2 neutral key the recorder does not produce yet); the method
  counts as unclassified in (a) exactly like a method without an entry. The records of every
  unclassified method (explicit entry or none, or a claim that does not hold), and records with
  unclassified post state, are not captured, so they also count toward the 3% exclusion cap and (e)
  (D-015 "records that are not captured count toward a 3% exclusion cap"). Methods moved from `rust_unit_test` keep the old text as
  `formerRustUnitTestReason`/`formerCategory` and leave `corpus/unit/rust_unit_tests/`.
- `captured`: `via` is a non-empty list of `postCall.<compiler|pass|postcondition>.<key>` (the
  `postcondition` section, "Postcondition data", is part of the `postCall` object replay compares by
  JSON equality), `testFieldsAfter.<field>`,
  `postconditions`, `outcome`, or, for `integration` records only, `observed.errors`,
  `observed.warnings` or `observed.output`. For `pass` keys the `pass.` prefix of the snapshot key may be written or left
  out (`postCall.pass.variableMap` and `postCall.pass.pass.variableMap` both name
  `postCall.pass["pass.variableMap"]`). Gate (a) checks, for **every** record of the method: the record passes in (a);
  each `postCall.*` entry is present in the record's `postCall` (replay compared it); each
  `testFieldsAfter.*` entry is compared by the selected case (post state `checked` and the field in
  the record's `comparedFields`, which includes `testFieldsAfterAlso` fields that did not change),
  and the field changed during the hooked call (is a key of `testFieldsAfter`) in
  at least one record of the method: a field that only carries the entry value the test set itself
  is input, not post-call state;
  `postconditions` needs `expected.postconditions > 0` and the count rule to hold; `outcome` needs
  an outcome with status `exception`, `assertion` false and a non-empty message (replay compares
  status, class and message exactly; for an asserted compiler exception such as `assertThrows(...)
  .hasMessageThat()`); each `observed.*`
  entry needs an `integration` record whose `observed` holds that key (replay's pass rule compares
  every recorded `observed` key JSON-equal, and the record must pass). A verified `captured` entry
  also resolves the method's `integration` api `compile` records (see "Post-call state").
  A claim the records do not support fails the gate.
- `rust_unit_test`: `reason` (non-empty) and `fragment`, a file under `corpus/unit/rust_unit_tests/`
  that names the method on its own line as `- <Class>#<method>: <what it checks>`. **Every** record of
  the method is **excluded** from (a) and counts toward the 3% exclusion cap (D-015: "records that are
  not captured count toward a 3% exclusion cap"). A compared `testFieldsAfter`
  field (post state `checked`) does not exempt a record, because the method is a Rust unit test
  precisely because no snapshot holds the asserted state; only a verified `captured` entry captures.
  Per D-015 a ("the assertion inspects Java-internal objects"): the entry also needs
  `category`, one of the closed list `jstype` (JSType/FunctionType/ObjectType objects, the type
  registry), `jsdocinfo` (JSDocInfo/comment objects), `color` (Color/ColorRegistry objects),
  `node-identity` (Node object identity, Node props/flags, source positions of Node objects), `graph`
  (scope/var/reference/module-metadata/call-graph object graphs), `exception` (the Java exception
  thrown by the hooked call) or `test-local-object` (objects built by test-local Java code). The gate
  rejects an entry whose `reason` contains `not java-internal`, `not a java-internal`, `interim`,
  `provisional`, `capturable`, `until then`, `harness request`, `deferred`,
  `in fact replayed` or `until the gate accepts` (case-insensitive); whose category is `graph` or
  `exception` (or missing) and whose `reason` names a value replay compares (`observed.` or
  `outcome`: the category then contradicts the reason; use `captured` with that via); or that lists `postCallKeysChecked`. Cross-check: when
  the method's records carry a pass-level snapshot key (`postCall.pass.*`, for example
  `referenceMap`, `crossChunkReferences`, `variableMap`) or a compiler-level producer of the entry's
  category (`moduleMetadataByPath` for `graph`; `typeMismatches`/`typeMismatchesError` for
  `jstype`), the `reason` must name each such key, saying why it does not hold the asserted state;
  otherwise the method is unclassified. Such a method is either
  captured (use `captured` with the key) or stays unclassified until a snapshot key exists. Neutral
  values (strings, numbers, sets/maps of them, diagnostics, output code; for example injected
  libraries, allowable features, package.json main entries, typed percent, ModuleMap bound names)
  are never a Rust unit test.

## Change-expecting records (gate 0.2(d), D-015, D-017 item 10)
A record is **change-expecting** (D-017 item 10, gate 0.2 (d)) when it expects:
- **an output AST different from its input.** `Gate02AstEquiv` (gates/lib/unit_gate02.py `EQUIV_JAVA`)
  parses the record's input sources (or chunk inputs) and the expected output file list in the
  record's language mode (`options.languageIn`; a missing `languageIn` means the CompilerOptions
  default, since the options dump holds only non-default fields), each file with the parser only
  (`CompilerInput.getAstRoot`, no module loading), and compares the scripts file by file with
  `Node.isEquivalentTo`, without normalization. When the record's `comparison.compareJsDoc` is set,
  the comparison includes JSDoc (`JsDocComparison.COMPARE`), as the harness
  comparison does (docs/PORTING.md §4.2: "including JSDoc when compareJsDoc is set"). A difference in text
  only counts as no-change; a record whose comparison could not be made (parse error) counts as
  change-expecting. `output` = `"SAME"` or null means no change.
- **a diagnostic** (`expected.diagnostics` non-empty; for `type_check` records, expected diagnostic
  types or descriptions); or
- **a postcondition whose expected value differs from the value observed on the parsed, unprocessed
  input** (`Gate02PostcondProbe` replays the record with the whole processor a no-op and compares
  `postCall.postcondition`; a record whose comparison could not be made counts as change-expecting).

`integration` records are not attributed to a pass.

## Limits of the classification
- **Post-call-only records are not change-expecting.** As gate 0.2 (d) / D-015 define it, a record
  whose only expectation is post-call state does not count as change-expecting: its compared state
  may equal its entry state, which passes under any no-op.
- **No-change-only classes** (Es6RewriteScriptsToModulesTest) cannot show that their descriptor runs
  the processor; the vacuity rule is mechanical, so such records stay notCaptured.
- **Postcondition lambdas** are unrepresentable, because no record holds their code, even where a
  verbatim helper rebuilds them; gate 0.2 (e) counts records with `unrepresentable` entries.
- **Empty-`getProcessor` classes** (CheckTemplateParamsTest) stay vacuous: their warnings come from
  JSDoc parsing, which is not a pass the agent can mutate (`vacuityNoopClass` `TypeCheck` catches
  none of their records), so their records stay notCaptured.
- **Vacuity and `postCall` keys:** a `postCall` difference is already an assertion-type failure in
  the no-op runs, so a producer key that a no-op changes makes the class non-vacuous.
- **Not captured by the recorder:** `compiler.typeJSDoc`, typed-scope property flags, a `typedScope`
  snapshot for `parseAndTypeCheckWithScope`, and a post-call snapshot for `integration` records.

## passTrace (D-017 item 8)
Every record written by a recording JVM that runs the NoopAgent in TRACE mode
(`-javaagent:noop-agent.jar=trace=<scope file>`, see RECORDING.md "Pass trace") has a top-level
`passTrace`: the sorted list of FQCNs of the in-scope `src/` top-level classes (scope file =
`scripts/unit_trace_scope.py`, the docs/PORTING.md §2 predicate `in_scope_src` over the pristine `src/`)
whose pass entry points executed between the outermost hooked entry and its exit (the post-call
snapshot included). Entry points are the no-op rewrite's set (HARNESS.md "Class-level no-op
mutation"): `process(Node,Node)`, `process(Node,Node,ReferenceMap)`, `hotSwapScript`,
`optimizeSubtree`/`transpileSubtree`, and the NodeTraversal callback methods `visit`,
`shouldTraverse`, `enterScope`/`exitScope`/`enterScopeWithCfg`/`exitScopeWithCfg`,
`enterChangedScopeRoot`; declared, concrete, non-static, non-bridge methods; a method of a nested
class is attributed to its top-level class. The trace is global to the JVM (passes may run on the
compiler thread). A record without `passTrace` is unverified: gate (a) counts it as excluded under
the 3% cap. Replay (ReplayMain under the same agent mode) compares the replayed set with
`passTrace` by JSON equality; a difference fails the record (`passTrace differs`), and
`--trace-out` writes one status line per record (`equal`, `mismatch`, `unverified`).
`processorIdentityNote` is not evidence of processor identity.

## Postcondition data (D-017 item 1)
For a `compiler_test_case` record whose call ran at least one postcondition
(`expected.postconditions > 0`), `postCall.postcondition` holds the state that postcondition
lambdas assert, read from the compiler the outermost call's postconditions verify (the harness's
local `compiler` at the postcondition loop, i.e. after a multistage serialize/deserialize; a nested
hooked call inside a lambda does not replace it; when no postcondition ran, the test's
`lastCompiler`):
- `sideEffectFlags`: `[node, callee qualified name or null, Node.getSideEffectFlags()]` for every
  CALL, NEW, TAGGED_TEMPLATELIT and OPTCHAIN_CALL node of externs and JS, pre-order
  (PureFunctionIdentifierTest);
- `jsdocTypes`: `[node, [toStringTree of each JSDoc type node]]` for every JS-root node whose
  JSDoc has type nodes (ScopedAliasesTest's TypeVerifyingPass);
- `externExport`: `compiler.getResult().externExport` (ExternExportsPassTest);
- `externProperties`: the sorted `compiler.getExternProperties()` (GatherExternPropertiesTest).
`node` is `TOKEN file:line:col`. Replay recomputes the object with the same recorder code
(`UnitRecorder.postCallSnapshot(..., postconditionCompiler)`) on the replayed compiler, and the
postCall equality covers it.

The lambdas of `UnitRecorder.POSTCONDITION_DATA_CLASSES` (PureFunctionIdentifierTest,
ExternExportsPassTest, GatherExternPropertiesTest, ScopedAliasesTest) are not listed in the
record-level `unrepresentable`; their indices are listed in `expected.postconditionsCaptured`, and
their `expected.postconditionValues[i]` (class name plus captures, which descriptors decode) is
kept as recorded. Gate (e) does not count the lambda placeholders inside
`expected.postconditionValues[i]` for `i` in `postconditionsCaptured` when
`postCall.postcondition` is present (`captured_postcondition_indices`). Other postcondition
lambdas (PolymerPassTest) stay unrepresentable.

## postCall key-set equality (D-017 item 3)
`ReplayMain.compare` compares `rec.postCall` with the replayed `postCall` by Gson
`JsonElement.equals`, which for `JsonObject` is map equality (same key set, equal values),
recursively. A key present on one side only fails the record. So replay compares postCall key
sets exactly, and a `postCallAbsent.<...>` capture path is valid under D-017 item 3.
In `corpus/unit/postcall/<Class>.json` a `captured` entry may name
`postCallAbsent.<compiler|pass>.<key>` in `via`: the asserted state is that the snapshot has no
such key. Gate (a) accepts it for a record only when the record has a `postCall` snapshot whose
section lacks the key (`_postcall_absent_key`) and the record passes in (a).

## Postcondition-lambda assertions are in-call (D-017 item 7)
`gates/lib/unit_postassert_scan.py` does not take an assertion for a post-call assertion when it
lies inside the argument list of a hooked call (or hooked helper call), after a `->` that starts
in that argument list, in a method whose recorded calls ran postconditions
(`expected.postconditions > 0`) or in a non-@Test helper of a class that has such methods. Such an
assertion runs inside the call as (or from) a Postcondition; the postcondition-count rule and
gate (e) govern it. The rule takes methods of ExternExportsPassTest, PureFunctionIdentifierTest
and PolymerPassTest out of post-call classification. In-call lambdas that are not postconditions (ReferenceCollectorTest's
Behavior callbacks, no postconditions recorded) stay under post-call classification.
