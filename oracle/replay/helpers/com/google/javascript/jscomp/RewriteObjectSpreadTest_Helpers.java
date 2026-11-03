/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/RewriteObjectSpreadTest.java (closure-compiler commit 977e2cf):
 *   - getProcessor, lines 54-62 (makePassFactory is the inherited CompilerTestCase helper).
 * The method references InjectTranspilationRuntimeLibraries::new and RewriteObjectSpread::new
 * cannot be expressed in the DSL, so the whole getProcessor body is the helper.
 * DSL name: RewriteObjectSpreadTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

final class RewriteObjectSpreadTest_Helpers {
  private static final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory(
              "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
      optimizer.addOneTimePass(makePassFactory("rewriteObjectSpread", RewriteObjectSpread::new));
      return optimizer;
    }
  }
}
