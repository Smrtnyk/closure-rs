// A build script written against google-closure-compiler's documented API, as a user writes it.
// It must type-check unchanged when 'google-closure-compiler' is the closure-rs package
// (package.json: "google-closure-compiler": "npm:closure-rs@<version>"). See check.mjs.
import ClosureCompiler, {compiler, CONTRIB_PATH, EXTERNS_PATH, JAR_PATH, javaPath} from 'google-closure-compiler';
import * as gcc from 'google-closure-compiler';
import type {Compiler, CompileOptions, CompilerOptions, CompilationLevel} from 'google-closure-compiler';
import {writeFileSync} from 'node:fs';
import path from 'node:path';

// README style: default import, inline options object.
const closureCompiler = new ClosureCompiler({
  js: ['src/a.js', 'src/b.js'],
  externs: path.join(CONTRIB_PATH, 'nodejs', 'fs.js'),
  compilation_level: 'ADVANCED',
  language_out: 'ECMASCRIPT_2020',
  warning_level: 'VERBOSE',
  formatting: ['PRETTY_PRINT', 'SINGLE_QUOTES'],
  jscomp_off: ['checkVars', 'uselessCode'],
  define: ['DEBUG=false'],
  isolation_mode: 'IIFE',
  dependency_mode: 'PRUNE',
  entry_point: 'goog:app',
  create_source_map: 'out.js.map',
  js_output_file: 'out.js',
  debug: true,
  rewrite_polyfills: false,
  summary_detail_level: 3,
  assumeFunctionWrapper: true,
  some_future_flag: 'value',
});
const compilerProcess = closureCompiler.run((exitCode: number, stdOut: string, stdErr: string) => {
  if (exitCode !== 0) {
    throw new Error(stdErr);
  }
  writeFileSync('out.js', stdOut);
});
compilerProcess.on('exit', () => {});

// @types/google-closure-compiler style: named `compiler`, options built up front (values widen to
// string), the Compiler interface, javaPath / logger / spawnOptions / getFullCommand.
const options = {
  js: 'in.js',
  compilation_level: 'SIMPLE',
  language_in: 'ECMASCRIPT_NEXT',
  checks_only: 'true',
};
const c: Compiler = new compiler(options, ['-Xms2048m']);
c.javaPath = javaPath;
c.logger = console.error;
c.spawnOptions = {stdio: 'pipe', cwd: process.cwd()};
console.log(c.getFullCommand());
const loose: CompileOptions = {js: ['x.js'], debug: true};
new gcc.compiler(loose);
new gcc.default(['--js', 'in.js', '--compilation_level', 'ADVANCED']);
const fromRecord: Record<string, string | boolean | string[]> = {js: 'x.js'};
new compiler(fromRecord);

// Static paths declared by @types/google-closure-compiler.
console.log(compiler.CONTRIB_PATH, compiler.COMPILER_PATH, compiler.JAR_PATH, JAR_PATH, EXTERNS_PATH);

// Compile from stdin through the returned child process.
const stdinRun = new compiler({compilation_level: 'ADVANCED'}).run((code, out, err) => console.log(code, out, err));
stdinRun.stdin?.write('var x = 1;');
stdinRun.stdin?.end();

// Promise wrapper, a common pattern.
function compile(flags: CompilerOptions): Promise<string> {
  return new Promise((resolve, reject) => {
    new ClosureCompiler(flags).run((code, out, err) => (code === 0 ? resolve(out) : reject(new Error(err))));
  });
}
const level: CompilationLevel = 'ADVANCED';
void compile({js: 'a.js', compilation_level: level, compilationLevel: 'advanced'});

// The typed flags still catch values that cannot be right.
// @ts-expect-error compilation_level takes a string, not a boolean
void compile({compilation_level: true});
// @ts-expect-error js_output_file is not repeatable
void compile({js_output_file: ['a.js', 'b.js']});
// @ts-expect-error summary_detail_level is an integer
void compile({summary_detail_level: true});

// gulp/grunt type-check (they throw at runtime in closure-rs).
type Gulp = typeof gcc.gulp;
type Grunt = typeof gcc.grunt;
export type {Gulp, Grunt};
