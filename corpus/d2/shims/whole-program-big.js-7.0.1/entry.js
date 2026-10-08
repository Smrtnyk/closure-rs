// closure-rs D2 whole-program shim for big.js@7.0.1 (case whole-program-big.js-7.0.1).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import Big from '../../../../corpus-cache/d2/whole-program/big.js@7.0.1/package/big.mjs';

globalThis['Big'] = Big;
