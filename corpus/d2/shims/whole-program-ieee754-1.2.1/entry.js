// closure-rs D2 whole-program shim for ieee754@1.2.1 (case whole-program-ieee754-1.2.1).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
var ieee754 = require('../../../../corpus-cache/d2/whole-program/ieee754@1.2.1/package/index.js');
globalThis['ieee754'] = {
  'read': ieee754.read,
  'write': ieee754.write
};
