# Java reference build

| Item | Value |
|---|---|
| Reference | google/closure-compiler @ `bb8c8e7cb8d0b14b27ff5e969d186bb97017eb06` (fetched with `scripts/fetch_reference.sh`) |
| Bazel | 8.0.0 (from the reference's `.bazelversion`), via bazelisk v1.29.0 |
| JDK | Temurin jdk-21.0.12.1+1 (`tools/jdk-21`); Bazel builds with its hermetic remote JDK |
| Uberjar | `build/reference/closure-compiler.jar`, sha256 `4ef5a893f30378aad76e53efc20353e06830289a4d835be0a12cd34bab2af821` |
| Bazel output_base | `~/.cache/bazel/_bazel_<user>/<hash>` (Bazel's default for the reference workspace) |

## Commands

```bash
. tools/env.sh
cd reference/closure-compiler
bazelisk build --local_resources=memory=HOST_RAM*0.3 --local_resources=cpu=HOST_CPUS*0.75 //:compiler_uberjar_deploy.jar
cp bazel-bin/compiler_uberjar_deploy.jar ../../build/reference/closure-compiler.jar
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
  `build/reference/closure-compiler.jar` as the only classpath entry, into
  `build/oracle/oracle.jar`. It checks the reference jar's sha256 first.
- **Run:** `java -cp build/oracle/oracle.jar:build/reference/closure-compiler.jar closurers.oracle.Main compile|request|server`.
  The protocol is in `oracle/PROTOCOL.md`, the parse_dump schema in `oracle/PARSE_DUMP.md`,
  and the seam and identity measurements in `oracle/SEAM.md`.
- **Smoke test:** `oracle/test/smoke.sh`.
