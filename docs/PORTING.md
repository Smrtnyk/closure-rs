# Porting closure-rs

This document describes what closure-rs ports, what "byte-identical" means, how the port mirrors
the Java code, how fidelity is verified, and how the port follows upstream. Interpretations and
deviations are recorded in [`DECISIONS.md`](../DECISIONS.md); the Rust conventions are in
[`crates/DESIGN.md`](../crates/DESIGN.md).

## 1. Goal and upstream reference

closure-rs is a Rust implementation of Closure Compiler whose observable behaviour is
**identical** to the Java reference at a pinned commit:

- the same output JS, byte for byte;
- the same diagnostics (type, level, file, line, column, message text);
- the same source maps;
- the same exit codes.

It is a faithful port, not a redesign: it does not "improve" the optimizer or its output. A
difference from the Java compiler at the pinned commit is a porting defect.

**Reference:** `github.com/google/closure-compiler` release `v20261006`, commit
`48f4107ca2aac52149546ccc42894522fcfdb17d` (2026-10-06), the release published on npm as
`google-closure-compiler@20261006.0.0` (registry tag `v20261006` in `scripts/references.tsv`).
`scripts/fetch_reference.sh` fetches it into `reference/closure-compiler-v20261006` (gitignored);
`oracle/REFERENCE.md` describes how the reference jar is built. The pin moves only when the port
is synced with upstream (§9), and only to an upstream release that is published on npm.

For orientation: the reference's `src` has about 278k lines of Java (`jscomp` 180k, `rhino` and
`rhino/jstype` together 30k) and its `test` about 398k lines. The externs and the runtime JS
libraries are data, reused verbatim (`crates/resources`).

## 2. Scope

**In scope:** everything reachable from `CommandLineRunner` with the flags that
[`scope/flags.txt`](../scope/flags.txt) lists as `in`. That file is generated from the pinned
`CommandLineRunner` by `scope/gen_flags.py`, which applies the exclusions below.

**Out of scope:**

- `refactoring/`
- `lint/` (the standalone linter)
- `instrumentation/`
- `ant/`
- `debugger`
- flags whose usage text says "DO NOT USE" or "experimental"
- J2CL-, Polymer- and Chrome-specific passes

A new exclusion needs a decision in `DECISIONS.md`, and only code reachable from fewer than 0.5%
of the D2 corpus runs (§4.4) may be excluded.

## 3. Fidelity criteria

The port is complete when all of the following hold on the same commit:

| ID | Criterion |
|----|-----------|
| D1 | Every record of the unit corpus passes (§4.2). |
| D2 | Every pair of the real-world differential corpus matches Java in every option profile (§4.4). |
| D3 | Inputs the port was never tuned on match (§4.5): originally the D2 holdout; since D-027 the differential fuzzer, real-world bundles outside the corpus and real users' code. |
| D4 | 48 consecutive hours of differential fuzzing find no mismatch (§4.6). |
| D5 | Source maps match for every D2 case that requests one (§4.7). |
| D6 | Determinism: 10 repeated runs, and runs with 1, 4 and 16 threads, all produce identical bytes (§4.7). |
| D7 | Performance: on every benchmark in `bench/`, wall-clock time and peak RSS are at most Java's (`java -jar`, cold start, as users run it) (§4.7). |
| D8 | `cargo clippy -D warnings`, `cargo test` and every invariant of §6 pass; there is no `unsafe` code. |

## 4. Verification

### 4.1 Oracle

`oracle/` is a Java program that uses the reference compiler **as a library**; Closure's own
`src/` is never modified. It runs as a CLI and as a long-running server (to avoid JVM start-up
costs) and provides:

- `compile(options, inputs) → {output, diagnostics, sourcemap, exit_code}`;
- `compile_with_pass_dumps(...)`, the source after every pass (`--print_source_after_each_pass`),
  for bisecting a difference;
- `checks_to_typedast(options, inputs) → .typedast` (`SerializeTypedAstPass`) and
  `optimize_from_typedast(options, typedast) → output`;
- `parse_dump(input) → canonical AST JSON` (node type, children, properties, JSDoc, source
  positions).

Its `compile` output is byte-identical to the reference uberjar's across the D2 corpus
(`gates/gate_0_1.sh`). The protocol is in `oracle/PROTOCOL.md`, the `parse_dump` schema in
`oracle/PARSE_DUMP.md`.

### 4.2 Unit corpus

Closure's Java tests are recorded **at runtime**, not parsed statically:

1. A recording hook is added to `CompilerTestCase`, `IntegrationTestCase` and
   `CompilerTypeTestCase` by a patch in `oracle/patches/` that applies to `test/` only.
2. The Java suite runs with the hook enabled. Every `test(...)`, `testSame(...)`,
   `testError(...)` and `testWarning(...)` call writes one record to
   `corpus/unit/records/<TestClass>.jsonl.gz`. Each record contains:
   - the inputs: externs and sources, including all chunks;
   - the harness flags (`enableNormalize`, `enableTypeCheck`, `enableComputeSideEffects`, the
     language mode, the repetition count, `compareJsDoc`, ...);
   - a reflective dump of the `CompilerOptions` fields that differ from the defaults;
   - the pass under test (the class from `getProcessor()`, plus a dump of its primitive, enum and
     String fields; fields that cannot be represented are listed in `unrepresentable`);
   - the expected result (source or diagnostics) and the comparison mode.
3. `CompilerTestCase` compares **ASTs** (`isEquivalentTo`, `assertNode`), not strings, and so does
   the Rust harness: it parses the expected source and compares structurally, including JSDoc
   when `compareJsDoc` is set.

The records replay green through a JSON-driven Java harness (`gates/gate_0_2.sh`), and the Rust
harness in `crates/testing` replays every record natively (`gates/unit_rust.sh`).
`corpus/unit/FORMAT.md`, `RECORDING.md`, `HARNESS.md`, `REPLAY.md` and `DSL.md` describe the
format, the recording, both harnesses and the per-pass replay parameters.

Tests that exercise Java-internal APIs (`Node`, `JSType`, graph classes, ...) and do not use these
harnesses are not recorded; they are ported as ordinary Rust unit tests next to the code
(`corpus/unit/RUST_UNIT_TESTS.md`).

### 4.3 The corpus is committed

The unit corpus, the D2 case list and the golden metadata are committed to git. Every change to
`corpus/`, `gates/` or `oracle/` is a separate commit that says why.

### 4.4 Real-world differential corpus (D2)

Real-world JS inputs from permissively licensed sources: popular npm packages (ESM and CJS), the
Closure Library, the test262 `test/language` files that the Java parser accepts, and Closure's own
runtime libraries and externs. Whole programs that ship with their own test suites are also
included: for these, the output must both match Java and pass the program's tests.

Each input runs under every option profile that applies to it (`corpus/d2/profiles.json`):

- `WHITESPACE_ONLY`, `SIMPLE`, `ADVANCED`, and `ADVANCED` with `--jscomp_error=*` and type
  checking;
- `--language_out` set to `ECMASCRIPT5`, `ECMASCRIPT_2015` and `ECMASCRIPT_NEXT`;
- `--formatting=PRETTY_PRINT`; `--create_source_map`;
- two chunk configurations (a chain and a fan-out) for cases with two or more inputs.

`gates/d2_rust.py` runs the Rust CLI on every (case, profile) pair and compares exit code, stdout,
stderr and every output file with the golden Java results, byte for byte and without
normalization (`gates/README_d2_rust.md`). `scripts/fetch_d2.sh` reproduces the inputs from
`corpus/d2/sources.lock.json`; `corpus/d2/FORMAT.md`, `PROFILES.md`, `WHOLE_PROGRAM.md` and
`JAVA_FAILURES.md` describe the cases, the profiles and the pairs Java itself cannot compile.

### 4.5 Holdout

A random 15% of the D2 groups was held out of the repository (`corpus/d2/HOLDOUT.md` records its
size and hash; DECISIONS D-002, D-008), to be compared only in aggregate, as a check that the
port is not fitted to the visible corpus. That private holdout was lost before it was ever
evaluated (D-027). Fitting to the visible corpus is checked instead with inputs no porter tuned
against: the differential fuzzer (§4.6, generated and mutated programs), large real-world
bundles outside the corpus (`bench/`: three.js, d3, fabric, lodash, with and without source
maps) and real users' builds. A new holdout, if ever made, must be stored durably outside
the repository, never under a temporary directory.

### 4.6 Fuzzing

`fuzz/` contains a seeded grammar-based JS program generator (its programs must be accepted by
the Java parser), a mutator over D2 inputs, an AST-level minimizer and a driver. Each generated
program is compiled by Java and by Rust under a random option profile; a mismatch is minimized
automatically (delta debugging at the AST level) and kept as a reproduction. `fuzz/DESIGN.md`
describes the crates. `gates/gate_0_3.sh` checks the fuzzer itself: the Java parser accepts its
programs, the programs reach on average at least 60% of the `DefaultPassConfig` passes (pass-entry
counters, averaged per program; D-016 adds a criterion on the passes they make effective), and a
one-hour driver run finishes without harness crashes.

### 4.7 Source maps, determinism and performance

- **Source maps (D5):** the `sourcemap` profile and every chunked case compare the `.map` files
  byte for byte with the D2 runner.
- **Determinism (D6):** the output may not depend on the run or the thread count; iteration order
  is always deterministic (§5, §8).
- **Performance (D7):** `bench/` pins real projects; `scripts/run_bench.py` compiles each with
  both compilers, each compile in a fresh process, and reports wall-clock time and peak RSS. The
  benchmarks also compare both compilers' output byte for byte (`bench/README.md`).

## 5. Structure: the Rust mirrors the Java

- **Structure mirrors Java.** Rust modules follow the Java packages and files, and Rust functions
  keep the Java names (snake_case) and control flow, so every Rust function can be diffed against
  its Java method. Each ported function carries `// port: <JavaClass>#<method>`.
- **AST:** an arena with `NodeId` handles. Each node holds parent, first child, next and previous
  sibling, mirroring Rhino's `Node`. The Node API keeps Java's method names (`get_first_child`,
  `replace_with`, `detach`, `add_child_to_front`, ...), so pass code ports line by line.
- **Types:** `JSType` and friends live in arenas with IDs (`TypeId`, `ScopeId`, `VarId`,
  `ColorId`); class hierarchies become enums or traits.
- **Strings:** JS string values are WTF-16 (lone surrogates survive). Identifiers are interned.
- **Collections:** deterministic iteration only. `IndexMap`/`IndexSet` stand for
  `LinkedHashMap`/`LinkedHashSet`. Where Java output depends on `HashMap`/`HashSet` iteration
  order, Java's exact order is ported (§8).
- **Compiler state:** the Java `Compiler` object is split only as far as the borrow checker
  requires, keeping Java's field and method names.

The port follows the order in which Java runs: core types and the Node API, the parser (checked
with `parse_dump`) and the code printer, the compiler and the unit-test harness, then the passes
in `DefaultPassConfig` order. `crates/DESIGN.md` gives the detailed Java-to-Rust mapping.

## 6. Enforced invariants

`gates/ci.sh` enforces:

1. `#![forbid(unsafe_code)]` in every crate.
2. `cargo fmt --check` and `cargo clippy -D warnings`.
3. `std::collections::HashMap` and `HashSet` are banned in `crates/` through
   `clippy::disallowed_types`. Use `IndexMap`/`IndexSet`, or a Java-order map where output
   depends on it.
4. No `#[ignore]`, skip lists or pass-rate allowlists in tests. A failing test stays failing
   until it is fixed.
5. Every tracked Rust file under `crates/` starts with the license notice of the code it
   translates (`scripts/license_headers.py --check`; `--apply` writes the headers and
   `LICENSES/headers.tsv`). Code translated from OpenJDK (GPL-2.0 with the Classpath exception)
   is kept in files of its own (`*_jdk.rs`), as is code translated from Closure's Rhino-derived
   MPL-1.1 / GPL-2.0-or-later files inside a port of its Apache-2.0 code (`*_rhino.rs`).

## 7. Porting a change

1. Read the Java source being ported, and every Java method it calls that is not ported yet.
2. Port it faithfully, keeping the control flow and the names. Do not redesign or "improve".
3. Run the unit records of the touched pass or class and the relevant D2 profiles. If any fail,
   compare with the Java oracle (per-pass dumps, `parse_dump`) until they match.
4. Run `gates/ci.sh`, and `gates/full.sh` (CI, the D2 runner and the unit-record runner) before
   merging. No unit record or D2 pair that passed before may regress: `gates/unit_ratchet.json`
   and `gates/d2_ratchet.json` hold the passing sets.

## 8. Known exact-match hazards

- **JS strings are UTF-16.** Lone surrogates must survive every step. Lengths, indexing and
  `charCodeAt` folding operate on UTF-16 code units. See `serialization/Wtf8.java` for how Java
  encodes these.
- **Number to string:** `rhino/dtoa` is ported exactly. Numbers are parsed exactly the way Java
  does, including `Double.parseDouble` edge cases on hex, octal and BigInt literals.
- **Regex:** `jscomp/regex` (`RegExpTree`, `CharRanges`, `CaseCanonicalize`) is ported
  faithfully. The `regex` crate is never used to evaluate JS regex semantics.
- **Ordering:** Java's `String.hashCode` is deterministic, so Java output can silently depend on
  the iteration order of a `HashMap` or `HashSet` keyed by `String`. At output-affecting sites
  the port reproduces Java's exact `HashMap` iteration order (hash spreading, table resizing,
  bucket order; `closure_rhino::java_lang::hash_map`).
- **Integer overflow:** Java `int` wraps silently. Use `wrapping_*` operations where the Java
  code depends on wrapping.
- **Exceptions used for control flow** become `Result` values, never panics.
- **Messages:** diagnostic message text, including formatting via `base/format`, must match the
  Java text exactly.
- **Name generation:** `RenameVars`, `RenameProperties` and `CrossChunk*` depend on generator
  state and on the order passes run in. That order comes from `DefaultPassConfig` and
  `PhaseOptimizer` (including loop passes and their convergence rules) and is ported exactly.

## 9. Syncing with upstream

closure-rs follows upstream Closure Compiler: upstream changes are ported, and the pin (§1) moves
forward.

**Syncs target npm releases only.** The pin moves from one upstream release to a later one, and
only to a release that is published on npm as a `google-closure-compiler` version (upstream tag
`vYYYYMMDD` = npm `YYYYMMDD.0.0`), never to an unreleased master commit, so closure-rs always
matches a compiler users can install (DECISIONS.md D-026). A new registry tag is the upstream
release tag (`bb8c8e7`, the first pin, is release `v20261005` = npm `20261005.0.0`).

**The reference registry.** `scripts/references.tsv` lists every pinned reference: per tag its
commit, the uberjar's sha256 (`-` until the jar is built and pinned), and its paths relative to
the main checkout: the checkout (`src`), the recording workspace (`recording_ws`), the uberjar
(`jar`) and the oracle jar (`oracle_jar`). Its `default` row names the reference everything uses.
`scripts/paths.sh`, `paths.py` and `paths.mjs` read the registry of their own checkout (so a
branch decides its default) and resolve `$CLOSURE_RS_REF=<tag>` (default: the `default` row) to
`REF_TAG`, `REF_COMMIT`, `REF_JAR_SHA256`, `REF_GOLDEN_TAG` (`ref-<sha8>`), `REF_SRC`,
`REF_RECORDING_WS`, `REF_JAR` and `ORACLE_JAR`; `bash scripts/paths.sh`, `python3 scripts/paths.py`
and `node scripts/paths.mjs` print them. Every script that fetches, builds, records or runs the
reference takes these values, and the fuzz crates read the same registry (`fuzz/references.rs`).

References live **alongside** each other: a new reference gets its own row and its own paths
(`reference/closure-compiler-<tag>`, `reference/closure-compiler-<tag>-recording`,
`build/reference-<tag>/`, `build/oracle-<tag>/`; the D2 golden results are keyed by the jar's
sha256 anyway), so the old reference's checkout, jars, recording workspace and golden results stay
usable until the sync is finished. `scripts/fetch_reference.sh` refuses to move an existing
checkout to another commit, `scripts/unit_make_recording_ws.sh` only deletes the registry's
recording workspace of the selected tag, and `oracle/build.sh` refuses a reference whose jar
sha256 is not pinned. `gates/d2_rust.py check --rebase` and `scripts/unit_ratchet_rebase.py`
compare a ratchet across the reference change, pair by pair and record by record.

A sync:

1. Adds the new reference to `scripts/references.tsv` (the version notes in `README.md`,
   `oracle/REFERENCE.md` and the license headers' upstream version follow when it becomes the
   default) and fetches it (`CLOSURE_RS_REF=<tag> scripts/fetch_reference.sh`).
2. Builds the new reference jar (`oracle/REFERENCE.md`), pins its sha256 in the registry, builds
   the oracle (`oracle/build.sh`), and re-runs `gates/gate_0_1.sh`.
3. Regenerates what is derived from the reference: `scope/flags.txt` (`scope/gen_flags.py`), the
   npm types (`scripts/gen_npm_types.mjs`), the jar resources
   (`crates/resources/tools/sync_from_jar.py "$REF_JAR" "$REF_SRC"`, which keeps the license
   header of `jar_contents.rs`), the unit corpus (`corpus/unit/RECORDING.md`,
   `scripts/unit_record_all.sh`, `scripts/unit_options_defaults.sh`) and the D2 golden results
   (`gates/lib/golden_all.py`).
4. Ports the Java diff between the old and the new pin (§7), and refreshes the license headers
   (`scripts/license_headers.py --apply`).
5. Verifies the result as in §4: every unit record and D2 pair that matched before still matches
   (`scripts/unit_ratchet_rebase.py`, `gates/d2_rust.py check --rebase`), then moves the
   registry's `default` row to the new tag.
