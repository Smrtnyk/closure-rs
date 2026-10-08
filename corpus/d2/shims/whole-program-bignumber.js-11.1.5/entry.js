// closure-rs D2 whole-program shim for bignumber.js@11.1.5 (case whole-program-bignumber.js-11.1.5).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import BigNumber from '../../../../corpus-cache/d2/whole-program/bignumber.js@11.1.5/package/dist/bignumber.mjs';

globalThis['BigNumber'] = BigNumber;
