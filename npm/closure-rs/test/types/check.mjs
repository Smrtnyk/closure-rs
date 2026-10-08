// Type-checks the sample build files against ../../index.d.ts the way a user's project sees it:
// node_modules/google-closure-compiler is this package (as with the npm alias). tsconfig.json:
// ESM build file (nodenext); tsconfig.cjs.json: CommonJS build file (nodenext); tsconfig.node16.json:
// the CommonJS file under node16 resolution (gets index.d.cts through "exports").
//   node npm/closure-rs/test/types/check.mjs <path to typescript/bin/tsc> <path to an @types/node dir>
// Uses whatever TypeScript is given; installs nothing.
import {execFileSync} from 'node:child_process';
import {mkdirSync, rmSync, symlinkSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const [tsc, typesNode] = process.argv.slice(2);
if (!tsc || !typesNode) {
  console.error('usage: check.mjs <typescript/bin/tsc> <@types/node dir>');
  process.exit(2);
}
const here = path.dirname(fileURLToPath(import.meta.url));
const nm = path.join(here, 'node_modules');
rmSync(nm, {recursive: true, force: true});
mkdirSync(path.join(nm, '@types'), {recursive: true});
const link = (target, at) => symlinkSync(path.resolve(target), at, 'junction');
link(path.join(here, '..', '..'), path.join(nm, 'google-closure-compiler'));
link(typesNode, path.join(nm, '@types', 'node'));
let failed = false;
for (const config of ['tsconfig.json', 'tsconfig.cjs.json', 'tsconfig.node16.json']) {
  try {
    execFileSync(process.execPath, [tsc, '-p', path.join(here, config)], {stdio: 'inherit'});
    console.log(`${config}: ok`);
  } catch {
    console.log(`${config}: FAILED`);
    failed = true;
  }
}
process.exit(failed ? 1 : 0);
