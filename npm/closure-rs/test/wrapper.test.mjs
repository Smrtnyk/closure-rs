// Tests of the npm wrapper against a real closure-rs binary (node:test, no dependencies).
//   CLOSURE_RS_BINARY=target/release/closure-rs node --test npm/closure-rs/test/
// The binary must be set: the wrapper resolves it at import time.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import {createRequire} from 'node:module';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {after, before, test} from 'node:test';
import {fileURLToPath} from 'node:url';

import ClosureCompiler, * as gcc from '../index.js';

const pkgDir = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const binary = process.env.CLOSURE_RS_BINARY;

const SMOKE = 'function greet(name){var unused=1;return "Hello, "+name}\n' +
    'window["greet"]=greet;\nconsole.log(greet("world"));\n';
const SMOKE_ADVANCED = 'function a(b){return"Hello, "+b}window.greet=a;console.log(a("world"));';

let dir;
let smoke;
before(() => {
  assert.ok(binary, 'set CLOSURE_RS_BINARY to the closure-rs binary');
  dir = mkdtempSync(path.join(tmpdir(), 'closure-rs-npm-'));
  smoke = path.join(dir, 'smoke.js');
  writeFileSync(smoke, SMOKE);
  writeFileSync(path.join(dir, 'a.js'), 'var a = 1 + 1;\n');
  writeFileSync(path.join(dir, 'b.js'), 'var b = a * 2;\n');
  writeFileSync(path.join(dir, 'bad.js'), 'var x = ;\n');
});
after(() => rmSync(dir, {recursive: true, force: true}));

/** run() as a promise of {code, stdout, stderr, child}. */
const run = (c, onChild) => new Promise((resolve) => {
  const child = c.run((code, stdout, stderr) => resolve({code, stdout, stderr, child}));
  if (onChild) onChild(child);
});

test('exports match the official package', () => {
  assert.equal(gcc.default, gcc.compiler);
  assert.equal(ClosureCompiler, gcc.compiler);
  for (const name of ['JAR_PATH', 'CONTRIB_PATH', 'EXTERNS_PATH', 'grunt', 'gulp', 'compiler', 'javaPath']) {
    assert.ok(name in gcc, `missing export ${name}`);
  }
  assert.equal(gcc.JAR_PATH, null);
  assert.equal(gcc.javaPath, path.resolve(binary));
  assert.equal(gcc.COMPILER_PATH, path.resolve(binary));
  assert.equal(gcc.CONTRIB_PATH, path.join(pkgDir, 'contrib'));
  assert.equal(gcc.EXTERNS_PATH, path.join(pkgDir, 'externs'));
  assert.equal(gcc.compiler.CONTRIB_PATH, gcc.CONTRIB_PATH);
  assert.equal(gcc.compiler.COMPILER_PATH, gcc.COMPILER_PATH);
});

test('require() of the package gives the same exports', () => {
  const [major, minor] = process.versions.node.split('.').map(Number);
  if (major < 20 || (major === 20 && minor < 19) || (major === 22 && minor < 12)) {
    return; // require(esm) needs Node 20.19+ / 22.12+, as for the official ESM-only package
  }
  const required = createRequire(import.meta.url)('../index.js');
  assert.equal(required.compiler, gcc.compiler);
  assert.equal(required.default, gcc.compiler);
});

test('installed as google-closure-compiler: import and require by package name', () => {
  // node_modules/google-closure-compiler -> this package, as the npm alias installs it.
  const app = path.join(dir, 'app');
  mkdirSync(path.join(app, 'node_modules'), {recursive: true});
  symlinkSync(pkgDir, path.join(app, 'node_modules', 'google-closure-compiler'), 'junction');
  writeFileSync(path.join(app, 'esm.mjs'), [
    "import ClosureCompiler, {compiler, javaPath, CONTRIB_PATH} from 'google-closure-compiler';",
    "import Low from 'google-closure-compiler/lib/node/index.js';",
    "if (ClosureCompiler !== compiler || Low !== compiler) throw new Error('exports differ');",
    "new compiler({js: process.argv[2], compilation_level: 'ADVANCED'}).run((code, out, err) => {",
    "  process.stdout.write(out); process.stderr.write(err); process.exitCode = code; });",
  ].join('\n'));
  writeFileSync(path.join(app, 'cjs.cjs'), [
    "const gcc = require('google-closure-compiler');",
    "if (gcc.default !== gcc.compiler) throw new Error('exports differ');",
    "new gcc.compiler(['--js', process.argv[2], '--compilation_level', 'ADVANCED']).run((code, out, err) => {",
    "  process.stdout.write(out); process.stderr.write(err); process.exitCode = code; });",
  ].join('\n'));
  const [major, minor] = process.versions.node.split('.').map(Number);
  const requireEsm = major > 22 || (major === 22 && minor >= 12) || (major === 20 && minor >= 19);
  for (const script of requireEsm ? ['esm.mjs', 'cjs.cjs'] : ['esm.mjs']) {
    const r = spawnSync(process.execPath, [script, smoke], {
      cwd: app, encoding: 'utf8', env: {...process.env, CLOSURE_RS_BINARY: path.resolve(binary)}});
    assert.equal(r.status, 0, `${script}: ${r.stderr}`);
    assert.equal(r.stdout.trim(), SMOKE_ADVANCED, script);
  }
});

test('options object becomes flags like the official formatArgument', () => {
  const c = new ClosureCompiler({
    js: ['a.js', 'b.js'],
    compilationLevel: 'ADVANCED',
    '--debug': true,
    pretty_print: false,
    third_party: null,
    summary_detail_level: 3,
  });
  assert.deepEqual(c.commandArguments, [
    '--js=a.js', '--js=b.js', '--compilation_level=ADVANCED', '--debug=true',
    '--pretty_print=false', '--third_party', '--summary_detail_level=3',
  ]);
  assert.equal(c.JAR_PATH, null);
  assert.equal(c.javaPath, path.resolve(binary));
  assert.equal(c.getFullCommand(), `${path.resolve(binary)} ${c.commandArguments.join(' ')}`);
});

test('ADVANCED compile through new compiler({...}).run(cb)', async () => {
  const c = new gcc.compiler({js: smoke, compilation_level: 'ADVANCED'});
  const logged = [];
  c.logger = (msg) => logged.push(msg);
  const {code, stdout, stderr, child} = await run(c);
  assert.equal(code, 0, stderr);
  assert.equal(stdout.trim(), SMOKE_ADVANCED);
  assert.equal(stderr, '');
  assert.equal(typeof child.pid, 'number');
  assert.deepEqual(logged, [c.getFullCommand() + '\n']);
});

test('array value repeats the flag (two --js inputs, --formatting twice)', async () => {
  const c = new ClosureCompiler({
    js: [path.join(dir, 'a.js'), path.join(dir, 'b.js')],
    formatting: ['PRETTY_PRINT', 'SINGLE_QUOTES'],
    compilation_level: 'WHITESPACE_ONLY',
  });
  const {code, stdout, stderr} = await run(c);
  assert.equal(code, 0, stderr);
  assert.equal(stdout, 'var a = 1 + 1;\nvar b = a * 2;\n\n'); // PRETTY_PRINT ends with a blank line
});

test('argument array is passed verbatim', async () => {
  const c = new ClosureCompiler(['--js', smoke, '--compilation_level', 'ADVANCED']);
  assert.deepEqual(c.commandArguments, ['--js', smoke, '--compilation_level', 'ADVANCED']);
  const {code, stdout, stderr} = await run(c);
  assert.equal(code, 0, stderr);
  assert.equal(stdout.trim(), SMOKE_ADVANCED);
});

test('stdin input through the returned child process', async () => {
  const c = new ClosureCompiler({compilation_level: 'ADVANCED'});
  const {code, stdout, stderr} = await run(c, (child) => {
    child.stdin.write(SMOKE);
    child.stdin.end();
  });
  assert.equal(code, 0, stderr);
  assert.equal(stdout.trim(), SMOKE_ADVANCED);
});

test('compile error: non-zero exit code, stderr starts with the full command', async () => {
  const c = new ClosureCompiler({js: path.join(dir, 'bad.js')});
  const {code, stdout, stderr} = await run(c);
  assert.notEqual(code, 0);
  assert.equal(stdout, '');
  assert.ok(stderr.startsWith(c.getFullCommand() + '\n\n'), stderr);
  assert.match(stderr, /JSC_PARSE_ERROR/);
  assert.match(stderr, /1 error\(s\)/);
});

test('unknown flag: exit code from the binary', async () => {
  const c = new ClosureCompiler({no_such_flag: true});
  const {code, stderr} = await run(c);
  assert.notEqual(code, 0);
  assert.match(stderr, /no_such_flag/);
});

test('missing executable: callback once with exit code 1', async () => {
  const c = new ClosureCompiler({js: smoke});
  c.javaPath = path.join(dir, 'does-not-exist');
  let calls = 0;
  const result = await new Promise((resolve) => {
    c.run((code, stdout, stderr) => {
      calls++;
      resolve({code, stderr});
    });
  });
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(calls, 1);
  assert.equal(result.code, 1);
  assert.match(result.stderr, /Process spawn error/);
  assert.match(result.stderr, /CLOSURE_RS_BINARY can point at/);
});

test('terminated by a signal: exit code 128 + signal number, stderr names the signal',
    {skip: process.platform === 'win32'}, async () => {
  const fake = path.join(dir, 'killed-by-signal.sh');
  writeFileSync(fake, '#!/bin/sh\nkill -KILL $$\n', {mode: 0o755});
  const c = new ClosureCompiler({js: smoke});
  c.javaPath = fake;
  const result = await new Promise((resolve) => {
    c.run((code, stdout, stderr) => resolve({code, stderr}));
  });
  assert.equal(result.code, 137);
  assert.match(result.stderr, /terminated by signal SIGKILL \(on Linux usually the out-of-memory killer/);
  assert.ok(result.stderr.startsWith(c.getFullCommand()));
});

test('gulp and grunt are stubs that throw', () => {
  assert.throws(() => gcc.gulp({js: 'a.js'}), /does not implement the gulp plugin/);
  assert.throws(() => gcc.grunt({}), /does not implement the grunt plugin/);
});

test('cli.js drops --platform and passes the rest to the binary', () => {
  const r = spawnSync(process.execPath, [path.join(pkgDir, 'cli.js'), '--platform', 'native,java',
    '--compilation_level=ADVANCED', '--js', smoke], {encoding: 'utf8'});
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout.trim(), SMOKE_ADVANCED);
  const bad = spawnSync(process.execPath, [path.join(pkgDir, 'cli.js'), '--platform=java',
    '--js', path.join(dir, 'bad.js')], {encoding: 'utf8'});
  assert.notEqual(bad.status, 0);
  assert.match(bad.stderr, /JSC_PARSE_ERROR/);
});
