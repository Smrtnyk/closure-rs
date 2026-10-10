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
[`v20261006`](https://github.com/google/closure-compiler/tree/v20261006) (commit
[`48f4107`](https://github.com/google/closure-compiler/tree/48f4107ca2aac52149546ccc42894522fcfdb17d)),
published on npm as
[`google-closure-compiler@20261006.0.0`](https://www.npmjs.com/package/google-closure-compiler/v/20261006.0.0).
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
every flag with its scope, and [DECISIONS.md D-028](DECISIONS.md) the exact rules.

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

Wall-clock time and peak memory of one compile. Both compilers produced byte-identical output and
source maps in every run.

| Bundle | Level | Java | closure-rs | Speedup | Java memory | closure-rs memory |
|---|---|---:|---:|---:|---:|---:|
| three.js r186 (1.3 MB) | ADVANCED | 6.98 s | 1.82 s | 3.8× | 697 MB | 346 MB |
| | SIMPLE | 4.47 s | 1.11 s | 4.0× | 627 MB | 312 MB |
| fabric.js 7.4.0 (0.8 MB) | ADVANCED | 5.77 s | 1.36 s | 4.2× | 665 MB | 288 MB |
| | SIMPLE | 3.28 s | 0.67 s | 4.9× | 566 MB | 253 MB |
| d3 7.9.0 (0.6 MB) | ADVANCED | 5.64 s | 1.40 s | 4.0× | 782 MB | 292 MB |
| | SIMPLE | 3.82 s | 0.86 s | 4.4× | 849 MB | 273 MB |
| lodash 4.17.21 (0.2 MB) | ADVANCED | 3.65 s | 0.64 s | 5.7× | 568 MB | 243 MB |
| | SIMPLE | 2.22 s | 0.33 s | 6.7× | 522 MB | 227 MB |

- **Inputs:** the bundle files (for three.js `three.core.js` and `three.module.js`) with their input
  source maps (`--source_map_input`), `--create_source_map`, `--source_map_include_content`,
  `--language_out=ECMASCRIPT_2015`.
- **Java:** the `google-closure-compiler` 20261006.0.0 jar on OpenJDK 21: `java -jar`, default JVM
  settings, a new JVM per compile, as the npm package's API runs it.
- **closure-rs:** 20261006.0.0, the profile-guided Linux x64 release binary.
- **Machine:** AMD Ryzen 9 9950X (16 cores, 32 threads), 62 GB, Fedora Linux 44; other processes
  used about a quarter of the threads during the runs. Medians of 5 runs; memory is peak RSS.

`scripts/run_bench.py` runs these benchmarks ([bench/README.md](bench/README.md)).

## Versioning

Versions are `<upstream>.<minor>.<patch>`:

- **major:** the upstream release whose output closure-rs matches (`20261006` is
  `google-closure-compiler@20261006.0.0`). A sync to a newer upstream release starts a new major
  version at `.0.0`.
- **minor:** releases that leave the output unchanged: speed, the npm wrapper, new platforms.
- **patch:** fixes: a difference from the Java compiler, a crash, a bug in the npm wrapper.

So a range such as `^20261006.0.0` stays on one upstream release. `latest` is the stable release;
experimental builds are prereleases (for example `20261006.1.0-exp.1`) under the dist-tag `exp`.

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
scripts/fetch_bench.sh --project d3-12 --project lodash-es --project three
python3 scripts/run_bench.py --job '^(d3-12/.*/ADVANCED|lodash-es/.*|three/.*)$' --write-args pgo-training
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
- **Unseen inputs:** a differential fuzzer and large real-world bundles outside the corpus
  (`fuzz/`, `bench/`) compare the two compilers on inputs the port was never tuned on.

[`docs/PORTING.md`](docs/PORTING.md) describes the scope, what byte-identical means, how the port
mirrors the Java code, how fidelity is verified and how the port follows upstream.
[`DECISIONS.md`](DECISIONS.md) records the interpretations and design decisions, and
[`crates/DESIGN.md`](crates/DESIGN.md) the Java-to-Rust conventions.

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
