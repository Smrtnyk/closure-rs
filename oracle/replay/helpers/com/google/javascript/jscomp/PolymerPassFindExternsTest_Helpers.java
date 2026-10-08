/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, its field
 * (mirroring PolymerPassFindExternsTest's field of the same name and type, line 64) and the no-arg
 * constructor are generated. getProcessor (with its anonymous CompilerPass) is copied VERBATIM from
 *   test/com/google/javascript/jscomp/PolymerPassFindExternsTest.java lines 71-79
 * (closure-compiler commit bb8c8e7); the @Override annotation is dropped, because the holder does
 * not extend CompilerTestCase. getProcessor overwrites findExternsCallback before the anonymous
 * pass reads it, so the field needs no restore (it is recorded as null in every record). The field
 * exists only for the test's own follow-up assertions (getPolymerElementExterns /
 * getPolymerElementProps), which are not part of the recorded harness call.
 * DSL: {"call": {"helper": "PolymerPassFindExternsTest_Helpers"}, "method": "getProcessor",
 *       "args": [{"compiler": true}]}
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class PolymerPassFindExternsTest_Helpers {
  private PolymerPassFindExterns findExternsCallback;

  PolymerPassFindExternsTest_Helpers() {}

  protected CompilerPass getProcessor(final Compiler compiler) {
    findExternsCallback = new PolymerPassFindExterns();
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        NodeTraversal.traverse(compiler, externs, findExternsCallback);
      }
    };
  }
}
