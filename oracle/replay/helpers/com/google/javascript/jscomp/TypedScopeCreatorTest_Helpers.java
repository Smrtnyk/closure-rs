/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated. Its
 * fields, the nested classes and the getProcessor method are copied VERBATIM from
 * test/com/google/javascript/jscomp/TypedScopeCreatorTest.java (closure-compiler commit bb8c8e7):
 *   fields registry .. labeledStatementMap, LabeledStatement:  lines 79-104
 *   ScopeFinder:                                              lines 116-139
 *   getProcessor:                                             lines 148-167 (only the @Override
 *                                                             annotation on line 148 is dropped,
 *                                                             because the holder is not a
 *                                                             CompilerTestCase)
 * The test's getProcessor writes registry and labeledStatementMap and returns a lambda that
 * captures the test instance (for ScopeFinder, which writes globalScope/lastLocalScope/
 * lastFunctionScope and reads/writes labeledStatementMap). Here a fresh holder instance plays the
 * test instance. All those fields are either reset by getProcessor or write-only, so no recorded
 * test field needs restoring; processClosurePrimitives only feeds getOptions, whose effect is the
 * record's options.
 * DSL use: {"call": {"helper": "TypedScopeCreatorTest_Helpers"}, "method": "getProcessor",
 *           "args": [{"compiler": true}]}
 */
package com.google.javascript.jscomp;

import static com.google.common.base.Preconditions.checkNotNull;
import static com.google.common.base.Preconditions.checkState;
import static com.google.common.truth.Truth.assertWithMessage;

import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;
import com.google.javascript.jscomp.modules.ModuleMapCreator;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.jstype.JSTypeRegistry;
import java.util.HashMap;
import java.util.Map;

final class TypedScopeCreatorTest_Helpers {

  private JSTypeRegistry registry;
  private TypedScope globalScope;
  private TypedScope lastLocalScope;
  private TypedScope lastFunctionScope;
  private boolean processClosurePrimitives = false;

  /**
   * Maps a label name to information about the labeled statement.
   *
   * <p>This map is recreated each time parseAndRunTypeInference() is executed.
   * TODO(bradfordcsmith): This map and LabeledStatement are also in TypeInferenceTest. It would be
   * good to unify them.
   */
  private Map<String, LabeledStatement> labeledStatementMap;

  /** Stores information about a labeled statement and allows making assertions on it. */
  static class LabeledStatement {
    final Node statementNode;
    final TypedScope enclosingScope;

    LabeledStatement(Node statementNode, TypedScope enclosingScope) {
      this.statementNode = checkNotNull(statementNode);
      this.enclosingScope = checkNotNull(enclosingScope);
    }
  }

  private class ScopeFinder extends AbstractPostOrderCallback {
    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      TypedScope scope = t.getTypedScope();
      if (scope.isGlobal()) {
        globalScope = scope;
      } else if (scope.isBlockScope()) {
        // TODO(bradfordcsmith): use labels to find scopes instead of lastLocalScope
        lastLocalScope = scope;
      } else if (scope.isFunctionScope()) {
        lastFunctionScope = scope;
      }
      if (parent != null && parent.isLabel() && !n.isLabelName()) {
        // First child of a LABEL is a LABEL_NAME, n is the second child.
        Node labelNameNode = checkNotNull(n.getPrevious(), n);
        checkState(labelNameNode.isLabelName(), labelNameNode);
        String labelName = labelNameNode.getString();
        assertWithMessage("Duplicate label name: %s", labelName)
            .that(labeledStatementMap)
            .doesNotContainKey(labelName);
        labeledStatementMap.put(labelName, new LabeledStatement(n, scope));
      }
    }
  }

  protected CompilerPass getProcessor(final Compiler compiler) {
    registry = compiler.getTypeRegistry();
    // Create a fresh statement map for each test case.
    labeledStatementMap = new HashMap<>();
    return (Node externs, Node root) -> {
      new GatherModuleMetadata(compiler, false, ResolutionMode.BROWSER).process(externs, root);
      new ModuleMapCreator(compiler, compiler.getModuleMetadataMap()).process(externs, root);
      new InferConsts(compiler).process(externs, root);

      TypedScopeCreator scopeCreator = new TypedScopeCreator(compiler);
      new TypeInferencePass(compiler, compiler.getReverseAbstractInterpreter(), scopeCreator)
          .inferAllScopes(root.getParent());
      NodeTraversal.builder()
          .setCompiler(compiler)
          .setCallback(new ScopeFinder())
          .setScopeCreator(scopeCreator)
          .traverseRoots(externs, root);
    };
  }
}
