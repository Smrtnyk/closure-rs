#!/usr/bin/env node
// Packs the main npm package (npm/closure-rs/: the google-closure-compiler-compatible wrapper).
// Stages a copy in <out-dir>/<name>/, sets name, version, description, license and the
// optionalDependencies on the platform packages from npm/config.json, adds the externs/ and
// contrib/ folders the official package ships (CONTRIB_PATH, EXTERNS_PATH), and runs
// `npm pack --ignore-scripts` (the package defines no scripts anyway).
//   node scripts/npm_pack_main.mjs <out-dir> [--contrib <dir> | --no-contrib] [--binaries <dir>]
// --binaries <dir>: bundle the native binaries into this one package instead of depending on
// per-platform packages: <dir>/<platform>-<arch>/closure-rs[.exe] goes to bin/<platform>-<arch>/;
// the wrapper picks the one for the running machine (lib/utils.js bundledBinaryPath).
// externs/ comes from crates/resources/data/externs (the jar's externs.zip, identical to the
// official package's externs/). contrib/ is the Closure Compiler repository's contrib/ folder at
// the pinned reference commit: by default reference/closure-compiler/contrib
// (scripts/fetch_reference.sh); --no-contrib packs without it (local testing only).
// Run `node scripts/gen_npm_types.mjs` first; this script refuses stale types.
import {execFileSync} from 'node:child_process';
import {chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {basename, join, resolve} from 'node:path';

const argv = process.argv.slice(2);
const out = argv[0];
const contribIdx = argv.indexOf('--contrib');
const noContrib = argv.includes('--no-contrib');
const binIdx = argv.indexOf('--binaries');
const binaries = binIdx >= 0 ? resolve(argv[binIdx + 1] || '') : null;
if (!out || out.startsWith('--') || (contribIdx >= 0 && !argv[contribIdx + 1])) {
  console.error('usage: npm_pack_main.mjs <out-dir> [--contrib <dir> | --no-contrib]');
  process.exit(2);
}
const root = resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const src = join(root, 'npm', 'closure-rs');
const cfg = JSON.parse(readFileSync(join(root, 'npm', 'config.json'), 'utf8'));
const contrib = contribIdx >= 0 ? resolve(argv[contribIdx + 1])
    : join(root, 'reference', 'closure-compiler', 'contrib');
if (!noContrib && !existsSync(join(contrib, 'externs'))) {
  console.error(`npm_pack_main: no contrib folder at ${contrib} (run scripts/fetch_reference.sh or pass --contrib)`);
  process.exit(2);
}
execFileSync(process.execPath, [join(root, 'scripts', 'gen_npm_types.mjs'), '--check'], {stdio: 'inherit'});

const pkg = JSON.parse(readFileSync(join(src, 'package.json'), 'utf8'));
pkg.name = cfg.name;
pkg.version = cfg.version;
pkg.description = cfg.description;
pkg.license = cfg.license;
if (binaries) {
  delete pkg.optionalDependencies;
  if (!pkg.files.includes('bin/')) pkg.files.push('bin/');
} else {
  pkg.optionalDependencies = Object.fromEntries(
      cfg.platforms.map((p) => [`${cfg.name}-${p}`, cfg.version]));
}

const dir = join(resolve(out), basename(cfg.name));
rmSync(dir, {recursive: true, force: true});
mkdirSync(dir, {recursive: true});
for (const entry of pkg.files) {
  const name = entry.replace(/\/$/, '');
  // added below from elsewhere in the repository
  if (['externs', 'contrib', 'bin', 'LICENSE', 'NOTICE', 'LICENSES'].includes(name)) continue;
  cpSync(join(src, name), join(dir, name), {recursive: true});
}
cpSync(join(root, 'crates', 'resources', 'data', 'externs'), join(dir, 'externs'), {recursive: true});
if (!noContrib) cpSync(contrib, join(dir, 'contrib'), {recursive: true});
// licenses of everything the package ships (the binaries contain translated third-party code)
for (const f of ['LICENSE', 'NOTICE', 'LICENSES']) {
  cpSync(join(root, f), join(dir, f), {recursive: true});
  if (!pkg.files.includes(f === 'LICENSES' ? 'LICENSES/' : f)) pkg.files.push(f === 'LICENSES' ? 'LICENSES/' : f);
}
if (binaries) {
  for (const p of cfg.platforms) {
    const exe = p.startsWith('win32-') ? 'closure-rs.exe' : 'closure-rs';
    const from = join(binaries, p, exe);
    if (!existsSync(from)) {
      console.error(`npm_pack_main: missing binary ${from}`);
      process.exit(2);
    }
    mkdirSync(join(dir, 'bin', p), {recursive: true});
    cpSync(from, join(dir, 'bin', p, exe));
    if (!p.startsWith('win32-')) chmodSync(join(dir, 'bin', p, exe), 0o755);
  }
}

writeFileSync(join(dir, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');

const tgz = execFileSync(process.platform === 'win32' ? 'npm.cmd' : 'npm',
    ['pack', '--ignore-scripts', '--pack-destination', resolve(out)],
    {cwd: dir, encoding: 'utf8', shell: process.platform === 'win32'}).trim().split('\n').pop();
console.log(join(resolve(out), tgz));
