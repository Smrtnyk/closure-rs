/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its no-arg
 * constructor are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/NodeUtilTest.java (closure-compiler commit 48f4107ca):
 *   getProcessor: lines 4837-4846, of the static nested test class
 *                 NodeUtilTest.CreateSynthesizedExternsSymbolTests (returns an anonymous
 *                 CompilerPass). The @Override annotation on line 4837 is dropped, because the
 *                 holder does not extend CompilerTestCase, and the method is one indentation
 *                 level (2 spaces) shallower because the nesting test class is not copied.
 * DSL: {"mutationPoint": "com.google.javascript.jscomp.NodeUtil",
 *       "value": {"call": {"helper": "NodeUtilTest_Helpers"}, "method": "getProcessor",
 *                 "args": [{"compiler": true}]}}
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class NodeUtilTest_Helpers {

  NodeUtilTest_Helpers() {}

  protected CompilerPass getProcessor(Compiler compiler) {
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node srcs) {
        String expectedExtern = "TEST_NAME";
        NodeUtil.createSynthesizedExternsSymbol(compiler, expectedExtern);
      }
    };
  }
}
