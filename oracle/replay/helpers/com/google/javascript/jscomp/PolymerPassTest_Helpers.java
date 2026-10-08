/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated.
 * Copied VERBATIM (the nested class keeps its original indentation) from
 *   test/com/google/javascript/jscomp/PolymerPassTest.java (closure-compiler commit bb8c8e7):
 *   - the private static nested class DoSomethingFunFinder, lines 2714-2724.
 * It is used by the descriptor's postcondition for testSimpleBehavior, which is the DSL form of
 * the postcondition lambda at lines 2659-2667:
 *   (Postcondition)
 *       compiler -> {
 *         Node root = compiler.getRoot();
 *         DoSomethingFunFinder visitor = new DoSomethingFunFinder();
 *         NodeUtil.visitPreOrder(root, visitor);
 *         assertThat(visitor.found).isTrue();
 *       }
 * DSL name: PolymerPassTest_Helpers.DoSomethingFunFinder
 */
package com.google.javascript.jscomp;

import static com.google.javascript.rhino.testing.NodeSubject.assertNode;

import com.google.javascript.jscomp.NodeUtil.Visitor;
import com.google.javascript.rhino.Node;

final class PolymerPassTest_Helpers {
  private static class DoSomethingFunFinder implements Visitor {
    boolean found = false;

    @Override
    public void visit(Node n) {
      if (n.matchesQualifiedName("A.prototype.doSomethingFun")) {
        assertNode(n).hasLineno(21);
        found = true;
      }
    }
  }
}
