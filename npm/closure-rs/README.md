# closure-rs

A Rust port of the [Closure Compiler](https://github.com/google/closure-compiler) command line
(`CommandLineRunner`), with an npm wrapper that has the programmatic API of the official
[`google-closure-compiler`](https://www.npmjs.com/package/google-closure-compiler) package
(matched against version 20261006.0.0). Not affiliated with or published by Google.

The compiler is a native binary. The package carries one per supported system (Linux x64 and
Windows x64 for now, under `bin/<platform>-<arch>/`) and runs the one for yours; no Java is needed.
The Linux binary is statically linked: it runs on any x86-64 Linux, glibc or musl (Alpine, without
gcompat), with no C library version requirement.

## Switching from google-closure-compiler

Change only `package.json`:

```json
"devDependencies": {
  "google-closure-compiler": "npm:closure-rs@latest"
}
```

Code that imports `google-closure-compiler` keeps working, TypeScript types included:

```ts
import ClosureCompiler from 'google-closure-compiler'; // or: import {compiler} from ...

new ClosureCompiler({
  js: ['src/a.js', 'src/b.js'],
  compilation_level: 'ADVANCED',
  js_output_file: 'out.js',
}).run((exitCode, stdout, stderr) => {
  // ...
});
```

## API

The same exports as the official package's `index.js`:

| Export | closure-rs |
|---|---|
| `compiler` (also the default export) | The `Compiler` class: `new compiler(args, extraCommandArgs?)`, `run(callback?)` returning the `ChildProcess`, `commandArguments`, `javaPath`, `JAR_PATH`, `logger`, `spawnOptions`, `getFullCommand()`, `prependFullCommand()`, `formatArgument()`. |
| `javaPath` | The executable `compiler` spawns: the closure-rs binary (see below). |
| `CONTRIB_PATH`, `EXTERNS_PATH` | Absolute paths of the bundled `contrib/` and `externs/` folders, the same files as in the official package. |
| `JAR_PATH` | `null`: there is no jar. |
| `gulp`, `grunt` | Present, but **not implemented**: calling either throws an error (see below). |
| `COMPILER_PATH` | Added: the path of the closure-rs binary. |

`compiler.JAR_PATH`, `compiler.COMPILER_PATH` and `compiler.CONTRIB_PATH` exist as statics too,
because `@types/google-closure-compiler` declares them.

The package is ESM like the official one (`"type": "module"`). `require('google-closure-compiler')`
works where Node supports `require()` of ES modules (Node 20.19+, 22.12+), as for the official
package. Deep imports such as `google-closure-compiler/lib/node/index.js` and
`google-closure-compiler/lib/utils.js` exist as in the official package.

### Options and flags

`new compiler(args)` takes either an array of arguments, passed to the compiler unchanged, or an
object whose keys are flag names without `--`:

- `{key: 'value'}` becomes `--key=value`; `true` / `false` become `--key=true` / `--key=false`.
- An array value repeats the flag: `{js: ['a.js', 'b.js']}` gives `--js=a.js --js=b.js`.
- camelCase keys become snake_case: `compilationLevel` is `--compilation_level`.
- `null` or `undefined` gives the bare `--key`, as in the official package, so leave flags you do
  not set out of the object.

No shell is involved, so values need no quoting. The flags are the Java compiler's
(`CommandLineRunner`), with the same meaning and output.

`run(callback)` starts the process and returns it. The callback receives `(exitCode, stdout,
stderr)` once the process has closed; for a non-zero exit code `stderr` starts with the full
command line. If the process is ended by a signal (for example killed by the out-of-memory
killer), `exitCode` is 128 + the signal number, as a shell reports it (137 for `SIGKILL`), and
`stderr` names the signal. The official package passes `null` in that case. Without `--js` the compiler reads the input from stdin: write it to the returned
process's `stdin` and end it.

### Types

`index.d.ts` is generated from the compiler's flag table (`scripts/gen_npm_types.mjs`). Every flag
is a typed, documented property of `CompilerFlags`: booleans, numbers, `string[]` for repeatable
flags, and value lists such as `CompilationLevel`, `WarningLevel`, `LanguageMode` or
`FormattingOption` for the editor's suggestions. To keep build files that compile against
`@types/google-closure-compiler` compiling, any string is still accepted where a flag takes a
value and unknown keys are allowed; the types reject booleans for flags that take strings and
arrays for flags that take one value. The names `@types/google-closure-compiler` declares
(`Compiler`, `CompileOptions`, `CompileOption`, `GulpPluginOptions`, ...) are exported too.

### The binary

The wrapper runs, in this order:

1. the file named by the environment variable `CLOSURE_RS_BINARY`, if set (for testing a local
   build);
2. the binary bundled in this package, `bin/<platform>-<arch>/closure-rs[.exe]`;
3. `bin/closure-rs` from a separate package `closure-rs-<platform>-<arch>`, if installed;
4. `closure-rs` on `PATH`.

### Java-only parts

- `JAR_PATH` is `null`, and `extraCommandArgs` (JVM arguments in the official package) are
  ignored. Setting `JAR_PATH` and `javaPath` yourself makes `run()` start that jar with Java, as
  the official class does.
- Setting `javaPath` replaces the executable that is started; it is not a path to Java.
- The `platform` choice of the official command line (`--platform=native,java`) and plugins is
  accepted, and every choice runs closure-rs.

### Not implemented: gulp and grunt plugins

The official gulp plugin streams Vinyl files through the compiler's `--json_streams` mode and merges
source maps with the `vinyl`, `vinyl-sourcemaps-apply` and `chalk` packages; the grunt task is built
on it. That is not a thin layer over the compiler class, and closure-rs has no dependencies, so
`gulp(...)` and `grunt(...)` throw an error that says so. Their TypeScript declarations are kept so
that code that only mentions them still compiles.

### Command line

`npx closure-rs ...` (or `npx google-closure-compiler ...` when installed under that alias) passes
all arguments to the binary, except `--platform`, which is removed.

## License

Apache-2.0. The bundled `externs/` and `contrib/` folders are from the Closure Compiler repository
(Apache-2.0). The binaries contain code under other licenses (translated Java libraries, Rust
crates, and in the Linux binary the musl C library and LLVM runtime code); `NOTICE` and
`LICENSES/` in the package list them with their license texts.
