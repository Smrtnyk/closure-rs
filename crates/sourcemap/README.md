# closure-sourcemap

Faithful port of `com.google.debugging.sourcemap` from the pinned Closure Compiler
reference. This crate is self-contained. The Gson modules implement the reader,
DOM, numeric conversions, exceptions, and writer reached by this package, using
Gson 2.9.1 bytecode from the reference jar. The generator writes its JSON by hand.

The printer-facing interfaces are in `source_map_generator`,
`source_map_generator_factory`, and `source_map_consumer_v3`. Traits retain Java
interface names. `OriginalMapping` preserves proto2 field presence and defaults.
Both compact and normal consumer entry layouts retain the Java representation.

## Strings

Every Java `String` in this package that holds JSON text or reaches JSON output
(source names, sources content, names, `file`, `sourceRoot`, `mappings`, section
values, extension keys and values, `OriginalMapping` strings, Gson keys, values and
number text, exception messages) is a `closure_rhino::js_string::JsString`, so lone
surrogates survive. Methods that take a Java `String` take `impl Into<JsString>`;
getters return `JsString` or `Option<JsString>` where Java may return null.

`appendTo` and `appendIndexMapTo` take Java's `Appendable` as `&mut dyn fmt::Write`
and also return the complete output as a `JsString`. `Util.escapeString` output is
ASCII, but raw section `map` values, extension keys and `JsonElement.toString()` can
contain lone surrogates, which a Rust `fmt::Write` cannot carry (it receives
U+FFFD for them); callers that need Java's exact output use the returned `JsString`.

JSON is read and written through a port of the Gson 2.9.1 classes this package
reaches (`src/gson/`), because the Java code parses and prints through Gson and the
output must match it byte for byte. `serde_json` is only a dev-dependency of the
fixture replay.

## Verification

The Java tests of this package are ported in `tests/`; the 19 compiler-dependent
SourceMapGeneratorV3Test methods are in `crates/jscomp/tests/source_map_generator_v3_test.rs`.
`tests/sourcemap_diff_test.rs` replays the committed JVM fixtures (`tests/fixtures/`) without a JVM.

The Java driver that produced the reference outputs (`SourcemapDiffDriver.java`) and its
deterministic input requests (`.ndjson`; random scripts use seed 20261007) are not in the
repository. Its JSONL outputs are cached under `corpus-cache/sourcemap/` of the main checkout
(not tracked).

The driver runs against the pinned reference jar with `-Xmx768m` and
`-XX:-OmitStackTraceInFastThrow`. The latter keeps runtime exception messages
stable instead of HotSpot omitting them after repeated throws. All Java work is
serial. The driver never invokes the compiler for these checks.

Replay all reference outputs:

```sh
source scripts/paths.sh   # ROOT: the main checkout
cargo run -p closure-sourcemap \
  --example sourcemap_diff -- \
  "$ROOT"/corpus-cache/sourcemap/*.jsonl
```

The full corpus has 1,859 D2 maps, 3,201 generator scripts, 3,232 parsing cases,
and 106 encoding/escaping cases. Every generated mapping visit and all requested
lookups are compared, along with metadata, reverse mappings, regenerated output,
offsets, and wrappers. Both regular and fast parsers are checked.

Large D2 streams use bounded JSONL chunks (`header`, `visits`, `queries`, `reverse`,
`end`) grouped by map ID; the reported total counts complete maps. The replay
rejects incomplete streams. Small parser/generator/encoding cases occupy one
record each. Unpaired strings are transported as `{"$utf16":[...]}`; maps with
unpaired keys use `{"$entries":[[key,value],...]}`. These retain exact code units.

The fixture sample is deterministic and below 1.5 MB: eight complete D2 maps,
29 generator scripts, 700 parser records covering every one of the 567 distinct
reference exception messages, and all 106 encoding records, including all 65,536
UTF-16 code units. Fixture-only `$rangeValues`, `$repeatValues`, `$repeatText`,
and `$concat` encodings expand losslessly before replay; comparisons check the
complete expanded data. No expectations are omitted or normalized away.
