/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated.
 * Copied from test/com/google/javascript/jscomp/GuardedCallbackTest.java (closure-compiler commit
 * bb8c8e7):
 *   GetProcessorPass:       the anonymous CompilerPass returned by getProcessor (lines 31-36).
 *                           Its process method (lines 32-35) is VERBATIM apart from one less
 *                           indentation level (2 spaces); the anonymous class became a named
 *                           static nested class whose constructor takes the captured
 *                           getProcessor parameter `compiler` (declared `final Compiler`).
 *   GuardSwitchingCallback: lines 39-62, VERBATIM (javadoc included).
 * Imports: the subset of lines 19-21 that these classes use, same text.
 * DSL name: GuardedCallbackTest_Helpers.GetProcessorPass (args: compiler)
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class GuardedCallbackTest_Helpers {

  // Generated: the anonymous `new CompilerPass() {...}` of GuardedCallbackTest#getProcessor, named.
  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(final Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      NodeTraversal.traverse(compiler, root, new GuardSwitchingCallback(compiler));
    }
  }

  /**
   * Replaces all guarded name and property references with "GUARDED_NAME" and "GUARDED_PROP",
   * respectively.
   */
  private static class GuardSwitchingCallback extends GuardedCallback<String> {

    GuardSwitchingCallback(AbstractCompiler compiler) {
      super(compiler);
    }

    @Override
    void visitGuarded(NodeTraversal traversal, Node n, Node parent) {
      if (n.isName() && isGuarded(n.getString())) {
        n.setString("GUARDED_NAME");
        traversal.reportCodeChange();
      } else if (n.isGetProp() || n.isOptChainGetProp()) {
        // prefix guarded resource name with "." to keep properties distinct from names
        if (isGuarded("." + n.getString())) {
          n.setString("GUARDED_PROP");
          traversal.reportCodeChange();
        }
      }
    }
  }
}
