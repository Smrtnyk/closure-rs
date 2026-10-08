// closure-rs D2 whole-program shim for base64-js@1.5.1 (case whole-program-base64-js-1.5.1).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
var base64js = require('../../../../corpus-cache/d2/whole-program/base64-js@1.5.1/package/index.js');
globalThis['base64js'] = {
  'byteLength': base64js.byteLength,
  'toByteArray': base64js.toByteArray,
  'fromByteArray': base64js.fromByteArray
};
