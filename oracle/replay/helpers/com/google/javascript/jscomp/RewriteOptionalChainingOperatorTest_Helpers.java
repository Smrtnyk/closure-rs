/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, the two method
 * signatures and their trailing `return testVarNameCreator;` are generated. Copied VERBATIM
 * (indentation unchanged) from test/com/google/javascript/jscomp/RewriteOptionalChainingOperatorTest.java
 * (closure-compiler commit bb8c8e7):
 *   - import of OptionalChainRewriter.TmpVarNameCreator, line 19;
 *   - BaseTestClass.getProcessor's TmpVarNameCreator statement (comment + anonymous class),
 *     lines 401-410 -> baseTestClassTmpVarNameCreator();
 *   - DeleteOptChainTests.getProcessor's TmpVarNameCreator statement, lines 434-443
 *     -> deleteOptChainTestsTmpVarNameCreator().
 * The remaining getProcessor statement, `return new RewriteOptionalChainingOperator(compiler,
 * testVarNameCreator);` (lines 411 and 444), is the descriptor's DSL "new".
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.OptionalChainRewriter.TmpVarNameCreator;

final class RewriteOptionalChainingOperatorTest_Helpers {
  static TmpVarNameCreator baseTestClassTmpVarNameCreator() {
      // Just name temporary variables "tmp0", "tmp1", etc. to make the tests clearer.
      TmpVarNameCreator testVarNameCreator =
          new TmpVarNameCreator() {
            int counter = 0;

            @Override
            public String createTmpVarName() {
              return "tmp" + counter++;
            }
          };
      return testVarNameCreator;
  }

  static TmpVarNameCreator deleteOptChainTestsTmpVarNameCreator() {
      // Just name temporary variables "tmp0", "tmp1", etc. to make the tests clearer.
      TmpVarNameCreator testVarNameCreator =
          new TmpVarNameCreator() {
            int counter = 0;

            @Override
            public String createTmpVarName() {
              return "tmp" + counter++;
            }
          };
      return testVarNameCreator;
  }
}
