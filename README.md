# closure-rs

[![npm](https://img.shields.io/npm/v/closure-rs)](https://www.npmjs.com/package/closure-rs)

closure-rs is a Rust port of [Google Closure Compiler](https://github.com/google/closure-compiler),
the JavaScript optimizer, checker and transpiler. Its goal is **byte-identical behaviour**: for the
same input files and flags, `closure-rs` produces exactly the same output, diagnostics, exit code and
source maps as the Java compiler, without needing a JVM.

**Upstream version:** closure-rs matches the npm package
[`google-closure-compiler@20261006.0.0`](https://www.npmjs.com/package/google-closure-compiler/v/20261006.0.0),
that is [Closure Compiler](https://github.com/google/closure-compiler) release
[`v20261006`](https://github.com/google/closure-compiler/tree/v20261006) (commit
[`48f4107ca2aac52149546ccc42894522fcfdb17d`](https://github.com/google/closure-compiler/tree/48f4107ca2aac52149546ccc42894522fcfdb17d)),
and is meant to stay byte-identical to it. closure-rs follows upstream *releases*, the versions
published on npm, not upstream master commits: each sync moves it to a newer npm release.

It is a translation of Closure Compiler's Java sources at that release. Every Rust function names
the Java method it ports, and the Java test suites are replayed against the port. This is an
independent project, **not affiliated with or endorsed by
Google**; for the original compiler, its documentation and support, see:

- Closure Compiler: <https://github.com/google/closure-compiler>
- Documentation: <https://developers.google.com/closure/compiler>
- The official npm package: [`google-closure-compiler`](https://www.npmjs.com/package/google-closure-compiler)

## Status

closure-rs is stable for the pinned upstream release. For the same input files and flags it
produces the same output, diagnostics, exit code and source maps as that Java compiler, and the
same bytes on every run. This is checked against the Java compiler itself (see
[How the port is checked](#how-the-port-is-checked)):

- every replayable record of Closure Compiler's own test suites that applies to the port passes;
- every input and option profile of the real-world differential corpus matches the Java compiler
  byte for byte, source maps included;
- a differential fuzzer and real-world bundles outside the corpus compare the two compilers on
  inputs the port was never tuned on.

A few flags configure parts of Closure Compiler that are outside the port: coverage
instrumentation (`--instrument_for_coverage_option`, `--instrument_mapping_report`,
`--production_instrumentation_array_name`), the Polymer, Chrome and J2CL passes
(`--polymer_version`, `--chrome_pass`, `--j2cl_pass`, `--remove_j2cl_asserts`) and
`--typed_ast_output_file`, which upstream marks "DO NOT USE". With these flags the output is not
guaranteed to match. [`scope/flags.txt`](scope/flags.txt) lists every flag with its scope.

**Platforms:** the npm package ships native binaries for Linux x64 (statically linked; runs on
glibc and musl distributions such as Alpine) and Windows x64. On other systems, build from source
([Building](#building)); those builds are not tested by the project.

## Performance

Wall-clock time and peak memory of one compile, against the Java compiler. Output and source maps
of both compilers were byte-identical in every run.

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

Conditions:

- **Inputs:** the bundle files (for three.js `three.core.js` and `three.module.js`) with their
  input source maps (`--source_map_input`), `--create_source_map`, `--source_map_include_content`,
  `--language_out=ECMASCRIPT_2015`.
- **Java:** the `google-closure-compiler` 20261006.0.0 jar on OpenJDK 21, `java -jar` with default
  JVM settings and a new JVM per compile, as the npm package's API runs it.
- **closure-rs:** 20261006.0.0, the profile-guided Linux x64 release binary.
- **Machine:** AMD Ryzen 9 9950X (16 cores, 32 threads), 62 GB, Fedora Linux 44; other processes
  used about a quarter of the threads during the runs.
- **Values:** medians of 5 runs; memory is the peak resident set size.

`scripts/run_bench.py` runs these benchmarks (see [bench/README.md](bench/README.md)).

## Usage

`closure-rs` accepts the same command-line flags as the Java compiler's `CommandLineRunner`:

```bash
closure-rs --compilation_level=ADVANCED --js=src/app.js --js_output_file=dist/app.min.js
```

### npm

The npm package `closure-rs` (source in [`npm/closure-rs/`](npm/closure-rs/README.md)) wraps the
native binary in the programmatic API of the official `google-closure-compiler` package, with
TypeScript types. The package carries the native binaries of the supported platforms (see
[Status](#status)) and uses the one for your system, so no Java is needed. An existing project can
switch with an npm alias in `package.json`, without changing its code:

```json
"devDependencies": {
  "google-closure-compiler": "npm:closure-rs@latest"
}
```

`import ClosureCompiler from 'google-closure-compiler'` and `npx google-closure-compiler ...` then
run closure-rs. The gulp and grunt plugins of the official package are not implemented; see the
package's README for the API and its differences. The package is built and tested by
`.github/workflows/release.yml`; publishing is a separate, manually approved run of that workflow,
with npm provenance.

## Versioning

Releases are numbered `<upstream>.<minor>.<patch>`:

- The **major** version is the upstream Closure Compiler release whose output closure-rs matches:
  `20261006` is `google-closure-compiler@20261006.0.0`, Closure Compiler `v20261006`.
- The **minor** version counts closure-rs releases on that upstream release that leave the output
  unchanged: speed, the npm wrapper, new platforms. A minor release resets the patch version to 0.
- The **patch** version counts fixes: output that differed from the Java compiler and now matches
  it, a crash, a bug in the npm wrapper.
- A sync to a newer upstream release starts a new major version at `.0.0`.

Within one major version the output only changes where it did not match the Java compiler, so a
range such as `^20261006.0.0` stays on one upstream release. Experimental builds are published as
prereleases (for example `20261006.1.0-exp.1`) under the npm dist-tag `exp`; `latest` is the
stable release.

## Bugs and issues

This repository does not take bug reports and does not fix bugs in the compiler's behaviour.
closure-rs exists to behave exactly like upstream Closure Compiler, so a bug in what the compiler
does is a bug in Closure Compiler: report it upstream at
<https://github.com/google/closure-compiler/issues>. The repository syncs with upstream
releases: the changes of a newer npm release are ported, and the pinned release above moves
forward to it.

A difference between closure-rs and the Java compiler at the pinned release is a porting defect,
not a compiler bug. The project finds those with its own differential testing (see
[How the port is checked](#how-the-port-is-checked)), so there is no need to report them either.

## Building

Requires Rust (the toolchain is pinned in `rust-toolchain.toml`).

```bash
cargo build --release --bin closure-rs
```

The binary is `target/release/closure-rs`. `gates/ci.sh` runs formatting, clippy, the
license-header check and the tests.

The released binaries are built profile-guided by `scripts/pgo_build.sh`, trained on compiles of
benchmark projects. The profile changes only the speed, never the output:

```bash
scripts/fetch_bench.sh --project d3-12 --project lodash-es --project three
python3 scripts/run_bench.py --job '^(d3-12/.*/ADVANCED|lodash-es/.*|three/.*)$' --write-args pgo-training
scripts/pgo_build.sh --training pgo-training   # needs the rustup component llvm-tools
```

Comparing against the Java compiler needs the pinned Java reference (`scripts/setup_tools.sh`,
`scripts/fetch_reference.sh`, [`oracle/REFERENCE.md`](oracle/REFERENCE.md)) and the corpus and
benchmark inputs (`scripts/fetch_d2.sh`, `scripts/fetch_bench.sh`), which are fetched by these
scripts, not stored in this repository.

## How the port is checked

- **Unit corpus:** Closure Compiler's Java tests were run once against an instrumented Java build
  and recorded (inputs, options, expected output and diagnostics); the Rust test harness replays
  every record (`corpus/unit/`, `crates/testing`).
- **Differential corpus:** real-world inputs are compiled by both compilers in 11 option profiles
  and compared byte for byte (`corpus/d2/`, `gates/d2_rust.py`). A ratchet keeps every pair that
  matches once matching forever.
- **Unseen inputs:** a differential fuzzer and large real-world bundles outside the corpus
  (`fuzz/`, `bench/`) guard against fitting the port to the corpus cases.

[`docs/PORTING.md`](docs/PORTING.md) describes the scope, what byte-identical means, how the port
mirrors the Java code, how fidelity is verified and how the port follows upstream.
[`DECISIONS.md`](DECISIONS.md) records the interpretations made along the way, and
[`crates/DESIGN.md`](crates/DESIGN.md) the Java-to-Rust conventions.

## How this port was built

closure-rs was written by AI agents, porting under the checks above; humans set the direction and
did no implementation work.

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
