# D2 option profiles

`corpus/d2/profiles.json` maps each profile name used in a case's `profiles` list to an
exact flag list for the pinned `CommandLineRunner` (reference `bb8c8e7`, uberjar sha256
`4ef5a893…`). The argv for a (case, profile) pair is built **only** by
`gates/lib/case_args.py` (`compiler_args()`, `case_profiles()`). The golden runner, the
oracle comparison and every later D2 gate call it. Nothing else may rebuild argv.

Every flag below was checked against
`reference/closure-compiler/src/com/google/javascript/jscomp/CommandLineRunner.java`.
`python3 gates/lib/case_args.py` parses that file's `@Option` declarations and re-checks
every flag used by a profile or a case. It also checks that the `multi_valued_flags` list
exactly matches the `List`-typed options. `golden_all.py` refuses to start if this check
fails.

## Profiles

| Profile | docs/PORTING.md §4.4 | Flags |
|---|---|---|
| `ws` | WHITESPACE_ONLY | `--compilation_level=WHITESPACE_ONLY` |
| `simple` | SIMPLE | `--compilation_level=SIMPLE` |
| `advanced` | ADVANCED | `--compilation_level=ADVANCED` |
| `advanced_strict` | ADVANCED + `--jscomp_error=*` + type checking | `--compilation_level=ADVANCED --warning_level=VERBOSE --jscomp_error=*` |
| `lang_es5` | `--language_out=ECMASCRIPT5` | `--compilation_level=SIMPLE --language_out=ECMASCRIPT5` |
| `lang_es2015` | `--language_out=ECMASCRIPT_2015` | `--compilation_level=SIMPLE --language_out=ECMASCRIPT_2015` |
| `lang_next` | `--language_out=ECMASCRIPT_NEXT` | `--compilation_level=SIMPLE --language_out=ECMASCRIPT_NEXT` |
| `pretty` | `--formatting=PRETTY_PRINT` | `--compilation_level=SIMPLE --formatting=PRETTY_PRINT` |
| `sourcemap` | `--create_source_map` | `--compilation_level=SIMPLE --create_source_map={out_dir}/out.js.map` |
| `chunks2` | chunk configuration 1 (chain) | `--compilation_level=ADVANCED --chunk=c0:K --chunk=c1:N-K:c0 --chunk_output_path_prefix={out_dir}/` |
| `chunks3` | chunk configuration 2 (fan-out) | `--compilation_level=ADVANCED --chunk=c0:K0 --chunk=c1:K1:c0 --chunk=c2:K2:c0 --chunk_output_path_prefix={out_dir}/` |

**The matrix (docs/PORTING.md §4.4).** Each input runs under every option profile: every profile is
auto-applied (`"auto_apply": true`, `profiles.json` `"matrix"`), so `case_profiles(case)` gives
a **candidate** case its own `profiles` list followed by every other profile that applies to it,
in `profiles.json` order. The 9 single-output profiles apply to every case; the 2 chunk
configurations apply to cases with at least 2 `--js` sources (`applies_to: multi_input`). No pair
is omitted by design: the candidate lists only fix the order. A pair leaves the final corpus only
through the drop rules in `FORMAT.md` (Java crash or timeout, trivial ADVANCED).

Notes:
- **Explicit compilation level.** Every profile sets `--compilation_level` explicitly,
  even where it is the default (SIMPLE), so no profile depends on a CLI default.
- **`--jscomp_error=*` is accepted.** `AbstractCommandLineRunner.setWarningGuardOptions`
  expands `*` to every registered diagnostic group except
  `DiagnosticGroups.wildcardExcludedGroups` (`reportUnknownTypes`, `analyzerChecks`,
  `missingSourcesWarnings`, `closureUnawareCodeAnnotationPresent`).
- **Type checking.** ADVANCED already calls `setCheckTypes(true)`
  (`CompilationLevel.applyFullCompilationOptions`). `--warning_level=VERBOSE` turns it on
  explicitly as well (`WarningLevel.addVerboseWarnings`). The `*` guard sets `checkTypes`
  to ERROR, which `Compiler.reconcileOptionsWithGuards` turns into `checkTypes=true`.
- **`lang_next`** equals the CLI default (`languageOut = "ECMASCRIPT_NEXT"`). The flag is
  passed anyway.
- **Language input.** No profile sets `--language_in`, so the default is `STABLE`. Cases
  that need more set it in `extra_flags`; test262 cases use `UNSTABLE`.

## argv layout

```
tools/jdk-21/bin/java <JVM flags> -jar build/reference/closure-compiler.jar \
  <profile flags> <case extra_flags> --externs=<e>... --js=<inputs then shims>... \
  [--chunk=...] <output flag>
```

- **Working directory and paths.** The working directory is the repository root. All
  paths are repo-relative, so the absolute checkout path never reaches the argv or the
  outputs. Input path strings are deduplicated by `findJsFiles` on their absolute paths
  only, and the original string is what gets used.
- **Output directory.** It is `build/golden-tmp/<case-id>/<profile>` (`out_dir_template`),
  a pure function of (case, profile).
  - The output path is observable: it is the `"file"` field of a source map, and it is the
    chunk file prefix. Any comparison (oracle, Rust) must therefore run the identical argv
    from the repository root. Do not substitute another directory.
- **Output flag:**
  - Single-output profiles end with `--js_output_file={out_dir}/out.js`.
  - `sourcemap` also writes `{out_dir}/out.js.map`.
  - `chunks2` writes `{out_dir}/c0.js` and `{out_dir}/c1.js`; `chunks3` also writes
    `{out_dir}/c2.js` (an empty chunk is written as a 1-byte file).
  - stdout is therefore normally empty.
- **Flag syntax.** Flags use the `--name=value` form. `CommandLineRunner.processArgs` splits
  it, and strips one level of surrounding quotes from the value.
- **Globs.** Paths containing `*` or starting with `!` would be treated as globs. No
  corpus path does, and `verify()` would fail on a missing path at run time.

## Precedence between profile flags and case `extra_flags`

1. **Order.** Profile flags come first, then the case's `extra_flags` in their listed
   order.
2. **Single-valued flags: the case wins.** A single-valued flag is any option whose
   `CommandLineRunner` field is not a `List`. If the case sets the same key as the
   profile, the profile's occurrence is **removed**, so every single-valued key appears
   exactly once in argv. args4j would also let the last occurrence win, but a duplicate
   key would make argv ambiguous to read and to port.
3. **Locked keys.** Each profile marks the keys that define it as `locked`. For example,
   `--compilation_level` is locked everywhere, and `--language_out` is locked for
   `lang_*`. If a case sets a locked key, the pair is invalid: `compiler_args()` raises
   `CaseProfileError`, and `verify()` reports it. **No current case does this.**
4. **Multi-valued flags accumulate,** profile values first. These are `--jscomp_error`,
   `--jscomp_warning`, `--jscomp_off`, `--entry_point`, `--formatting`, `--define`,
   `--chunk` and the rest of `multi_valued_flags`. The order of the warning-guard flags
   matters, and it is preserved.
5. **Runner-owned flags.** These are `--js`, `--externs`, `--js_output_file`, `--chunk`,
   `--chunk_output_path_prefix`, `--create_source_map`, `--flagfile` and
   `--json_streams`. They must never appear in a case's `extra_flags`. Inputs, externs and
   shims come from the case's own fields.

Current overlaps are all non-conflicting. test262's `--language_in=UNSTABLE` and
`--strict_mode_input=false`, closure-self's `--env=CUSTOM`, and the npm, whole-program and
Closure Library `--dependency_mode`, `--entry_point`, `--module_resolution` and
`--process_common_js_modules` flags are not set by any profile.

## Chunk profiles: `chunks2` and `chunks3`

- **Which cases.** Both apply only to multi-input cases: `len(inputs) + len(shims) >= 2`.
  They are **auto-applied** like every profile (see "The matrix"). For a final `cases.jsonl`
  case (`case_args.is_final_case`), `case_profiles()` returns the explicit `profiles` list
  unchanged, so pairs dropped for Java crashes are never re-added. No candidate case lists a
  chunk profile itself.
- **Split, `chunks2` (`ceil_half`, a chain).** The `--js` list (inputs, then shims) is split in
  order. Chunk `c0` gets the first `ceil(N/2)` files; `c1` gets the rest and depends on `c0`.
- **Split, `chunks3` (`fanout_thirds`, a fan-out graph).** `c0` gets the first
  `K0 = ceil(N/3)` files, `c1` the next `K1 = ceil((N-K0)/2)` and depends on `c0`, `c2` the
  remaining `K2 = N-K0-K1` and also depends only on `c0`. `c1` and `c2` are siblings, so code
  shared by both can only move to `c0`, which a chain cannot exercise. For `N = 2`, `c2` gets 0
  files: `JsChunkSpec.create` allows that ("We will allow chunks of zero input",
  AbstractCommandLineRunner.java:3177), and the reference writes `c2.js` as an empty chunk.
- **Compilation level.** It is ADVANCED, so that `CrossChunkCodeMotion`,
  `CrossChunkMethodMotion` and cross-chunk renaming are exercised. Those are
  ADVANCED-only, and docs/PORTING.md §8 calls them out.
- **With `--dependency_mode=PRUNE`.** `JSChunkGraph.manageDependencies` moves each input
  to the deepest chunk that needs it. When `c0` has no entry point, most files end up in
  `c1`. That is still the reference behaviour, and it is recorded as such.
- **Two configurations.** docs/PORTING.md §4.4 has 2 chunk configurations: `chunks2` (chain) and
  `chunks3` (fan-out).

## Golden results

`gates/lib/run_reference.py` writes one file per pair:
`corpus-cache/d2/_golden/ref-4ef5a893/<case-id>/<profile>.json`.

**Fields:**
- `args`: the exact full argv. `compiler_args` is the part after the jar.
- `exit_code`, `timed_out`, `stdout`, `stderr`.
- `outputs`: `{relative file name in out_dir: content}`.
- `wall_ms`, `peak_rss_kb`, `cpu_ms`, `env`, `out_dir`, `runner_version`.

**`peak_rss_kb` caveat (runner_version 1).** Linux stores the *old* address space's
high-water RSS into a process's maxrss at `execve()`, and CPython spawns children with
vfork. A JVM started straight from the driver therefore reported
max(driver peak RSS, JVM peak RSS). In the first full run, 16,260 of 17,970 results carry
the driver's 1,345,772 kB. Runner version 2 starts the JVM through
`gates/lib/rss_spawn.py`, a small fork+exec wrapper that leaves argv, env, cwd and stdio
unchanged, so `peak_rss_kb` is the JVM's own peak. `golden_all.py` ignores v1 RSS values.
The compiler outputs of v1 results are unaffected. `wall_ms` was measured on a heavily
loaded host (load average 70–100), so use it for scheduling only, not for D7.

Text that is not valid UTF-8 is stored as `{"base64": …}`.

**JVM flags.** They do not change compiler behaviour:
- `-Xmx3g`
- `-XX:+UseSerialGC`
- `-XX:TieredStopAtLevel=1`
- `-XX:-UsePerfData`
- `-Xlog:disable -Xlog:all=off`, which keeps JVM logging out of stdout
- `-Xshare:auto -XX:SharedArchiveFile=build/golden-tmp/cds.jsa`. This is a dynamic AppCDS
  archive created once by a throwaway compile.

**Environment.** The child process gets a fixed minimal environment: `PATH`, `HOME`,
`LANG`/`LC_ALL=C.UTF-8` and `TZ=UTC`. Nothing else is inherited.

**Timeout.** Each run has a 180 s timeout. A timed-out run is recorded with
`timed_out: true`.
