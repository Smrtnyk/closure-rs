# DiagnosticGroups reference fixture

The fixture covers all 79 registrations (including anonymous/deprecated aliases)
in Java's LinkedHashMap registration order, followed by all 75 public static
DiagnosticGroup fields sorted by field name. Every row contains the registration
or field name, the group's own nullable name, and its ordered member tuples
`[key, default level, format]`. Duplicate types follow DiagnosticGroup's ordered
set behavior. Formats are captured directly from the pinned jar, including Java
text blocks, concatenated strings and MessageFormat quotes.

Run from the checkout:

```sh
source scripts/paths.sh   # ROOT: the main checkout (build/, corpus-cache/); REF_JAR: the pinned reference jar
source "$ROOT/tools/env.sh"
javac -J-Xmx2g -cp $REF_JAR \
  -d $ROOT/build/compiler-java \
  crates/jscomp/tests/java/CompilerDiagnosticGroups.java
java -Xmx2g -cp $ROOT/build/compiler-java:$REF_JAR \
  com.google.javascript.jscomp.CompilerDiagnosticGroups \
  $ROOT/corpus-cache/compiler/diagnostic-groups.json
cp $ROOT/corpus-cache/compiler/diagnostic-groups.json \
  crates/jscomp/tests/data/compiler_diagnostic_groups.json
cargo test -p closure-jscomp --test diagnostic_groups_test
```

Ordinary tests read the committed fixture and need no JVM. The optional second
and third generator arguments select and capture DiagnosticType fields for
transcription: one fully qualified Java class and field name separated by a space
per selection line. These scratch selections/captures stay in `build/` and
`corpus-cache/compiler/`.
