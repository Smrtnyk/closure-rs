/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class wrapper `GetProcessor` (its `name` field, constructor and getName()) are generated: the
 * anonymous CompilerPass calls the test instance's getName(), which is CompilerTestCase.getName()
 * = getClass().getSimpleName() = "CreateSyntheticBlocksTest"; the wrapper returns the recorded
 * harness.effective.getName, passed as its constructor argument. Copied VERBATIM (re-indented
 * only) from test/com/google/javascript/jscomp/CreateSyntheticBlocksTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the constants START_MARKER and END_MARKER, lines 31-32;
 *   - getProcessor, lines 50-68 (an anonymous CompilerPass).
 * DSL name: CreateSyntheticBlocksTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.parsing.parser.FeatureSet;
import com.google.javascript.rhino.Node;

final class CreateSyntheticBlocksTest_Helpers {
  private static final String START_MARKER = "startMarker";
  private static final String END_MARKER = "endMarker";

  private static final class GetProcessor {
    private final String name;

    private GetProcessor(String name) {
      this.name = name;
    }

    public String getName() {
      return name;
    }

    protected CompilerPass getProcessor(final Compiler compiler) {
      return new CompilerPass() {
        @Override
        public void process(Node externs, Node js) {
          new CreateSyntheticBlocks(compiler, START_MARKER, END_MARKER).process(externs, js);
          Normalize.createNormalizeForOptimizations(compiler).process(externs, js);
          new PeepholeOptimizationsPass(
                  compiler,
                  getName(),
                  new MinimizeExitPoints(),
                  new PeepholeRemoveDeadCode(),
                  new PeepholeMinimizeConditions(/* late= */ true),
                  new PeepholeFoldConstants(true, false /* useTypes */))
              .process(externs, js);
          new Denormalize(compiler, FeatureSet.BARE_MINIMUM).process(externs, js);
        }
      };
    }
  }
}
