// closure-rs D2 whole-program shim for ms@2.1.3 (case whole-program-ms-2.1.3).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
globalThis['ms'] = require('../../../../corpus-cache/d2/whole-program/ms@2.1.3/package/index.js');
