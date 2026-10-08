/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/Es6ConvertSuperTest.java (closure-compiler commit bb8c8e7):
 *   - getProcessor, lines 66-72 (makePassFactory is the inherited CompilerTestCase helper).
 * The method references Es6NormalizeClasses::new and Es6ConvertSuper::new cannot be expressed in
 * the DSL, so the whole getProcessor body is the helper.
 * DSL name: Es6ConvertSuperTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

final class Es6ConvertSuperTest_Helpers {
  private static final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new));
      optimizer.addOneTimePass(makePassFactory("es6ConvertSuper", Es6ConvertSuper::new));
      return optimizer;
    }
  }
}
