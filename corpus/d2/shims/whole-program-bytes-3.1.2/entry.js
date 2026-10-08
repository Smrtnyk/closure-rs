// closure-rs D2 whole-program shim for bytes@3.1.2 (case whole-program-bytes-3.1.2).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
var bytes = require('../../../../corpus-cache/d2/whole-program/bytes@3.1.2/package/index.js');
bytes['format'] = bytes.format;
bytes['parse'] = bytes.parse;
globalThis['bytes'] = bytes;
