# closure-testing

Test-support crate (package `closure-testing`). It preserves the existing typed corpus reader and
executes the JSON replay through Rust ports of `CompilerTestCase`, `IntegrationTestCase`,
`CompilerTypeTestCase` and the `TypeCheckTestCase` builder/direct paths. It is never linked into the
compiler. Compiler-owned operations use `jscomp_api.rs`; working adapters stay in
`harness_passes.rs`, while missing implementations live in explicit `stand_in/` files re-exported
through `jscomp_api.rs`. Stand-ins carry no port markers and preserve the first missing class name.

## What it reads

| File | Spec | Rust entry point |
|---|---|---|
| `records/<TestClass>.jsonl.gz`, one record per line | FORMAT.md | `corpus::record_files`, `corpus::records` (streamed, one line in memory at a time) or `corpus::load_records` -> `LoadedRecord`s (`record::Record`, the raw JSON, the 0-based line `index`) |
| `descriptors/<TestClass>.json` | DSL.md "Descriptor file", "Case keys", "Expressions" | `corpus::load_descriptor_for(stem)` -> `descriptor::Descriptor` (cases, `dsl::Expr` trees) |
| `options_defaults.json` | FORMAT.md "Options defaults and warnings-guard order" | `corpus::load_options_defaults` -> `derived::OptionsDefaults` |
| `derived/expected_pipeline.jsonl.gz` | FORMAT.md "comparison", HARNESS.md step 8e | `corpus::load_expected_pipeline` -> `Vec<derived::ExpectedPipeline>` |

The corpus directory is `../../corpus/unit` relative to this crate, or `$CLOSURE_RS_CORPUS_UNIT`.

### Fidelity rules the reader keeps

- **Strings are UTF-16.** `json::JsString` holds UTF-16 code units, so the lone surrogates FORMAT.md
  allows (9 record lines have them) survive reading and writing. `serde_json` rejects them, so the
  crate has its own small JSON reader and writer (`json.rs`).
- **Nothing is dropped.** Every typed section is read with `reader::Obj`, which fails on a key it
  does not know. A key FORMAT.md does not document is a load error, not a silent loss. Value tags
  (`value::Value`) follow FORMAT.md "Value encoding", including the neutral encodings, protobuf
  messages, Guava tables, `typedScope` and `captures`. An unknown or ambiguous tag is an error.
- **Round trip.** `Record::to_json(Record::from_json(x)) == x` holds for all 24,782 records (JSON
  equality, so key order does not matter). The same holds for all 255 descriptors, for
  `options_defaults.json` and for every `expected_pipeline` line. A harness can therefore trust
  that the typed model carries the whole record.
- **Descriptor notes.** Keys that DSL.md calls notes for humans (`note`, `unrepresentableAudit`,
  `vacuityStatus`, ... and case-level `note`) are kept verbatim in `notes`, because replay ignores
  them. Every key replay or gate 0.2 reads is typed: `source`, `classMap`, `cases`,
  `unrepresentable`, `testFieldsAfterUnchecked`, `postCallAssertions`, `vacuityNoopClass`, and the
  case keys `when`, `processor`, `compilerSetup`, `options`, `postconditions`, `testFieldsAfter`,
  `testFieldsAfterAlso`, `testFieldsAfterSkip`, `testFieldsAfterUnchecked`.
- **Optional post-call data.** `Record::post_call` (FORMAT.md "Post-call snapshot"),
  `Record::pass_trace` (FORMAT.md "passTrace"), `Record::test_fields_after` (FORMAT.md "Post-call
  state") and the matching `*_error` fields are `Option`s, present only on the records that carry
  them. Every `postCall` key is typed (`post_call::PostCallValue`): referenceable values as
  `value::Value`, and the untagged shapes FORMAT.md documents (`moduleMetadataByPath`,
  `typeMismatches`, `pass.referenceMap`, `pass.crossChunkReferences`, `sideEffectFlags`,
  `jsdocTypes`, ...) as structs. A key outside the FORMAT.md tables is a load error.

## How the replay harness consumes it

Per record file (one Java test class):

1. `records(path)` (a record at a time) and `load_descriptor_for(stem)`. Every non-empty record file has exactly one
   descriptor; `tests/descriptors_load.rs` checks this.
2. **Case selection** (DSL.md "Case selection"): `descriptor::select_case(Some(&d), &lr.raw)` for
   `compiler_test_case` records, where no match is a harness error, and `select_case_or_null` for
   `integration` and `type_check` records, which are replayed without customisation when nothing
   matches. These are ports of `ReplayDsl#selectCase` / `#selectCaseOrNull` / `#matches` /
   `#path`, run on the raw record JSON. They keep Java's `String.split("\\.")` (trailing empty
   segments dropped) and Gson 2.9.1 `JsonElement.equals` (parsed numbers compare as doubles). All
   21,308 `compiler_test_case` records select a case (`tests/cross_checks.rs`).
3. **Options**: start from the real `CompilerOptions::new()` (its defaults are checked against every field
   captured in `options_defaults.json`), replace each key present in `Record::options` (an
   `@class` names a subclass), apply the case's `options.skip` / `options.then`
   (DSL.md "Case keys"), then `harness.fieldsAfterGetOptions` every time `getOptions()` runs
   (FORMAT.md "harness", HARNESS.md "Harness field index"). The Rust `CompilerOptions` defaults are
   checked field by field against `options_defaults.json`.
4. **Run** the API the record names (`record::Api`): `testInternal` / `testExternChanges` for
   `compiler_test_case` (HARNESS.md "`testInternal`", "`testExternChanges`"), the
   `IntegrationTestCase` entry points for `integration`, and `TypeTestBuilder.run` /
   `parseAndTypeCheckWithScope` for `type_check` (HARNESS.md "IntegrationTestCase pipelines",
   "TypeCheckTestCase pipelines"). Inputs come from `Record::inputs`, the processor from the
   selected case's `processor` `Expr` (evaluated once per repetition), and compiler setup from
   `compilerSetup`. The harness implements Expr evaluation per DSL.md "Evaluation order" and
   "Expressions", and resolves names through `Descriptor::map_class`.
5. **Expected side**: for `compiler_test_case`, look up `ExpectedPipeline` by (record file stem,
   line index). Its flags, not the recorded `comparison.normalizeExpected` /
   `closurePassForExpected`, decide how the expected output is processed (FORMAT.md "comparison"
   warning, HARNESS.md step 8e). `tests/cross_checks.rs` checks one entry per `compiler_test_case`
   record, with matching class, method and call.
6. **Compare** with `Record::comparison` (`ast` / `string` / `ast_assertNode` / type-check modes,
   HARNESS.md "Comparison modes") against `Record::expected`, and compare observed diagnostics and
   the outcome with `Record::observed` / `Record::outcome` (FORMAT.md "Outcome contract", HARNESS.md
   "Diagnostic matching").
7. **Post-call checks**, only when the record has them: `post_call` (HARNESS.md "Post-call snapshot
   step", FORMAT.md "Post-call state: neutral equality", "postCall key-set equality"),
   `test_fields_after` through the case's `testFieldsAfter` / `testFieldsAfterAlso` /
   `testFieldsAfterSkip` (DSL.md "Case keys"), and postconditions through the case's `postconditions`
   Exprs and
   `expected.postconditionValues` (FORMAT.md "Postcondition data", "Postconditions count rule").
   Descriptor `unrepresentable` membership and the post-state classification are report metadata;
   they do not change the pass/fail/unported outcome. `pass_trace` is retained for gate tooling;
   native replay does not run Java mutation/no-op modes.

Not modelled here: `corpus/unit/postcall/<Class>.json` (the gate's post-call classification) and
the replay helpers in `oracle/replay/helpers`, which are Java code that Rust ports with the classes
that need them.

## Tests

`CARGO_BUILD_JOBS=2 cargo test -p closure-testing` (the crate and its gzip decoder are built with
`opt-level = 3` in the dev profile; the structural runner test also replays the full corpus):

- `tests/records_load.rs`: all 24,782 records in 432 files (255 non-empty) load with 0 errors and
  round-trip; the counts per kind and per API are checked; `call` numbering per method is checked;
  the per-class table must equal `tests/data/record_counts.tsv`, which
  `tests/data/gen_record_counts.py` writes independently with Python's `json` module.
- `tests/descriptors_load.rs`: all 255 descriptors (374 cases) load and round-trip, and their stems
  equal the non-empty record files; `options_defaults.json` (205 fields) and `expected_pipeline`
  (21,308 lines) load and round-trip.
- `tests/cross_checks.rs`: case selection for every record; descriptor `unrepresentable` entries
  name existing records; `expected_pipeline` is keyed one-to-one onto the `compiler_test_case`
  records; the 9 lone-surrogate lines keep their surrogates through the typed model and the writer.
- Unit tests in `json.rs`, `value.rs`, `dsl.rs` and `descriptor.rs`.

Regenerate the counts table after a corpus change with
`python3 crates/testing/tests/data/gen_record_counts.py`.

## Replay runner

From the checkout (`gates/unit_rust.sh` runs it this way and compares the result with
`gates/unit_ratchet.json`):

```sh
CARGO_BUILD_JOBS=2 cargo run --release -p closure-testing --bin unit_replay -- --all --report build/unit-replay/report.json --records-out build/unit-replay/records.jsonl
```

The binary accepts `--all` or `--classes A,B,...`, plus `--report FILE.json`,
`--records-out FILE.jsonl` and `--corpus DIR`. Omitting class selection means all classes. It prints
one line for every selected record file, including empty files:

```text
REPLAY <Class> records=N pass=P fail=F unported=U
REPLAY_TOTAL records=N pass=P fail=F unported=U
```

It exits 1 when any record fails. Loader or CLI errors also exit 1. The report schema is
`{"v":1,"totals":{"records":N,"pass":P,"fail":F,"unported":U},"classes":{"Class":{"records":N,"pass":P,"fail":F,"unported":U,"unportedBy":{"item":N}}}}`.
Each record line contains `class,index,method,call,kind,api,status,why,unportedBy,postState,unrepresentable`.
`unrepresentable` holds its descriptor reason or null. `postState` holds ReplayMain's classification
and postcondition counts, or null.

A **pass** reproduces the recorded outcome, every observed key except `compilerClass`, the
post-call snapshot under FORMAT.md neutral equality, and opted-in test fields. Assertions need
an assertion outcome; explicit Java exception mappings compare class and truncated message.
A caught Rust panic reproduces a recorded non-assertion exception only for
`java.lang.IllegalStateException`, `java.lang.IllegalArgumentException`,
`java.lang.NullPointerException` or `java.lang.ClassCastException`, and only when its message
matches after the recorder's exact 4,000 UTF-16-unit truncation (including its suffix).
Diagnostics and post-state must still match. Typed Throwables still require the same Java class;
assertion outcomes and every other panic remain failures. Non-string panic payloads have no
message to compare and always fail.
A **fail** is any other mismatch or a harness error. An **unported** record reports the first missing
class or resolved signature. It never substitutes expected data for execution.

The harness uses the native `closure_jscomp::Compiler`, `CompilerOptions`, chunks, diagnostics,
change tracker, `ChangeVerifier`, `PassFactory`, `PhaseOptimizer` and `CodePrinter`. Expected trees
retain their second compiler; `NodeSubject` compares both arenas through the shared Rhino
cross-arena equivalence implementation. Test externs come from the ported `TestExternsBuilder`,
whose constants and all builder sections are checked against the pinned Java helper.

The stable record ID is the pair `(class,index)`, where `index` is the zero-based line in that
class's `.jsonl.gz` record file. A human-readable spelling is `Class[index]`. `method` and `call`
are supplementary source identifiers, not replacements for the stable pair. Report version 1
and its fields are unchanged.

Missing native factories are preserved through the compiler's panic wrappers and reported as
unported. Rust panics follow the precondition rule above. The replay harness never synthesizes a
skipped pipeline.

### Registering a pass

`replay/registry.rs` resolves the exact `(descriptor stem, mapped lookup, arity)` triple through
`corpus/unit/replay_signatures.tsv`; it does not infer overloads. A missing TSV row is a harness
error. A resolved row with no implementation is unported. Register the exact
`declaringClass#signature` with `Registry::register` in the registration setup.
`replay/native_registry.rs` contains the merged compiler and Rhino registrations. An entry has type
`fn(&mut Ctx, Vec<DslValue>) -> Result<DslValue, Throwable>`; instance calls receive the receiver
before the adapted arguments. For example, the AliasStrings constructor key is:

```text
com.google.javascript.jscomp.AliasStrings#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.JSChunkGraph,boolean,com.google.javascript.jscomp.CompilerOptions$AliasStringsMode)
```

A constructor can return `DslValue::Native(Rc<RefCell<dyn NativeObject>>)`, retaining the real
Rust pass inside its adapter. Implement `class_name`, `process` (or `process_with_ctx` for callback factories), field access and the result getters
the recorder uses. Implement `is_instance` for Java superclass/interface relationships when needed.
Register instance methods against their own resolved TSV signatures;
`as_any_mut` lets an entry reach the concrete Rust receiver. `NativeObject::call` supplies the
recorder's fixed getter calls, while `fields` supplies referenceable dumps and reachable producer
roots. A plain `DslValue::Pass` works for a pass with no instance-method or post-call state needs.
Keep adapter state shared: descriptor `once`, lambdas and holders depend on object identity.

If the harness itself runs the class, replace that class's stand-in with a real adapter in `harness_passes.rs` calling its
constructor/process method at the existing call site. These seams cover module setup and rewriting,
validation, type checking/inference, normalization, runtime libraries, colors/TypedASTs, accessor
collection, change checking, pretty-printing and IntegrationTestCase's DefaultPassConfig pipeline.
The real Google/Closure coding conventions and `TestExternsBuilder` are connected, and the
compiler's real `JSTypeRegistry` is loaned with its `Ast` (`Compiler::get_type_registry_and_ast`).

A real Rust constructor that needs the currently executing compiler can also register a
`BorrowedEntry` with `Registry::register_with_compiler`. This uses the existing mutable compiler
loan when a real pass factory invokes a DSL callback. It avoids reborrowing the compiler's
`RefCell`. Node/IR constructors, compiler getters, visitor adapters and iterable callbacks use
this path. `NativeObject::as_traversal_callback`, `as_node_visitor` and `as_custom_pass` expose
actual native implementations to the corresponding compiler APIs. Missing adapters report their
concrete Java helper class. Native factories preserve descriptor `once` state and lexical captures.

`oracle/replay/helpers/**`, `oracle/replay/agent/**` and `ProcessorIdentity.java` are not ported as
such: the helpers' Rust counterparts live next to the replay adapters of their passes, and the
Java agent and identity code belong to the Java gate tooling.
Rust replay does not run mutation/no-op modes.

### Stand-ins and compiler integration

| File under `src/stand_in/` | Why | Missing implementation |
|---|---|---|
| `polymer_pass.rs` | out of scope (docs/PORTING.md §2) | PolymerPass: the harness pass reports the Java class as unported. |

VariableMap and SourceMapInput option values are real (`replay/options_values.rs`): VariableMap
through its constructor and `toMap` (raw insertion order), SourceMapInput through the additive
`replay_fields`/`from_replay_fields` access in `crates/jscomp/src/source_map_input.rs`. A captured
non-null `parsedSourceMap` reports `SourceMapConsumerV3#fields` unported (no corpus record has one).
A captured `messageBundle` option decodes `EmptyMessageBundle` (`replay/options_values.rs`). The
ReplaceMessagesTest helper (`TEST_ID_GENERATOR`, `SimpleMessageBundle` and its holder), the
`JsMessage` value decoding and the `ReplaceMessages` constructor and `get*Pass()` entries are in
`replay/replace_messages_helpers.rs`.

`mod.rs` only declares modules. A stand-in is replaced in place when its implementation is ported;
an Unported-only body is never marked as a port. Replay assigns all 205 fields on the real
CompilerOptions without invoking setters, and constructor defaults are checked before assignment.
NodeSubject's equality, mismatch and serializer code is ported; its other Truth convenience
subjects are outside the replay comparison path and are not ported.

### Native harness tests

`tests/replay_harness.rs` checks diagnostics on real JSErrors, duplicate pairing/order, Java hash
replacement, string normalization, all option defaults, warning-guard order, value decoding,
DSL evaluation and TSV resolution, recorder dumps, outcome comparison and neutral post-state
comparison. `tests/unit_replay_runner.rs` classifies every one of the 24,782 records exactly once,
checks accounting and zero harness errors, and round-trips the report; it makes no assertion about
pass/fail counts. Rhino NodeSubject tests exercise semantic properties, subclass payloads,
declared types, shadows, JSDoc flags, serializer cleanup and failure prefixes.

`tests/native_compiler_harness.rs` exercises native initialization, cross-arena expected parsing,
real diagnostic/source-location matching, extern processors and string/tree comparison,
integration compile and missing-factory propagation, type-builder parsing up to module setup,
Node/IR signature registration, and real phase factories that share the compiler loan and replay
state, and a complete successful `testInternal` (real AstValidator, SourceInfoCheck,
GatherGetterAndSetterProperties and the fixture processor). Records that need an out-of-scope class
(docs/PORTING.md §2: PolymerPass, ChromePass, the J2CL passes, CoverageInstrumentationPass, ...)
report `unported`. No test is disabled.

