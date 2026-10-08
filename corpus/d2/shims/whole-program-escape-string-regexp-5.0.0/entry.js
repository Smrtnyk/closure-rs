// closure-rs D2 whole-program shim for escape-string-regexp@5.0.0 (case whole-program-escape-string-regexp-5.0.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import escapeStringRegexp from '../../../../corpus-cache/d2/whole-program/escape-string-regexp@5.0.0/package/index.js';

globalThis['escapeStringRegexp'] = escapeStringRegexp;
