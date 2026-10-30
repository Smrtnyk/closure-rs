## InlineSimpleMethodsTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.InlineSimpleMethodsTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

The `_knownBug` methods (b/236744850) wrap the hooked call in `assertThrows(AssertionError.class, ...)` and check the message of the harness's Truth `Node tree inequality` diff; the Rust ports check the Rust harness's assertion message for the same text.

- InlineSimpleMethodsTest#testNoInlineOfGetterThatReadsPrivateField_knownBug: testSame fails because `new C().getA()` is inlined to a read of the private field; the assertion message contains `var x = (new C()).#a;`
- InlineSimpleMethodsTest#testNoInlineOfPrivateMethodCall_knownBug: testSame fails because `o.#m()` is replaced by the value `#m` returns; the assertion message contains `+    return 2;`
