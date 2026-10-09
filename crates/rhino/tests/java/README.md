The generators call the pinned Closure jar and project JDK 21. Runnable copies and compiled
classes go to `corpus-cache/dtoa/` (not tracked). Run one JVM at a time with `-Xmx1g`; Rust CI needs no Java.

```bash
export CARGO_BUILD_JOBS=6
source scripts/paths.sh   # ROOT: the main checkout, which holds build/ and corpus-cache/
source "$ROOT/tools/env.sh"
export CLOSURE_RS_ROOT="$ROOT"   # DToAHarness writes under $CLOSURE_RS_ROOT/corpus-cache/dtoa
cache=$ROOT/corpus-cache/dtoa
reference_jar=$ROOT/build/reference/closure-compiler.jar
mkdir -p "$cache/harness"
cp crates/rhino/tests/java/*.java "$cache/harness/"
javac -J-Xmx1g -cp "$reference_jar" -d "$cache/harness" "$cache/harness/"*.java
java -Xmx1g -XX:ActiveProcessorCount=6 -XX:-OmitStackTraceInFastThrow -cp "$cache/harness:$reference_jar" DToAHarness standard
java -Xmx1g -XX:ActiveProcessorCount=6 -XX:-OmitStackTraceInFastThrow -cp "$cache/harness:$reference_jar" DToAHarness modes
java -Xmx1g -XX:ActiveProcessorCount=6 -XX:-OmitStackTraceInFastThrow -cp "$cache/harness:$reference_jar" DToAHarness raw
python3 crates/rhino/tests/java/sample_golden.py
```

`standard` generates 7,097,372 doubles; `modes` generates 407,372 doubles, four modes per input;
`raw` generates 20,068 doubles, modes -1..10 and both bias settings. `jdk` generates the
7,097,372-row Double.toString corpus. Seed: `0x44546f415f323031`.

Every DToA string is encoded as `=` followed by printable ASCII, escaping backslash as `\\`
and each other UTF-16 unit as `\u` plus four lowercase hex digits. Exceptions are `!<class>: <message>`.
The Double.toString column is plain ASCII. Successful raw rows also carry decimal-point and sign
columns; exceptional/infeasible raw rows carry only the encoded outcome after the input columns.

`javaKEstimate` calls DToA#d2b reflectively and copies the reference estimate lines verbatim.
For finite, nonzero |d|, only `20000 < abs((long)k) && abs((long)k) < 715827894` is infeasible.
Those rows record `?infeasible k=<k>`; all other inputs run, including immediately throwing powers.
HotSpot fast-throw optimization is disabled so repeated array-bounds exceptions retain their
exact messages rather than becoming null. Category logs report input and infeasibility counts. Neither Java algorithms nor inputs are patched.

The full mix includes every power of two/ten and neighbors, all signed one-to-three-bit subnormals,
random signed subnormals, integers, threshold neighbors, parsed short decimal literals, and uniform
64-bit patterns plus their sign-flipped partners. Precisions cover 0..100 (exponential up to 101).
Raw modes include negative precisions and invalid-mode fallback.

`Probe 000000000003ffff record` independently records the complete 151,804-unit result; its k=151471
falls inside the full generator's specified exclusion interval. `Probe 000000007fffffff` records the
immediate overflow. `CharProbe` reports UTF-16 units for the non-ASCII and lone-surrogate cases.
`BigIntegerRangeHarness`, run with `--add-opens java.base/java.math=ALL-UNNAMED`, checks actual JDK
constructor/shift/multiply/square/pow boundaries using sparse magnitude arrays, without huge powers.

`XCheck.java` is an independent cross-check with its own seed (`0x5eed2026`) and generators
(random bits, `<long>e<exp>` literals, `<int>.<int>` literals). It skips subnormals, which the
main corpus covers under the infeasibility rule, and writes `standard`-format rows:

```bash
java -Xmx1g -cp "$cache/harness:$reference_jar" XCheck 600000 | gzip > "$cache/xcheck.tsv.gz"
zcat "$cache/xcheck.tsv.gz" | cargo run --release -p closure-rhino --example dtoa_compare -- standard
```

`ConcurrentHashMapOrder.java` needs only the JDK. It prints the `keySet()` order of
`ConcurrentHashMap<String, String>` after scripted `putAll`/`put` sequences (resizes, collision
and tree bins, re-puts), which `../java_concurrent_hash_map_test.rs` replays:

```bash
mkdir -p build/chm && "$ROOT/tools/jdk-21/bin/javac" -d build/chm crates/rhino/tests/java/ConcurrentHashMapOrder.java
"$ROOT/tools/jdk-21/bin/java" -cp build/chm ConcurrentHashMapOrder > crates/rhino/tests/data/java_concurrent_hash_map_order.tsv
```
