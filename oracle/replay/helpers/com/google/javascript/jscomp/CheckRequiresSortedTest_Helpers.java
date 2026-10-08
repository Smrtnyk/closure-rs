/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its no-arg
 * constructor are generated. getProcessor (with its local `callback` and its anonymous
 * CompilerPass) is copied VERBATIM from
 *   test/com/google/javascript/jscomp/lint/CheckRequiresSortedTest.java lines 40-49
 * (closure-compiler commit bb8c8e7); the @Override annotation is dropped, because the holder does
 * not extend CompilerTestCase. The test class declares no fields; the anonymous pass captures only
 * getProcessor's `compiler` parameter and the local `callback`.
 * DSL: {"call": {"helper": "CheckRequiresSortedTest_Helpers"}, "method": "getProcessor",
 *       "args": [{"compiler": true}]}
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.lint.CheckRequiresSorted;
import com.google.javascript.rhino.Node;

final class CheckRequiresSortedTest_Helpers {
  CheckRequiresSortedTest_Helpers() {}

  protected CompilerPass getProcessor(Compiler compiler) {
    CheckRequiresSorted callback =
        new CheckRequiresSorted(CheckRequiresSorted.Mode.COLLECT_AND_REPORT);
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        NodeTraversal.traverse(compiler, root, callback);
      }
    };
  }
}
