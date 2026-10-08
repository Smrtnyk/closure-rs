# CLI port notes

`closure-cli` ports `AbstractCommandLineRunner`, `CommandLineRunner` and the parts of args4j,
Gson and the JDK the pinned CLI reaches. The `closure-rs` binary (`src/main.rs`) is the runner
with the process streams; tests inject their own streams. These notes record where the Rust shape
differs from Java and why. Of `CommandLineRunnerTest`, only the three tests that need
CoverageInstrumentationPass (`testInstrumentCodeByLine`, `testInstrumentCodeByBranch`,
`testInstrumentCodeProductionCreatesInstrumentationMapping`) are not ported: instrumentation is out
of scope (docs/PORTING.md §2).

## Flags and args4j

- All `Flags` declarations of the pinned `CommandLineRunner` (in-scope and out-of-scope ones) are
  registered in Java field order in the static `command_line_runner.rs::OPTIONS` table and written
  through `Flags::set_value`. Java builds this table by reflection over `@Option`/`@Argument`
  annotations; the port has static `NamedOptionDef` descriptors whose handler is a `HandlerKind`
  (`args4j/spi/option_handler.rs`), and field and `AnnotatedElement` reflection are represented by
  the same descriptor. `CmdLineParser#addOption`/`#addArgument` therefore have no runtime
  registration path.
- Only the args4j surface the pinned CLI reaches is ported: option/argument parsing, the custom
  boolean semantics, `=` and spaced values, aliases, at-file expansion, flag-file tokenization and
  ordering, nested-flag-file errors, usage wrapping and sorting, categories, markdown help and
  version. General-purpose args4j APIs that the CLI never calls are not ported.
- The ordered JS/ZIP/warning flags use the concrete `MultiFlagSetter`.
  `FlagEntry#equals`/`#hashCode` have explicit bodies; the hash adds the flag's `JavaHashCode` to
  Java `String.hashCode` with Java integer overflow.
- Rust identifiers spell Java's `Allowlist` as `allow_list` (`add_allow_list_warnings_guard`,
  `set_warnings_allow_list_file`); the flag strings and messages keep Java's text, and the
  historical alias `--warnings_whitelist_file` stays registered.
- The flag tables, `CommandLineConfig`, the option setup and the compilation-level bodies were
  first transcribed mechanically from the pinned sources by the generators in `tools/`
  (`generate_flags.py`, `generate_config.py`, `port_levels.py`, ...); the Java helpers there
  (`CliMetadata.java`, `CliOptionSetup.java`, `CliJson.java`, `CliGlob.java`) dump the reference
  behaviour. The generators read their inputs from the gitignored `corpus-cache/` and `build/`
  directories of the checkout they ran in and are kept as provenance.

## Runner structure

- Java's abstract base class is the `RunnerMethods<A, B>` trait plus the shared
  `AbstractCommandLineRunner<A, B>` state. Where a Rust borrow would cross the abstract/concrete
  boundary, the Java abstract and shared method bodies stay together in the concrete runner. The
  six abstract declarations (`createCompiler`, `prepForBundleAndAppendTo`, `appendRuntimeTo`,
  `createOptions`, `addAllowlistWarningsGuard`, `getVersionText`) are ported as the
  `CommandLineRunner#...` overrides.
- `getBuiltinExterns` delegates to `closure-resources`; its private helper `getExternsInput` is
  ported there (`crates/resources/src/abstract_command_line_runner.rs`), not duplicated.
- The CLI drives `closure_jscomp::compiler::Compiler` directly, including
  `saveState`/`restoreState` and the typed-AST filesystem entry points. The compiler's output
  stream is `Send`: it writes into an ordered channel that the runner drains into the injected
  error stream after reports and when `doRun` returns or unwinds (`java_io.rs`).
- `--logging_level` is parsed into the full `java.util.logging.Level`
  (`java_util_logging_level.rs`; named and integer levels) and passed to
  `Compiler#setLoggingLevel` before `createCompiler()`, as in Java.
- `--print_ast` calls the real `DotFormatter::append_dot(compiler, root, Some(&cfg), ..)`.

## Streams, encodings and exceptions

- Compiler output stays a `JsString` (UTF-16) through both `writeOutput` overloads, the JavaScript
  escaper, chunk output and the JSON `StringBuilder`s; `EncodedWriter` applies the charset only at
  the output boundary, so lone surrogates are encoded exactly as Java does.
- The shared `closure_rhino::java_lang::charset::Charset` supports UTF-8, US-ASCII, ISO-8859-1,
  UTF-16, UTF-16BE and UTF-16LE (with their JDK aliases). Other JDK charsets (the reference JDK
  lists 167 more canonical encodings, such as Shift_JIS, GB18030 or windows-1252) are not
  supported: `Charset::for_name` panics on them. Support belongs in rhino's `java_lang`, not in a
  CLI-local decoder.
- Injected `PrintStream`s keep Java's suppressed-`IOException` ("trouble") state and shared close
  ownership; `closeAppendable` flushes before closing; a writer abandoned on an exceptional path
  leaves its stream open.
- `run` returns -1 for `FlagUsageException` and -2 for any other throwable. I/O, Gson
  (`IllegalStateException`, `EOFException`, `JsonSyntaxException`) and `VariableMap` parse errors
  keep their Java exception class and cause chain; cause stacks use
  `Throwable#printEnclosedStackTrace`'s shared-tail elision, and file-output failures keep the
  pinned JDK frames. Exceptions thrown inside the compiler are not converted into invented Java
  exceptions.

## Gson and globs

- `src/gson/` ports the Gson reader and writer subset the CLI uses: UTF-16 string values, the
  object reader's leniency, `@SerializedName` aliases, nulls, compact output even though the
  externally supplied `JsonWriter` asks for pretty printing, HTML escaping disabled, and
  U+2028/U+2029 escaping.
- `jdk_globs.rs` ports the JDK Unix glob conversion and matching (JDK error indices and messages,
  ranges, intersections, braces, escapes, supplementary characters, malformed classes). Directory
  walks use the native directory order, follow links and detect ancestor cycles; explicit path
  lists are sorted by Java UTF-16 order where Java uses a `TreeMap`.

## Remaining gaps

- `src/stand_in/chrome_coding_convention.rs` is the only stand-in: ChromeCodingConvention is
  Chrome-specific and out of scope (docs/PORTING.md §2).
- The option-setup fixture compares a projection of `CompilerOptions`. Fields holding objects
  without a value representation are not asserted: `experimentalOutputFeatureSet`,
  `messageBundle`, `inputSourceMaps`, `inputVariableMap`, `inputPropertyMap`, `nameGenerator`,
  `customPasses`, `cssRenamingMap`, `idGenerators`, `xidHashFunction`, `chunkIdHashFunction`,
  `stateCompressionWrapper`, `stateDecompressionWrapper`, `warningsGuard`,
  `extraReportGenerators`, `errorHandler`, `conformanceConfigs`, `conformanceRemoveRegexFromPath`,
  `zoneInputPattern`.
- The shared `CheckLevel` has no `JavaHashCode` (Java's enum identity hash is JVM-dependent); no
  ordinal-based substitute is used.

## Test fixtures

`tests/data/` holds goldens recorded from the reference jar: CLI argv runs (`cli_golden/`: exit
code, stdout and stderr bytes, including help, markdown help and version), option setup
(`option_setup.json`), Gson (`json_golden.json`), globs (`glob_golden.json`) and runtime exception
boundaries (`exception_golden.json`). They were produced by the matching `tools/generate_*.py`
scripts; Rust tests read them without a JVM.
