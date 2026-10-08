## J2clPassTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.J2clPassTest` whose assertions after the hooked call inspect Java-internal state; their records stay in the corpus (input, expected output and diagnostics replay) and the post-call checks below are ported as Rust unit tests next to the pass.

- J2clPassTest#testQualifiedInlines_markImplementor: after inlining FooInterface.$markImplementor(Foo) into a block, the NAME Foo inside the inlined GETPROP has the same color as the NAME Foo of the `var Foo` declaration
