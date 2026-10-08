## InlineAndCollapsePropertiesTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.InlineAndCollapsePropertiesTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

- InlineAndCollapsePropertiesTest#testCollapseKeepsSourceInfoForAliases: after collapsing a.use / a.b into a$use / a$b, the four output statements and their NAME/FUNCTION/OBJECTLIT children carry the source file name and the original line/column of the statements they came from (lines 2-5, columns as in the original input)
