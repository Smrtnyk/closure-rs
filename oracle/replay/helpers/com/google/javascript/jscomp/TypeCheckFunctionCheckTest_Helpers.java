/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the name of
 * the nested class are generated; the original is the anonymous CompilerPass
 * TypeCheckFunctionCheckTest$1 returned by getProcessor. Its class body is copied VERBATIM from
 *   test/com/google/javascript/jscomp/TypeCheckFunctionCheckTest.java lines 37-42
 * (closure-compiler commit bb8c8e7):
 *
 *   protected CompilerPass getProcessor(Compiler compiler) {
 *     return new CompilerPass() {
 *       @Override
 *       public void process(Node externs, Node root) {}
 *     };
 *   }
 *
 * DSL name: TypeCheckFunctionCheckTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class TypeCheckFunctionCheckTest_Helpers {
  private TypeCheckFunctionCheckTest_Helpers() {}

  /** TypeCheckFunctionCheckTest$1 (the anonymous CompilerPass; it captures nothing). */
  private static final class GetProcessorPass implements CompilerPass {
      @Override
      public void process(Node externs, Node root) {}
  }
}
