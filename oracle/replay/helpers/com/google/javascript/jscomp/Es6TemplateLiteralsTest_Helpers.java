/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration line are generated; the nested class body is getProcessor(Compiler), copied
 * VERBATIM (including @Override) from
 *   test/com/google/javascript/jscomp/Es6TemplateLiteralsTest.java lines 55-63
 * (closure-compiler commit bb8c8e7). The nested class extends CompilerTestCase (as the test does)
 * only so that the verbatim body resolves makePassFactory to the inherited package-private static
 * CompilerTestCase#makePassFactory; no other CompilerTestCase state is used. The descriptor calls
 * getProcessor on a fresh instance, so the processor is the PhaseOptimizer the test builds.
 * Same shape as LateEs6ToEs3ConverterTest_Helpers.
 * DSL name: Es6TemplateLiteralsTest_Helpers.GetProcessorHost
 */
package com.google.javascript.jscomp;

final class Es6TemplateLiteralsTest_Helpers {
  private static final class GetProcessorHost extends CompilerTestCase {

  @Override
  protected CompilerPass getProcessor(final Compiler compiler) {
    PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
    optimizer.addOneTimePass(
        makePassFactory(
            "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
    optimizer.addOneTimePass(makePassFactory("lateEs6ToEs3Converter", LateEs6ToEs3Converter::new));
    return optimizer;
  }
  }
}
