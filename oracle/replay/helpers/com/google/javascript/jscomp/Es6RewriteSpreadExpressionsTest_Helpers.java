/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the class
 * declaration/constructor of GetProcessorLambda are generated; the process body is the lambda
 * returned by getProcessor, copied VERBATIM from
 *   test/com/google/javascript/jscomp/Es6RewriteSpreadExpressionsTest.java lines 38-43
 * (closure-compiler commit bb8c8e7):
 *
 *   protected CompilerPass getProcessor(Compiler compiler) {
 *     return (externs, root) -> {
 *       new InjectTranspilationRuntimeLibraries(compiler).process(externs, root);
 *       new Es6RewriteSpreadExpressions(compiler).process(externs, root);
 *     };
 *   }
 *
 * The lambda constructs Es6RewriteSpreadExpressions only AFTER InjectTranspilationRuntimeLibraries
 * has run. That order matters in principle: the Es6RewriteSpreadExpressions constructor calls
 * compiler.getTranspilationNamespace(), which eagerly builds and caches a GlobalNamespace that is
 * supposed to see the injected runtime library (Compiler.java lines 2032-2048). A DSL
 * {"sequence":[...]} constructs both passes up front, so the lambda is kept verbatim. (On the 36
 * recorded cases the sequence form happens to replay identically too, presumably because the test
 * externs from addJSCompLibraries() already declare the $jscomp runtime.) The equivalent DSL
 * {"lambda":...} form is not used because the evaluator's reflective "call" wraps the pass's
 * RuntimeException in a new RuntimeException, which changes the message that
 * testSpreadIntoSuperThrows records; this Java body rethrows it unchanged. The descriptor wraps
 * this helper in {"mutationPoint":"...Es6RewriteSpreadExpressions"} for gate 0.2(d).
 *
 * DSL name: Es6RewriteSpreadExpressionsTest_Helpers.GetProcessorLambda
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class Es6RewriteSpreadExpressionsTest_Helpers {
  private Es6RewriteSpreadExpressionsTest_Helpers() {}

  /** The getProcessor lambda; {@code compiler} is the captured getProcessor argument. */
  private static final class GetProcessorLambda implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorLambda(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      new InjectTranspilationRuntimeLibraries(compiler).process(externs, root);
      new Es6RewriteSpreadExpressions(compiler).process(externs, root);
    }
  }
}
