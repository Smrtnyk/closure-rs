/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class's declaration, field and constructor are generated: the original is the anonymous
 * `new CompilerPass() {...}` (runtime name PolymerPassSuppressBehaviorsAndProtectKeysTest$1)
 * returned by getProcessor, which captures only the final parameter `compiler`. The process()
 * method is copied VERBATIM from
 *   test/com/google/javascript/jscomp/PolymerPassSuppressBehaviorsAndProtectKeysTest.java
 *   lines 69-74 (anonymous class at lines 68-75)
 * (closure-compiler commit bb8c8e7).
 * DSL name: PolymerPassSuppressBehaviorsAndProtectKeysTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class PolymerPassSuppressBehaviorsAndProtectKeysTest_Helpers {
  private PolymerPassSuppressBehaviorsAndProtectKeysTest_Helpers() {}

  /** The anonymous CompilerPass returned by getProcessor. */
  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(final Compiler compiler) {
      this.compiler = compiler;
    }

      @Override
      public void process(Node externs, Node root) {
        PolymerPassSuppressBehaviorsAndProtectKeys suppressBehaviorsCallback =
            new PolymerPassSuppressBehaviorsAndProtectKeys(compiler);
        NodeTraversal.traverse(compiler, root, suppressBehaviorsCallback);
      }
  }
}
