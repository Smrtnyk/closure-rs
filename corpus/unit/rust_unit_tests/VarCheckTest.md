## VarCheckTest (post-call assertions, gate 0.2 (a))

No method of `com.google.javascript.jscomp.VarCheckTest` is a Rust unit test. The only flagged method, `testSimpleValidityCheck` (it catches the RuntimeException thrown through the hooked `testExternChanges` call and asserts that its message contains `Unexpected variable x`), is classified `captured` via `outcome` in `corpus/unit/postcall/VarCheckTest.json`: replay compares the recorded exception's status, class and message exactly.
