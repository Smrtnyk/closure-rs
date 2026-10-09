#!/usr/bin/env node
// Command line entry point: `npx closure-rs ...` or, installed as the google-closure-compiler
// alias, `npx google-closure-compiler ...`. All arguments go to the closure-rs binary unchanged,
// except the wrapper-only --platform flag, which is removed (google-closure-compiler@20261006.0.0
// cli.js lines 23-31 and 62-84). Every accepted platform ('native', 'java') runs closure-rs.
import Compiler from './lib/node/index.js';
import {getFirstSupportedPlatform} from './lib/utils.js';

const args = process.argv.slice(2);
let platforms = ['native', 'java'];
for (let i = 0; i < args.length; i++) {
  const m = /^--platform(?:=(.*))?$/.exec(args[i]);
  if (m) {
    let value = m[1];
    let count = 1;
    if (value === undefined && i + 1 < args.length) {
      value = args[i + 1];
      count = 2;
    }
    if (value !== undefined) {
      platforms = value.split(',');
    }
    args.splice(i, count);
    break;
  }
}
getFirstSupportedPlatform(platforms);

const compiler = new Compiler(args);
compiler.spawnOptions = {stdio: 'inherit'};
const child = compiler.run((exitCode, stdout, stderr) => {
  // stdio is inherited, so the binary's own output is already on the terminal; only the wrapper's
  // note about a process ended by a signal (exit code 128 + signal number) is printed here.
  const signal = /closure-rs was terminated by signal .*/.exec(stderr);
  if (signal) {
    process.stderr.write(`${signal[0]}\n`);
  }
  process.exitCode = exitCode;
});
child.on('error', (e) => {
  process.stderr.write(`closure-rs: cannot run ${compiler.javaPath}: ${e.message}\n`);
});
