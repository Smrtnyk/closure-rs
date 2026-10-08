## ScopedAliasesTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.ScopedAliasesTest` whose assertions after the hooked call inspect Java-internal objects (the scan treats hook-reaching helpers as hooked calls). The records still replay but are excluded from gate (a) counting under D-015; the checks are ported as Rust unit tests (category `node-identity`).

- ScopedAliasesTest#testSourceInfo: after testScoped() (one hooked test() call, record index 10) rewrites `var d = dom; var e = event; alert(e.EventType.MOUSEUP); alert(d.TagName.DIV);` inside goog.scope to `alert(event.EventType.MOUSEUP); alert(dom.TagName.DIV);`, the qualified-name node `dom` of the output AST has a greater source line number (`Node.getLineno()`) than the node `event` (ScopedAliasesTest.java:105-119); the AST comparison ignores line numbers and the record's postCall keys (`compiler.accessorSummary`, `compiler.moduleMetadataByPath`) do not hold node positions
