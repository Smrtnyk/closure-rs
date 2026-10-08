/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the class
 * declaration/constructor of GetProcessorLambda are generated; the process body is the lambda
 * returned by getProcessor, copied VERBATIM from
 * test/com/google/javascript/jscomp/DeadAssignmentsEliminationTest.java (closure-compiler commit
 * bb8c8e7), lines 37-41:
 *
 *   protected CompilerPass getProcessor(final Compiler compiler) {
 *     return (externs, js) ->
 *         NodeTraversal.traverse(compiler, js, new DeadAssignmentsElimination(compiler));
 *   }
 *
 * DSL name: DeadAssignmentsEliminationTest_Helpers.GetProcessorLambda
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class DeadAssignmentsEliminationTest_Helpers {
  private DeadAssignmentsEliminationTest_Helpers() {}

  /** The getProcessor lambda; {@code compiler} is the captured getProcessor argument. */
  private static final class GetProcessorLambda implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorLambda(final Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node js) {
      NodeTraversal.traverse(compiler, js, new DeadAssignmentsElimination(compiler));
    }
  }
}
