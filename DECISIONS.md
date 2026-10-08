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
