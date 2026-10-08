/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class's declaration, field and constructor are generated: the original is the anonymous
 * `new CompilerPass() {...}` (runtime name OptimizeCallsIntegrationTest$1) returned by
 * getProcessor, which captures only the final parameter `compiler`. The process() method is copied
 * VERBATIM from
 *   test/com/google/javascript/jscomp/OptimizeCallsIntegrationTest.java lines 58-74 (anonymous
 *   class at lines 56-75)
 * (closure-compiler commit bb8c8e7). DSL name: OptimizeCallsIntegrationTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class OptimizeCallsIntegrationTest_Helpers {
  private OptimizeCallsIntegrationTest_Helpers() {}

  /** The anonymous CompilerPass returned by getProcessor. */
  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(final Compiler compiler) {
      this.compiler = compiler;
    }

      @Override
      public void process(Node externs, Node root) {
        new PureFunctionIdentifier.Driver(compiler).process(externs, root);
        new RemoveUnusedCode.Builder(compiler)
            .removeLocalVars(true)
            .removeGlobals(true)
            .build()
            .process(externs, root);

        OptimizeCalls.builder()
            .setCompiler(compiler)
            .setConsiderExterns(false)
            .addPass(new OptimizeReturns(compiler))
            .addPass(new OptimizeParameters(compiler))
            .build()
            .process(externs, root);
      }
  }
}
