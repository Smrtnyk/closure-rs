## CrossChunkMethodMotionTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.CrossChunkMethodMotionTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

The `_knownBug` methods (b/236744850) wrap the hooked call in `assertThrows(AssertionError.class, ...)` and check the message of the harness's Truth `Node tree inequality` diff; the Rust ports check the Rust harness's assertion message for the same text.

- CrossChunkMethodMotionTest#doNotMoveClassMethodContainingPrivateName_knownBug: testSame on the two-chunk input fails because `getX`, which reads `this.#x`, is moved out of `Foo`; the assertion message contains `Foo.prototype.getX = JSCompiler_unstubMethod(0, function() {`
- CrossChunkMethodMotionTest#doNotMovePrivateClassMethod_knownBug: testSame on the two-chunk input fails because the private method `#m` of `Foo` is moved to the second chunk; the assertion message contains `Foo.prototype.#m = JSCompiler_unstubMethod(0, function() {});`
