/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor` are generated. Copied VERBATIM (re-indented only) from
 * test/com/google/javascript/jscomp/ManageClosureUnawareCodeTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder fields runUnwrapPass and gatheredShadowNodeRoot, lines 56-57. runUnwrapPass is
 *     restored from testFields by the DSL "outer" list. gatheredShadowNodeRoot is recorded only
 *     as a Node ref; it is a detached IR.root() that the gathering pass appends shadow clones to
 *     for the test's own follow-up assertNode, so the fresh IR.root() initializer is kept;
 *   - getProcessor, lines 69-106 (a PhaseOptimizer of a gatherShadowNodes pass and, when
 *     runUnwrapPass, ManageClosureUnawareCode::unwrap).
 * DSL name: ManageClosureUnawareCodeTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.IR;
import com.google.javascript.rhino.Node;
import java.util.ArrayList;
import java.util.List;

final class ManageClosureUnawareCodeTest_Helpers {
  private boolean runUnwrapPass = true;
  private Node gatheredShadowNodeRoot = IR.root();

  private final class GetProcessor {
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
                                  if (!runUnwrapPass) {
                                    node.setClosureUnawareShadow(null);
                                  }
                                  gatheredShadowNodeRoot.addChildToBack(shadow.cloneTree());
                                }
                              });
                        }
                      })
              .build());

      if (runUnwrapPass) {
        passes.add(
            PassFactory.builder()
                .setName("unwrapClosureUnawareCode")
                .setInternalFactory(ManageClosureUnawareCode::unwrap)
                .build());
      }
      phaseopt.consume(passes);
      return phaseopt;
    }
  }
}
