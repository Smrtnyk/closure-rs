# Decisions

> Commit IDs of this repository cited below refer to its private development history, which was
> squashed into a single commit for publication. Pins of other repositories (Closure Compiler
> `bb8c8e7`, Closure Library, test262) are their public commits.

Each entry records an interpretation of, or deviation from, [`docs/PORTING.md`](docs/PORTING.md),
together with its evidence; § references are to that document. Entry IDs are stable, so the
numbering has gaps where entries about the private development process were left out. A
superseded entry is marked as such.

## D-002 — Holdout location (2026-10-06)
The holdout (§4.5) is stored outside the repository, in a location known only to the
maintainers. The repository holds only its case count and a manifest hash
(`corpus/d2/HOLDOUT.md`).

## D-003 — Project-local toolchains (2026-10-06)
The host has a Java 25 JRE but no JDK, no Bazel and no protoc. The project therefore installs
bazelisk and a Temurin JDK 21 (the reference's `java_language_version=21`) under `tools/`,
which is gitignored. Nothing is installed system-wide.

## D-005 — Toolchain pins (2026-10-06)
- bazelisk v1.29.0, sha256 `5a408715e932c0250d28bd84555f12edbf70117de42f9181691c736eacc4a992`.
- Temurin JDK `jdk-21.0.12.1+1`, tarball sha256
  `ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94`.
- Both are reproducible with `scripts/setup_tools.sh` and used via `. tools/env.sh`.
- `BAZELISK_HOME` is `tools/bazelisk-home`. The reference pins Bazel 8.0.0 through
  `.bazelversion`.
- Bazel keeps its default output root (`~/.cache/bazel`).
- The whole-suite test target is `//:compiler_tests`. It is the `gen_java_tests`
  test_suite, which is narrower than `//:all`, and §4.2 means this target.

## D-007 — D2 corpus sourcing (2026-10-06)
**Group keys and pins:**
- The group key `npm:<name>@<version>` is shared across the npm and whole-program
  collectors, so the holdout split moves a package's cases together.
- test262 groups are directories: 3 path levels under `language/`, 2 under `built-ins/`.
- Closure Library is pinned to its final release tag, v20230802 (`ccf3f1dc`).
- test262 is pinned to `5992dc3b`.

**closure-self inputs:** these point straight at the pinned reference checkout and are
verified by sha256 rather than downloaded.

**npm package cases:**
- They compile from a committed shim entry, using `--module_resolution=NODE`,
  `--dependency_mode=PRUNE` and `--entry_point`.
- The shim writes the package exports to `globalThis` so that ADVANCED output is
  non-trivial.

**Whole-program cases:**
- They run their own zero-dependency test suites with
  `node corpus/d2/shims/<id>/run.mjs {output}`.
- 21 libraries were collected, and all 21 test suites pass on the original sources.

## D-008 — Holdout split and scrub (2026-10-06)
**Method:**
- The split unit is the group.
- 15% of groups are drawn per source, taking sources from fewest groups to most. Each
  quota is `floor(0.15*n+0.5)`, and groups already chosen by an earlier source count
  toward it.
- Groups are picked in `sha256(SALT ‖ group)` order. The 32-byte SALT exists only in the
  private holdout directory.

**Result:**
- **Held out:** 92 groups and 345 cases (3,398 pairs).
- **Visible:** 528 groups and 2,085 cases (21,155 pairs).
- All 5 sources remain visible.

**Scrub:**
- `corpus/d2/candidates` was replaced with visible-only copies. `finalize.py` run on them
  reproduces `cases.jsonl`.
- The collector generator scripts were moved out of the repository, because they
  enumerate every candidate.
- One holdout id was redacted from `oracle/SEAM.md`, and the holdout entries were removed
  from `JAVA_FAILURES.md` and `WHOLE_PROGRAM.md`.
- Holdout-bearing `build/` work directories and logs were moved to the holdout archive.
- The test262 and Closure Library upstream checkouts in `corpus-cache` were pruned to
  exactly the visible lock (48,725 files moved out). This closes the "diff against
  upstream" channel.

**Leak check:** a token-bounded grep for 484 holdout needles (case ids, group keys and
package versions), covering everything except `.git`, `reference/` and `tools/`, finds
0 hits, and the path-name scan also finds 0.

**Residual risks:**
- Inputs of the 3 holdout closure-self groups remain in `reference/`. This is unavoidable.
- Harness files (test262 `assert.js`/`sta.js`, Closure Library `base.js`) are shared
  across the split.

**Evaluating the holdout:** only the maintainers do this. It runs
`fetch_holdout.sh --check`, installs into a private root, and compares against the
holdout's golden results. Only aggregate pass counts are reported.


## D-009 — D2 profile matrix and drop rules (2026-10-06)
**Profiles:**
- Every case runs under every profile that applies to it: 9 single-output profiles, plus
  two chunk configurations for cases with 2 or more `--js` sources.
- **chunks2:** a chain, where c1 depends on c0.
- **chunks3:** a fan-out, where c1 and c2 each depend only on c0.
- Both chunk configurations are ADVANCED.

**Argument order:** argv precedence is implemented only in `gates/lib/case_args.py`. The
order is profile flags, then case flags, then `--externs`, then `--js`, then `--chunk`,
then the output flag.

**Drop rules:** a pair leaves D2 only if Java:
- crashes (an internal compiler error, or exit 254 from an uncaught exception);
- times out after 180 s;
- produces trivial ADVANCED output (empty output and zero diagnostics); or
- references a missing file.

Dropped pairs are listed in `corpus/d2/JAVA_FAILURES.md`. Pairs that end in compile errors
(including the exit 127 error cap) **stay**, as diagnostics-only comparisons. The `tags`
field is informational.

**Whole-program cases:** for each profile, the test requirement applies only if Java's own
output passes the library's tests. For example, acorn's advanced and chunks2 outputs fail
acorn's tests.

## D-010 — Reference environment contract (2026-10-06)
Every Java reference run (golden runs, oracle servers, every Java-vs-Rust comparison) uses
`run_reference.child_env()`:
- `LANG=LC_ALL=C.UTF-8`, `TZ=UTC`, a minimal `PATH`, and nothing inherited from the caller;
- the pinned JVM flags from `run_reference.py`.

Clients refuse a server whose ready-line environment differs from
`oracle_client.GOLDEN_JVM_ENV`. The reason: under the `de_DE` locale, Java prints
`98,2% typed` instead of `98.2% typed`.

**Rules for gates:**
- Oracle servers run with `-Xmx1536m`. A request that runs out of memory is retried once
  on a fresh server with `-Xmx3g`, and every retry is listed.
- Results involving `StackOverflowError` or "too deep recursion" depend on the JIT state.
  They are classed as depth-sensitive and reported separately, and the fuzzer caps
  nesting depth.
- Any rebuild of the oracle jar makes the gate results stale. Before the oracle is used as
  ground truth again, Gate 0.1 must be re-run along with a confirmation run under a new
  seed.

## D-011 — TypedAST seam contract (2026-10-06)
A Rust back end that is fed a Java TypedAST is compared against Java's
`optimize_from_typedast` on the same TypedAST, never against `compile`.

**What the seam carries from the checks response:**
- the inputs in order, with their SourceKind (weak inputs passed as `weak_inputs`);
- chunk membership after dependency management;
- fill inputs.

The reference construction is `oracle/test/seam3.py:opt_args`.

**When the checks run exits non-zero:** the expected result is the compile's exit code and
stderr, and no output file.

**Known seam exceptions** (`oracle/SEAM.md`): D6 (pretty-print number spelling, input
source maps), D7 (`hasRegExpGlobalReferences` under SIMPLE-level profiles) and D8 (errors
promoted to fatal). For these the Java JS output is not used as the Rust expected output.
**Follow-up:** add an oracle option that pins `hasRegExpGlobalReferences` to the compile's
value.

## D-012 — Corpus pipeline, golden store and timing data (2026-10-06)
- **Pipeline scripts:** the validation pipeline that generates `cases.jsonl`,
  `sources.lock.json` and a statistics report (`corpus/d2/validate/`) is version-controlled. Only its working
  files live in `build/`.
- **Golden store:** results are kept at
  `corpus-cache/d2/_golden/ref-4ef5a893/<case>/<profile>.json`.
- **Output directory:** every comparison reuses the same argv and the fixed output
  directory `build/golden-tmp/<case>/<profile>`, because that path is observable in the
  outputs. Gates must not run concurrently with `golden_all.py` on overlapping pairs.
- **Timing:** golden `wall_ms` and `peak_rss_kb` must never be used for D7. They were
  recorded on a loaded host, and the RSS values recorded by the first runner version are
  wrong. D7 measures Java separately on a quiet host.

## D-015 — Gate 0.2 definitions corrected (2026-10-07)
The first full measurement of Gate 0.2 (`gates/gate_0_2.sh`, criteria (a)–(e)) gave these
results:
- **(a)** 24,032/24,032 counted records replay green, but 1,095 methods with post-call
  assertions were unclassified.
- **(b)** passes.
- **(c)** 0.868.
- **(d)** 3 of 30 passes reach the threshold.
- **(e)** 33.96%.

The causes:
- **(c) contradicted §4.2.** §4.2 excludes non-harness test classes (177 classes, for
  example CommandLineRunner, SymbolTable and deps), yet (c) used the whole suite as the
  denominator. Measured against the harness-based classes, the ratio is 0.9874. The
  denominator is now the harness-based classes, the whole-suite ratio is still reported,
  and non-harness in-scope classes are listed for porting as Rust unit tests.
- **(d) could not be satisfied as written, for two reasons.**
  - Records that expect *no* change pass under a no-op by construction. They are valid and
    important tests, because they catch over-optimization.
  - The descriptor-level mutation never reached passes that the harness or helpers build:
    TypeCheck had 2,771 records at 0%, and RemoveUnusedCode had 1,011 records unreachable.

  The fixes: mutate at the class level, wherever the pass is constructed; measure only
  change-expecting records; and rank only in-scope passes, so that `lint/` is excluded.
- **(e) counted values no replay needs.** Examples are lambdas in test fields and deep
  object graphs, while the affected records still replayed green. The fix is in the
  recorder: dump only what descriptors or the harness can reference, and record other
  objects as class-name references. The measure itself is unchanged.
- **(a) needed a defined end state for post-call assertions.** Every flagged method must be
  classified as captured (by a post-call snapshot) or as a Rust unit test, and records that
  are not captured count toward a 3% exclusion cap.

These amendments must not weaken the intent of §4.2; in particular, the change-expecting
filter must not hide vacuous descriptors. The processor-identity check and the no-op vacuity
rule stay in force.

## D-016 — Fuzz profile weighting and the Gate 0.3 reach criteria (2026-10-07)
**Measurement:** the first full Gate 0.3 run gave:
- (a) 99.98% parse acceptance: pass;
- (c) a 1-hour driver run with 0 harness crashes and 0 mismatches: pass;
- (b) 49.8% average pass reach, using entry counters over all 138 DefaultPassConfig names:
  fail.

**Cause:** with uniformly drawn profiles, the best possible average is 53.6%. The pass set
that runs depends on the profile, not the program. ADVANCED-family runs average 67%.

**Decision (literal metric kept, nothing relaxed):**
1. **Profile weighting:** the fuzz driver weights the ADVANCED family (advanced,
   advanced_strict, chunks2, chunks3) at 65% combined, ws at 3%, and the remaining profiles
   at 32%. ADVANCED is the mode that matters most to users, so it gets the most fuzzing.
2. **(b) is unchanged:** entry counters, averaged per program, over all 138 names, with a
   ≥ 60% threshold.
3. **An extra criterion is added:** (b2), the union of passes the fuzzer makes *effective*
   (passes that change output), must be ≥ 85% of the passes effective on D2. Today it is
   42 of 47 (89.4%). This keeps the gate from being met by profile choice alone.
4. **`scope/flags.txt`** (required by §2) is generated from `CommandLineRunner`. The driver's
   random option flags come from it.
5. **Follow-up work, not gating:**
   - generator coverage for the passes never reached so far (closurePrimitives,
     deadPropertyAssignmentElimination, extractPrototypeMemberDeclarations,
     optimizeConstructors, removeWeakSources, crossChunkMethodMotion);
   - a type-clean or "strict-aware" generation mode, because advanced_strict runs always
     stop at the checks today.

   D4 (48 h of clean fuzzing, §3) depends on them.

## D-017 — Gate 0.2 final definitions and freeze (2026-10-07)
After the D-015 changes, further gaps were found:
- the helper scan missed 248 methods that assert after the hooked call;
- some rust_unit_test reasons were provisional;
- (c) counted out-of-scope `src/`.

This entry settles every open question and freezes the definitions.

**Accepted as implementation work (no change to a definition):**
1. **Postcondition data for (e):** what postcondition lambdas assert becomes replay-checked
   post-call state:
   - PureFunctionIdentifierTest (450 records): side-effect flags of call and new nodes in
     the output AST;
   - ExternExportsPassTest (69): the generated externs;
   - GatherExternPropertiesTest (32): the gathered set;
   - ScopedAliasesTest (24) and the 2 others.
2. **Neutral snapshot keys:**
   - `compiler.injectedLibraries`, `allowableFeatures`, `packageJsonMainEntries`,
     `typedPercent`, `moduleMapBoundNames`, `externExport` and `typeJSDoc`;
   - nested `goog.loadModule` module metadata;
   - typed-scope property flags;
   - neutral encoding of protobuf messages and Guava tables. This covers the records with
     unclassified post state.
3. **`postCallAbsent.<…>` as a capture path:** valid only if replay compares postCall key
   sets exactly.
4. **NoopAgent fix:** the agent also neutralizes inherited entry points, for example
   `ReplaceMessages$FullReplacementPass`.
5. **Attribution fix:** the name-prefix bug in the pass map is fixed
   (Es6RewriteClassExtendsExpressionsTest).
6. **Explicit classification:** every flagged method gets an explicit `postcall/` entry. A
   rust_unit_test category is assigned by reading the test source, not by keyword matching.

**Definitions made precise (§4.2 amended):**

7. **(a) Postcondition lambdas:** an assertion inside a lambda that a hooked helper passes as a
   Postcondition is an *in-call* assertion. The postcondition-count rule and (e) govern it,
   not post-call classification. Without this rule the same assertion would be counted
   twice.
8. **(a) Processor identity** is verified mechanically for every counted record. Prose notes
   (`processorIdentityNote`) are no longer accepted as verification.
   - **Mechanism:** the NoopAgent in trace mode records the set of in-scope pass classes
     whose entry points execute during the hooked call. This happens both in the recording
     run (trace stored in the record) and in replay, and the two sets must be equal.
   - **Unverified records:** a record without a verifiable trace counts as excluded, under
     the 3% cap.
9. **(c) "Harness-based test class"** means a class whose original run makes at least one
   hooked harness call, so it produces at least one record. Seven classes inherit a harness
   but never call it, for example TypeTransformationTest. They are non-harness classes and
   must be listed in RUST_UNIT_TESTS.md. On this basis the ratio is 0.987; the whole-suite
   ratio is still reported. The in-scope `src/` predicate (the §2 directories plus
   J2cl/Polymer/Chrome/Debugger-specific classes) is confirmed.
10. **(d) Change-expecting**, implemented as §4.2 words it:
    - **Code:** parse the input and the expected output in the record's language mode and
      compare them with `isEquivalentTo`, without normalization. A difference in text only
      counts as no-change.
    - **Postcondition:** change-expecting only if the expected value differs from the value
      observed on the parsed, unprocessed input. For example, "no call is marked pure"
      counts as no-change, like every other no-change record.
    - **Diagnostics:** unchanged.
11. **(d) Attribution:**
    - A record is attributed to the ranked passes whose entry points its trace shows
      executing (item 8).
    - If the test class is named after a pass, the record goes to that pass only. This uses
      the existing name rule, fixed per item 5.
    - Otherwise, if two or more ranked passes executed, the record goes to that **group**.
      The group is turned into a no-op jointly and must also reach 90%; an example is
      PeepholeIntegrationTest.
    - **TypeCheck** is ranked as the type-check unit {TypedScopeCreator, TypeInferencePass,
      TypeCheck}, because its tests assert diagnostics emitted across all three. The rate
      for TypeCheck alone is reported for information.
    - No record leaves the measurement. The rule that only assertion-type failures count
      stays in force.

**Rejected:**
- Counting crashes as caught.
- Per-descriptor prose notes as processor-identity evidence (replaced by item 8).

**Freeze:** these definitions are final. A change may be raised only for two reasons:
1. the implementation deviates from the text of §4.2, D-015 or D-017;
2. a concrete fidelity defect shown on named records: a record that replays green but would
   not detect a behavior change in the Rust port.

## D-019 — Straight port; measurement extras dropped (2026-10-07)
**Principle:** port the code; the tests have to pass and the outputs have to be identical. Nothing
is reinvented and no extra process is added.

The oracle, the D2 corpus (golden outputs for 2,085 visible cases), the unit corpus, the D2
holdout and the fuzzer are kept as built. 8 unit records do not replay green in Java either
(5 CheckConformance custom-rule records and 3 SerializeAndDeserializeAst file-system records);
they are not port failures.

D-015 to D-017 are superseded wherever they conflict with this entry: the `postCall` and
`passTrace` record fields remain available to the Rust harness, but nothing gates on them.

**Decision:**
1. **One core design that mirrors Java** (§5): an arena AST with Node's method names, and
   modules and function names following the Java files.
2. **Order: front to back, as Java runs.** Core types, then parser and printer, then the
   compiler skeleton and a Rust port of the unit-test harness that runs `corpus/unit` records
   natively, then passes. No TypedAST reader or serializer is needed for the port itself.
3. **The tests are the arbiter** (§7). A change is accepted when CI (fmt, clippy, no unsafe, no
   std HashMap/HashSet, no ignored tests) passes and no previously passing unit record or D2
   pair regresses.
4. **The fidelity criteria (§3) are unchanged:** identical outputs on the unit corpus, D2, the
   holdout, source maps and determinism.

## D-025 — Performance work: structure may diverge from Java when output is identical (2026-10-08)
Relaxes D-019's straight-port rule for performance work. A change may leave the Java structure
when stdout, stderr, output files, source maps and exit codes stay byte-identical (CI, unit-record
ratchet, complete D2 corpus, benchmark output identity) and runs stay deterministic
(docs/PORTING.md §4.7). Code that still implements a Java method keeps its `// port:` marker; a
divergence carries a short comment saying what Java does and why the Rust differs, and is listed
here (one line each) to ease upstream syncs.
- `jstype/property_map.rs`: `properties` / `known_symbols` are `Arc`, copy-on-write
  (`Arc::make_mut`), so the clones that release the registry borrow are O(1).
- `jscomp/data_flow_analysis.rs` `UniqueQueue`: `swap_remove` on the seen set; sorted queue
  insert by binary search (Java: `HashSet` + `PriorityQueue`).
- `parsing/config.rs`, `js_doc_info_parser.rs`: annotation / suppression / primitive-name maps
  are `Arc`-shared, not copied per JSDoc comment.
- `parsing/parser/scanner.rs` `peek_token_ref_at`: the parser peeks token types and lines by
  reference instead of cloning the token.
- `rhino/jscomp_colors/color.rs`: `Color` memoizes its hash code (immutable value).
- `rhino/node.rs` `lookup_property_ref`: the property list is walked by reference.
- `jscomp/serialization/malformed_typed_ast_exception.rs` `DebugParam`, `remove_unused_code.rs`
  unremovable log: messages are formatted only when used (Java formats eagerly).
- `jscomp/reference.rs`, `node_traversal.rs` `get_input_id_of_input`: references share the
  traversal's `InputId` `Arc` instead of copying it after an input lookup.
- `cli/src/main.rs`: the binary uses mimalloc as its global allocator (Java: the JVM's heap);
  about 9% faster compiles for about 100 MB more peak memory.
- `Cargo.toml` `[profile.release]`: fat LTO and one codegen unit, about 5% faster again.
- `jscomp/parallel_parse.rs`: inputs are parsed on up to 8 worker threads into arenas of their
  own (`Ast::new_for_preparse`) while the compiler runs; `CompilerInput#parse` moves a finished
  parse into the compiler's arena (`Ast::append_preparsed`) with the node ids, object sharing
  and error order that parsing in place gives, or parses itself (Java: on the compiler thread).
- `rhino/rhino_string_pool.rs`: the intern pool is 64 independently locked shards, with a
  per-thread cache for the parser threads (Java: one weak interner).
- `rhino/fast_hash.rs`: every `IndexMap`/`IndexSet` uses a multiplicative word hasher instead
  of SipHash (insertion order, so iteration and output, are unaffected).
- `rhino/node.rs`: token and tree links of all nodes live in one dense array (`NodeLinks`) apart
  from the rest of the node; template literal strings are boxed; `get_string_ref`,
  `get_jsdoc_info_ref` read without copying; `removeProp`/`putProp` skip the list rebuild when
  the property is absent.
- `rhino/js_string.rs` `JsStrLike`: `matchesName`, `matchesQualifiedName`, `indexOf`,
  `startsWith`, `endsWith`, `Property.Key#matches` take string literals without allocating.
- `rhino/jscomp_parsing_parser/util/source_position.rs`: positions name their `SourceFile` by a
  copyable `SourceFileId` instead of holding a reference to it.
- `parsing/parser/keywords.rs`: keyword lookup matches bytes (Java: a map); the token type is
  computed without building the keyword string.
- `jscomp/scope.rs`, `var.rs`, `typed_scope.rs`: the fields of scopes and vars that never change
  are mirrored outside the shared arena lock (`ScopeMirror`, `typed_scope_mirror`); `getVar` and
  `declare` of syntactic scopes take the lock once.
- `jscomp/typed_scope_creator.rs`: the never-iterated `reservedNamesForScope` and
  `assignedVarNames` remove entries with `swap_remove`.
- `jstype/property_map.rs` `findClosest`, `object_type.rs` `getOwnSlot`/`getSlot`: the walk reads
  each property map in place instead of copying it.
- `jscomp/compiler_input.rs`: a `CompilerInput` is one `Arc` around its fields.
- `jscomp/rhino_error_reporter.rs` `JSErrorQueue`: the registry error queue's empty check is a
  flag load, not a lock.
- `jscomp/compiler.rs` `get_extern_properties_js`: the extern property names are converted to JS
  strings once per value, not on every RemoveUnusedCode run.
- `jscomp/node_traversal.rs` `get_input`, `syntactic_scope_creator.rs` `ScopeScanner`: the
  CompilerInput found for the current input id is kept (Java keeps the object) instead of being
  looked up by id again; `ImplicitVar::js_name` makes the implicit var names once.
- `rhino/java_lang/charset.rs` `decode`: one output buffer and a fast path for ASCII runs of
  UTF-8 (Java's decoder loop gives the same text).
- `rhino/js_string.rs`: `indexOf` scans for the needle's first unit, then compares the rest;
  `==` with an ASCII `str` compares unit by byte; `""` conversions share one empty string
  (Java's `""` literal is one interned object).
- `jscomp/compiler_options.rs`: the conformance path regex is compiled once per process, not
  per options object (CodePrinter.Builder makes default options per `toSource`).
- `jscomp/scope.rs` `NameArg`, `get_var_of_node`, `typed_scope.rs` `get_var`,
  `abstract_var.rs` `name_equals`: variable names are read by reference instead of copied, and
  `TypedScope#getVar` walks the scope chain under one lock for non-implicit names;
  `ImplicitVar::of` checks the name's length first.
- `jscomp/data_flow_analysis.rs` `flow`: the stored lattice element is compared with the new one
  before it is replaced instead of being copied first (Java keeps references); the
  `flowThrough` of MustBeReachingVariableDef, MaybeReachingVariableUse and
  LiveVariablesAnalysis use the owned input instead of copying it; `MustDef`'s map is shared
  copy-on-write.
- `jscomp/node_traversal.rs`: `getInput` returns the cached input without recomputing its id;
  `getChunk` is read once per script.
- `jscomp/source_file.rs`: the source kind is mirrored in an atomic, so `getKind`/`isExtern`
  take no lock.
- `jscomp/thread_safe_delegating_error_manager.rs`, `compiler.rs` `hasHaltingErrors`: a flag
  set when the manager found no halting errors and cleared by every change to it lets the
  per-node check of CombinedCompilerPass skip the locks.
- `rhino/node.rs` `Ast::prop_masks`: per node, one bit per property type present in its
  property list, in a dense array, so a lookup of an absent property reads neither the node nor
  the list.
- `rhino/node.rs` `NodeData`, `NodeCold`, `NodeType`: a node's fields are split over three
  dense arrays, the payload and property list (read by most passes), the source position and
  original name, and the type or color (Java: one object).
- Name lookups read the NAME node's string in place instead of copying it
  (`ScopeId::get_var_of_node`; PolyfillUsageFinder, OptimizeCalls, VarCheck,
  RemoveUnusedCode, DataFlowAnalysis#computeEscaped, InlineFunctions, GatherModuleMetadata,
  PeepholeFoldConstants); OptimizeCalls keeps the extern property names as JS strings instead
  of converting each property name to compare it.
- `jscomp/scope.rs` `declare`, `allocate`: a new var or scope takes the arena lock once.
- `jscomp/chunked_vec.rs`: the syntactic scope arena and its mirror grow in fixed-size chunks
  instead of one `Vec`, so growing never copies the scopes and vars already made.
- `rhino/js_string.rs`: a string caches its `hashCode()` in front of its code units (Java's
  String caches it too); `Hash` writes the cached value, `equals` and `compareTo` short-cut on
  identity.
- Source maps: `sourcemap/source_map_generator_v3.rs` `JavaAppendable` appends code units
  without making a JS string per write; `util.rs` `escapeString` copies unescaped runs and
  escapes directly; `jscomp/source_map.rs`, `compiler_source_excerpt_provider.rs` reuse the
  file name conversions of the previous mapping and the parsed input map
  (`SourceMapInput::get_cached_source_map`); `cli/java_io.rs` `EncodedWriter` writes UTF-8
  text to a UTF-8 stream without the UTF-16 round trip.

## D-026 — Upstream syncs follow npm releases; first sync to 20261006.0.0 (2026-10-09)
closure-rs moves its Closure Compiler pin only to upstream **releases that are published on npm**
(`google-closure-compiler@YYYYMMDD.0.0` = upstream tag `vYYYYMMDD`), never to unreleased master
commits, so it always matches a compiler that users can install. The first pin, `bb8c8e7`, is
release `v20261005` (npm `20261005.0.0`). The first sync targets release `v20261006` = commit
`48f4107ca2aac52149546ccc42894522fcfdb17d` (npm `20261006.0.0`, seven upstream commits after
`bb8c8e7`), registry tag `v20261006`, uberjar sha256
`cfa8886f9bcb9c05d29006dab5cd7012221a7ab2337d14c6fbd8d685b31f2264` (unstamped, reproducible
after `bazelisk clean`).

The references live alongside each other through `scripts/references.tsv` (docs/PORTING.md §9):
the new one has its own checkout (`reference/closure-compiler-v20261006`), recording workspace,
uberjar (`build/reference-v20261006/`), oracle jar (`build/oracle-v20261006/`) and D2 golden
store (`ref-cfa8886f`); the `bb8c8e7` checkout, jars, recording workspace and golden store
`ref-4ef5a893` are kept unchanged. A registry tag of a new reference is its upstream release tag.

The closure-self D2 cases keep their inputs at `bb8c8e7`: their case ids (`@bb8c8e7`) and their
input paths under `reference/closure-compiler/` stay as they are, so `reference/closure-compiler`
is kept as a permanent input checkout and is not removed when a later reference becomes the
default. Only the compiler that processes those inputs changes with the reference.
