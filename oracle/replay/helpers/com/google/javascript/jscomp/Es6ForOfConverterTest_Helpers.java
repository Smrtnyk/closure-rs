/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/Es6ForOfConverterTest.java (closure-compiler commit bb8c8e7):
 *   - getProcessor, lines 56-64 (makePassFactory is the inherited CompilerTestCase helper).
 * The method references InjectTranspilationRuntimeLibraries::new and Es6ForOfConverter::new
 * cannot be expressed in the DSL, so the whole getProcessor body is the helper.
 * DSL name: Es6ForOfConverterTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

final class Es6ForOfConverterTest_Helpers {
  private static final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory(
              "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
      optimizer.addOneTimePass(makePassFactory("es6ForOfConverter", Es6ForOfConverter::new));
      return optimizer;
    }
  }
}
