# closure-rs

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

## Status: alpha

The port is in progress and checked continuously against the Java compiler: Closure Compiler's own
test suites are replayed against it, and a large corpus of real-world code is compiled by both
compilers and compared byte for byte (see [How the port is checked](#how-the-port-is-checked)).
Until it is complete, compare its output with the Java compiler before relying on it.

## Usage

`closure-rs` accepts the same command-line flags as the Java compiler's `CommandLineRunner`:

```bash
closure-rs --compilation_level=ADVANCED --js=src/app.js --js_output_file=dist/app.min.js
```

### npm

The npm package `closure-rs` (source in [`npm/closure-rs/`](npm/closure-rs/README.md)) wraps the
native binary in the programmatic API of the official `google-closure-compiler` package, with
TypeScript types. The package carries the native binaries (Linux x64 and Windows x64 for now) and
uses the one for your system, so no Java is needed. The Linux binary is statically linked: it runs
on any x86-64 Linux, glibc or musl (Alpine), with no C library version requirement. An existing
project can switch with an npm alias in `package.json`, without changing its code:

```json
"devDependencies": {
  "google-closure-compiler": "npm:closure-rs@latest"
}
```

`import ClosureCompiler from 'google-closure-compiler'` and `npx google-closure-compiler ...` then
run closure-rs. The gulp and grunt plugins of the official package are not implemented; see the
package's README for the API and its differences. The packages are built and tested by
`.github/workflows/release.yml`; publishing is a separate manual step.

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

The binary is `target/release/closure-rs`. `gates/ci.sh` runs formatting, clippy, the license-header check and the tests.
The released binaries are built profile-guided by `scripts/pgo_build.sh` (trained on benchmark
projects; same output, less CPU time).
Comparing against the Java compiler needs the pinned Java reference (`scripts/fetch_reference.sh`)
and the corpus inputs, which are fetched by scripts, not stored in this repository.

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
