// closure-rs D2 whole-program shim for leven@4.1.0 (case whole-program-leven-4.1.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import leven, { closestMatch } from '../../../../corpus-cache/d2/whole-program/leven@4.1.0/package/index.js';

globalThis['leven'] = { 'default': leven, 'closestMatch': closestMatch };
