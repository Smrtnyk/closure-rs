# Compiler WHITESPACE_ONLY differential check

Run commands from the checkout. This compares Compiler.compile followed
by Compiler.toSource, plus ordered error/warning lists (type, description, source,
line, column and length). The Java helper links the pinned reference jar and uses
CommandLineRunner.createOptions/setRunOptions with these exact flags:

```
--compilation_level=WHITESPACE_ONLY
--language_in=ECMASCRIPT_NEXT
--language_out=ECMASCRIPT_NEXT
```

The Rust setup calls CompilationLevel.setOptionsForCompilationLevel and reproduces
those CLI defaults, including trustedStrings. Both compilers format reports into
silent streams: report formatting can itself load input source maps and report
resolution warnings. A black-hole error manager would change that behavior.

All 2,085 candidate records are read from the five candidate JSONL files.
Each record retains its input, explicit extern and shim order and source names.
Source bytes are decoded as UTF-8 without newline translation (all files are
valid UTF-8). Profile/case flags are replaced with the three common flags above;
builtin environment externs are not added to either API call. The current working
directory is identical for both runs, including relative source-map resolution.
This is a single fixed compiler profile, not the D2 profile matrix.

The committed fixture takes the eight smallest eligible cases from each of the
four individual-source families. Whole-program and goog.module cases are not in the fixture;
the complete scratch comparison covers them.
Selection does not consult expected or actual outputs. The regression compares
all 32 selected cases (43 inputs, 104,258 bytes), including two parse-error cases;
there are no exceptions, ignored tests, or conditional comparison paths.

Reference results are generated once and cached; ordinary Rust tests need no JVM.
Only one JVM, capped at 2 GiB, runs at a time. Use the helper's optional third
argument to resume after a known number of complete JSONL rows, after truncating
any partial final row. Full regeneration:

```sh
source scripts/paths.sh   # ROOT: the main checkout (build/, corpus-cache/); REF_JAR: the pinned reference jar
source "$ROOT/tools/env.sh"
python3 crates/jscomp/tests/java/compiler_whitespace_fixture.py prepare \
  $ROOT "$PWD"
javac -J-Xmx2g -cp $REF_JAR \
  -d $ROOT/build/compiler-java \
  crates/jscomp/tests/java/CompilerWhitespace.java
java -Xmx2g -cp $ROOT/build/compiler-java:$REF_JAR \
  com.google.javascript.jscomp.CompilerWhitespace \
  $ROOT/corpus-cache/compiler/whitespace-requests.jsonl \
  $ROOT/corpus-cache/compiler/whitespace-reference.jsonl
python3 crates/jscomp/tests/java/compiler_whitespace_fixture.py fixture \
  $ROOT "$PWD"
cargo run --release -p closure-jscomp --example compiler_whitespace_check -- \
  $ROOT/corpus-cache/compiler/whitespace-requests.jsonl \
  $ROOT/corpus-cache/compiler/whitespace-rust.jsonl
python3 crates/jscomp/tests/java/compiler_whitespace_report.py \
  $ROOT/corpus-cache/compiler
cargo test -p closure-jscomp --test compiler_whitespace_test
```

The report records every mismatch, both raw Java diagnostic strings and the Java
UTF-8 output boundary. Lone UTF-16 surrogates use `{ "utf16": [...] }` in the raw
Java capture; they are never silently replaced during capture.
