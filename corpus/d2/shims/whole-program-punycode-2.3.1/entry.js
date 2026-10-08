// closure-rs D2 whole-program shim for punycode@2.3.1 (case whole-program-punycode-2.3.1).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
// punycode.js already builds its API object with quoted keys.
globalThis['punycode'] = require('../../../../corpus-cache/d2/whole-program/punycode@2.3.1/package/punycode.js');
