// closure-rs D2 whole-program shim for jsesc@3.1.0 (case whole-program-jsesc-3.1.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
var jsesc = require('../../../../corpus-cache/d2/whole-program/jsesc@3.1.0/package/jsesc.js');
jsesc['version'] = jsesc.version;
globalThis['jsesc'] = jsesc;
