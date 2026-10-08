## ClosureIntegrationTest (post-call assertions, D-015 (a))

IntegrationTestCase subclass with 12 methods that assert on the compiler after the hooked call. The 11 `compile()` methods assert diagnostics or output that the record holds in `observed` (errors, warnings, output) and Java replay compares exactly; they are classified `captured` (via `observed.*`) in corpus/unit/postcall/ClosureIntegrationTest.json and are not listed here. `testGetOriginalQualifiedNameAfterEs6RewriteClasses` checks Node original-name properties, which no replayed value holds, and is a Rust unit test.

- ClosureIntegrationTest#testGetOriginalQualifiedNameAfterEs6RewriteClasses: after test(), the compiled call target `module$contents$a_Foo.method` keeps original qualified name 'Foo.method' and original name 'method' (Node.getOriginalQualifiedName / getOriginalName; regression test for Es6RewriteClasses) [test/com/google/javascript/jscomp/integration/ClosureIntegrationTest.java lines 1354-1396]
