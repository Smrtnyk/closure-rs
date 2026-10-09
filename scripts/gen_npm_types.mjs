#!/usr/bin/env node
// Generates npm/closure-rs/index.d.ts (and the identical index.d.cts), the TypeScript declarations
// of the npm wrapper.
//
//   node scripts/gen_npm_types.mjs           write npm/closure-rs/index.d.ts and index.d.cts
//   node scripts/gen_npm_types.mjs --check   exit 1 if they are not what the sources give
//
// Sources (read only):
//   scope/flags.txt                          every CommandLineRunner flag in source order, with its
//                                            usage text, default, --help category and port scope
//                                            (generated from the pinned Java source; docs/PORTING.md §2)
//   crates/cli/src/command_line_runner.rs    OPTIONS, the binary's flag table: option handler
//                                            (boolean / int / string / enum values / map),
//                                            aliases, hidden, multi-valued
//   crates/jscomp/src/compilation_level.rs   CompilationLevel::from_string (--compilation_level)
//   crates/jscomp/src/compiler_options.rs    LanguageMode and valid_command_line_names
//                                            (--language_in, --language_out)
// The two flag lists must agree (names, aliases, hidden); the script stops otherwise.
//
// The declarations of the exports themselves (compiler, gulp, grunt, the paths) are the fixed
// template at the end of this file. They follow the runtime in npm/closure-rs/ and stay
// assignment-compatible with @types/google-closure-compiler@20231112.0.0, the types users of the
// official package (which ships none) compile against.
import {readFileSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const OUT = join(ROOT, 'npm', 'closure-rs', 'index.d.ts');
const read = (p) => readFileSync(join(ROOT, p), 'utf8').replace(/\r\n/g, '\n');
const fail = (msg) => {
  console.error(`gen_npm_types: ${msg}`);
  process.exit(2);
};

// ---- scope/flags.txt ---------------------------------------------------------------------------
const COLS = ['scope', 'flag', 'aliases', 'type', 'default', 'hidden', 'category', 'reason', 'usage'];
const scopeRows = read('scope/flags.txt').split('\n')
    .filter((l) => l && !l.startsWith('#'))
    .map((l) => {
      const cells = l.split('\t');
      if (cells.length !== COLS.length) fail(`scope/flags.txt: bad row: ${l}`);
      return Object.fromEntries(COLS.map((c, i) => [c, cells[i]]));
    });

// ---- crates/cli OPTIONS -------------------------------------------------------------------------
const runner = read('crates/cli/src/command_line_runner.rs');
const tableStart = runner.indexOf('pub static OPTIONS: &[NamedOptionDef] = &[');
if (tableStart < 0) fail('OPTIONS table not found in crates/cli/src/command_line_runner.rs');
const tableEnd = runner.indexOf('\n];', tableStart);
const table = runner.slice(tableStart, tableEnd);
const strList = (s) => [...s.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((m) => m[1]);
const rustOptions = table.split('NamedOptionDef {').slice(1).map((block) => {
  const field = (name) => {
    const m = new RegExp(`\\n\\s*${name}: ([\\s\\S]*?),\\n\\s*(?:[a-z_]+:|\\},?)`).exec(block);
    if (!m) fail(`OPTIONS entry without ${name}: ${block.slice(0, 120)}`);
    return m[1].trim();
  };
  const handlerSrc = field('handler');
  const hm = /^HandlerKind::(\w+)(?:\(&\[([\s\S]*)\]\))?$/.exec(handlerSrc);
  if (!hm) fail(`unknown handler ${handlerSrc}`);
  return {
    name: strList(field('name')).join(''), // also covers concat!("..", "..")
    aliases: strList(field('aliases')),
    hidden: field('hidden') === 'true',
    multi: field('multi_valued') === 'true',
    handler: hm[1],
    values: hm[2] ? strList(hm[2]) : null,
  };
});

// ---- value sets parsed from strings by the runner -----------------------------------------------
const levelSrc = read('crates/jscomp/src/compilation_level.rs');
const fromString = /pub fn from_string\([\s\S]*?\n {4}\}/.exec(levelSrc);
if (!fromString) fail('CompilationLevel::from_string not found');
const compilationLevels = [...fromString[0].matchAll(/^\s*((?:"\w+"\s*\|?\s*)+)=>/gm)]
    .flatMap((m) => strList(m[1]));
if (compilationLevels.length < 5) fail('CompilationLevel::from_string: too few names');

const optionsSrc = read('crates/jscomp/src/compiler_options.rs');
const lmEnum = /pub enum LanguageMode \{([\s\S]*?)\}/.exec(optionsSrc);
if (!lmEnum) fail('enum LanguageMode not found');
const lmVariants = [...lmEnum[1].matchAll(/^\s*(\w+),/gm)].map((m) => m[1]);
const lmValid = /fn valid_command_line_names\(\)[\s\S]*?names\.extend\(\[([^\]]*)\]/.exec(optionsSrc);
if (!lmValid || !/!= Self::UNSUPPORTED/.test(lmValid[0]) || !/strip_prefix\("ECMASCRIPT"\)/.test(lmValid[0])) {
  fail('LanguageMode::valid_command_line_names has changed shape; update gen_npm_types.mjs');
}
// valid_command_line_names: every mode but UNSUPPORTED, plus "ES<suffix>" for "ECMASCRIPT<suffix>",
// plus the extra ES6 spellings.
const languageModes = [];
for (const v of lmVariants.filter((v) => v !== 'UNSUPPORTED')) {
  languageModes.push(v);
  if (v.startsWith('ECMASCRIPT')) languageModes.push('ES' + v.slice('ECMASCRIPT'.length));
}
languageModes.push(...strList(lmValid[1]));

// ---- join and check ------------------------------------------------------------------------------
if (rustOptions.length !== scopeRows.length) {
  fail(`scope/flags.txt has ${scopeRows.length} flags, the OPTIONS table ${rustOptions.length}`);
}
const flags = scopeRows.map((row, i) => {
  const opt = rustOptions[i];
  const aliases = row.aliases === '-' ? [] : row.aliases.split(',');
  if (opt.name !== row.flag || opt.aliases.join(',') !== aliases.join(',') ||
      opt.hidden !== (row.hidden === 'yes')) {
    fail(`flag ${i}: scope/flags.txt says ${row.flag} [${aliases}] hidden=${row.hidden}, ` +
        `OPTIONS says ${opt.name} [${opt.aliases}] hidden=${opt.hidden}`);
  }
  return {...row, ...opt, aliases};
});

// ---- types --------------------------------------------------------------------------------------
const STRING_ENUMS = {
  '--compilation_level': ['CompilationLevel', compilationLevels],
  '--language_in': ['LanguageMode', languageModes],
  '--language_out': ['LanguageMode', languageModes],
};
const RENAMED_ENUMS = {Format: 'SourceMapFormat'};
const enumTypes = new Map(); // name -> {values, flags}
const addEnum = (name, values, flag) => {
  const prev = enumTypes.get(name);
  if (prev && prev.values.join() !== values.join()) fail(`enum ${name} has two value sets`);
  if (prev) prev.flags.push(flag);
  else enumTypes.set(name, {values, flags: [flag]});
};

const valueType = (f) => {
  let t;
  switch (f.handler) {
    case 'ClosureBoolean': t = 'BooleanFlag'; break;
    case 'Boolean': t = 'null'; break;
    case 'Int': t = 'IntFlag'; break;
    case 'Map': t = 'string'; break;
    case 'Enum': {
      const javaType = f.type.replace(/^List<(.*)>$/, '$1').split('.').pop();
      const name = RENAMED_ENUMS[javaType] || javaType;
      addEnum(name, f.values, f.flag);
      t = `EnumFlag<${name}>`;
      break;
    }
    case 'String': {
      const known = STRING_ENUMS[f.flag];
      if (known) {
        addEnum(known[0], known[1], f.flag);
        t = `EnumFlag<${known[0]}>`;
      } else {
        t = 'string';
      }
      break;
    }
    default: fail(`unhandled handler ${f.handler} for ${f.flag}`);
  }
  return f.multi ? `Repeatable<${t}>` : t;
};

const cleanDefault = (d) => {
  if (d === 'null' || d.startsWith('new ')) return null;
  if (/^".*"$/.test(d)) return d.slice(1, -1);
  if (/^Level\.(\w+)\.getName\(\)$/.test(d)) return d.replace(/^Level\.(\w+)\.getName\(\)$/, '$1');
  return d.split('.').pop();
};
const docText = (s) => s.replace(/\*\//g, '*\\/');
const camel = (s) => s.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
const propKey = (k) => (/^[A-Za-z_$][\w$]*$/.test(k) ? k : `'${k}'`);

const members = [];
for (const f of flags) {
  const type = valueType(f);
  const doc = [];
  if (f.usage) doc.push(docText(f.usage), '');
  const facts = [`\`${f.flag}\``];
  if (f.aliases.length) facts.push(`(alias ${f.aliases.map((a) => `\`${a}\``).join(', ')})`);
  const def = cleanDefault(f.default);
  let line = facts.join(' ') + '.';
  if (def !== null && def !== '') line += ` Default: \`${def}\`.`;
  if (f.category !== '(uncategorized)') line += ` Category: ${f.category}.`;
  if (f.hidden === true) line += ' Hidden in --help.';
  doc.push(line);
  if (f.multi) doc.push('Repeatable: an array gives the flag once per item.');
  if (f.handler === 'Boolean') doc.push('Takes no value: pass null to emit the bare flag.');
  if (f.scope === 'out') doc.push(`Outside the closure-rs port scope (${docText(f.reason)}).`);
  const name = f.flag.replace(/^--/, '');
  // Long aliases work as keys too; uppercase ones do not (the wrapper turns camelCase into
  // snake_case, so `D` would become `--_d`).
  const keys = [name, ...f.aliases.filter((a) => a.startsWith('--') && !/[A-Z]/.test(a))
      .map((a) => a.slice(2))];
  for (const key of keys) {
    for (const k of new Set([key, camel(key)])) {
      const extra = k !== name ? [`Same flag as \`${name}\`.`] : [];
      const lines = [...doc, ...extra];
      members.push(
          '  /**\n' + lines.map((l) => (l ? `   * ${l}` : '   *')).join('\n') + '\n   */\n' +
          `  ${propKey(k)}?: ${type};`);
    }
  }
}

const enumDecls = [...enumTypes].map(([name, {values, flags: fl}]) =>
  `/** Values of ${fl.map((f) => `\`${f}\``).join(', ')}. */\n` +
  `export type ${name} =\n${values.map((v) => `  | '${v}'`).join('\n')};`).join('\n\n');

const out = `// GENERATED by scripts/gen_npm_types.mjs; do not edit by hand.
// Flags: scope/flags.txt (${flags.length} CommandLineRunner flags) and the OPTIONS table of
// crates/cli/src/command_line_runner.rs. Exports: the runtime in npm/closure-rs/, compatible with
// @types/google-closure-compiler@20231112.0.0 and google-closure-compiler@20261006.0.0.

/// <reference types="node" />
import type {ChildProcess, SpawnOptions} from 'child_process';

// ---- flag values --------------------------------------------------------------------------------

// Every flag value type below also admits any string (\`string & {}\` keeps the listed literals as
// editor suggestions). The official package and @types/google-closure-compiler pass strings
// through, and TypeScript widens the values of an options object declared before the call to
// \`string\`; rejecting those would break build files that compile against the official types.
// What the types do reject: booleans for non-boolean flags, arrays for flags that take one value.

/**
 * Value of a boolean flag. \`true\`/\`false\` become \`--flag=true\`/\`--flag=false\`; the strings are
 * the spellings CommandLineRunner's BooleanOptionHandler accepts (any case).
 */
export type BooleanFlag = boolean | 'true' | 'false' | 'on' | 'off' | 'yes' | 'no' | '1' | '0' | (string & {});

/** Value of an integer flag. */
export type IntFlag = number | \`\${number}\` | (string & {});

/**
 * Value of a flag with a fixed set of values: one of the listed values, or its lower-case spelling
 * (the compiler ignores case).
 */
export type EnumFlag<T extends string> = T | Lowercase<T> | (string & {});

/** A flag that may be given several times: an array gives the flag once per item. */
export type Repeatable<T> = T | T[];

${enumDecls}

// ---- options ------------------------------------------------------------------------------------

/**
 * Every closure-rs (= CommandLineRunner) flag, by its name without the leading \`--\`, and by its
 * camelCase spelling (the wrapper turns \`compilationLevel\` into \`--compilation_level\`).
 *
 * Each entry becomes \`--name=value\`; an array value repeats the flag; \`null\` or \`undefined\`
 * gives the bare \`--name\` (as in the official package, so leave unset flags out of the object).
 */
export interface CompilerFlags {
${members.join('\n')}
}

/** One value of the @types/google-closure-compiler options object. */
export type CompileOption = string | boolean;

/** The options type of @types/google-closure-compiler: an argument array or a flags object. */
export type CompileOptions = string[] | {[key: string]: CompileOption | CompileOption[]};

/**
 * Options object accepted by \`new compiler(...)\`: the typed flags, plus any other key (passed
 * through as \`--key=value\`).
 */
export interface CompilerOptions extends CompilerFlags {
  [flag: string]: CompileOption | CompileOption[] | number | null | undefined;
}

// ---- compiler -----------------------------------------------------------------------------------

/** Shape of a compiler instance (the \`Compiler\` interface of @types/google-closure-compiler). */
export interface Compiler {
  /** The executable \`run()\` spawns: the closure-rs binary. */
  javaPath: string;
  logger: (...args: any[]) => void;
  spawnOptions: SpawnOptions & {[key: string]: any};
  run(callback?: (exitCode: number, stdout: string, stderr: string) => void): ChildProcess;
  getFullCommand(): string;
}

/**
 * Runs the closure-rs binary (google-closure-compiler's \`compiler\` class).
 *
 * \`\`\`ts
 * import ClosureCompiler from 'google-closure-compiler';
 * new ClosureCompiler({js: 'in.js', compilation_level: 'ADVANCED'})
 *   .run((exitCode, stdout, stderr) => { ... });
 * \`\`\`
 */
export class compiler implements Compiler {
  /**
   * @param args A flags object (see {@link CompilerFlags}) or an argument array passed verbatim.
   * @param extraCommandArgs JVM arguments in the official package; closure-rs runs no JVM and
   *   ignores them unless \`JAR_PATH\` is set.
   */
  constructor(args: CompilerOptions | string[], extraCommandArgs?: string[]);

  /**
   * null at runtime: closure-rs has no jar. Declared \`string\` as in @types/google-closure-compiler
   * (the official class has no such static at runtime either: there it is undefined).
   */
  static JAR_PATH: string;
  /** Path of the closure-rs binary. */
  static COMPILER_PATH: string;
  /** Path of the bundled contrib/ folder. */
  static CONTRIB_PATH: string;
  /** Path of the bundled externs/ folder. */
  static EXTERNS_PATH: string;

  /** The arguments \`run()\` passes to the binary. */
  commandArguments: string[];
  extraCommandArgs: string[] | undefined;
  /** null: no jar. If set (with \`javaPath\` pointing at java), \`run()\` runs that jar instead. */
  JAR_PATH: string | null;
  /** The executable \`run()\` spawns; defaults to the closure-rs binary. */
  javaPath: string;
  /** When set, called with the full command line before the process starts. */
  logger: (...args: any[]) => void;
  /** Options for child_process.spawn (undefined by default). */
  spawnOptions: SpawnOptions & {[key: string]: any};

  /**
   * Starts the compiler and returns the child process (write to its stdin to compile from stdin).
   * The callback gets the exit code and the collected stdout and stderr; for a non-zero exit code
   * stderr starts with the full command line.
   */
  run(callback?: (exitCode: number, stdout: string, stderr: string) => void): ChildProcess;
  /** The executable and its arguments, joined with spaces. */
  getFullCommand(): string;
  /** \`msg\` preceded by the full command line. */
  prependFullCommand(msg: string): string;
  /** \`--key=value\`, or \`--key\` when \`val\` is null or undefined; camelCase keys become snake_case. */
  formatArgument(key: string, val?: string | boolean | number | null): string;
}

export default compiler;

/** The executable \`compiler\` runs by default: the closure-rs binary (\`'closure-rs'\` on PATH if none is installed). */
export const javaPath: string;
/** Always null: closure-rs has no jar (the official package exports the jar's path). */
export const JAR_PATH: string | null;
/** Path of the closure-rs binary. */
export const COMPILER_PATH: string;
/** Path of the bundled contrib/ folder (contrib/externs, contrib/nodejs). */
export const CONTRIB_PATH: string;
/** Path of the bundled externs/ folder (the default externs). */
export const EXTERNS_PATH: string;

// ---- gulp and grunt (not implemented: calling them throws) --------------------------------------

/** A file record of the --json_streams format. */
export interface JSONStreamFile {
  path: string;
  src: string;
  srcmap?: string | undefined;
}

export interface GulpInitOptions {
  extraArguments?: string[];
}
export type LogFunction = (message: string) => void;
export interface GulpPluginOptions {
  streamMode?: string;
  logger?: LogFunction | {warn: LogFunction};
  pluginName?: string;
  requireStreamInput?: boolean;
  platform?: string | string[];
}
export type Transform = import('stream').Transform;
export interface CompilationStream extends Transform {
  platform: string;
  src(): this;
}

/**
 * The gulp plugin. Not implemented in closure-rs: calling it throws. The first signature is the
 * one @types/google-closure-compiler declares, the second the official package's current one.
 */
export function gulp(initOptions?: GulpInitOptions): (opts: CompileOptions, pluginOpts?: GulpPluginOptions) => CompilationStream;
export function gulp(compilationOptions: CompilerOptions | string[], pluginOptions?: GulpPluginOptions): CompilationStream;

/** The parts of grunt's IGrunt the grunt plugin uses. */
export interface PartialIGrunt {
  registerMultiTask(
    taskName: string,
    taskDescription: string,
    taskFunction: (this: any, ...args: any[]) => void,
  ): void;
  file: {
    write(filepath: string, contents: string | Buffer): void;
  };
  log: {
    ok(msg: string): any;
    warn(msg: string): any;
  };
  fail: {
    warn(error: string): void;
  };
}
export type GruntTaskFunction<IGrunt extends PartialIGrunt> = Parameters<IGrunt['registerMultiTask']>[2];
export type GruntPluginOptions = string[] | {
  platform?: string | string[];
  extraArguments?: string[];
  max_parallel_compilations?: number;
  /** @deprecated use max_parallel_compilations */
  compile_in_batches?: number;
};

/** The grunt plugin. Not implemented in closure-rs: calling it throws. */
export function grunt<IGrunt extends PartialIGrunt>(
  grunt: IGrunt,
  pluginOptions?: GruntPluginOptions,
): GruntTaskFunction<IGrunt>;
`;

// index.d.cts is the same declarations for CommonJS importers (package.json "exports" gives it to
// `require` under TypeScript's node16/nodenext resolution; Node itself loads the ESM index.js).
const outputs = [[OUT, out], [OUT.replace(/\.d\.ts$/, '.d.cts'), out]];
if (process.argv.includes('--check')) {
  for (const [file, text] of outputs) {
    let current = '';
    try {
      current = readFileSync(file, 'utf8');
    } catch {}
    if (current !== text) {
      console.error(`gen_npm_types: ${file} is stale; run node scripts/gen_npm_types.mjs`);
      process.exit(1);
    }
  }
  console.log(`npm/closure-rs/index.d.ts and index.d.cts are up to date (${flags.length} flags)`);
} else {
  for (const [file, text] of outputs) writeFileSync(file, text);
  console.log(`wrote npm/closure-rs/index.d.ts and index.d.cts (${flags.length} flags, ${enumTypes.size} value sets)`);
}
