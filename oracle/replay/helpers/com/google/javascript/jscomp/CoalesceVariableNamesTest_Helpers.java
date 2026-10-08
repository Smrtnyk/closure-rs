/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM
 * (re-indented only) from test/com/google/javascript/jscomp/CoalesceVariableNamesTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder field usePseudoName, line 32 (restored from testFields by the DSL "outer" list);
 *   - getProcessor with its anonymous CompilerPass (CoalesceVariableNamesTest$1), lines 41-53.
 * The anonymous pass reads usePseudoName from the holder, exactly as it reads the test field.
 * DSL name: CoalesceVariableNamesTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class CoalesceVariableNamesTest_Helpers {
  private boolean usePseudoName = false;

  private final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      return new CompilerPass() {
        @Override
        public void process(Node externs, Node root) {
          // enableNormalize would require output of CoalesceVariableNames to be normalized,
          // so we just manually normalize the input instead.
          Normalize normalize = Normalize.createNormalizeForOptimizations(compiler);
          normalize.process(externs, root);
          new CoalesceVariableNames(compiler, usePseudoName).process(externs, root);
        }
      };
    }
  }
}
