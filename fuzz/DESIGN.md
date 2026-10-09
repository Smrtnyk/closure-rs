# Fuzzer design (docs/PORTING.md §4.6)

The fuzzer lives in the Cargo workspace rooted at `/Cargo.toml`, together with `crates/*`. Every crate opts into the workspace lints, which include
`unsafe_code = "forbid"` (§6.1). The toolchain is pinned in `rust-toolchain.toml` (1.95.0).

## Crates

| Crate | Kind | Role |
|---|---|---|
| `fuzz/jsgen` | lib + bin `jsgen` | Grammar-based generator. It is seeded and deterministic, and its depth is capped at 64 or less (D-010). |
| `fuzz/mutate` | lib + bin `mutate` | Token-level mutator over D2 inputs (`corpus/d2/cases.jsonl`). |
| `fuzz/minimize` | lib + bin `minimize` | AST delta debugging over `parse_dump` ranges, then line-level ddmin. |
| `fuzz/oracle` (`fuzz-oracle`) | lib | Rust client for the Java oracle server. It enforces the golden environment, and `ArgsHelper` builds argv through `case_args.py`. |
| `fuzz/driver` (`fuzz-driver`) | bin | The differential loop: parse filter, random D2 profile, engine A against engine B, compare, minimize, file. |

Dependency direction: `driver → {jsgen, mutate, minimize, fuzz-oracle}` and `mutate → jsgen`.
`minimize` and `fuzz-oracle` do not depend on each other. The driver connects them through
`minimize::TreeSource` and `minimize::Predicate`.

## Determinism

- `jsgen::generate_nth(seed, index, cfg)` is a pure function. Its only randomness is
  `jsgen::rng::Rng` (SplitMix64, owned by this crate, so no external RNG crate can change the
  stream). It never iterates a hash map. Program `i` does not depend on how many programs are
  generated.
- `mutate::mutate(src, seed, n)` is also pure.
- The driver derives the profile choice for each program from `Rng::fork(seed ^ const, i)`.
  The work queue is parallel, so the *order* of log lines varies, but each program, its
  profile and its argv do not.

## Depth cap (D-010)

`Config::max_depth` is clamped to `1..=jsgen::MAX_DEPTH_CAP` (64); the default is 10.
`Gen::stmt`, `Gen::expr` and `Gen::block` each add one level. At the cap only productions
marked `leaf: true` are eligible, and those never recurse. The reason: results involving
`StackOverflowError` depend on JIT state, so the fuzzer must not produce them.

## Module layout

The dispatch core is `fuzz/jsgen/src/engine.rs`; the productions live in separate modules.

**Core ECMAScript: `fuzz/jsgen/src/lang/`**
- `stmt.rs`: declarations and control flow.
- `expr.rs`: operators, calls and member access.
- `literal.rs`: literals.
- `func.rs`: functions, arrows, generators and async functions.
- `class.rs`: classes.
- New files are allowed. Each file exports `pub static STATEMENTS` and/or `EXPRESSIONS`
  of type `&[Production]`, and `lang/mod.rs` lists them (one line per file).

**Closure idioms and JSDoc: `fuzz/jsgen/src/closure/`**
- `jsdoc.rs`: type expressions.
- `idioms.rs`: annotated declarations, `@enum`, `@constructor`, `@record`, `@typedef`,
  `@nocollapse` and casts.
- Same table convention, registered in `closure/mod.rs`. This module is enabled by
  `Config::closure`.

**Mutation and minimization: `fuzz/mutate/` and `fuzz/minimize/`**
- `mutate`: the lexer, mutation operators and input selection.
- `minimize`: reduction strategies. The public API is `reduce`, `ddmin_lines`, `Tree`,
  `TreeSource` and `Predicate`.

### Production contract

```rust
Production { name, weight, leaf, when: fn(&Ctx) -> bool, emit: fn(&mut Gen) }
```

- `emit` writes text through `g.w`, `g.nl`, `g.expr`, `g.pexpr`, `g.stmt` and `g.block`.
  It declares names with `g.declare(name, Binding::…)` and finds them with
  `g.visible(filter)`.
- Context-sensitive syntax is gated by `when` together with `g.with_ctx(...)`. This covers
  `return`, `break`/`continue`, labels, `yield` and `await`. Function and class bodies reset
  `in_loop`, `in_switch`, `labels`, `in_generator` and `in_async`.
- Subexpressions are emitted with `g.pexpr()` (parenthesized), so precedence never makes a
  program invalid. A production that writes a top-level comma must parenthesize it.
- Every production must keep the Java parse-acceptance rate (Gate 0.3) at 95% or above.
  Check it with `fuzz-driver parse-rate --count 1000`.

## Driver flow (`fuzz-driver run`)

1. **Program.** The source is `jsgen` or `mutate` (`--source gen|mutate|mixed`). The
   category of program `i` comes from a seeded fork. `mixed` is 30% lang (jsgen, closure
   off), 25% closure (jsgen single file), 15% multi (`generate_closure_nth`: 2 to 6 files,
   goog.module, goog.provide, ES modules or CommonJS; the module forms include destructuring `goog.require`/`require`, `goog.module.get`, whole-object and
   whole-value `exports =`/`module.exports =`, `export let`/`var`, `export default`
   function/class declarations, `import {a as b}`, `export {x} from` and side-effect imports,
   guarded by the unit test `module_forms_appear`), 5% sloppy (jsgen sloppy dialect,
   `--strict_mode_input=false` for parse and compile) and 25% mutate (`mutate_with`, whole
   D2 pool as donors). `gen` keeps the four jsgen categories at 30:25:15:5. The
   `unsupported` dialect is never used by the driver.
2. **Parse filter.** The program goes through the oracle's `parse_dump` with CLI default
   options. A program with `errors != []` is rejected and counted.
3. **Profile.** A D2 profile is drawn among those that apply, with the D-016 weights
   (`PROFILE_WEIGHTS_PCT` and `pick_profile_n` in `fuzz/driver`): first a group, the ADVANCED
   family (`advanced`, `advanced_strict`, `chunks2`, `chunks3`) 65%, `ws` 3% and every other
   profile 32%, then a profile uniformly within the group among those that apply. Only
   multi-file programs can get `chunks2`/`chunks3`, so single-file ADVANCED-family draws split
   between `advanced` and `advanced_strict`. The draw comes from `Rng::fork(seed ^ PROFILE_SALT,
   i)`, so it is deterministic per program (unit test `profile_weights_are_d016_and_deterministic`).
   Single-file jsgen programs whose profile has a `--language_out` below ES2018 are generated in
   `low_target` mode (no BigInt, no ES2018+ regexp features); multi-file programs are not (a
   known gap).
   On top of the profile, one program in three (never under `ws`) gets one or two random
   in-scope option flags from `OPTION_POOL` (groups of mutually exclusive alternatives, each
   with a condition such as "chunk profiles only" or "goog programs only"; e.g.
   `--isolate_polyfills`, `--rename_prefix_namespace`, `--assume_function_wrapper`,
   `--angular_pass`, `--chunk_output_type=ES_MODULES`, `--generate_exports`). Flags a profile
   locks are never drawn. The pool comes from `scope/flags.txt` (D-016 item 4, generated by
   `scope/gen_flags.py` from the pinned `CommandLineRunner`): every in-scope flag is either in
   `OPTION_POOL` or in `NOT_DRAWN` with the reason it is not drawn, and
   `check_option_pool` enforces that at driver start-up and in `cargo test`. The drawn flags are
   recorded in the program's `extra_flags` and its origin (`:opts=`); the fuzzer is meant to draw
   random option profiles, and the D2 profiles alone never enable some in-scope passes. The argv
   comes from `gates/lib/case_args.py` through `gates/lib/fuzz_args.py` (the program's
   `extra_flags` go through case precedence), with the files under `<work>/w<k>/in/` and the
   output directory `<work>/w<k>/out`.
4. **Engines.** Engine A is the Java oracle. Engine B is one of:
   - `java`: a second, independent oracle JVM. This proves the comparison path.
   - `java-perturbed`: a synthetic bug. Output files (or stderr) get an extra line when the
     input contains `switch`. This proves the minimize-and-file path.
   - `rust`: the closure-rs CLI (`--rust-bin`, default `$CARGO_TARGET_DIR/release/closure-rs`),
     run with the same argv as engine A, from the repository root, in the golden environment
     with stdin `/dev/null`, as `gates/d2_rust.py` runs it. Its outcome is the exit status
     (128 + signal for a signal; a run past 120 s is killed and filed), stdout, stderr and every
     file under the run's out_dir. Output-file keys of both engines are compared after
     collapsing `//` and `/./`. Each worker then owns one oracle JVM.
   `--wide` adds two draws on top of the D-016 ones (off by default): `--jscomp_off=checkTypes`
   or `--jscomp_warning=checkTypes` for one program in four (never `ws`), and, for single-file
   programs under `simple`, `advanced`, `advanced_strict`, `pretty` or `sourcemap`,
   `--language_out=ECMASCRIPT5` or `ECMASCRIPT_2015` for one in three (generated in `low_target`
   mode). Both are seeded per program (`WIDE_SALT`).
5. **Compare.** The comparison covers the exit code, stdout, stderr and every output file,
   byte for byte (`fuzz_oracle::Outcome::diff`).
6. **Minimize.** A mismatch is minimized with `minimize::reduce`, one file at a time for
   multi-file programs (the budget is split across the files). The tree comes from
   `parse_dump` (`source_offset` and `length` in UTF-16 units, converted to bytes). The
   predicate is "still parses, and A and B still differ in the same way". Then line-level
   ddmin runs, all under a predicate-call budget (`--budget`).
7. **File.** The result is written to `fuzz/findings/<fz-hash>.md`. The file holds the
   profile, argv, the difference, the minimized repro, both outcomes and the original
   program with its origin (seed and index, or the mutated D2 file). Each such file is one
   porting defect to fix (docs/PORTING.md §7).

**Arguments and exit status.** `--work` must lie inside the repository: `case_args.py`
refuses an absolute `out_dir` ("out_dir must be repo-relative"), and the driver passes the
inputs and `out_dir` relative to the repository root. An absolute path that resolves inside the
repository (after `.`/`..` are resolved lexically) is accepted; any other `--work` is refused
before a server starts, with exit 2. `--findings` and `--report` may be anywhere. A run that
cannot compare anything must not look like success, so `run` exits with a status that the
report's `status` field repeats:

| exit | `status` | meaning |
| --- | --- | --- |
| 0 | `ok` | at least one program compared, no harness crash |
| 2 | (no report) | usage error, including a `--work` outside the repository |
| 3 | `harness_crash` | a worker thread panicked (`harness_crashes`) |
| 4 | `oracle_failure` | `ORACLE_FAIL_WINDOW` (50) consecutive programs past the parse filter ended as oracle errors: the run stops early and prints the first error of the streak (`abort_reason`) |
| 5 | `nothing_compared` | the run ended with `compared == 0` |

Java crashes (D-009 drops) and compared programs end an oracle-error streak and parse
rejections do not count either way, so a normal run with a few percent Java crashes or the
odd oracle error is unaffected.

Heap: each driver worker owns 2 oracle JVMs at `-Xmx1536m`. All JVMs start through
`fuzz_oracle::Server::start`, which clears the environment, sets `golden_env()`, and refuses
to use a server whose ready-line `env` differs from `GOLDEN_JVM_ENV`.

## Reach measurement (`gates/lib/fuzz_reach.py`)

Which passes *enter* is set mainly by the options (the input moves it only through errors), so entry counters measure an input poorly.
The script measures **effective** passes instead: passes after which the printed source
changed, taken from `compile_with_pass_dumps` (`Compiler.maybePrintSourceAfterEachPass`
prints only on change). The denominator is every `PassFactory` name in
`DefaultPassConfig`: 139 factories and 138 unique names once the
`processDefines_<mode>` names are expanded and duplicate names are merged.

The subcommands are `denominator`, `d2` (a seeded sample of D2 pairs), `dir` (a
directory of generated programs, each with a profile) and `report`.

Gate 0.3 (b) is gated only on its definition: pass-entry counters
(`fuzz_gate03.py entry`, `--tracer_mode=TIMING_ONLY`) must reach at least 60% of all 138
names on average per program; the union over the programs of the names entered is
reported next to it but gates nothing, because the definition says "on average". Entry depends mainly on the profile distribution of
the driver, which is a fuzzer design choice; ADVANCED-family runs alone enter about 67% of
the names, which is why D-016 weights the profile draw (Driver flow, step 3).

D-016 adds **(b2)**, gated in addition to (b) and not instead of it: the union over the
programs of the passes the effect measure finds effective, counted over runs where Java did not
crash (exit 254, which D-009 drops from every comparison), must cover at least 85% of the
D2-effective set (the passes effective on seeded samples of D2 pairs,
`fuzz_gate03.D2_SAMPLES`). The other effect aggregates (the per-program average, the union
against all 138 names) remain diagnostics, and no relaxed reading of (b) built on them is
proposed.

Two facts matter when reading (b) (Java `SortingErrorManager.hasHaltingErrors`,
`PhaseOptimizer`, `Result.success`):
- Only an error whose diagnostic type is ERROR by default halts the pass loop. A warning promoted
  to an error (`advanced_strict`'s `--jscomp_error=*`) does not: the compile runs every
  optimization pass, then writes no output and exits with the error count.
- So a non-zero exit usually means *more* entered names, not fewer, and the differential
  comparison sees only diagnostics and the exit code of such a run, never its optimized code.
  The Gate 0.3 report therefore gives, as diagnostics that do not change (b), the (b) mean over
  exit-0 runs only and the share of measured runs without comparable output, split into runs
  that reached the end of their profile's pipeline (marker: `latePeepholeOptimizations`; for
  `ws`, the names every exit-0 `ws` run enters) and runs that halted early. On the D-016 data
  the exit-0-only mean is just below the 60% threshold; a type-clean or strict-aware generation
  mode for `advanced_strict` would raise it (a known gap).

The report marks itself STALE when the fuzz/ sources on disk (`*.rs` and `Cargo.toml`) differ
from the ones the results were measured with.

## Commands

```bash
cargo build -j 8 --release
target/release/jsgen --seed 1 --index 0
target/release/fuzz-driver parse-rate --seed 20261006 --count 1000 --servers 3
target/release/fuzz-driver run --seed 1 --count 200 --servers 3 --engine-b java --source mixed
target/release/fuzz-driver run --seed 11 --count 5 --servers 1 --engine-b java-perturbed \
    --findings build/fuzz/selftest-findings
target/release/fuzz-driver minimize-selftest --seed 5 --count 2
target/release/fuzz-driver run --seed 20261006 --duration 3600 --servers 2 --source mixed
target/release/fuzz-driver export --seed 20261006 --count 5000 --source mixed --out build/fuzz/gate/export
gates/gate_0_3.sh   # Gate 0.3: export + parse acceptance, reach, 1-hour run, report
python3 gates/lib/fuzz_reach.py d2 --sample 300 --seed 20261006 --servers 5
python3 gates/lib/fuzz_reach.py report --fuzz-results build/fuzz/reach/dir-20261006.jsonl
```
