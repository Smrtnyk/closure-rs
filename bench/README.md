# Benchmarks (docs/PORTING.md §3, D7)

D7 requires that **on every benchmark in this directory** the Rust CLI's wall-clock time is
at most Java's and its peak RSS is at most Java's. Java is measured as users run it: a cold
`java -jar closure-compiler.jar ...` per compile (the reference uberjar of the D2 golden
pipeline: `$REF_JAR`, for the default reference `build/reference-v20261117/closure-compiler.jar`,
sha256 `0081162b...`, see docs/PORTING.md §9; run with the project's JDK 21 at `tools/jdk-21`,
with no extra JVM flags). Each compile is a separate process for both compilers, so JVM start-up
and warm-up count, as they do for users.

The benchmarks also compare the two compilers' results byte for byte (exit code, stdout,
stderr, output file, and the source map where a job writes one). A difference is a porting defect to report and fix, not a benchmark
failure. This file has no numbers on purpose: results depend on the machine and the commit and
go to `bench/results/` (git-ignored, see `results/README.md`).

## The benchmarks

`projects.json` pins each input's repository, tag, commit, license and tree hash. The suite
compiles each job at SIMPLE and ADVANCED. The library and bundle jobs use the following flags
and the default warning level.

```
--language_in=ECMASCRIPT_NEXT --language_out=ECMASCRIPT_2015
--module_resolution=NODE --dependency_mode=PRUNE --entry_point=<entry> --js=<file>... --js_output_file=<out>
```

The ONLYOFFICE spreadsheet job uses the editor's ES5 chunk build. Diagnostics are compared
at the warning level configured for each job.

| Project | What | Why |
|---|---|---|
| `onlyoffice-cell` | ONLYOFFICE spreadsheet editor 9.2.0.119, 394 JavaScript files (34.93 MB) and seven extern files. | Exercises optimization across two dependent chunks of a production application, including the formula engine, layout and shared editor code. |
| `d3-12` | 12 d3 packages (d3-array, d3-color, d3-format, d3-interpolate, d3-path, d3-shape, d3-time, d3-time-format, d3-scale, d3-selection, d3-hierarchy, d3-geo), **each compiled separately** from its `src/index.js` into one standalone ES2015 file: 12 compiles per level. | Mirrors the primary real workload: a project of 12 independent module directories, each with an `index.js` entry point, of varying sizes, built into 12 standalone ES2015 files by 12 separate compiles. Its result is the total of the 12 compiles. The packages are laid out as `bench-cache/d3-12/node_modules/<package>/`; a package's job passes its own sources and those of the d3 packages it imports (and `internmap`, d3-array's dependency), with their `package.json` files, and NODE resolution finds them. |
| `three` | three.js r186, entry `src/Three.js` (about 750 ES modules, 4.6 MB). | One large ES-module graph with heavy JSDoc. |
| `lodash-es` | lodash 4.17.21-es, entry `lodash.js` (about 640 tiny ES modules). | Many small modules: per-file overheads, module rewriting. |
| `fabric` | fabric.js 7.4.0, `dist/index.mjs` (one bundled ES module, about 790 KB). | One large single file. The repository's sources are TypeScript and its `dist/` is not committed, so the release tarball's ES-module build (from registry.npmjs.org, the build of tag `v740`, commit `ce64f450`) is used; only `dist/index.mjs`, `package.json` and `LICENSE` are extracted, nothing is installed or run. |
| `three-bundle` | three.js r186, `build/three.module.js` (the release's pre-bundled ES module, about 660 KB, which imports `build/three.core.js`, about 1.46 MB), as committed at the pinned tag. | Pre-bundled single-file inputs, compiled the way a project compiles its own bundles: one compile per bundle, with the time spent in the passes walking one huge script rather than in per-file work. |
| `d3-bundle` | d3 7.9.0, `dist/d3.js` (the umbrella package's single-file UMD bundle, about 590 KB) from the npm release tarball (the build of tag `v7.9.0`, commit `1f8dd3b9`). | As `three-bundle`; a script (UMD), not a module. |
| `lodash-bundle` | lodash 4.17.21, `lodash.js` (the monolithic single-file build, about 540 KB) from the npm release tarball (commit `c6e281b8`). | As `three-bundle`; a script (an IIFE). |
| `fabric-srcmap` | `fabric`'s `dist/index.mjs` with the tarball's `dist/index.mjs.map` (rollup's map, with `sourcesContent`) as the input source map. | A bundle compiled with its bundler's source map, as projects ship them: adds `--create_source_map=%outname%.map --source_map_include_content --source_map_input=<bundle>\|<bundle>.map`, so parsing the input map, mapping every node back through it and writing the output map with all sources are measured (and the map compared byte for byte). |
| `three-srcmap`, `d3-bundle-srcmap`, `lodash-bundle-srcmap` | The `three-bundle`, `d3-bundle` and `lodash-bundle` files, each re-emitted with a source map back to the original (see below). | As `fabric-srcmap`, for bundles that ship no map. |
| `bundles-chunks` | The bundles of `three-bundle`, `fabric-srcmap`, `d3-bundle` and `lodash-bundle` in one compile of five dependent chunks: `three.core.js` is the chunk the other four depend on. ES2021 output, a source map per chunk, fabric's own map as an input source map. | The code-split output of a bundler compiled as chunks: the passes that move code between chunks run, and every chunk and source map is compared. |

### Workload details

- **ONLYOFFICE spreadsheet editor.** The browser SDK build takes 38 files for `sdk-all-min`
  and 356 for `sdk-all`, in the order specified by `configs/cell.json`. The benchmark uses
  the editor's externs, defines, ES5 output, quiet diagnostics and browser wrapper. The
  checkout includes the input map referenced by the Socket.IO extern. The upstream `--map`
  option writes a source map with embedded sources for each chunk and a variable renaming
  report. ADVANCED also writes a property renaming report. Both chunks, their maps and the
  reports are compared byte for byte. ADVANCED, the upstream default, emits 10.9 MB of
  JavaScript and 88.4 MB of source maps. The benchmark omits the date-dependent packaging
  banner. [Upstream build](https://github.com/ONLYOFFICE/sdkjs/blob/d3a855ac3507629dbd4b5506aafc3213b8715d2f/build/Gruntfile.js#L264-L291) · [Ordered inputs](https://github.com/ONLYOFFICE/sdkjs/blob/d3a855ac3507629dbd4b5506aafc3213b8715d2f/configs/cell.json)

- **Generated bundles with maps** (`three-srcmap`, `d3-bundle-srcmap`, `lodash-bundle-srcmap`,
  kind `closure-map` in `projects.json`): `scripts/fetch_bench.sh` makes them from the fetched
  files with the pinned Java reference compiler (no downloaded tool runs): each input is
  re-emitted by `--compilation_level=WHITESPACE_ONLY --formatting=PRETTY_PRINT
  --language_out=NO_TRANSPILE --jscomp_off=moduleLoad --create_source_map=<out>.map
  --source_map_include_content`, which drops the comments as a bundler does and writes a
  token-level map with the original as `sourcesContent`. The output is deterministic and its
  tree hash is pinned.

- **ADVANCED entry points** (`entries/<job>.js`): a library's entry point exports everything and
  has no side effects, so ADVANCED with the library entry as `--entry_point` removes all of it
  and the output is empty. The ADVANCED jobs therefore use a two-line application entry,
  `import * as lib from "<library entry>"; window["<job>"] = lib;`, which keeps every export,
  as an application's `index.js` that uses the library does. SIMPLE compiles the library's own
  entry point.
- **Self-contained scripts** (`d3-bundle`, `lodash-bundle`): the bundle publishes the library
  itself (a UMD wrapper assigning to the global object, an IIFE assigning `_`), so ADVANCED
  keeps it without an application entry, and both levels compile the bundle as the only input
  and entry point.
- **three.js** (`three` and `three-bundle`): `--externs=bench/externs/webxr.js` declares `XRWebGLLayer`, which Closure's
  default externs lack (ADVANCED stops at `JSC_UNDEFINED_VARIABLE`, an error, without it), and
  `--jscomp_off=visibility`, because three.js marks exported module functions `@private`, which
  ADVANCED reports as `JSC_BAD_PRIVATE_GLOBAL_ACCESS` errors. With both, every job of every
  project compiles without errors in Java (warnings remain and must match).

## Running

```sh
scripts/fetch_bench.sh                      # fetch and verify all projects into bench-cache/
cargo build --release -p closure-cli        # target/release/closure-rs
python3 scripts/run_bench.py                # all jobs, 3 repetitions, medians
python3 scripts/run_bench.py --project d3-12 --level ADVANCED --reps 5
python3 scripts/run_bench.py --project onlyoffice-cell --level ADVANCED --reps 5 --fail-on-mismatch
python3 scripts/run_bench.py --job '^three/' --impl java    # only Java
python3 scripts/run_bench.py --job '^fabric/SIMPLE$' --reps 1 --keep-failing --no-save
python3 scripts/run_bench.py --job '^fabric/SIMPLE$' --print-args  # the argv, one argument per line
python3 scripts/run_bench.py --reps 1 --variants --fail-on-mismatch  # identity check, as CI runs it
```

`fetch_bench.sh` makes shallow, blobless, sparse git fetches of exactly the pinned commits (and
downloads the fabric tarball, checking its sha256 and npm integrity), copies only the listed
paths into `bench-cache/<project>/` and checks each tree's sha256. It is idempotent: verified
trees are left alone, wrong or partial ones are fetched again; `--check` only verifies.

`run_bench.py` derives the repository root from its own location, runs every compile with
cwd = that root (the argv holds repo-relative paths, which appear in warnings), a fixed minimal
environment and stdin `/dev/null`, under `/usr/bin/time` for the peak RSS. Java and the jar are
taken from the data root (`--data-root`, else `$CLOSURE_RS_DATA_ROOT`, else this checkout if it
has the reference jar, else the main checkout of a worktree); `--java`, `--jar` and `--bin`
override them. Each repetition runs Java then Rust; the medians are reported, per job and as
totals per project and level (time summed, RSS maximum). Run it on an otherwise idle machine,
or at least note the load: the result file records the load average at start and end.

`--variants` adds the extra compiles of `scripts/platform_outputs.py` (its `VARIANTS` on the jobs
they name: ES5 output, pretty print with the sources in the source map, an IIFE wrapper, each
with a source map; and its `--json_streams=BOTH` compile), reported at the level `VARIANT`.
`--fail-on-mismatch` makes a difference between the compilers (or between repetitions) exit 1.
Pull requests, pushes to `main` and release runs compare the packaged Linux x64 binary with
the pinned Java reference on every job and variant. For ONLYOFFICE, both SIMPLE and ADVANCED
must compile successfully and match byte for byte. The five platform binaries also compile
both modes and compare their chunks, maps and renaming reports with one another.

`.github/workflows/java-comparison.yml` verifies the reference jar's SHA-256, fetches the
pinned inputs and puts the table in the run summary. On a mismatch it uploads the differing
outputs. A standalone run builds its own Rust release binary.

`--print-args` prints each selected job's argv, one argument per line, and exits; `--write-args
DIR` writes it to `DIR/<job id>.args` instead (`/` in the id becomes `-`). The profile-guided
release build (`scripts/pgo_build.sh --training DIR`) runs those files as its training compiles.

### Comparing two closure-rs builds

```sh
python3 scripts/bench_ab.py --base OLD/closure-rs --head NEW/closure-rs --variants   # 5 rounds
python3 scripts/bench_ab.py --report build/bench/ab                                  # reprint
```

`bench_ab.py` answers "is the new build slower?" without an idle machine. It runs the selected
jobs with `run_bench.py --impl rust --reps 1`, alternating the two binaries over several rounds
(base, head; head, base; ...), and reports head relative to base: the geometric mean over the
jobs of the wall time, CPU time and peak RSS ratios, from the medians and within each round
alone, plus the change per project and level. The spread of the per-round ratios is the noise of
that machine; a change smaller than it cannot be told apart from noise. Separate runs, such as two
CI runs on different runners, differ by 10-30% for the same binary, so they compare poorly.

Run by hand, `java-comparison.yml` does this on one GitHub runner: it builds the closure-rs of the
`ab_base` input (default `main`) next to the run's own and puts the report in the run summary
(input `ab_rounds`, default 5; an empty `ab_base` skips it). Both are plain release builds, not
profile-guided.

## Reproducing the README tables

Run `.github/workflows/benchmark-release.yml` via workflow dispatch (inputs `current_version`,
`prev_version`, `java_reps`, `ab_rounds`), or run the same commands locally from the repository
root on Linux x64 with the pinned JDK and reference jar installed:

```sh
scripts/fetch_bench.sh
npm install --prefix build/bench/current --no-save --ignore-scripts closure-rs@20261117.7.0
python3 scripts/run_bench.py --bin build/bench/current/node_modules/closure-rs/bin/linux-x64/closure-rs \
  --project onlyoffice-cell --project three-srcmap --project fabric-srcmap \
  --project d3-bundle-srcmap --project lodash-bundle-srcmap --reps 5 --fail-on-mismatch
```

For the release comparison, install `20261117.6.0` as the baseline and compare against `20261117.7.0` above.

```sh
npm install --prefix build/bench/prev --no-save --ignore-scripts closure-rs@20261117.6.0
python3 scripts/bench_ab.py --base build/bench/prev/node_modules/closure-rs/bin/linux-x64/closure-rs \
  --head build/bench/current/node_modules/closure-rs/bin/linux-x64/closure-rs --variants --rounds 6
```

## Adding a benchmark

Add a project (sources with `tree_sha256` left empty, jobs) to `projects.json`, run
`scripts/fetch_bench.sh --print-hashes --project <name>` and pin the printed hashes, then check
that Java compiles every job without errors (`run_bench.py --impl java --job ...`). If Java
reports errors, either keep them (they are deterministic and Rust must match them, but then the
job measures nothing after the check that failed) or add the flags or externs a user would add,
and document why here.
