## PeepholeFoldConstantsTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.PeepholeFoldConstantsTest` whose assertions after the hooked call inspect Java `Node` objects of the output AST, or run the processor directly outside the hooked API. Their records stay in the corpus (inputs, expected output, diagnostics and outcome replay); the post-call checks below are ported as Rust unit tests next to the pass.

- PeepholeFoldConstantsTest#testFoldAddTemplateLiterals_validateRawAndCookedStringForLiteralBackslash_andValidateChildren: folding `${foo()}\\` + `n\t${()=>{return b;}}` yields one TEMPLATELIT with 5 children: "" / SUB call foo() / cooked `\n<TAB>` raw `\\n\t` / SUB arrow function with empty name / ""
- PeepholeFoldConstantsTest#testFoldAddTemplateLiterals_validateRawAndCookedStringForSpecialChars_andValidateChildren: folding `${a}\n` + `\t${b}` yields one TEMPLATELIT with 5 children: "" / SUB a / cooked `<LF><TAB>` raw `\n\t` / SUB b / ""
