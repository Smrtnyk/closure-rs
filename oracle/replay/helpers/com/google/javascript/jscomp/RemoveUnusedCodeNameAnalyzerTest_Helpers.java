/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated; the
 * nested class is copied VERBATIM from
 * test/com/google/javascript/jscomp/RemoveUnusedCodeNameAnalyzerTest.java (closure-compiler commit
 * bb8c8e7):
 *   MarkNoSideEffectCallsAndRemoveUnusedCodeRunner: lines 75-99
 * DSL name: RemoveUnusedCodeNameAnalyzerTest_Helpers.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class RemoveUnusedCodeNameAnalyzerTest_Helpers {

  private static class MarkNoSideEffectCallsAndRemoveUnusedCodeRunner implements CompilerPass {
    final PureFunctionIdentifier.Driver pureFunctionIdentifier;
    final RemoveUnusedCode removeUnusedCode;

    MarkNoSideEffectCallsAndRemoveUnusedCodeRunner(Compiler compiler) {
      this.pureFunctionIdentifier = new PureFunctionIdentifier.Driver(compiler);
      this.removeUnusedCode =
          new RemoveUnusedCode.Builder(compiler)
              .removeGlobals(true)
              .removeLocalVars(true)
              .removeUnusedPrototypeProperties(true)
              .removeUnusedThisProperties(true)
              .removeUnusedObjectDefinePropertiesDefinitions(true)
              // Removal of function expression names isn't what these tests are about.
              // It just adds noise to the tests when we can't use testSame() because of it.
              .preserveFunctionExpressionNames(true)
              .build();
    }

    @Override
    public void process(Node externs, Node root) {
      pureFunctionIdentifier.process(externs, root);
      removeUnusedCode.process(externs, root);
    }
  }
}
