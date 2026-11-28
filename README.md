# closure-rs

[![npm](https://img.shields.io/npm/v/closure-rs)](https://www.npmjs.com/package/closure-rs)

closure-rs is a Rust port of [Google Closure Compiler](https://github.com/google/closure-compiler),
the JavaScript optimizer, checker and transpiler. For the same input files and flags it produces
**byte-identical** output, diagnostics, exit code and source maps as the Java compiler, with no JVM.

It is an independent project, **not affiliated with or endorsed by Google**. For the original
compiler, its documentation and support, see [Closure Compiler](https://github.com/google/closure-compiler),
its [documentation](https://developers.google.com/closure/compiler) and the official npm package
[`google-closure-compiler`](https://www.npmjs.com/package/google-closure-compiler).

## Status

closure-rs is stable and matches Closure Compiler release
[`v20261117`](https://github.com/google/closure-compiler/tree/v20261117) (commit
[`143e31c`](https://github.com/google/closure-compiler/tree/143e31c0385a93c8de8bb81c9ada8db16c453b0a)),
published on npm as
[`google-closure-compiler@20261117.0.0`](https://www.npmjs.com/package/google-closure-compiler/v/20261117.0.0).
It follows upstream *releases*, not master commits: each sync moves it to a newer npm release
([Versioning](#versioning)). Its output is deterministic;
[How the port is checked](#how-the-port-is-checked) describes how it is compared with the Java
compiler.

A few flags configure parts of Closure Compiler that are outside the port: coverage instrumentation
(`--instrument_for_coverage_option`, `--instrument_mapping_report`,
`--production_instrumentation_array_name`), the Polymer, Chrome and J2CL passes
(`--polymer_version`, `--chrome_pass`, `--j2cl_pass`, `--remove_j2cl_asserts`) and
`--typed_ast_output_file`, which upstream marks "DO NOT USE". closure-rs refuses to compile when
one of them would take effect: it prints that it does not support the feature and exits with code
255, as for a flag error. This includes J2CL input (files named `*.java.js`), for which the Java
compiler runs the J2CL passes by default; `--j2cl_pass=OFF` compiles it without them. With their
default values these flags work as in the Java compiler. [`scope/flags.txt`](scope/flags.txt) lists
every flag with its scope, and
[docs/PORTING.md §2.1](docs/PORTING.md#21-out-of-scope-features-are-refused) the exact rules.

## Usage

`closure-rs` takes the same command-line flags as the Java compiler:

```bash
closure-rs --compilation_level=ADVANCED --js=src/app.js --js_output_file=dist/app.min.js
```

The npm package `closure-rs` has the programmatic API of the official `google-closure-compiler`
package, TypeScript types included, and carries native binaries for **Linux x64 and arm64**
(statically linked: run on glibc and musl distributions such as Alpine), **macOS arm64 and x64**
and **Windows x64**, so no Java is needed. An existing project switches with an alias in
`package.json`, without code changes:

```json
"devDependencies": {
  "google-closure-compiler": "npm:closure-rs@latest"
}
```

`import ClosureCompiler from 'google-closure-compiler'` and `npx google-closure-compiler ...` then
run closure-rs. The gulp and grunt plugins are not implemented; the
[package README](npm/closure-rs/README.md) describes the API and its differences. On other
systems, [build from source](#building); those builds are not tested by the project.

## Performance

### Java and closure-rs

The table compares the Java reference compiler at `v20261117` with the profile-guided Linux
x64 binary shipped in `closure-rs@20261117.8.0`. Wall time and peak resident set size (RSS)
are medians of five fresh processes per compiler on one GitHub Actions `ubuntu-24.04` runner
(4-vCPU AMD EPYC 7763, Ubuntu 24.04.5 LTS). Java ran on Temurin 21.0.12.1 with
default JVM settings. Speedup is Java wall time divided by closure-rs wall time, calculated
before rounding.

| Project (JavaScript input) | Level | Java | closure-rs | Speedup | Java peak RSS | closure-rs peak RSS |
|---|---|---:|---:|---:|---:|---:|
| ONLYOFFICE spreadsheet SDK 9.2.0.119 (34.9 MB) | ADVANCED | 77.14 s | 16.61 s | 4.6× | 2,567 MiB | 1,684 MiB |
|  | SIMPLE | 56.76 s | 11.17 s | 5.1× | 2,232 MiB | 1,470 MiB |
| three.js r186 (1.3 MB) | ADVANCED | 15.33 s | 1.01 s | 15.2× | 595 MiB | 229 MiB |
|  | SIMPLE | 10.41 s | 0.59 s | 17.7× | 553 MiB | 204 MiB |
| fabric.js 7.4.0 (0.8 MB) | ADVANCED | 12.85 s | 0.74 s | 17.4× | 496 MiB | 183 MiB |
|  | SIMPLE | 8.13 s | 0.38 s | 21.3× | 490 MiB | 144 MiB |
| d3 7.9.0 (0.6 MB) | ADVANCED | 12.65 s | 0.83 s | 15.3× | 562 MiB | 209 MiB |
|  | SIMPLE | 8.40 s | 0.53 s | 15.8× | 530 MiB | 154 MiB |
| lodash 4.17.21 (0.2 MB) | ADVANCED | 8.43 s | 0.32 s | 26.7× | 418 MiB | 149 MiB |
|  | SIMPLE | 5.05 s | 0.18 s | 27.4× | 356 MiB | 112 MiB |

The ONLYOFFICE browser build compiles 394 source files into two dependent ES5 chunks with
source maps and renaming reports. At ADVANCED it took closure-rs 17× as long as three.js.
The four library bundles use ES2015 output and input source maps. All 50 paired runs exited
successfully, with byte-identical JavaScript, source maps and diagnostics.
[Benchmark inputs and reproduction commands](bench/README.md#reproducing-the-readme-tables)

### Change from the previous release

On the same runner, the published Linux x64 binaries for `20261117.8.0` and
`20261117.7.0` compiled all 52 jobs over six rounds, alternating which version ran first.
The percentages are the geometric mean of the per-job median ratios. A negative value means
less time or memory.

| Workload | Jobs | Wall time | CPU time | Peak RSS |
|---|---:|---:|---:|---:|
| SIMPLE | 24 | −4.2% | −3.5% | −7.2% |
| ADVANCED | 24 | −7.9% | −6.7% | −4.7% |
| Flag variants | 4 | −7.4% | −5.0% | 0.0% |
| All jobs | 52 | −6.2% | −5.1% | −5.5% |

The sum of median wall times fell from 54.21 s to 51.14 s (5.7%). Four library bundles compiled
as five dependent chunks dropped 1.5% in SIMPLE and 20.6% in ADVANCED; on ONLYOFFICE, median wall
time dropped 2.2% in SIMPLE and 3.8% in ADVANCED (median paired reductions of 1.9% and 3.9%)
and peak RSS 3.6% and 5.2%. All runs produced identical output.
[Benchmark method and commands](bench/README.md#reproducing-the-readme-tables)

## Versioning

Versions are `<upstream>.<minor>.<patch>`:

- **major:** the upstream release whose output closure-rs matches (`20261117` is
  `google-closure-compiler@20261117.0.0`). A sync to a newer upstream release starts a new major
  version at `.0.0`.
- **minor:** releases that leave the output unchanged: speed, the npm wrapper, new platforms.
- **patch:** fixes: a difference from the Java compiler, a crash, a bug in the npm wrapper.

So a range such as `^20261117.0.0` stays on one upstream release. `latest` is the stable release;
experimental builds are prereleases (for example `20261117.1.0-exp.1`) under the dist-tag `exp`.

## Bugs and issues

- **closure-rs differs from the Java compiler.** Output, diagnostics, exit code or source map that
  differ from `google-closure-compiler` at the version named by closure-rs' major version are a bug
  in closure-rs: [report it](https://github.com/Smrtnyk/closure-rs/issues/new/choose) with the
  closure-rs version, the flags, a small input and what differs. It is fixed in a patch release.
  Crashes and bugs in the npm wrapper are reported the same way.
- **The Java compiler does the same.** Then it is Closure Compiler's behaviour, and closure-rs keeps
  it: report it upstream at <https://github.com/google/closure-compiler/issues>. closure-rs takes the
  fix with the sync to the upstream release that contains it.

## Building

Requires Rust (the toolchain is pinned in `rust-toolchain.toml`):

```bash
cargo build --release --bin closure-rs   # target/release/closure-rs
```

`gates/ci.sh` runs formatting, clippy, the license-header check and the tests. Released binaries are
built profile-guided by `scripts/pgo_build.sh`; the profile changes only the speed, never the output:

```bash
scripts/fetch_bench.sh --project d3-12 --project lodash-es --project three --project fabric-srcmap
python3 scripts/run_bench.py --job '^(d3-12/.*/ADVANCED|lodash-es/.*|three/.*|fabric-srcmap/ADVANCED)$' --write-args pgo-training
scripts/pgo_build.sh --training pgo-training   # needs the rustup component llvm-tools
```

Comparing against the Java compiler needs the pinned Java reference (`scripts/setup_tools.sh`,
`scripts/fetch_reference.sh`, [`oracle/REFERENCE.md`](oracle/REFERENCE.md)) and the corpus and
benchmark inputs (`scripts/fetch_d2.sh`, `scripts/fetch_bench.sh`); the scripts fetch them, the
repository does not store them.

## How the port is checked

closure-rs is a translation of Closure Compiler's Java sources at the pinned release: every Rust
function names the Java method it ports. It is checked against the Java compiler itself:

- **Unit corpus:** Closure Compiler's own Java tests were recorded once from an instrumented Java
  build (inputs, options, expected output and diagnostics). The Rust harness replays every record,
  and every record that applies to the port passes (`corpus/unit/`, `crates/testing`).
- **Differential corpus:** real-world inputs are compiled by both compilers in 11 option profiles
  and compared byte for byte, source maps included; every pair matches (`corpus/d2/`,
  `gates/d2_rust.py`).
- **Real-world inputs:** CI compiles the benchmark projects with the pinned Java compiler and
  the packaged Linux x64 binary on pull requests, pushes to `main` and releases. The
  ONLYOFFICE spreadsheet SDK must match in SIMPLE and ADVANCED, including both chunks,
  their source maps and the renaming reports. The differential fuzzer uses additional inputs
  outside the corpus (`fuzz/`).

[`docs/PORTING.md`](docs/PORTING.md) describes the scope, what byte-identical means, how the port
mirrors the Java code, how fidelity is verified and how the port follows upstream, and
[`crates/DESIGN.md`](crates/DESIGN.md) the Java-to-Rust conventions, including where performance
work leaves the Java structure.

## License

closure-rs is a translation of Closure Compiler and of parts of the Java libraries it runs on, and
keeps their licenses. Each Rust file starts with the license notice of the code it was translated
from and names the upstream files:

- Apache License 2.0 ([LICENSE](LICENSE)): Closure Compiler's code, closure-rs' own code, and the
  ports of Guava, Gson and the Apache Xerces parser that ships in the JDK.
- Mozilla Public License 1.1, or alternatively GPL 2.0 or later: the files ported from Closure
  Compiler's Rhino-derived sources (parts of `com.google.javascript.rhino`).
- GPL 2.0 with the Classpath exception: the ports of OpenJDK 21 classes (`java.lang`, `java.util`,
  regex, number formatting and others the compiler's output depends on).
- BSD 3-Clause (Protocol Buffers for Java), MIT (args4j), the Unicode data license (character
  tables) and David M. Gay's dtoa notice.

SPDX: `Apache-2.0 AND (MPL-1.1 OR GPL-2.0-or-later) AND GPL-2.0-only WITH Classpath-exception-2.0
AND BSD-3-Clause AND MIT AND Unicode-DFS-2016 AND dtoa`. The license texts are in
[LICENSES/](LICENSES/README.md); [NOTICE](NOTICE) has the attributions and which files port what.
The Rust crates linked into the binary keep their own licenses (MIT, Apache-2.0, Unlicense, Zlib),
listed with their full texts in [LICENSES/THIRD_PARTY_RUST.md](LICENSES/THIRD_PARTY_RUST.md).
