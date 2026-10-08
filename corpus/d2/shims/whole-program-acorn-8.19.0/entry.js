// closure-rs D2 whole-program shim for acorn@8.19.0 (case whole-program-acorn-8.19.0).
// Puts the library's public API on globalThis under quoted names, so ADVANCED compilation
// keeps it and the test harness (run.mjs) can reach it in the compiled output.
// The inputs are acorn's multi-file ES module sources (acorn/src/), not the npm bundle.
import * as acorn from '../../../../corpus-cache/d2/whole-program/acorn@8.19.0/repo/acorn/src/index.js';

globalThis['acorn'] = {
  'Node': acorn.Node,
  'Parser': acorn.Parser,
  'Position': acorn.Position,
  'SourceLocation': acorn.SourceLocation,
  'TokContext': acorn.TokContext,
  'Token': acorn.Token,
  'TokenType': acorn.TokenType,
  'defaultOptions': acorn.defaultOptions,
  'getLineInfo': acorn.getLineInfo,
  'isIdentifierChar': acorn.isIdentifierChar,
  'isIdentifierStart': acorn.isIdentifierStart,
  'isNewLine': acorn.isNewLine,
  'keywordTypes': acorn.keywordTypes,
  'lineBreak': acorn.lineBreak,
  'lineBreakG': acorn.lineBreakG,
  'nonASCIIwhitespace': acorn.nonASCIIwhitespace,
  'parse': acorn.parse,
  'parseExpressionAt': acorn.parseExpressionAt,
  'tokContexts': acorn.tokContexts,
  'tokTypes': acorn.tokTypes,
  'tokenizer': acorn.tokenizer,
  'version': acorn.version
};
