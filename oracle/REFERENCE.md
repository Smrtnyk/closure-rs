# Java reference build

| Item | Value |
|---|---|
| Reference | google/closure-compiler release `v20261006` @ `48f4107ca2aac52149546ccc42894522fcfdb17d` = npm `google-closure-compiler@20261006.0.0` (registry tag `v20261006` in `scripts/references.tsv`, the default; fetched with `scripts/fetch_reference.sh` into `reference/closure-compiler-v20261006`) |
| Bazel | 8.0.0 (from the reference's `.bazelversion`), via bazelisk v1.29.0 (`tools/bin/bazelisk`) |
| JDK | Temurin jdk-21.0.12.1+1 (`tools/jdk-21`); Bazel builds with its hermetic remote JDK |
| Tools | `scripts/setup_tools.sh` installs bazelisk, the JDK and `tools/env.sh` under `tools/`, pinned by version and sha256 (DECISIONS.md D-003) |
| Uberjar | `build/reference-v20261006/closure-compiler.jar`, sha256 `cfa8886f9bcb9c05d29006dab5cd7012221a7ab2337d14c6fbd8d685b31f2264` |
| Oracle | `build/oracle-v20261006/oracle.jar` |
| Bazel output_base | `~/.cache/bazel/_bazel_<user>/<hash>` (Bazel's default for the reference workspace; one per checkout) |

Every reference stays installed alongside the newer ones (docs/PORTING.md §9, DECISIONS.md D-026):

| Registry tag | Upstream | Checkout (under the main checkout) | Uberjar sha256 | Oracle |
|---|---|---|---|---|
| `v20261006` (default) | release `v20261006`, `48f4107ca2aac52149546ccc42894522fcfdb17d`, npm `20261006.0.0` | `reference/closure-compiler-v20261006` | `build/reference-v20261006/closure-compiler.jar`, `cfa8886f9bcb9c05d29006dab5cd7012221a7ab2337d14c6fbd8d685b31f2264` | `build/oracle-v20261006/oracle.jar` |
| `bb8c8e7` | release `v20261005`, `bb8c8e7cb8d0b14b27ff5e969d186bb97017eb06`, npm `20261005.0.0` | `reference/closure-compiler` (kept: the closure-self D2 cases read their inputs from it) | `build/reference/closure-compiler.jar`, `4ef5a893f30378aad76e53efc20353e06830289a4d835be0a12cd34bab2af821` | `build/oracle/oracle.jar` |

Each checkout has its own untracked `MODULE.bazel.lock`, copied from the previous reference's
checkout when `MODULE.bazel` is unchanged. The uberjars are unstamped:
their `build-data.properties` holds `build.time=Thu Jan 01 00\:00\:00 1970 (0)` and
`build.timestamp.as.int=0`, and a rebuild after `bazelisk clean` gives the same sha256. They are
installed read-only (mode 0555) next to a `closure-compiler.jar.sha256`.

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
[ -e "$REF_JAR" ] || { mkdir -p "$(dirname "$REF_JAR")" && cp bazel-bin/compiler_uberjar_deploy.jar "$REF_JAR" \
  && chmod 0555 "$REF_JAR" && (cd "$(dirname "$REF_JAR")" && sha256sum closure-compiler.jar > closure-compiler.jar.sha256); }
sha256sum "$REF_JAR"          # a new reference: record it as jar_sha256 in scripts/references.tsv
# One test class:
bazelisk test //:test/com/google/javascript/jscomp/PeepholeFoldConstantsTest
# Whole Java suite (432 test targets):
bazelisk test //:compiler_tests
```

An uberjar build from a cold Bazel cache takes in the order of 15 minutes, most of it spent
compiling protoc from source.

## Oracle

- **Build:** `oracle/build.sh` compiles `oracle/src` with JDK 21 (`--release 21`), using
  the reference jar `$REF_JAR` (`build/reference-v20261006/closure-compiler.jar` for the default)
  as the only classpath entry, into `$ORACLE_JAR` (`build/oracle-v20261006/oracle.jar`). It checks the reference
  jar's sha256 against the registry first, and refuses a reference whose sha256 is not pinned.
- **Run:** `java -cp build/oracle-v20261006/oracle.jar:build/reference-v20261006/closure-compiler.jar closurers.oracle.Main compile|request|server`
  (another reference: `$ORACLE_JAR:$REF_JAR`). The oracle uses the jar `CommandLineRunner` was
  loaded from as its reference jar (`-Doracle.reference_jar=PATH` overrides).
  The protocol, the seam caveats and the identity checks are in `oracle/PROTOCOL.md`, the
  parse_dump schema in `oracle/PARSE_DUMP.md`.
- **Smoke test:** `oracle/test/smoke.sh`.
