# Java reference build

| Item | Value |
|---|---|
| Reference | google/closure-compiler @ `bb8c8e7cb8d0b14b27ff5e969d186bb97017eb06` (registry tag `bb8c8e7` in `scripts/references.tsv`, the default; fetched with `scripts/fetch_reference.sh`) |
| Bazel | 8.0.0 (from the reference's `.bazelversion`), via bazelisk v1.29.0 |
| JDK | Temurin jdk-21.0.12.1+1 (`tools/jdk-21`); Bazel builds with its hermetic remote JDK |
| Uberjar | `build/reference/closure-compiler.jar`, sha256 `4ef5a893f30378aad76e53efc20353e06830289a4d835be0a12cd34bab2af821` |
| Bazel output_base | `~/.cache/bazel/_bazel_<user>/<hash>` (Bazel's default for the reference workspace) |

## Commands

Every reference has its own checkout and jar paths in `scripts/references.tsv`, next to the
others (docs/PORTING.md §9); `scripts/paths.sh` resolves them for `$CLOSURE_RS_REF` (default: the
registry's default). A jar is built into its own `build/reference-<tag>/` directory, never over
another reference's jar:

```bash
. scripts/paths.sh            # CLOSURE_RS_REF=<tag> selects another reference
. tools/env.sh
scripts/fetch_reference.sh    # $REF_SRC at $REF_COMMIT
cd "$REF_SRC"
bazelisk build --local_resources=memory=HOST_RAM*0.3 --local_resources=cpu=HOST_CPUS*0.75 //:compiler_uberjar_deploy.jar
[ -e "$REF_JAR" ] || { mkdir -p "$(dirname "$REF_JAR")" && cp bazel-bin/compiler_uberjar_deploy.jar "$REF_JAR"; }
sha256sum "$REF_JAR"          # a new reference: record it as jar_sha256 in scripts/references.tsv
# One test class:
bazelisk test //:test/com/google/javascript/jscomp/PeepholeFoldConstantsTest
# Whole Java suite (432 test targets):
bazelisk test //:compiler_tests
```

## Measurements (2026-10-06, 32 cores, about 20 GB of RAM free)

- Uberjar build from a cold cache: about 15 minutes. Most of the time goes on compiling
  protoc from source.
- `PeepholeFoldConstantsTest`: passes, taking 23 s including the incremental build.
- Smoke compile (`--version`, plus a tiny ADVANCED compile): 2.4 s wall-clock, most of it
  JVM startup.

## Oracle

- **Build:** `oracle/build.sh` compiles `oracle/src` with JDK 21 (`--release 21`), using
  the reference jar `$REF_JAR` (`build/reference/closure-compiler.jar` for the default) as the
  only classpath entry, into `$ORACLE_JAR` (`build/oracle/oracle.jar`). It checks the reference
  jar's sha256 against the registry first, and refuses a reference whose sha256 is not pinned.
- **Run:** `java -cp build/oracle/oracle.jar:build/reference/closure-compiler.jar closurers.oracle.Main compile|request|server`
  (another reference: `$ORACLE_JAR:$REF_JAR`). The oracle uses the jar `CommandLineRunner` was
  loaded from as its reference jar (`-Doracle.reference_jar=PATH` overrides).
  The protocol is in `oracle/PROTOCOL.md`, the parse_dump schema in `oracle/PARSE_DUMP.md`,
  and the seam and identity measurements in `oracle/SEAM.md`.
- **Smoke test:** `oracle/test/smoke.sh`.
