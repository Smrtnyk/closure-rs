/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration line are generated; the nested class body is getProcessor(Compiler), copied
 * VERBATIM from
 *   test/com/google/javascript/jscomp/lint/CheckProvidesSortedTest.java lines 40-49
 * (closure-compiler commit bb8c8e7), i.e. lines 39-49 minus the @Override on line 39 (the host
 * has no superclass declaring getProcessor). The returned anonymous CompilerPass is the test's
 * CheckProvidesSortedTest$1. The holder lives in package com.google.javascript.jscomp (the test is
 * in ...jscomp.lint) because the DSL resolves helper holders there; CheckProvidesSorted, its Mode
 * enum and constructor, and NodeTraversal.traverse are all public. The descriptor calls
 * getProcessor on a fresh host, so the processor is exactly the pass the test builds.
 * DSL name: CheckProvidesSortedTest_Helpers.GetProcessorHost
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.lint.CheckProvidesSorted;
import com.google.javascript.rhino.Node;

final class CheckProvidesSortedTest_Helpers {
  private static final class GetProcessorHost {
  protected CompilerPass getProcessor(Compiler compiler) {
    CheckProvidesSorted callback =
        new CheckProvidesSorted(CheckProvidesSorted.Mode.COLLECT_AND_REPORT);
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        NodeTraversal.traverse(compiler, root, callback);
      }
    };
  }
  }
}
