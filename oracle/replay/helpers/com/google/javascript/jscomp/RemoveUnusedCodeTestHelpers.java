/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder; its three fields
 * mirror RemoveUnusedCodeTest's fields of the same names (lines 49, 50 and 97) and are restored
 * from the record's testFields by the DSL "outer" list. The method getProcessor below is copied
 * VERBATIM (without its @Override line, since the holder has no superclass) from
 * test/com/google/javascript/jscomp/RemoveUnusedCodeTest.java (closure-compiler commit 8ac03b6):
 *   getProcessor (anonymous CompilerPass RemoveUnusedCodeTest$1): lines 119-134
 * The generated inner class Processor only delegates to the pass that getProcessor returns, so
 * the anonymous class keeps its captured `compiler` and reads removeGlobal,
 * preserveFunctionExpressionNames and disambiguateProperties from the outer instance at process
 * time, as in the test.
 * DSL name: RemoveUnusedCodeTestHelpers.Processor
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class RemoveUnusedCodeTestHelpers {
  private boolean removeGlobal;
  private boolean preserveFunctionExpressionNames;
  private boolean disambiguateProperties = false;

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

  // ---- verbatim from RemoveUnusedCodeTest.java lines 119-134 ----
  protected CompilerPass getProcessor(final Compiler compiler) {
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        new RemoveUnusedCode.Builder(compiler)
            .removeLocalVars(true)
            .removeGlobals(removeGlobal)
            .removeUnusedPolyfills(true)
            .preserveFunctionExpressionNames(preserveFunctionExpressionNames)
            .disambiguateProperties(disambiguateProperties)
            .build()
            .process(externs, root);
      }
    };
  }
}
