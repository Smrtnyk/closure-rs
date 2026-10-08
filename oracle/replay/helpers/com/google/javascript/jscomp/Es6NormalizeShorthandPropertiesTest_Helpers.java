/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated.
 * Copied VERBATIM from
 *   test/com/google/javascript/jscomp/Es6NormalizeShorthandPropertiesTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the private static method assertNotShorthandProperty, lines 70-74;
 *   - the postcondition argument of testNormalizationInSource, lines 63-67
 *       postcondition(
 *           (compiler) ->
 *               NodeUtil.visitPreOrder(
 *                   compiler.getRoot(),
 *                   Es6NormalizeShorthandPropertiesTest::assertNotShorthandProperty))
 *     with the one change that the method reference names this holder instead of the test class
 *     (rule 6 keeps the test class off the replay classpath).
 * Only the factory method testNormalizationInSourcePostcondition() around that expression is
 * generated. It replaces the earlier DSL-lambda rebuild of the same postcondition: a DSL lambda invokes assertNotShorthandProperty reflectively and so rethrows its
 * Truth AssertionError wrapped in RuntimeException (ReplayDsl.invoke), which the gate's
 * assertion-type rule does not count; a real Java lambda lets the AssertionError reach testInternal
 * unchanged, exactly as in the original test.
 * DSL use (corpus/unit/descriptors/Es6NormalizeShorthandPropertiesTest.json "postconditions"):
 *   {"static":"com.google.javascript.jscomp.Es6NormalizeShorthandPropertiesTest_Helpers",
 *    "method":"testNormalizationInSourcePostcondition","args":[]}
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertWithMessage;
import static com.google.javascript.jscomp.CompilerTestCase.postcondition;

import com.google.javascript.rhino.Node;

final class Es6NormalizeShorthandPropertiesTest_Helpers {
  static CompilerTestCase.Postcondition testNormalizationInSourcePostcondition() {
    return postcondition(
            (compiler) ->
                NodeUtil.visitPreOrder(
                    compiler.getRoot(),
                    Es6NormalizeShorthandPropertiesTest_Helpers::assertNotShorthandProperty));
  }

  private static void assertNotShorthandProperty(Node node) {
    assertWithMessage("Detected shorthand property node <%s>.", node)
        .that(node.isShorthandProperty())
        .isFalse();
  }
}
