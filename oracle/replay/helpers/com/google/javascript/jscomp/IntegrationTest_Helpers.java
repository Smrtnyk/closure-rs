/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the two
 * nested class declarations are generated: the originals are anonymous classes (IntegrationTest$1
 * and IntegrationTest$2) that the test stores into CompilerOptions. The class bodies are copied
 * VERBATIM from test/com/google/javascript/jscomp/integration/IntegrationTest.java
 * (closure-compiler commit bb8c8e7):
 *   XidRenamingMap (IntegrationTest$1, testReplaceIdGeneratorsTest): body lines 1337-1340
 *   AlwaysRunSafetyCheckPass (IntegrationTest$2, testAlwaysRunSafetyCheck): body lines 3878-3883
 * DSL names: IntegrationTest_Helpers.XidRenamingMap, IntegrationTest_Helpers.AlwaysRunSafetyCheckPass
 *
 * Used by the "options" overrides (skip + then) in corpus/unit/descriptors/IntegrationTest.json,
 * which re-run the test's own setIdGenerators / addCustomPass calls on these classes.
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertThat;

import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.Token;

final class IntegrationTest_Helpers {
  private static class XidRenamingMap implements RenamingMap {
              @Override
              public String get(String value) {
                return ":" + value + ":";
              }
  }

  private static class AlwaysRunSafetyCheckPass implements CompilerPass {
          @Override
          public void process(Node externs, Node root) {
            Node var = root.getLastChild().getFirstChild();
            assertThat(var.getToken()).isEqualTo(Token.VAR);
            var.detach();
          }
  }
}
