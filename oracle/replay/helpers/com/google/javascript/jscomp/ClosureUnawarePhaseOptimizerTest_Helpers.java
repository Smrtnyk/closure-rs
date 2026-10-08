/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper") for ClosureUnawarePhaseOptimizerTest.
 * Source: test/com/google/javascript/jscomp/ClosureUnawarePhaseOptimizerTest.java at
 * closure-compiler commit bb8c8e7. This holder stands in for one test instance (one JUnit test
 * method = one record; the descriptor creates it once per record with {"once":...}).
 *
 * Copied VERBATIM (only indentation unchanged, nothing else edited):
 *   fields shadowNodes, aditionalPasses:           lines 57-58
 *   constants ARBITRARY_NUMERIC_CHANGE_*:          lines 60-62
 *   ChangedScopeNodesIterativePass (inner class):  lines 73-114
 *   getProcessor body:                             lines 117-157 (the @Override on line 116 is
 *                                                  dropped: the holder has no superclass)
 *   test-method prologues (the aditionalPasses.add(...) statements that run before test(...)):
 *     testClosureUnawareCodePassModifiesMainAST_incorrectly: lines 161-194
 *     testMainASTPassModifiesClosureUnawareCode_incorrectly: lines 236-269
 * Generated (not from the test): the holder class itself, the two prologue method signatures
 * (named after the test methods) and their trailing "return this;", which lets the DSL chain
 * {"call": prologue} -> {"call": getProcessor}. The @Before resetAditionalPasses (lines 52-55)
 * clears a list that is already empty on a fresh holder, so it has no counterpart.
 * Field languageInOverride (line 59) only feeds getOptions(), whose effect is in record.options.
 */
package com.google.javascript.jscomp;

import static com.google.common.base.Preconditions.checkState;

import com.google.javascript.rhino.Node;
import java.util.ArrayList;
import java.util.List;

final class ClosureUnawarePhaseOptimizerTest_Helpers {
  private final List<Node> shadowNodes = new ArrayList<>();
  private final List<PassFactory> aditionalPasses = new ArrayList<>();
  private static final String ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE =
      "arbitraryNumericChangeClosureUnawareCode";
  private static final String ARBITRARY_NUMERIC_CHANGE_MAIN_AST = "arbitraryNumericChangeMainAST";

  private final class ChangedScopeNodesIterativePass implements CompilerPass {
    private final String passName;
    private final AbstractCompiler compiler;

    private ChangedScopeNodesIterativePass(String passName, AbstractCompiler compiler) {
      this.passName = passName;
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      var cb =
          new NodeTraversal.AbstractPostOrderCallback() {
            @Override
            public void visit(NodeTraversal t, Node node, Node parent) {
              if (node.isNumber()) {
                double value = node.getDouble();
                if (value <= 3.0) {
                  node.setDouble(4.0);
                  compiler.reportChangeToEnclosingScope(node);
                }
              }
            }
          };
      for (List<Node> changedScopeNodes =
              compiler.getChangeTracker().getChangedScopeNodesForPass(passName);
          changedScopeNodes == null || !changedScopeNodes.isEmpty();
          changedScopeNodes = compiler.getChangeTracker().getChangedScopeNodesForPass(passName)) {
        // changedScopeNodes is only null if this is the first run of this pass.
        if (changedScopeNodes != null) {
          NodeTraversal.traverseScopeRoots(
              compiler, changedScopeNodes, cb, /* traverseNested= */ false);
          continue;
        }
        if (passName.equals(ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE)) {
          NodeTraversal.traverseScopeRoots(compiler, shadowNodes, cb, /* traverseNested= */ false);
        } else {
          NodeTraversal.traverse(compiler, root, cb);
        }
      }
    }
  }

  protected CompilerPass getProcessor(Compiler compiler) {
    PhaseOptimizer phaseopt = new PhaseOptimizer(compiler, null);
    List<PassFactory> passes = new ArrayList<>();
    passes.add(
        PassFactory.builder()
            .setName("gatherShadowNodes")
            .setInternalFactory(
                (c) ->
                    new CompilerPass() {

                      @Override
                      public void process(Node externs, Node root) {
                        NodeUtil.visitPreOrder(
                            root,
                            (Node node) -> {
                              Node shadow = node.getClosureUnawareShadow();
                              if (shadow != null) {
                                /*
                                 * shadow is a ROOT node that follows the pattern:
                                 * ROOT -> SCRIPT -> EXPR_RESULT -> CALL -> FUNCTION.
                                 */
                                Node exprResult = shadow.getFirstFirstChild();
                                checkState(exprResult.isExprResult(), exprResult);
                                Node shadowFunction = exprResult.getFirstChild().getSecondChild();
                                checkState(shadowFunction.isFunction(), shadowFunction);
                                shadowNodes.add(shadowFunction);
                              }
                            });
                      }
                    })
            .build());
    passes.addAll(aditionalPasses);
    phaseopt.consume(passes);
    phaseopt.setValidityCheck(
        PassFactory.builder()
            .setName("validityCheck")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(ValidityCheck::new)
            .build());
    return phaseopt;
  }

  /** Generated wrapper; body = test method prologue, lines 161-194 verbatim. */
  ClosureUnawarePhaseOptimizerTest_Helpers testClosureUnawareCodePassModifiesMainAST_incorrectly() {
    aditionalPasses.add(
        PassFactory.builder()
            .setName(ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE)
            .setInternalFactory(
                (c) ->
                    new ChangedScopeNodesIterativePass(
                        ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE, c))
            .setRunInFixedPointLoop(true)
            .build());

    aditionalPasses.add(
        PassFactory.builder()
            .setName(ARBITRARY_NUMERIC_CHANGE_MAIN_AST)
            .setInternalFactory(
                (c) ->
                    new CompilerPass() {

                      @Override
                      public void process(Node externs, Node root) {
                        NodeUtil.visitPreOrder(
                            root,
                            (Node node) -> {
                              if (node.isNumber()) {
                                double value = node.getDouble();
                                if (value == 1.0 || value == 2.0) {
                                  node.setDouble(value + 1);
                                  c.reportChangeToEnclosingScope(node);
                                }
                              }
                            });
                      }
                    })
            .setRunInFixedPointLoop(true)
            .build());
    return this;
  }

  /** Generated wrapper; body = test method prologue, lines 236-269 verbatim. */
  ClosureUnawarePhaseOptimizerTest_Helpers testMainASTPassModifiesClosureUnawareCode_incorrectly() {
    aditionalPasses.add(
        PassFactory.builder()
            .setName(ARBITRARY_NUMERIC_CHANGE_MAIN_AST)
            .setInternalFactory(
                (c) -> new ChangedScopeNodesIterativePass(ARBITRARY_NUMERIC_CHANGE_MAIN_AST, c))
            .setRunInFixedPointLoop(true)
            .build());

    aditionalPasses.add(
        PassFactory.builder()
            .setName(ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE)
            .setInternalFactory(
                (c) ->
                    new CompilerPass() {

                      @Override
                      public void process(Node externs, Node root) {
                        for (Node shadowRoot : shadowNodes) {
                          NodeUtil.visitPreOrder(
                              shadowRoot,
                              (Node node) -> {
                                if (node.isNumber()) {
                                  double value = node.getDouble();
                                  if (value == 1.0 || value == 2.0) {
                                    node.setDouble(value + 1);
                                    c.reportChangeToEnclosingScope(node);
                                  }
                                }
                              });
                        }
                      }
                    })
            .setRunInFixedPointLoop(true)
            .build());
    return this;
  }
}
