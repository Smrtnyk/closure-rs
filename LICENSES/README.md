# License texts

closure-rs is a translation of Java code into Rust. Each Rust file under `crates/` starts with the
license notice(s) of the code it was translated from, copied verbatim from the upstream file,
followed by `// Ported from ...` lines that name the upstream project, version and files. A file
with several origins carries every applicable notice. Files written for closure-rs carry the
Apache-2.0 notice of The closure-rs Authors. `scripts/license_headers.py` derives the headers from
the `// port: Class#method` markers and regenerates them; `FILES.md` lists every file that is not
Apache-2.0 only, by origin.

Code translated from OpenJDK (GPL-2.0 with the Classpath exception) is kept apart from code under
other licenses: where a Closure Compiler, Rhino or protobuf port reproduces JDK internals (a
`java.util.HashMap` bucket order, `BigInteger` overflow checks, `GZIPOutputStream` framing,
`Long#parseLong` messages, `Throwable#printStackTrace`, ...), those functions live in a sibling
`*_jdk.rs` module (or, for tests, under `tests/jdk/` or in the JDK port they test), so every such
file carries the notices of one license family. One exception is inherent to the JDK itself:
`crates/rhino/src/java_lang/sax_parser.rs` ports JDK 21's `java.xml` SAX parser, which combines
Oracle's GPL-2.0 + Classpath-exception code with the Apache Xerces code (Apache-2.0) that the JDK
bundles, so it keeps both notices, as the JDK's own sources do (`xml_char.rs` is Xerces only).
In the same way, where a port of Closure Compiler's Apache-2.0 code also translates methods of its
Rhino-derived MPL-1.1 / GPL-2.0-or-later files (`Node#matchesQualifiedName`, `NodeSubject`
assertions, `StaticScope` defaults, `JSTypeRegistry` calls of the test harness, ...), those live
in a sibling `*_rhino.rs` module (for tests, under `tests/rhino/`). Additional notices that come
with one upstream file stay with it: the Unicode data license on the character tables, and David
M. Gay's notice next to the MPL block of `d_to_a.rs`.

`headers.tsv` is the classification in machine-readable form: for every tracked Rust file the
SHA-256 of its header, its licenses (SPDX) and its upstream files. `license_headers.py --apply`
writes it with the headers and `FILES.md`. Where the upstream sources are not available (a CI
runner, a fresh clone), `license_headers.py --check` verifies the headers against it: every file
must start with a license notice, and a listed file must carry exactly the recorded header.

Layout: the Apache License 2.0, which covers most of the code, stays in `LICENSE` at the repository
root (as in Closure Compiler). Every other license text is in this directory, named after its SPDX
identifier and, where it is a particular project's copy, that project:

| File | SPDX | Applies to |
| --- | --- | --- |
| `../LICENSE` | Apache-2.0 | Closure Compiler code, closure-rs' own code, Guava and Gson ports, the Apache Xerces code (via OpenJDK) |
| `MPL-1.1.txt` | MPL-1.1 | Closure Compiler's Rhino-derived files (alternatively GPL-2.0-or-later; the GPL 2 text is in the OpenJDK file below) |
| `GPL-2.0-only-WITH-Classpath-exception-2.0.txt` | GPL-2.0-only WITH Classpath-exception-2.0 | Code translated from OpenJDK 21. Verbatim copy of the JDK's `legal/java.base/LICENSE` (GPL 2 followed by the Classpath exception) |
| `OpenJDK-ADDITIONAL_LICENSE_INFO.txt` | (information) | Verbatim copy of the JDK's `legal/java.base/ADDITIONAL_LICENSE_INFO` |
| `OpenJDK-ASSEMBLY_EXCEPTION.txt` | (information) | Verbatim copy of the JDK's `legal/java.base/ASSEMBLY_EXCEPTION` |
| `Apache-2.0-Xerces-NOTICE.md` | Apache-2.0 | The Apache Xerces NOTICE and license as the JDK ships them (`legal/java.xml/xerces.md`): `crates/rhino/src/java_lang/sax_parser.rs`, `xml_char.rs` |
| `Unicode-DFS-2016.txt` | Unicode-DFS-2016 | Unicode Character Database tables taken from JDK 21's `java.lang.Character`. Verbatim from the JDK's `legal/java.base/unicode.md` |
| `BSD-3-Clause-protobuf.txt` | BSD-3-Clause | Protocol Buffers for Java runtime code. Verbatim `LICENSE` of protobuf v30.2 |
| `MIT-args4j.txt` | MIT | args4j. The args4j license as Closure Compiler's `THIRD_PARTY_NOTICES` reproduces it |
| `dtoa.txt` | dtoa | David M. Gay's notice, kept by Closure's `DToA.java` (`crates/rhino/src/dtoa/d_to_a.rs`) |
| `THIRD_PARTY_RUST.md` | MIT, Apache-2.0, Unlicense, Zlib, Apache-2.0 WITH LLVM-exception | The crates.io crates and the Rust standard library linked into the `closure-rs` binary, and the C runtime statically linked into the linux-x64 binary: component, version, license and full texts. Written by `scripts/third_party_licenses.py` from `Cargo.lock` and the files below; the npm package ships it with `LICENSE` and `NOTICE` |
| `MIT-musl.txt` | MIT | The musl C library and startup objects, statically linked into the linux-x64 binary. Verbatim `COPYRIGHT` of musl 1.2.5 |
| `Apache-2.0-WITH-LLVM-exception-libunwind.txt` | Apache-2.0 WITH LLVM-exception | LLVM's libunwind, statically linked into the linux-x64 binary. Verbatim `libunwind/LICENSE.TXT` of the LLVM that Rust's toolchain bundles |
| `Apache-2.0-WITH-LLVM-exception-compiler-rt.txt` | Apache-2.0 WITH LLVM-exception | LLVM compiler-rt's `crtbegin`/`crtend` objects, statically linked into the linux-x64 binary, and the compiler-rt builtins of the Rust standard library. Verbatim `compiler-rt/LICENSE.TXT` of the same LLVM |

The SPDX expression of the whole (`Cargo.toml`, `npm/config.json`) is
`Apache-2.0 AND (MPL-1.1 OR GPL-2.0-or-later) AND GPL-2.0-only WITH Classpath-exception-2.0 AND
BSD-3-Clause AND MIT AND Unicode-DFS-2016 AND dtoa`: every part stays under the license its file
header states. It covers the source code; the licenses of the third-party code linked into the
binaries (`THIRD_PARTY_RUST.md`) are not part of it. See `../NOTICE` for the attributions.
