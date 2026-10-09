These 20,000 deterministic TSV records come from the pinned Java reference and project JDK;
Rust output was never used to create expectations. Rust CI runs them without Java.

| File | Lines | Java source |
| --- | ---: | --- |
| standard.tsv | 8,000 | full standard.tsv.gz: DToA.numberToString and Double.toString |
| modes.tsv | 3,000 | full modes.tsv.gz: reflective JS_dtostr |
| jdk.tsv | 8,000 | full jdk.tsv.gz: Double.toString |
| raw.tsv | 1,000 | full raw.tsv.gz: JS_dtoa modes -1..10, both bias settings |

Standard/modes/raw use the exact `=`, `!`, `?` outcome encoding described in ../java/README.md.
The decoder preserves every UTF-16 code unit, including lone surrogates, and checks exception
class AND message. Infeasible rows retain their encoded k; only the documented k interval is
excluded from conversion. Double.toString columns remain unencoded ASCII.

Every input category is represented: special values and NaN payloads, power neighbors, sparse
and random signed subnormals, integers, thresholds, short decimal literals, and random bit patterns.
The sample includes the pinned bug's non-ASCII and lone-surrogate results, immediate magnitude
overflows, negative-power exceptions, and quick-path array-bounds exceptions. Explicit tests also
retain the hand-picked Java expectations and exact malformed UTF-16 cases.

To keep the repository small, sample eligibility excludes only normal results longer than 2,000
UTF-16 code units. Large eligible results remain in the full cached corpora and are compared there.
The 151,804-unit result is separately generated and compared in full; its k=151471 falls inside the
mandated generator exclusion interval. The combined committed TSV size is below 3 MB.

`python3 crates/rhino/tests/java/sample_golden.py` reproduces the standard, modes and raw samples
from the full gzip corpora and category-head/stride samples. It preserves each
exceptional/non-ASCII/surrogate outcome kind explicitly and selects the rest deterministically.
It does not touch `jdk.tsv`.
Java classes and full corpora live under `corpus-cache/dtoa/` of the main checkout (not tracked).
Reference jar: `build/reference/closure-compiler.jar` of the main checkout.
JDK: tools/jdk-21 (21.0.12.1+1-LTS). The modes/raw generator disables HotSpot fast-throw optimization
so repeated array-bounds exceptions retain exact messages. Neither DToA nor BigInteger is patched.

SHA-256:

```
8bd3b79fba82d5e8f11c37625ec3317996dd162b4364f5e3268e1a6fea4b31d1  standard.tsv
9d07b3497a60e06abbf7b5e9b52d46f53984119de3cac47974648835ac91ac1c  modes.tsv
20c3182d6fb740c59bd77e4b822d4af32f29bd428f97a579312d72b5136436ff  jdk.tsv
af2b303c48a4483a090b508f0d96fac0fae15421daa2c6c662eacdf3ca30d48f  raw.tsv
```
