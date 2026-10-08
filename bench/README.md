# Benchmarks (docs/PORTING.md §3, D7)

D7 requires that **on every benchmark in this directory** the Rust CLI's wall-clock time is
at most Java's and its peak RSS is at most Java's. Java is measured as users run it: a cold
`java -jar closure-compiler.jar ...` per compile (the reference uberjar of the D2 golden
pipeline, `build/reference/closure-compiler.jar`, sha256 `4ef5a893...`, run with the project's
JDK 21 at `tools/jdk-21`, with no extra JVM flags). Each compile is a separate process for both
compilers, so JVM start-up and warm-up count, as they do for users.

The benchmarks also compare the two compilers' results byte for byte (exit code, stdout,
stderr, output file). A difference is a porting defect to report and fix, not a benchmark
failure. This file has no numbers on purpose: results depend on the machine and the commit and
go to `bench/results/` (git-ignored, see `results/README.md`).

## The benchmarks

`projects.json` pins every input: repository, tag, exact commit, license, the sha256 of the
fetched tree (and of the tarball, for fabric) and the jobs. Each job is compiled twice, at
`--compilation_level=SIMPLE` and at `ADVANCED`, always with

```
--language_in=ECMASCRIPT_NEXT --language_out=ECMASCRIPT_2015
--module_resolution=NODE --dependency_mode=PRUNE --entry_point=<entry> --js=<file>... --js_output_file=<out>
```

with the default warning level, so the many type-check warnings that third-party code produces
are part of the output that must match.

| Project | What | Why |
|---|---|---|
| `d3-12` | 12 d3 packages (d3-array, d3-color, d3-format, d3-interpolate, d3-path, d3-shape, d3-time, d3-time-format, d3-scale, d3-selection, d3-hierarchy, d3-geo), **each compiled separately** from its `src/index.js` into one standalone ES2015 file: 12 compiles per level. | Mirrors the primary real workload: a project of 12 independent module directories, each with an `index.js` entry point, of varying sizes, built into 12 standalone ES2015 files by 12 separate compiles. Its total (the 12 compiles summed) is the headline number. The packages are laid out as `bench-cache/d3-12/node_modules/<package>/`; a package's job passes its own sources and those of the d3 packages it imports (and `internmap`, d3-array's dependency), with their `package.json` files, and NODE resolution finds them. |
| `three` | three.js r186, entry `src/Three.js` (about 750 ES modules, 4.6 MB). | One large ES-module graph with heavy JSDoc. |
| `lodash-es` | lodash 4.17.21-es, entry `lodash.js` (about 640 tiny ES modules). | Many small modules: per-file overheads, module rewriting. |
| `fabric` | fabric.js 7.4.0, `dist/index.mjs` (one bundled ES module, about 790 KB). | One large single file. The repository's sources are TypeScript and its `dist/` is not committed, so the release tarball's ES-module build (from registry.npmjs.org, the build of tag `v740`, commit `ce64f450`) is used; only `dist/index.mjs`, `package.json` and `LICENSE` are extracted, nothing is installed or run. |

Inputs and flags that are not the library's own:

- **ADVANCED entry points** (`entries/<job>.js`): a library's entry point exports everything and
  has no side effects, so ADVANCED with the library entry as `--entry_point` removes all of it
  and the output is empty. The ADVANCED jobs therefore use a two-line application entry,
  `import * as lib from "<library entry>"; window["<job>"] = lib;`, which keeps every export,
  as an application's `index.js` that uses the library does. SIMPLE compiles the library's own
  entry point.
- **three.js**: `--externs=bench/externs/webxr.js` declares `XRWebGLLayer`, which Closure's
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
python3 scripts/run_bench.py --job '^three/' --impl java    # only Java
python3 scripts/run_bench.py --job '^fabric/SIMPLE$' --reps 1 --keep-failing --no-save
python3 scripts/run_bench.py --job '^fabric/SIMPLE$' --print-args > /tmp/args  # the argv
```

`fetch_bench.sh` makes shallow, blobless, sparse git fetches of exactly the pinned commits (and
downloads the fabric tarball, checking its sha256 and npm integrity), copies only the listed
paths into `bench-cache/<project>/` and checks each tree's sha256. It is idempotent: verified
trees are left alone, wrong or partial ones are fetched again; `--check` only verifies.

`run_bench.py` derives the repository root from its own location, runs every compile with
cwd = that root (the argv holds repo-relative paths, which appear in warnings), a fixed minimal
environment and stdin `/dev/null`, under `/usr/bin/time` for the peak RSS. Java and the jar are
taken from the data root (this checkout if it has `build/reference/closure-compiler.jar`, else
`$CLOSURE_RS_DATA_ROOT`, else the main checkout of a worktree); `--java`, `--jar` and `--bin`
override them. Each repetition runs Java then Rust; the medians are reported, per job and as
totals per project and level (time summed, RSS maximum). Run it on an otherwise idle machine,
or at least note the load: the result file records the load average at start and end.

## Adding a benchmark

Add a project (sources with `tree_sha256` left empty, jobs) to `projects.json`, run
`scripts/fetch_bench.sh --print-hashes --project <name>` and pin the printed hashes, then check
that Java compiles every job without errors (`run_bench.py --impl java --job ...`). If Java
reports errors, either keep them (they are deterministic and Rust must match them, but then the
job measures nothing after the check that failed) or add the flags or externs a user would add,
and document why here.
