// closure-rs D2 whole-program shim for mri@1.2.0 (case whole-program-mri-1.2.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
import mri from '../../../../corpus-cache/d2/whole-program/mri@1.2.0/package/lib/index.mjs';

globalThis['mri'] = mri;
