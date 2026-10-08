## Es6ForOfConverterTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.Es6ForOfConverterTest` whose assertions after the hooked call inspect Java-internal state (category `node-identity`: a Node property flag). Their records stay in the corpus (input, expected output and diagnostics replay) and are excluded from gate (a) counting; the post-call checks below are ported as Rust unit tests next to Es6ForOfConverter. The records carry no `postCall.pass.*` key; their `postCall.compiler` keys (`externProperties`, `accessorSummary`, `typeMismatchesError`) do not hold the asserted flag.

- Es6ForOfConverterTest#testConstnessPreservedInNewDeclarations: `for (let CID of [1, 2, 3]) { alert(CID); }` transpiles to the record's expected output, and in the output the NAME node `CID` of the new `let CID = KEY$1$CID.value` declaration (script > TRY > BLOCK > FOR > BLOCK > LET) still has the boolean Node property `IS_CONSTANT_NAME` (Es6ForOfConverterTest.java:287-301)
