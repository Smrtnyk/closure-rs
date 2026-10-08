/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, its field
 * lastCheckViolationMessages (mirroring AstValidatorTest's field of the same name and type) and
 * the name of the nested class are generated. The field is initialised to a fresh ArrayList
 * because AstValidatorTest.createValidator (lines 71-87) assigns
 * `lastCheckViolationMessages = new ArrayList<>()` on every getProcessor call, and the DSL makes a
 * fresh holder for every evaluation. The nested class body is the anonymous ViolationHandler
 * copied VERBATIM from
 *   test/com/google/javascript/jscomp/AstValidatorTest.java lines 76-81
 * (closure-compiler commit bb8c8e7). DSL name: AstValidatorTest_Helpers.CreateValidatorViolationHandler
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.AstValidator.ViolationHandler;
import com.google.javascript.rhino.Node;
import java.util.ArrayList;
import java.util.List;

final class AstValidatorTest_Helpers {
  private List<String> lastCheckViolationMessages = new ArrayList<>();

  private class CreateValidatorViolationHandler implements ViolationHandler {
              @Override
              public void handleViolation(String message, Node n) {
                lastCheckViolationMessages.add(message);
              }
  }
}
