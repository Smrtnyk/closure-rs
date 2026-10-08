## ClosureRewriteModuleTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.ClosureRewriteModuleTest` whose assertions after the hooked call assert the JSType and source position of nodes of the output AST. Their records stay in the corpus (inputs, expected output, diagnostics and outcome replay); the post-call checks below are ported as Rust unit tests next to the pass.

- ClosureRewriteModuleTest#testTypeAndSourceInfoOfGoogRequireFromModule: after rewriting `const {Bar} = goog.require('mod.one'); new Bar();`, the `module$exports$mod$one.Bar` GETPROP has JSType reference name `mod.one.Bar` and the source position of `Bar` in `new Bar();` (line 3, char 4, length 3), shared by its NAME child
- ClosureRewriteModuleTest#testTypeOfGoogRequireFromLegacyModule: after rewriting a goog.require of a declareLegacyNamespace module, the `mod.one.Bar` GETPROP in `new mod.one.Bar()` has JSType reference name `mod.one.Bar`
