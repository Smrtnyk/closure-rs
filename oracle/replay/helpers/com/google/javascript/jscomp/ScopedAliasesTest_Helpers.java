/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated.
 * Copied VERBATIM (original indentation kept) from
 *   test/com/google/javascript/jscomp/ScopedAliasesTest.java (closure-compiler commit bb8c8e7):
 *   - the private static nested class TypeVerifyingPass, lines 1534-1574.
 * It is used by the descriptor's postcondition, which is the DSL form of the test's
 * VERIFY_TYPES constant (lines 76-78):
 *   (Compiler compiler) ->
 *       new TypeVerifyingPass(compiler).process(compiler.getExternsRoot(), compiler.getJsRoot());
 * DSL name: ScopedAliasesTest_Helpers.TypeVerifyingPass
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertWithMessage;
import static com.google.javascript.rhino.testing.NodeSubject.assertNode;

import com.google.javascript.rhino.JSDocInfo;
import com.google.javascript.rhino.Node;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import org.jspecify.annotations.Nullable;

final class ScopedAliasesTest_Helpers {
  private static class TypeVerifyingPass implements CompilerPass, NodeTraversal.Callback {
    private final Compiler compiler;
    private @Nullable List<Node> actualTypes = null;

    TypeVerifyingPass(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      NodeTraversal.traverse(compiler, root, this);
    }

    @Override
    public boolean shouldTraverse(NodeTraversal nodeTraversal, Node n, Node parent) {
      return true;
    }

    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      JSDocInfo info = n.getJSDocInfo();
      if (info != null) {
        Collection<Node> typeNodes = info.getTypeNodes();
        if (!typeNodes.isEmpty()) {
          if (actualTypes != null) {
            List<Node> expectedTypes = new ArrayList<>();
            expectedTypes.addAll(info.getTypeNodes());
            assertWithMessage("Wrong number of JsDoc types")
                .that(actualTypes.size())
                .isEqualTo(expectedTypes.size());
            for (int i = 0; i < expectedTypes.size(); i++) {
              assertNode(actualTypes.get(i)).isEqualTo(expectedTypes.get(i));
            }
          } else {
            actualTypes = new ArrayList<>();
            actualTypes.addAll(info.getTypeNodes());
          }
        }
      }
    }
  }
}
