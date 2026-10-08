/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder; its two fields
 * mirror RemoveUnusedCodeTest's fields of the same names (lines 48-49) and are restored from the
 * record's testFields by the DSL "outer" list. The method getProcessor below is copied VERBATIM
 * (without its @Override line, since the holder has no superclass) from
 * test/com/google/javascript/jscomp/RemoveUnusedCodeTest.java (closure-compiler commit bb8c8e7):
 *   getProcessor (anonymous CompilerPass RemoveUnusedCodeTest$1): lines 115-128
 * The generated inner class Processor only delegates to the pass that getProcessor returns, so
 * the anonymous class keeps its captured `compiler` and reads removeGlobal and
 * preserveFunctionExpressionNames from the outer instance at process time, as in the test.
 * DSL name: RemoveUnusedCodeTestHelpers.Processor
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class RemoveUnusedCodeTestHelpers {
  private boolean removeGlobal;
  private boolean preserveFunctionExpressionNames;

  /** Generated: the CompilerPass returned by the verbatim getProcessor, for a given compiler. */
  private class Processor implements CompilerPass {
    private final CompilerPass delegate;

    private Processor(Compiler compiler) {
      this.delegate = getProcessor(compiler);
    }

    @Override
    public void process(Node externs, Node root) {
      delegate.process(externs, root);
    }
  }

  // ---- verbatim from RemoveUnusedCodeTest.java lines 115-128 ----
  protected CompilerPass getProcessor(final Compiler compiler) {
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        new RemoveUnusedCode.Builder(compiler)
            .removeLocalVars(true)
            .removeGlobals(removeGlobal)
            .removeUnusedPolyfills(true)
            .preserveFunctionExpressionNames(preserveFunctionExpressionNames)
            .build()
            .process(externs, root);
      }
    };
  }
}
