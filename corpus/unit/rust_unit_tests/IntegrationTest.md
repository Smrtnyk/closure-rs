## IntegrationTest

Post-call assertions of IntegrationTest (kind `integration`, gate 0.2 (a), D-015). Integration records carry no `postCall` snapshot. The first group asserts only on values the record holds in `observed` or `outcome` (replay compares those JSON-equal), but the postcall via schema has no entry for them. The second group asserts on compiler state that no record holds. Port each as a Rust integration test: compile the record's input with its options and assert the listed fact.

- IntegrationTest#testGetOriginalQualifiedNameAfterEs6RewriteClasses: the GETPROP callee in module$contents$a_Bar.prototype.foo has original qualified name 'Foo.method' and original name 'method' after Es6RewriteClasses (reads Node.getOriginalQualifiedName()/getOriginalName() of a GETPROP in the compiled AST (original-name properties of output AST nodes))
