## DevirtualizeMethodsTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.DevirtualizeMethodsTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

- DevirtualizeMethodsTest#testRewritePrototypeMethodsWithCorrectColors: after rewriting A.prototype.{foo,bar,baz} to JSCompiler_StaticMethods_*, the rewritten call results keep their colors (NUMBER, NUMBER, NULL_OR_VOID) and the callee NAMEs have color TOP_FUNCTION (input and expected output are in the record; the colors are not)
