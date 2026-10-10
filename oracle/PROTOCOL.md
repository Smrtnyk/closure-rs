# Oracle protocol (docs/PORTING.md §4.1)

The oracle is a Java program that uses the pinned reference uberjar `$REF_JAR`
(`build/reference-v20261006/closure-compiler.jar` for the default reference; `scripts/paths.sh`,
`oracle/REFERENCE.md`) **as a library**. The reference `src/` is never modified.

## Building and running

```bash
. scripts/paths.sh     # REF_JAR, ORACLE_JAR of the reference ($CLOSURE_RS_REF, default v20261006)
oracle/build.sh        # javac (JDK 21, --release 21) against $REF_JAR -> $ORACLE_JAR
CP=$ORACLE_JAR:$REF_JAR
GENV=(env -i PATH=/usr/bin:/bin HOME="$HOME" LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC)   # golden environment, required (see "Environment")
"${GENV[@]}" tools/jdk-21/bin/java -cp $CP closurers.oracle.Main compile ARGV...    # behaves like: java -jar closure-compiler.jar ARGV...
"${GENV[@]}" tools/jdk-21/bin/java -cp $CP closurers.oracle.Main request < req.json  # one JSON request -> one JSON response line
"${GENV[@]}" tools/jdk-21/bin/java -cp $CP closurers.oracle.Main server [--isolate=none|request]   # JSON-lines server
oracle/test/smoke.sh   # exercises every operation (CLI and server, both isolation modes)
```

Without `GENV` the JVM inherits the caller's locale and time zone, and its output is **not** ground
truth (next section). `oracle_client.Oracle` / `cli_compile` apply it for you.

Always start the server with **cwd = the repository root**. Relative paths in `args` are
then resolved exactly as `java -jar` resolves them from the repository root. A Python client is in
`oracle/oracle_client.py`.

## Environment (required for ground truth)

The compiler's output depends on the environment of the JVM that runs it, so the oracle is
ground truth **only** when it runs in the same environment as the golden runs:

- Closure formats the summary line with `String.format` and the JVM default (FORMAT) locale:
  `"%d error(s), %d warning(s), %.1f%% typed%n"` (PrintStreamErrorManager.java:72,
  PrintStreamErrorReportGenerator.java:64). Under `LANG=de_DE.UTF-8` the line reads
  `86,7% typed`; under `C.UTF-8` it reads `86.7% typed`. Many D2 golden results contain such a
  line, all with `.`.
- `stdout.encoding` and `stderr.encoding` (and `sun.jnu.encoding` for file names) come from
  `LANG`/`LC_ALL`; they decide how non-ASCII diagnostics are written.

**The golden environment** is `child_env()` in `gates/lib/run_reference.py` (identical to
`golden_env()` in `oracle/oracle_client.py`): nothing inherited, `PATH=/usr/bin:/bin`, `HOME`,
`LANG=C.UTF-8`, `LC_ALL=C.UTF-8`, `TZ=UTC`. Every golden result records it in `env`.

**The rule.** Start every oracle JVM (`server`, `request`, `compile`) with exactly that
environment; never let it inherit the caller's. `oracle_client.Oracle` and
`oracle_client.cli_compile` do this by default, and Gate 0.1 does it too.

**The check.** The server's ready line carries `"env"`: what the JVM derived from its
environment (`Main.jvmEnv()`): `user.language`, `user.country`, `user.variant`,
`locale.default`, `locale.format`, `locale.display`, `native.encoding`, `stdout.encoding`,
`stderr.encoding`, `sun.jnu.encoding`, `file.encoding`, `timezone`, `java.version`,
`java.vm.version`, and the raw `getenv.LANG`/`LC_ALL`/`LC_NUMERIC`/`LC_CTYPE`/`TZ`.
Clients compare it with `oracle_client.GOLDEN_JVM_ENV` (measured with `tools/jdk-21` under the
golden environment: `user.language=en`, no country or variant, `locale.default` and
`locale.format` = `en`, every encoding `UTF-8`, `timezone=UTC`).
`oracle_client.Oracle` raises `OracleEnvMismatch` and stops the server on any difference;
`gates/lib/gate01.py` aborts the same way (gate version 3). Pass `env=...,
check_env=False` only for experiments whose results are not used as ground truth. `Main
compile` (CLI mode) prints only the compiler's output, so it cannot report its environment:
launch it with `golden_env()` (`cli_compile` does). Inline-file requests run a child JVM that
inherits the server's environment.

`python3 oracle/test/env_check.py` shows the difference on `closure-self-ext-es3` /
`advanced_strict`: in C.UTF-8 the server's stderr equals the golden stderr (`... 98.2% typed`);
in `de_DE.UTF-8` it does not (`98,2% typed`), and `oracle_client.Oracle` refuses to start there.
The time zone does not reach compiler output (no `TimeZone`/date-formatting use in in-scope
`src/`; the only hit is the out-of-scope `ant/CompileTask.java`), but it is pinned and checked
anyway.

## Source layout

| File | Role |
|---|---|
| `src/closurers/oracle/Main.java` | Launcher: CLI modes, the server loop, classloader isolation and inline files. It touches the compiler only through `OracleWorker.handle(String) -> String`. |
| `src/com/google/javascript/jscomp/OracleWorker.java` | Runs the operations. It lives in the `jscomp` package for package-private access. |
| `src/com/google/javascript/jscomp/OracleRunner.java` | A `CommandLineRunner` subclass. It adds no behaviour; see "compile" below. |
| `src/com/google/javascript/jscomp/ParseDump.java` | The `parse_dump` operation. The schema is in `PARSE_DUMP.md`. |
| `src/com/google/javascript/rhino/OracleNodeAccess.java` | Enumerates `Node.Prop`, which is package-private, and reads the raw property list. |

## Server framing

- **On startup:** the server writes one line,
  `{"ready":true,"isolate":"none","protocol":"closure-rs-oracle/1","env":{...}}` (`env`: see
  Environment above).
- **Requests:** the client sends one JSON object per line on stdin.
- **Responses:** for each request, the server writes exactly one JSON line on its real stdout,
  in request order. Requests are processed one at a time.
- **Shutdown:** the server exits when stdin is closed.
- **Every response** carries `"ok": true|false`. When the request had an `"id"`, the response
  echoes it. When `ok` is false, `"error"` holds a Java stack trace, which means an oracle
  failure, not a compiler failure. For a failure that escapes the worker (for example an
  `OutOfMemoryError`), the server unwraps the reflective `InvocationTargetException`, puts the
  real throwable's class in `"error_class"`, and puts its full stack trace, including every
  `Caused by:`, in `"error"`. Compiler failures are reported normally, through
  `exit_code` and `stderr`.
- **Byte fields** are base64 encoded and named `*_b64`. Output-file contents are also base64.

### Isolation (`--isolate`)

- **`none` (the default):** one shared loader, and the JIT stays warm. No static state leaks
  into the output across requests: `oracle/test/identity.py` compares one long-lived server
  with one fresh JVM per request (and with `java -jar`) byte for byte, and Gate 0.1
  (`gates/gate_0_1.sh`) compares long-lived servers with the golden `java -jar` runs on every
  D2 pair.
- **`request`:** each request runs in a fresh
  `URLClassLoader([oracle.jar, closure-compiler.jar], parent = platform loader)`. Every static
  field in Closure and its bundled libraries (Guava, Gson, protobuf, args4j) is therefore fresh.
  JDK-global state is shared:
  - `System.{in,out,err}`, which is redirected per request and restored afterwards;
  - `java.util.logging`;
  - system properties.

  Cost: about 2.5 s per small request. That is roughly the same as a fresh JVM, because the
  time goes on cold class loading and JIT, not on JVM startup.
  The loader is deliberately unnamed. A named loader would prefix stack frames with `name//`,
  so crash output would differ from `java -jar`.

## Operations

All `args` arrays are `CommandLineRunner` argv, exactly as given to `java -jar`.

### `compile`

```json
{"op":"compile","args":["--compilation_level=ADVANCED","--js=foo.js"],"stdin":"optional text","stdin_b64":"optional"}
-> {"ok":true,"exit_code":0,"stdout_b64":"...","stderr_b64":"...","output_files":{"out.js":"<b64>",...}}
```

`compile` replicates `CommandLineRunner.main` (CommandLineRunner.java:2249) statement for
statement:
1. It sets the `PhaseOptimizer` logger to `OFF`.
2. It runs `new CommandLineRunner(args, System.in, System.out, System.err)`. Because
   `OracleRunner` adds no behaviour, this is equivalent to `new CommandLineRunner(args)`. Before
   the call, `System.in`, `System.out` and `System.err` are swapped for an in-memory stdin and
   two capture streams. The capture streams use the same charsets as the JVM's own
   (`stdout.encoding` and `stderr.encoding`).
3. If `shouldRunCompiler()`, it calls `run()`.
4. If `hasErrors()`, the exit code is -1.

**Exit codes.** `System.exit` is replaced with `setExitCodeReceiver`. The receiver captures the
first code and applies the same conversion as `SystemExitCodeReceiver`:
`(byte) code`, with 0 mapped to -1 if the code was non-zero. The result is reported as the
unsigned process status the shell sees: -1 → 255, -2 → 254. An exception that escapes `main`
is printed as the JVM's default handler prints it (`Exception in thread "main" ` plus the
stack trace) and gives exit code 1. Stack frames in that case include oracle frames, which is a
known difference.

**`output_files`** contains every file opened through
`AbstractCommandLineRunner.filenameToOutputStream`. That covers `--js_output_file`, chunk
outputs, source maps, renaming maps, the output manifest and so on. It also contains the paths
named by `--typed_ast_output_file`, `--json_warnings_file`, `--tracer_output` and
`--save_state`. Each entry is the file's content after the compile. The files stay on disk,
as they would with `java -jar`.

**`input_order`** lists the source names of `Compiler.getInputsInOrder()` after the run, that
is, after dependency sorting and pruning. It is null if no compiler was created. It includes
WEAK inputs (with `--checks_only` they are not removed), so it is not enough for the seam.

**`inputs`** is the same list as `[{"name", "kind"}]`, where `kind` is the input's `SourceKind`
(`STRONG`, `WEAK`, ...). **`chunks`** is `JSChunkGraph.getAllChunks()` after dependency
management: `[{"name", "deps":[...], "inputs":[{"name","kind","fill_file"}]}]`, including the
synthetic `$strong$`/`$weak$` chunks and fill files (`<chunk>$fillFile`). Both are null if no
compiler was created. The seam needs them: see `optimize_from_typedast` and "Seam caveats".

**Inline files.** `{"files":{"relpath":"content",...}}` writes each file to
`tmp/inline-<pid>-<n>/relpath` next to the oracle jar (`build/oracle-v20261006/tmp/` for the
default reference). The request then runs in a child JVM (`Main request`) whose cwd is that
directory, so relative paths in `args` and in the output mean the same as for `java -jar` run in
that directory. The response adds `"inline_dir"`.
This costs one fresh JVM per request.

### `compile_with_pass_dumps`

The request is the same as `compile`. The oracle appends `--print_source_after_each_pass`,
and the response adds `"passes":[{"pass":name,"source":js},...]` in pass order.

**Source maps are stripped.** `--create_source_map` (both `=v` and separate-value forms) is
removed from the argv and listed in the response's `"stripped_args"`. Reason: in Closure
itself, `maybePrintSourceAfterEachPass` calls `getCurrentJsSource`, which calls
`resetAndIntitializeSourceMap` and `toSource` (Compiler.java:1550-1582); with source-map
generation on, this throws `IllegalStateException: Incorrect source mappings order` in
`SourceMapGeneratorV3.addMapping`, exits 254 and stops the dumps (`java -jar` with
`--print_source_after_each_pass` does the same). Source-map generation does not change the JS,
so the dumps and out.js equal those of the argv without the flag; no `.map` file is written.
Bisect source-map (D5) differences with `compile` itself.

**Where Closure prints these.** `PhaseOptimizer` (PhaseOptimizer.java:245) calls
`compiler.afterPass(name)` after every pass, which calls
`Compiler.maybePrintSourceAfterEachPass` (Compiler.java:1550). That method prints straight to
`System.err`, not to the runner's err stream:

```
"\n" ["// DEBUG: " msg "\n"] "// " + passName + " yields:\n"
"// ************************************\n" + toSource() + "\n"
"// ************************************\n"
```

A block is printed **only when the source changed** since the last printed block
(`lastJsSource`). Passes that leave the source unchanged therefore do not appear.

**Parsing.** A block starts at a header that matches
`^// (DEBUG: …\n// )?(.+) yields:\n// \*{36}\n`. Its source runs up to the **last**
`\n// ************************************\n` before the next header, or before the end of
stderr. This works even if a source contains the marker line itself. Ordinary warnings and
errors are written at the end of the compile, after the last block.

The parse is only ambiguous in one case: a diagnostic printed after the final block that itself
contains the marker line followed by a newline.

The raw stderr is still returned in `stderr_b64`. The `--print_source_after_each_pass` flag
is part of the argv that runs, so stdout and exit code are those of that argv.

### `checks_to_typedast`

```json
{"op":"checks_to_typedast","args":[...]} -> {...compile fields..., "typedast_b64":"<gzip TypedAst.List>"|null}
```

The oracle runs `args + ["--checks_only", "--typed_ast_output_file=build/oracle/tmp/reqNNN.typedast.gz"]`
(the path is relative to the server's cwd, the repository root) and returns the file's bytes: a
gzipped `TypedAst.List` holding one `TypedAst`. Then it deletes the file.

Why these flags:
- `DefaultPassConfig.getChecks()` (DefaultPassConfig.java:473) adds `serializeTypedAst`
  (`SerializeTypedAstPass.createFromPath`) as the **last** check, after `BEFORE_SERIALIZATION`,
  whenever `options.getTypedAstOutputFile() != null`.
- `--checks_only` sets `setChecksOnly(true)` and `OutputJs.NONE` (CommandLineRunner.java:1871).
  With that set, `shouldOptimize()` is false, so `stage2Passes` and `stage3Passes` are no-ops,
  which matches how `TypedAstIntegrationTest.precompileLibrary` drives it.
- Without `--checks_only` the compile would also optimize. In addition, `typedAstOutputFile`
  redirects runtime-library injection into a synthetic type-summary input
  (`Compiler.getNodeForRuntimeCodeInsertion`).

### `optimize_from_typedast`

```json
{"op":"optimize_from_typedast","args":[...],"typedast_b64":"...",   (or "typedast_path": "file")
 "weak_inputs":["path", ...], "fill_inputs":["c0$fillFile", ...]}    (both optional)
-> {...compile fields...}
```

- **`weak_inputs`:** `--js` files to mark `SourceKind.WEAK` when inputs are created
  (`OracleRunner.createInputs`; the kind `--weakdep` would give, which `CommandLineRunner`
  lacks). Stage 2's `removeWeakSources` then empties them, as the full compile does. Pass the
  checks response's inputs whose `kind` is `WEAK`. In chunk mode list them last and count them
  in the last chunk: `JSChunkGraph` moves inputs marked WEAK into `$weak$`.
- **`fill_inputs`:** `--js` names of the form `<chunk>$fillFile` to create as empty in-memory
  inputs (as `Compiler.fillEmptyChunks` does). Needed when dependency management emptied a
  chunk: a root chunk may not be empty (`JSC_EMPTY_ROOT_CHUNK_ERROR`).
- `oracle/test/seam3.py:opt_args` builds the optimize argv, `--chunk` flags and both lists from
  a checks response.

**How it runs.** The oracle constructs `OracleRunner(args)` and calls
`getCommandLineConfig().setTypedAstListInputFilename(tmpfile)`, which has no CLI flag. Then it
follows the same `main` replica as `compile`.

**The stage that runs.** With `saveAfterCompilationStage == -1`,
`shouldRestoreTypedAstsPerformStages2And3()` is true, so `runCompilerPasses` performs
"skip-checks compile": stage 2 (optimizations) and stage 3 (finalizations). The inputs are
initialised by `Compiler.initWithTypedAstFilesystem`, or by `initChunksWithTypedAstFilesystem`
when chunks are given. Both call `options.setMergedPrecompiledLibraries(true)` and
deserialise every `--js` and `--externs` file from the TypedAST.

**Required args.** `args` must name the same `--js` and `--externs` files as the checks run.
`Compiler.initOptions` (Compiler.java:466) **throws** if dependency management is on
(`SORT_ONLY`, `PRUNE` or `PRUNE_LEGACY`). The optimize args must therefore drop
`--entry_point` and use `--dependency_mode=NONE`; the `--js` files must therefore be passed in
the checks run's post-sort order (`inputs`), or the output concatenates the files in a different
order.

### Seam caveats

`optimize_from_typedast(checks_to_typedast(argv))` is not always output-identical to
`compile(argv)` (DECISIONS.md D-011):

- **Order, weak inputs, chunks.** The optimize run must get the inputs in the checks run's
  post-sort order, with WEAK inputs as `weak_inputs` (`--checks_only` skips stage 1's
  `removeWeakSources`, DefaultPassConfig.java:463), and the chunks as the checks response's
  `chunks` with their fill files: dependency management moves inputs between chunks, which
  cannot be rebuilt from the original `--chunk` flags. `oracle/test/seam3.py:opt_args` does
  all three.
- **Diagnostics.** The full compile prints one sorted report for all stages; the seam prints the
  checks run's report and the optimize run's report separately, so stderr differs whenever
  optimization passes warn (for example `JSC_PARTIAL_NAMESPACE` from CollapseProperties) or
  the `% typed` suffix applies. Warnings lose their `Originally at:` blocks (input source maps
  are not restored), and node lengths (`[length: N]`, caret underlines) differ because the full
  compile renders check-phase warnings after optimizations changed the nodes.
- **Data the TypedAST does not carry.** Under `--formatting=PRETTY_PRINT`, CodePrinter takes a
  number's spelling from the original source text (CodePrinter.java:315-345), so `0.5` becomes
  `.5`; with `--create_source_map`, input source maps are not restored, so `sources`, `names`
  and `mappings` differ.
- **`hasRegExpGlobalReferences` (SIMPLE-level profiles).** `AstAnalyzer` treats `/re/.test(x)`
  and `/re/.exec(x)` as side-effect free only if `!hasRegexpGlobalReferences`
  (AstAnalyzer.java:229-230). The flag lives on the `Compiler`, starts as `true`, and in a full
  compile is set only by `checkRegExp`, which runs only under ADVANCED
  (`shouldComputeFunctionSideEffects`). The restore always runs `checkRegExpForOptimizations`
  (DefaultPassConfig.java:843-849), which sets it from the AST (`false` unless the code reads a
  global RegExp property such as `RegExp.$1`). So under SIMPLE-level profiles the seam can
  inline or remove code the full compile keeps; `oracle/test/seam_regexp.py` reproduces this.
- **Promoted errors.** An ERROR whose diagnostic type is a WARNING by default (for example via
  `--jscomp_error=*`) does not halt the pass loop (`SortingErrorManager.hasHaltingErrors`). The
  checks run still writes a TypedAST and exits with the error count; the full compile runs all
  stages and writes **no** JS, with the same exit code; `optimize_from_typedast` exits 0 and
  writes JS. If the checks response has `exit_code != 0`, the expected result is therefore the
  compile's exit code, stderr and absence of output. Halting errors (an ERROR-level type such as
  `JSC_PARSE_ERROR`) write no TypedAST, and the compile stops in stage 1 like the checks run.
- **WHITESPACE_ONLY** writes no TypedAST, so the `ws` profile has no seam.

A Rust back end fed a Java TypedAST is therefore compared with `optimize_from_typedast`, never
with `compile`.

### `parse_dump`

```json
{"op":"parse_dump","path":"rel/file.js" | "content":"...", "name":"optional source name",
 "language_in":"ECMASCRIPT_2020", "args":[optional extra CLI flags], "jsdoc_parsing":"optional Config.JsDocParsing",
 "kind":"extern" (optional)}
```

The schema is in `PARSE_DUMP.md`.

## Identity checks

`oracle/test/identity.py --isolate none --n 300` compares, byte for byte:
- one fresh `Main compile` JVM per request against `java -jar`, on every 5th request;
- one `--isolate=none` server, fed the requests in a seeded shuffled order, against fresh JVMs,
  on all requests.

`gates/gate_0_1.sh` (Gate 0.1) compares long-lived `--isolate=none` servers with the golden
`java -jar` results on every D2 pair. `oracle/test/seam.py`, `seam3.py` and `seam3_report.py`
compare `compile` with the seam (see "Seam caveats").
