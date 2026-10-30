## OptimizeParametersTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.OptimizeParametersTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

The `_knownBug` methods (b/236744850) wrap the hooked call in `assertThrows(AssertionError.class, ...)` and check the message of the harness's Truth `Node tree inequality` diff; the Rust ports check the Rust harness's assertion message for the same text.

- OptimizeParametersTest#testPrivateNameArgumentIsNotMoved_knownBug: testSame fails because the constant argument `C.#n` is moved into `fn`; the assertion message contains `var p1 = C.#n;`
