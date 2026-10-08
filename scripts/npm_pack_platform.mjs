#!/usr/bin/env node
// Builds the npm platform package for one closure-rs binary (the pattern of the official compiler's
// per-platform packages: the main package lists them as optionalDependencies and npm installs the
// one matching os/cpu). Writes <out>/<name>-<platform>-<arch>/ and packs it with `npm pack` (the
// generated package.json defines no scripts, so packing runs none).
//   node scripts/npm_pack_platform.mjs <binary> <platform: linux|win32|darwin> <arch: x64|arm64> <out-dir>
import { execFileSync } from 'node:child_process'
import { copyFileSync, chmodSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'

const [bin, platform, arch, out] = process.argv.slice(2)
if (!bin || !platform || !arch || !out) {
  console.error('usage: npm_pack_platform.mjs <binary> <platform> <arch> <out-dir>')
  process.exit(2)
}
const root = resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'))
const cfg = JSON.parse(readFileSync(join(root, 'npm', 'config.json'), 'utf8'))
const pkgName = `${cfg.name}-${platform}-${arch}`
const dir = join(resolve(out), basename(pkgName))
rmSync(dir, { recursive: true, force: true })
mkdirSync(join(dir, 'bin'), { recursive: true })
const exe = platform === 'win32' ? 'closure-rs.exe' : 'closure-rs'
copyFileSync(bin, join(dir, 'bin', exe))
if (platform !== 'win32') chmodSync(join(dir, 'bin', exe), 0o755)
// The binary contains code under several licenses (NOTICE): ship their notices and texts with it,
// and the licenses of the Rust crates it links (LICENSES/THIRD_PARTY_RUST.md, written by
// scripts/third_party_licenses.py) next to LICENSE and NOTICE.
const thirdParty = join(root, 'LICENSES', 'THIRD_PARTY_RUST.md')
if (!existsSync(thirdParty)) {
  console.error('LICENSES/THIRD_PARTY_RUST.md is missing: run python3 scripts/third_party_licenses.py')
  process.exit(1)
}
for (const f of ['LICENSE', 'NOTICE']) copyFileSync(join(root, f), join(dir, f))
copyFileSync(thirdParty, join(dir, 'THIRD_PARTY_RUST.md'))
cpSync(join(root, 'LICENSES'), join(dir, 'LICENSES'), {
  recursive: true,
  filter: (src) => resolve(src) !== resolve(thirdParty),
})
writeFileSync(join(dir, 'package.json'), JSON.stringify({
  name: pkgName,
  version: cfg.version,
  description: `${cfg.description} Native binary for ${platform}-${arch}.`,
  license: cfg.license,
  os: [platform],
  cpu: [arch],
  files: [`bin/${exe}`, 'LICENSE', 'NOTICE', 'THIRD_PARTY_RUST.md', 'LICENSES'],
}, null, 2) + '\n')
const tgz = execFileSync(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['pack', '--pack-destination', resolve(out)],
  { cwd: dir, encoding: 'utf8', shell: process.platform === 'win32' }).trim().split('\n').pop()
console.log(join(resolve(out), tgz))
