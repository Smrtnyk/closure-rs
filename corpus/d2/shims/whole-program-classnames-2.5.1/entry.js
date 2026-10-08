// closure-rs D2 whole-program shim for classnames@2.5.1 (case whole-program-classnames-2.5.1).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
var classNames = require('../../../../corpus-cache/d2/whole-program/classnames@2.5.1/package/index.js');
var classNamesDedupe = require('../../../../corpus-cache/d2/whole-program/classnames@2.5.1/package/dedupe.js');
var classNamesBind = require('../../../../corpus-cache/d2/whole-program/classnames@2.5.1/package/bind.js');
globalThis['classnames'] = {
  'index': classNames,
  'dedupe': classNamesDedupe,
  'bind': classNamesBind
};
