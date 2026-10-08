/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its two
 * fields (mirroring VarCheckTest's fields of the same names and types, restored from the record's
 * testFields by the DSL "outer" list) are generated. Copied from
 * test/com/google/javascript/jscomp/VarCheckTest.java (closure-compiler commit bb8c8e7):
 *   GetProcessorPass: the anonymous CompilerPass returned by getProcessor (lines 80-93). Its
 *                     process method (lines 81-92) is VERBATIM apart from one less indentation
 *                     level (2 spaces); the anonymous class became a named
 *                     inner class whose constructor takes the captured getProcessor parameter
 *                     `compiler`.
 *   VariableTestCheck: lines 1232-1255, VERBATIM.
 * DSL names: VarCheckTest_Helpers.GetProcessorPass (outer: validityCheck, declarationCheck)
 */
package com.google.javascript.jscomp;

import static com.google.javascript.jscomp.testing.ScopeSubject.assertScope;

import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.rhino.Node;

final class VarCheckTest_Helpers {
  private boolean validityCheck = false;

  private boolean declarationCheck;

  // Generated: the anonymous `new CompilerPass() {...}` of VarCheckTest#getProcessor, named.
  private class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(final Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      new VarCheck(compiler, validityCheck).process(externs, root);
      if (!validityCheck && !compiler.hasErrors()) {
        // If the original test turned off sanity check, make sure our synthesized
        // code passes it.
        new VarCheck(compiler, true).process(externs, root);
      }
      if (declarationCheck) {
        new VariableTestCheck(compiler).process(externs, root);
      }
    }
  }

  private static final class VariableTestCheck implements CompilerPass {

    final AbstractCompiler compiler;

    VariableTestCheck(AbstractCompiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      NodeTraversal.traverseRoots(
          compiler,
          new AbstractPostOrderCallback() {
            @Override
            public void visit(NodeTraversal t, Node n, Node parent) {
              if (n.isName() && !parent.isFunction() && !parent.isLabel()) {
                assertScope(t.getScope()).declares(n.getString());
              }
            }
          },
          externs,
          root);
    }
  }
}
