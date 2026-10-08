## RewriteCatchWithNoBindingTest (post-call assertions, gate 0.2 (a))

Post-call assertions of `com.google.javascript.jscomp.RewriteCatchWithNoBindingTest` (test/com/google/javascript/jscomp/RewriteCatchWithNoBindingTest.java, bb8c8e7) that inspect `Color` objects on output AST nodes, which no `postCall` key, test field or postcondition records and `Node.isEquivalentTo` does not compare. The record still replays (input, expected output, diagnostics, outcome) but is excluded from gate (a) counting under D-015; the check is ported as a Rust unit test of the RewriteCatchWithNoBinding transpilation (run with type checking, colors and multistage compilation, as in the test's setUp).

- RewriteCatchWithNoBindingTest#typeOfAddedBindingIsUnknown: after transpiling `try { stuff(); } catch { onError(); }`, the added catch binding NAME (`UNUSED_CATCH$0`, ROOT > SCRIPT > TRY > BLOCK > CATCH > NAME) has the color `StandardColors.UNKNOWN` (lines 111-139).
