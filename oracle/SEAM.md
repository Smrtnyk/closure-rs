# Seam measurement and server identity (oracle, 2026-10-06)

Everything here was measured with `build/oracle/oracle.jar`, built by `oracle/build.sh`
against the reference jar (sha256 `4ef5a893…af821`).

**Environment of these measurements (recorded 2026-10-06).** Sections 1
and 2 (identity, seam v1/v2/v3) ran before the oracle clients pinned the environment
(`oracle/PROTOCOL.md`, "Environment"), so every JVM inherited the harness environment:
`LANG=en_GB.UTF-8`, no `LC_ALL`, no `TZ`. The JDK derives `user.language=en`,
`user.country=GB`, FORMAT locale `en-GB`, time zone `Europe/Ljubljana`, and `UTF-8` for
`native/stdout/stderr/sun.jnu/file.encoding` from it (measured with the new ready-line `env`).
Against the golden environment (C.UTF-8, UTC: locale `en`, no country, `UTF-8` everywhere, `UTC`)
only the locale tag and the time zone differ; the decimal separator (`.`) and every encoding are
the same, and no in-scope `src/` code reads the time zone. Evidence: all 202 `NN.N% typed`
summaries in the stored results use `.` (identity CLI 6, seam v2 98, seam v3 98; 0 with `,`).
Every comparison in this file is also between two runs in that same environment. The results
therefore stand; re-runs now use `golden_env()` automatically (`oracle_client.Oracle` checks the
JVM's ready-line `env` against `GOLDEN_JVM_ENV`, and `seam3.py` stores it per result as
`jvm_env`).

**Re-measured in the golden environment (after the pin).** Every JVM
below ran in `golden_env()` and passed the ready-line check (`locale.format=en`,
`timezone=UTC`, `UTF-8` encodings, recorded per result):
- seam **v4**: `seam3.py --out build/oracle/seam/v4 --per-profile 20`, all 11 D2 profiles
  including `chunks3`, 220 pairs (table "Results (v4)" below; `seam3_report.py` prints the env
  of all 220 results);
- the **D7 experiment** (`seam_regexp.py`): the same six out.js hashes as in the D7 table;
- **server identity v2** (section 1, `identity.py --out build/oracle/identity-v2`);
- **Gate 0.1 (gate version 3, `gates/gate_0_1.sh`)**: every one of the 24,553 D2 pairs through
  long-lived `--isolate=none` servers, compared with the golden `java -jar` runs.

## Case argv

`gates/lib/case_args.py` and `corpus/d2/profiles.json` did not exist yet, so the measurements
build argv with `oracle/test/case_args.py`:

```
--externs=E…  --js=INPUT…  --js=SHIM…  <extra_flags>  <profile flags>
```

| Profile | Flags |
|---|---|
| `simple` | `--compilation_level=SIMPLE` |
| `advanced` | `--compilation_level=ADVANCED` |
| `ws` | `--compilation_level=WHITESPACE_ONLY` |
| `pretty` | `SIMPLE` plus `--formatting=PRETTY_PRINT` |
| `lang_es5` | `SIMPLE` plus `--language_out=ECMASCRIPT5` |

Output goes to stdout. Cases whose inputs total more than 300 kB were excluded from both
samples to bound the run time.

## 1. Server identity

**Script:** `oracle/test/identity.py --isolate none --n 300` (seed 20261006).
**Results:** `build/oracle/identity/summary-none.json`.

**Sample.** 300 (case × profile) compile requests drawn from all of
`corpus/d2/candidates/*.jsonl`, across the profiles ws, simple, advanced, pretty and lang_es5.
Exit codes in the sample: 272 × 0, 21 × 1, and 7 others (2, 3, 10, 13, 20, 20, 24).

**Method.**
- **Server:** all 300 requests were sent through **one** server in a seeded shuffled order
  (seed 20261007), with `--isolate=none`: one classloader, a warm JIT and shared static state.
- **Reference:** one fresh JVM per request (`Main compile ARGV`).
- **Comparison:** stdout, stderr and exit code, byte for byte.

| Comparison | Result |
|---|---|
| Server vs. fresh JVM per request | **300 / 300 identical, 0 differing** |
| Fresh-JVM oracle vs. `java -jar closure-compiler.jar`, every 5th request | **60 / 60 identical** |
| Server time for 300 requests | 360 s (median 1.0 s per request); a fresh JVM costs about 2.5 s or more per request |

**Re-run in the golden environment (v2):** `identity.py --isolate none
--n 300 --cli-par 2 --out build/oracle/identity-v2`, same seed and sample, every JVM in
`golden_env()` (server ready-line env `locale.format=en`, `timezone=UTC`, `UTF-8`, stored in
`build/oracle/identity-v2/summary-none.json`): server vs fresh JVM **300 / 300 identical**,
fresh-JVM oracle vs `java -jar` **60 / 60 identical**, server time 232 s. The 300 requests
(case, profile, argv) are the same as in v1, and the fresh-JVM results of v1 (inherited
`en_GB.UTF-8`, `Europe/Ljubljana`) and v2 (golden environment) are byte-identical in 300 of 300
(exit code, stdout, stderr): direct evidence that the v1 environment did not change these
results. Gate 0.1 (gate
version 3) repeats the server-vs-`java -jar` comparison on all 24,553 D2 pairs in the same
environment: 22 long-lived `--isolate=none` server logs, each server handling 867 to 1,328
requests from one seeded shuffled queue, all 24,553 results byte-identical to the golden runs.

**Verdict: no static state leaks into the output across requests.** `--isolate=none` is
therefore the default.

`--isolate=request` (a fresh unnamed `URLClassLoader` per request) is kept as a fallback. It
gives no speed-up, because each request pays cold class loading and JIT. Its byte-identity
was checked by `oracle/test/smoke.sh`, and by an internal-crash case (exit code 254, an NPE
stack trace in `RemoveUnusedCode`) that matched `java -jar` exactly in both modes.

**A bug the measurement found.** The first version of the per-request classloader was
*named*. A named loader prefixes every frame of a printed stack trace with `name//`, so crash
output differed from `java -jar`. The loader is now unnamed.

## 2. Seam: `compile(argv)` vs. `optimize_from_typedast(checks_to_typedast(argv))`

**Script:** `oracle/test/seam.py --n 320 --method input_order`, with 3 servers using
`--isolate=none`. **Report:** `oracle/test/seam_report.py build/oracle/seam/v2`.

**Sample.** 320 (case × profile) pairs, seed 20261006:
- by profile: 258 simple and 62 advanced;
- by source: npm 172, test262 97, closure-library 48, whole-program 3. (closure-self
  inputs were drawn too, but none fell into this sample.)

**The three requests in each pair:**
1. `compile` with `argv`.
2. `checks_to_typedast` with `argv`. This runs `argv + --checks_only + --typed_ast_output_file`.
3. `optimize_from_typedast` with the optimize argv, built as follows:
   - drop `--entry_point` and `--dependency_mode`, then add `--dependency_mode=NONE`. This is
     required, because `Compiler.initOptions` throws *"Using precompiled libraries (i.e.
     TypedAST) is incompatible with flags that automatically order/prune dependencies"*;
   - put the `--js` files in the order that the checks run's compiler held them **after**
     dependency sorting and pruning, dropping pruned files. The checks response reports this
     order as `input_order`, taken from `Compiler.getInputsInOrder()`.

### Results (v2, input_order method)

| Class | Pairs | Meaning |
|---|---:|---|
| identical (stdout, stderr, exit code) | **112** | |
| stdout and exit code identical, stderr differs | **186** | diagnostics are split between the two runs (see below) |
| checks failed with errors, so no TypedAST was written | **21** | `compile` fails identically: same exit code, same stderr as the checks run, empty stdout in all 21 |
| **stdout differs** | **1** | `closure-library-prune-goog.storage.storage` (simple) |

**Where checks succeed (299 pairs), the JS output and exit code are identical in 298, or
99.7%.** Where checks fail with a **halting** error (21 pairs, all "no TypedAST"), the full
compile also stops in stage 1, with the same errors and no output. That boundary is clean, but
only for halting errors. **Correction:** errors that do not halt (a
WARNING-type diagnostic promoted to ERROR, e.g. by `--jscomp_error=*` or the `es5Strict` group)
behave differently: the checks run still writes a TypedAST, the full compile runs stages 2 and
3 and then writes no JS. See D8; this is the common case for `advanced_strict`.

### Difference classes, each with an example

**D1: dependency ordering (only when the optimize argv keeps argv order; fixed by method v2).**
- **Run:** a first run (v1, `--method argv`, 106 pairs, results in `build/oracle/seam/results`)
  kept the `--js` files in argv order.
- **Affected pairs:** all 8 stdout differences in that run were `--dependency_mode=PRUNE`
  cases with several files.
- **Cause:** the full compile sorts its inputs into dependency order, but the TypedAST
  restore runs with `NONE` and keeps argv order. The output therefore holds the same code with
  the files concatenated in a different order.
- **Example:** `npm-wretch-3.0.9-pkg-esm` (simple). The output is 8710 bytes in both runs, but
  the compile starts with `function extractContentType$$module$…utils` and the optimize run
  starts with `const JSON_MIME$$module$…constants`.
- **Fix:** v2 passes the post-sort `input_order`.
- **Consequence for a hybrid pipeline:** it must feed the optimizer the inputs in the
  post-sort order from the checks stage. The TypedAST alone does not carry the order of
  `--js` arguments.

**D2: diagnostics split across the two runs (186 pairs, stderr only).**

The full compile prints one report: every warning from stages 1 to 3 in one sorted list,
followed by a single summary line. The hybrid prints two reports. The breakdown:

| Pairs | What happens |
|---:|---|
| 173 | `compile.stderr == checks.stderr` **and** `optimize.stderr` is empty. All warnings come from checks, and the optimize run is silent. The example in the next table is index 2, `test262-language--statements--with--S12.10_A1.11_T4`. |
| 13 | Warnings that only optimization passes emit. These appear in the optimize run's own report, so the compile's single report is the merge of the two, with merged counts. |

The 13 optimization-warning pairs break down like this:

| Example (pair index) | Warning type | What differs |
|---|---|---|
| idx 45, `test262-negparse-…-Script-equals-negated` | `JSC_MALFORMED_REGEXP` | Reported by an optimization-time regexp check. |
| 5 pairs, e.g. idx 50 | `JSC_PARTIAL_NAMESPACE` | Reported by CollapseProperties. |
| 2 pairs, e.g. idx 65, `npm-signature_pad-5.1.4-pkg-esm` | `JSC_RECEIVER_AFFECTED_BY_COLLAPSE` | Reported by CollapseProperties. See also D3. |
| 3 pairs, e.g. idx 122 | — | The summary suffix `, NN.N% typed` is printed only by the run that ran type checking, which is the checks run. The optimize run's summary lacks it. |
| 2 pairs, e.g. idx 7 and idx 301 | — | Other count or ordering effects of the same split. |

**D3: `Originally at:` lost after the TypedAST restore (stderr only).**
- In the full compile, warnings on inputs that carry a `//# sourceMappingURL` include an
  `Originally at: <original file:line>` block, resolved through the input source map.
- The same warning from `optimize_from_typedast` has no such block. Input source maps are
  not part of the restored state.
- **Example:** idx 65 (`signature_pad`, `JSC_RECEIVER_AFFECTED_BY_COLLAPSE`); also seen in v1
  on `npm-luxon-3.7.2-pkg-cjs`.

**D4: weak (requireType-only) inputs survive the seam (1 pair in v2; root-caused, fixed in v3).**
- **Case:** `closure-library-prune-goog.storage.storage` (simple, `PRUNE`). storage.js:16 is
  `goog.requireType('goog.storage.mechanism.Mechanism')`, so mechanism.js is a WEAK input.
- **Root cause:** `checks_to_typedast` adds `--checks_only`, which skips `removeWeakSources`
  in stage 1 (DefaultPassConfig.java:463, `if (!options.isChecksOnly())`). Stage 2 removes weak
  sources only if the restored `SourceFile` is weak (DefaultPassConfig.java:845,
  RemoveWeakSources.java:34). v2's `input_order` listed mechanism.js and the optimize argv passed
  it as a plain (STRONG) `--js`; CommandLineRunner has no `--weakdep` flag. Its code was kept.
- **Fix (v3):** the checks response now reports each input's `SourceKind` (`inputs`, `chunks`);
  `optimize_from_typedast` takes `"weak_inputs"` and marks those files WEAK through
  `OracleRunner.createInputs` (the same kind `--weakdep` gives). Measured: v3 output is
  byte-identical to compile for this case under simple and advanced
  (`python3 oracle/test/seam3.py --case closure-library-prune-goog.storage.storage --profiles simple,advanced`).
- **Artifacts:** `build/oracle/seam/v2/diffs/71/` (v2), `build/oracle/seam/v3/results/case-closure-library-prune-goog.storage.storage-*.json` (v3).

**D5: chunk membership after dependency management (fixed in v3).** With `--chunk` plus PRUNE,
dependency management moves inputs between chunks; that cannot be rebuilt from a flat
`input_order` and the original `--chunk=name:count` flags. Example: `npm-whatwg-fetch-3.6.20-pkg-cjs`
chunks2: compile writes c0.js 1 byte / c1.js 9678 bytes, v2 wrote 9525 / 154. v3 rebuilds the
`--js` order and `--chunk=name:count:deps` flags from the checks response's `chunks`
(`JSChunkGraph.getAllChunks()` after dependency management) and passes chunk fill files
(`c0$fillFile`) as empty in-memory inputs (`"fill_inputs"`; a root chunk may not be empty,
JSC_EMPTY_ROOT_CHUNK_ERROR). Measured: c0.js and c1.js byte-identical to compile.

**D6: profiles whose output needs data the TypedAST does not carry (all 10 profiles).** Measured
on seeds 11/12:
- **pretty:** `CodePrinter` pretty mode takes a number's spelling from the original source
  text (CodePrinter.java:315-345, `getNumberFromSource`), which the restored AST cannot supply:
  compile `0.5`, `2500000000`, `3.6e6` vs seam `.5`, `25E8`, `36E5` (2 of 15 pairs).
- **sourcemap:** input source maps are not restored, so `sources` names the `.js` instead of
  the original `.ts`, and `names`/`mappings` differ (7 of 17 pairs; out.js identical).
- **stderr:** node lengths (`[length: N]`, caret underlines) are lost by the restore; the full
  compile also renders check-phase warnings after optimizations changed node lengths (idx 122,
  197, 312: compile's caret for date.js:83:113 is 76 characters, checks' is 3; optimize.stderr
  is empty, and both compile and checks print the same `% typed` suffix — the earlier `% typed`
  explanation was wrong). idx 7 and 301 are **crash-both-sides**: compile and optimize both
  exit 254 (INTERNAL COMPILER ERROR / NPE), checks exits 0.
- **v3 per-profile measurement:** `build/oracle/seam/v3/results/` (`seam3.py --per-profile 20`,
  all 10 profiles, comparing exit code, stdout, stderr and every output file). The summary per
  profile is produced by `python3 oracle/test/seam3_report.py`.

### Results (v3, 2026-10-06, `seam3.py --per-profile 20`, seed 20261006, inputs <= 400 KB)

| profile | pairs | identical | JS identical, stderr differs | JS/exit differs | no TypedAST | oracle failure |
|---|---:|---:|---:|---:|---:|---:|
| advanced | 21 | 3 | 15 | 1 | 2 | 0 |
| advanced_strict | 20 | 2 | 0 | 18 | 0 | 0 |
| chunks2 | 21 | 2 | 14 | 0 | 5 | 0 |
| lang_es2015 | 20 | 6 | 14 | 0 | 0 | 0 |
| lang_es5 | 20 | 8 | 12 | 0 | 0 | 0 |
| lang_next | 20 | 7 | 11 | 0 | 2 | 0 |
| pretty | 20 | 4 | 8 | 8 | 0 | 0 |
| simple | 21 | 7 | 11 | 1 | 2 | 0 |
| sourcemap | 20 | 4 | 8 | 6 | 2 | 0 |
| ws | 20 | 0 | 0 | 0 | 20 | 0 |

Reading the table: "JS/exit differs" compares exit code, stdout and every output file.
- **advanced_strict (18 of 20) and advanced-16: D8, not an exit-code-only difference.** In all 19
  pairs the checks run exits non-zero (16–127) and still writes a TypedAST; compile exits with the
  same code and writes **no** output file; `optimize_from_typedast` exits 0 and writes a full
  `out.js`. So the JS differs as absent vs present. advanced-16 (a holdout case whose id is
  not published, docs/PORTING.md §4.5; plain ADVANCED) is the same case: its 2 errors are `JSC_DUPLICATE_MEMBER`, a WARNING type
  (StrictModeCheck.java:82) promoted to ERROR by the `es5Strict` group. Root cause and rule: D8.
- **simple (1 of 21): D7, a real JS divergence.** `simple-10` (`whole-program-minimist-1.2.8`):
  same exit code (0), same empty stderr, different `out.js` (compile `ccaa63ed67d1…`, seam
  `2d2dcab83746…`). Root-caused below (D7: `hasRegExpGlobalReferences`).
- **pretty (8) / sourcemap (6):** D6 (number spelling from source text; input source maps).
- **ws (20 of 20) no TypedAST:** `--checks_only` with WHITESPACE_ONLY writes no TypedAST, so
  **no ws pair was compared**; the ws profile has no optimizer back end to compare.

### Results (v4, 2026-10-06, golden environment, `seam3.py --out build/oracle/seam/v4 --per-profile 20`)

Same method and seed as v3, drawn from the final `cases.jsonl` (full matrix of docs/PORTING.md
§4.4, so the sample now includes npm single-file cases under ADVANCED, most of which stop with a
halting `JSC_UNDEFINED_VARIABLE`), plus the `chunks3` profile. Columns D8/other split "JS/exit
differs" (`seam3_report.py`: D8 = checks exit ≠ 0, compile wrote no file, optimize exit 0).

| profile | pairs | identical | JS identical, stderr differs | JS/exit differs | of which D8 | other | no TypedAST | oracle failure |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| advanced | 20 | 1 | 6 | 0 | 0 | 0 | 13 | 0 |
| advanced_strict | 20 | 0 | 0 | 9 | 9 | 0 | 11 | 0 |
| chunks2 | 20 | 1 | 12 | 0 | 0 | 0 | 7 | 0 |
| chunks3 | 20 | 1 | 16 | 0 | 0 | 0 | 3 | 0 |
| lang_es2015 | 20 | 6 | 9 | 0 | 0 | 0 | 5 | 0 |
| lang_es5 | 20 | 7 | 12 | 0 | 0 | 0 | 1 | 0 |
| lang_next | 20 | 3 | 12 | 0 | 0 | 0 | 5 | 0 |
| pretty | 20 | 8 | 10 | 2 | 0 | 2 | 0 | 0 |
| simple | 20 | 5 | 14 | 1 | 1 | 0 | 0 | 0 |
| sourcemap | 20 | 4 | 8 | 5 | 0 | 5 | 3 | 0 |
| ws | 20 | 0 | 0 | 0 | 0 | 0 | 20 | 0 |

- **D8 in 10 pairs**, including one under SIMPLE: `simple-9`
  (`test262-language--expressions--class--elements--static-field-redeclaration`) has a promoted
  error, so the rule is not specific to `advanced_strict`. Over v3 and v4, every pair whose checks
  run exits non-zero and still writes a TypedAST (29 of 29) has compile exit = checks exit,
  no compile output file, and optimize exit 0.
- **Errors raised in stage 2** (checks exit 0, compile exit ≠ 0: `JSC_CANNOT_CONVERT_YET` and
  `JSC_NON_CONST_DEFINE` under `lang_es5`/`lang_es2015`; 3 pairs in v3, 1 in v4) cross the seam
  intact: optimize exits with the same code and also writes no file.
- **chunks3** (fan-out, c1 and c2 both depending on c0): no JS difference in the 17 pairs with
  a TypedAST; the v3 chunk reconstruction (D5) also handles the fan-out graph.
- **pretty (2) / sourcemap (5):** D6. **No D7 pair was drawn** in v4 (D7 is reproduced by its
  own experiment below).

**D7: `Compiler.hasRegExpGlobalReferences` is recomputed differently by the restore (SIMPLE-level
profiles; root-caused 2026-10-06).**
- **Mechanism.** `AstAnalyzer` treats a call `/re/.test(x)` or `/re/.exec(x)` on a regexp
  literal as side-effect free only if `!hasRegexpGlobalReferences && assumeKnownBuiltinsArePure`
  (AstAnalyzer.java:229-230). The flag lives on the `Compiler`, not in the AST, and starts as
  `true` (Compiler.java:241). In a full compile it is set only by the `checkRegExp` check, which
  runs only if `options.shouldComputeFunctionSideEffects()` (DefaultPassConfig.java:367-368);
  that option is set only by ADVANCED (CompilationLevel.java:205). So in every SIMPLE-level
  compile (`simple`, `lang_es5`, `lang_es2015`, `lang_next`, `pretty`, `sourcemap`) the flag stays
  `true`. `optimize_from_typedast` sets `mergedPrecompiledLibraries`, so stage 2 always runs
  `checkRegExpForOptimizations` (DefaultPassConfig.java:843-849), which sets the flag from the
  AST: `false` unless the code reads a global RegExp property such as `RegExp.$1`. The TypedAST
  does not carry the flag. With `false`, `function y(a,c){return f.allBools&&/^--[^=]+$/.test(c)||…}`
  becomes inlinable, so the seam's `inlineFunctions` inlines it and the compile's does not.
  This matches a bisect of the pass dumps: `normalize` prints the same source (sha1 `ff62bcbf`) in both
  runs, and the first divergence is the seam-only `inlineFunctions` block.
- **Experiment** (`oracle/test/seam_regexp.py`; same argv as the case, with a copy of the
  shim plus at most one appended line):

  | variant | simple: compile vs seam out.js | advanced |
  |---|---|---|
  | copy of the shim, unchanged | differs: `ccaa63ed67d1` vs `2d2dcab83746` (the v3 values) | JS identical |
  | plus `globalThis['rxprobe'] = 1;` (control) | differs: `a56d271739f6` vs `9a0ab2dfc291` | JS identical |
  | plus `globalThis['rxprobe'] = RegExp.$1;` | **identical** (`b39e8a15981e`) | JS identical |

  Referencing a global RegExp property makes `checkRegExpForOptimizations` compute `true`, which
  is the full compile's value, and the outputs become byte-identical. Under ADVANCED, where
  `checkRegExp` runs in the compile's checks too, both sides compute the same value.
- **Consequence.** For SIMPLE-level profiles the Java seam itself is not output-preserving
  whenever a regexp-literal `test`/`exec` call feeds an optimization decision. A Rust back end
  that ports `checkRegExpForOptimizations` faithfully reproduces `optimize_from_typedast`, not
  `compile`. To reproduce `compile`, the hybrid must carry `hasRegExpGlobalReferences` across the
  seam: `true` when the checks run did not run `checkRegExp` (not ADVANCED), else the value
  `checkRegExp` computed.

**D8: promoted (non-halting) errors (advanced_strict, and any profile with a promoted error;
root-caused 2026-10-06).**
- **Mechanism.** `Compiler.hasErrors()` is `hasHaltingErrors()` (Compiler.java:3489-3505), and
  `SortingErrorManager.hasHaltingErrors()` is `originalErrorCount != 0` (SortingErrorManager.java:64).
  An ERROR whose `DiagnosticType` default level is WARNING counts as `promotedErrorCount`
  instead (lines 48-58). `--jscomp_error=*` promotes every warning, and some groups (here
  `es5Strict`) do the same. Promoted errors therefore do not stop `runCompilerPasses`
  (AbstractCommandLineRunner.java:1388-1406): the full compile runs stage 1, stage 2 and
  stage 3, then skips writing JS because `result.success` is false (line 1534); the exit code
  is the error count, capped at 127. The checks run (`--checks_only`) stops after stage 1, and
  its last check, `serializeTypedAst`, has already written the TypedAST.
- **Measured, all 19 pairs:** compile exit = checks exit; compile writes no file; optimize
  exits 0 and writes `out.js`; compile's summary counts equal checks + optimize
  (errors, warnings) in 19 of 19 (for example advanced-16: compile `2 error(s), 1342 warning(s)`
  = checks `2, 1317` + optimize `0, 25`, the 25 being `JSC_RECEIVER_AFFECTED_BY_COLLAPSE` from
  `InlineAndCollapseProperties`); compile stderr ≠ checks stderr in 19 of 19, because stage-2
  warnings are added and check-phase carets are rendered after optimization (D6).
- **Rule.** If the checks response has `exit_code != 0`, the full compile emits **no JS**,
  whether or not a TypedAST was written. For such a pair: the expected exit code and the absence
  of output come from `compile` (equal to the checks exit code in 19 of 19); the expected stderr
  is `compile`'s, which needs the optimizer's diagnostics too (so the Rust back end must still
  run); Rust optimizer JS output may be compared only against `optimize_from_typedast`, never
  against `compile`. In D2 this concerns most `advanced_strict` pairs: 2,340 of the 2,362 in the
  final corpus exit non-zero (every non-zero-exit golden pair has no output file). Each such pair is either D8 (promoted errors, TypedAST written) or halting (no TypedAST);
  the v4 sample of 20 `advanced_strict` pairs split 9 D8 / 11 halting. D8 also occurs under other
  profiles (v3 `advanced-16`, v4 `simple-9`).
- **Halting errors** (an ERROR-level type, e.g. `JSC_PARSE_ERROR`) are different: `PhaseOptimizer`
  stops at the next pass boundary, no TypedAST is written ("no TypedAST" rows), and compile stops
  in stage 1 with the same output as the checks run (v2: 21 of 21).

## Implications for a hybrid pipeline (Java front end + Rust back end)

1. **`compile` out.js is not the expected value for a Rust back end fed the TypedAST.** The
   expected value is `optimize_from_typedast` out.js, run on the same TypedAST with the v3
   seam data (D4, D5). In the v3 sample, compile and seam JS match only where none of these
   known seam exceptions applies:
   - **ws:** not measured; WHITESPACE_ONLY writes no TypedAST (20 of 20).
   - **D6:** `pretty` (number spelling from source text), `sourcemap` (input source maps).
   - **D7:** SIMPLE-level profiles (`simple`, `lang_*`, `pretty`, `sourcemap`):
     `hasRegExpGlobalReferences` (1 of 21 simple pairs in v3, `simple-10`).
   - **D8:** any pair whose checks run exits non-zero (promoted errors): compile writes no JS
     (v3: 18 of 20 advanced_strict, plus advanced-16; v4: 9 of 20 advanced_strict, plus simple-9).

   Measured JS agreement outside those cases (v3 and v4): advanced, chunks2, chunks3 and lang_*
   pairs with a TypedAST and checks exit 0. Per-profile numbers: `seam3_report.py`. Until D7 is carried across the
   seam (see 3), a hybrid pipeline must compare the Rust back end with `optimize_from_typedast`, and record
   compile-vs-seam differences as seam exceptions, not as Rust bugs.
2. **Diagnostics are not seam-clean.** stderr parity with compile needs post-optimization
   node lengths, which the TypedAST does not carry (D6), plus a merge of checks-run and
   optimize-run reports and the `Originally at:` blocks (D3). A hybrid pipeline compares Rust
   optimize-stage stderr against Java `optimize_from_typedast` stderr.
3. **What must cross the seam:** the checks response's `inputs` (order + SourceKind; weak
   inputs go to `weak_inputs`) and `chunks` (membership after dependency management; fill
   files go to `fill_inputs`); `oracle/test/seam3.py:opt_args` is the reference construction.
   Also the compiler-level state the TypedAST does not carry: `hasRegExpGlobalReferences` (D7),
   and the checks run's exit code and diagnostics (D8: with promoted errors, compile writes no JS
   and merges both reports).

## Reproduce

```bash
oracle/build.sh
python3 oracle/test/identity.py --isolate none --n 300     # about 8 minutes, 4 JVMs
python3 oracle/test/seam.py --n 320 --servers 3 --method input_order   # about 7 minutes
python3 oracle/test/seam_report.py build/oracle/seam/v2
python3 oracle/test/seam3.py --per-profile 20 --servers 6   # v3: all 10 profiles, output files compared
python3 oracle/test/seam3_report.py
python3 oracle/test/seam3.py --out build/oracle/seam/v4 --per-profile 20 --servers 3   # v4: golden env, 11 profiles (about 5 minutes)
python3 oracle/test/seam3_report.py build/oracle/seam/v4
python3 oracle/test/identity.py --isolate none --n 300 --cli-par 2 --out build/oracle/identity-v2   # identity v2, golden env
python3 oracle/test/seam_regexp.py   # D7 experiment, about 1 minute
python3 oracle/test/env_check.py     # PROTOCOL.md "Environment": golden_env vs de_DE.UTF-8
```

Both scripts are resumable: per-item result files are skipped if present.
