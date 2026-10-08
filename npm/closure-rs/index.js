// closure-rs: drop-in replacement for the google-closure-compiler npm package (programmatic API).
//
// Exports the same names as google-closure-compiler@20261005.0.0 index.js (lines 25-37): JAR_PATH,
// CONTRIB_PATH, EXTERNS_PATH, grunt, gulp, compiler (also the default export) and javaPath. It
// adds COMPILER_PATH (the binary's path; the official README documents that name).
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import Compiler, {javaPath} from './lib/node/index.js';

const here = path.dirname(fileURLToPath(import.meta.url));

/** No jar exists in closure-rs; null (the official package exports the jar's path). */
export const JAR_PATH = null;
/** The closure-rs binary that `compiler` runs. */
export const COMPILER_PATH = javaPath;
/** The Closure Compiler contrib/ folder (contrib/externs, contrib/nodejs), shipped as in the official package. */
export const CONTRIB_PATH = path.resolve(here, './contrib');
/** The default externs (externs/), shipped as in the official package. */
export const EXTERNS_PATH = path.resolve(here, './externs');

export {default as grunt} from './lib/grunt/index.js';
export {default as gulp} from './lib/gulp/index.js';
export {Compiler as compiler, Compiler as default, javaPath};
