## RewriteObjectSpreadTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.RewriteObjectSpreadTest` whose assertions after the hooked call inspect the Color objects of the output AST; their records stay in the corpus (input and expected output replay, with type-info validation inside the call) and the post-call checks below are ported as Rust unit tests next to RewriteObjectSpread.

- RewriteObjectSpreadTest#testTyping_ofSpreadResult_isObject: after `const obj = ({first, ...spread})` becomes `Object.assign({}, {first}, spread)`, the NAME node `obj` has color StandardColors.TOP_OBJECT and the new Object.assign CALL node has the same color as the node following the `Object.assign` qualified name in the externs AST (test lines 77-103; record 8)
