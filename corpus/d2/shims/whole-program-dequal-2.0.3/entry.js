// closure-rs D2 whole-program shim for dequal@2.0.3 (case whole-program-dequal-2.0.3).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import { dequal } from '../../../../corpus-cache/d2/whole-program/dequal@2.0.3/package/dist/index.mjs';
import { dequal as dequalLite } from '../../../../corpus-cache/d2/whole-program/dequal@2.0.3/package/lite/index.mjs';

globalThis['dequal'] = {
  'src': { 'dequal': dequal },
  'lite': { 'dequal': dequalLite }
};
