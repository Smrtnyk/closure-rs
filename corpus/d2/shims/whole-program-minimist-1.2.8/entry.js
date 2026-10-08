// closure-rs D2 whole-program shim for minimist@1.2.8 (case whole-program-minimist-1.2.8).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
globalThis['minimist'] = require('../../../../corpus-cache/d2/whole-program/minimist@1.2.8/package/index.js');
