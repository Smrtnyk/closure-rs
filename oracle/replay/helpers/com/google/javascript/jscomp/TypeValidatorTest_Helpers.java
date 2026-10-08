/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder for the anonymous
 * no-op CompilerPass that TypeValidatorTest.getProcessor (lines 47-55) returns. Copied VERBATIM
 * from test/com/google/javascript/jscomp/TypeValidatorTest.java (closure-compiler commit bb8c8e7):
 *   Anon1 = TypeValidatorTest$1: anonymous class body, lines 50-53.
 * Only the static nested class declaration is generated. The test runs this pass purely for the
 * harness type check (enableTypeCheck in setUp, lines 40-45).
 * DSL name: TypeValidatorTest_Helpers.Anon1
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class TypeValidatorTest_Helpers {
  /** TypeValidatorTest$1 (getProcessor): anonymous CompilerPass, body lines 50-53. */
  static final class Anon1 implements CompilerPass {
      @Override
      public void process(Node externs, Node n) {
        // Do nothing: we're in it for the type-checking.
      }
  }
}
