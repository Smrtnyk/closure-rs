// closure-rs D2 whole-program shim for decimal.js@10.6.0 (case whole-program-decimal.js-10.6.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import Decimal from '../../../../corpus-cache/d2/whole-program/decimal.js@10.6.0/package/decimal.mjs';

globalThis['Decimal'] = Decimal;
