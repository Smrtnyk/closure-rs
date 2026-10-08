/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated.
 * Copied from test/com/google/javascript/jscomp/PeepholeOptimizationsPassTest.java
 * (closure-compiler commit bb8c8e7):
 *   RemoveNodesNamedXUnderVarOptimization: lines 115-139, VERBATIM (javadoc included).
 *   RemoveNodesNamedXOptimization:         lines 141-157, VERBATIM (javadoc included).
 *   RemoveParentVarsForNodesNamedX:        lines 159-176, VERBATIM (javadoc included).
 *   RenameYToX:                            lines 178-195, VERBATIM (javadoc included).
 *   Anon1 (= PeepholeOptimizationsPassTest$1, `note1Applied` in testOptimizationOrder):
 *     the anonymous class body of lines 80-89; its optimizeSubtree (lines 81-88) is VERBATIM
 *     apart from 6 spaces less indentation. The anonymous class became a named static nested
 *     class whose constructor takes the captured local `visitationLog` (line 77, declared
 *     `final List<String>`).
 *   Anon2 (= PeepholeOptimizationsPassTest$2, `note2Applied`): lines 92-101, treated as Anon1.
 *   Anon3 (= PeepholeOptimizationsPassTest$3, first element in testAddFeatureToEnclosingScript):
 *     lines 249-259; its optimizeSubtree (lines 250-258) is VERBATIM apart from 10 spaces less
 *     indentation. It captures nothing, so the named class has the default constructor.
 *   Anon4 (= PeepholeOptimizationsPassTest$4, second element): lines 260-268, treated as Anon3.
 * Imports: the subset of lines 19-33 that these classes use, same text.
 * DSL names: PeepholeOptimizationsPassTest_Helpers.{RemoveNodesNamedXUnderVarOptimization,
 *   RemoveNodesNamedXOptimization, RemoveParentVarsForNodesNamedX, RenameYToX} (no args),
 *   .Anon1 / .Anon2 (args: visitationLog), .Anon3 / .Anon4 (no args).
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.parsing.parser.FeatureSet.Feature;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.Token;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

final class PeepholeOptimizationsPassTest_Helpers {

  /**
   * A peephole optimization that, given a subtree consisting of a VAR node, removes children of
   * that node named "x".
   */
  private static class RemoveNodesNamedXUnderVarOptimization extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isVar()) {
        Set<Node> nodesToRemove = new HashSet<>();

        for (Node child = node.getFirstChild(); child != null; child = child.getNext()) {
          if ("x".equals(child.getString())) {
            nodesToRemove.add(child);
          }
        }

        for (Node childToRemove : nodesToRemove) {
          reportChangeToEnclosingScope(node);
          childToRemove.detach();
        }
      }

      return node;
    }
  }

  /**
   * A peephole optimization that, given a subtree consisting of a name node named "x" removes that
   * node.
   */
  private static class RemoveNodesNamedXOptimization extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isName() && "x".equals(node.getString())) {
        reportChangeToEnclosingScope(node);
        node.detach();

        return null;
      }

      return node;
    }
  }

  /**
   * A peephole optimization that, given a subtree consisting of a name node named "x" whose parent
   * is a VAR node, removes the parent VAR node.
   */
  private static class RemoveParentVarsForNodesNamedX extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isName() && "x".equals(node.getString())) {
        Node parent = node.getParent();
        if (parent.isVar()) {
          reportChangeToEnclosingScope(parent);
          parent.detach();
          return null;
        }
      }
      return node;
    }
  }

  /**
   * A peephole optimization that, given a subtree consisting of a name node named "y", replaces it
   * with a name node named "x";
   */
  private static class RenameYToX extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isName() && "y".equals(node.getString())) {
        Node replacement = Node.newString(Token.NAME, "x");

        node.replaceWith(replacement);
        reportChangeToEnclosingScope(replacement);

        return replacement;
      }
      return node;
    }
  }

  // Generated: the anonymous class `note1Applied` of testOptimizationOrder
  // (PeepholeOptimizationsPassTest$1), named; `visitationLog` is the captured local.
  private static final class Anon1 extends AbstractPeepholeOptimization {
    private final List<String> visitationLog;

    private Anon1(final List<String> visitationLog) {
      this.visitationLog = visitationLog;
    }

    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isName()) {
        visitationLog.add(node.getString() + "1");
      }

      return node;
    }
  }

  // Generated: the anonymous class `note2Applied` of testOptimizationOrder
  // (PeepholeOptimizationsPassTest$2), named; `visitationLog` is the captured local.
  private static final class Anon2 extends AbstractPeepholeOptimization {
    private final List<String> visitationLog;

    private Anon2(final List<String> visitationLog) {
      this.visitationLog = visitationLog;
    }

    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isName()) {
        visitationLog.add(node.getString() + "2");
      }

      return node;
    }
  }

  // Generated: the first anonymous class of testAddFeatureToEnclosingScript
  // (PeepholeOptimizationsPassTest$3), named.
  private static final class Anon3 extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isAdd()) {
        this.addFeatureToEnclosingScript(Feature.LET_DECLARATIONS);
        this.addFeatureToEnclosingScript(Feature.LET_DECLARATIONS);
        this.addFeatureToEnclosingScript(Feature.CLASSES);
      }
      return node;
    }
  }

  // Generated: the second anonymous class of testAddFeatureToEnclosingScript
  // (PeepholeOptimizationsPassTest$4), named.
  private static final class Anon4 extends AbstractPeepholeOptimization {
    @Override
    public Node optimizeSubtree(Node node) {
      if (node.isSub()) {
        this.addFeatureToEnclosingScript(Feature.CONST_DECLARATIONS);
      }
      return node;
    }
  }
}
