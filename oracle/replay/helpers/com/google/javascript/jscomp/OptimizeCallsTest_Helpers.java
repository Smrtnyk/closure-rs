/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class's declaration and process(...) signature are generated: the original is the
 * OptimizeCalls.CallGraphCompilerPass lambda `(externs, root, references) -> this.references =
 * references` that getProcessor passes to OptimizeCalls.Builder.addPass (runtime name
 * OptimizeCallsTest$$Lambda). Copied VERBATIM from
 *   test/com/google/javascript/jscomp/OptimizeCallsTest.java
 *     field `references` (with its comment): lines 36-37
 *     lambda body `this.references = references`: line 56
 * (closure-compiler commit bb8c8e7). In the test, `this` is the test instance and `references`
 * is a test field the lambda only writes (the test's own follow-up assertions read it); here the
 * field lives on the nested class, so `this.references = references` is the same statement.
 * DSL name: OptimizeCallsTest_Helpers.ReferencesCapturingPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class OptimizeCallsTest_Helpers {
  private OptimizeCallsTest_Helpers() {}

  /** The CallGraphCompilerPass lambda that getProcessor adds to OptimizeCalls. */
  private static final class ReferencesCapturingPass implements OptimizeCalls.CallGraphCompilerPass {
  // Will be assigned with the references collected from the most recent pass.
  private OptimizeCalls.ReferenceMap references;

    @Override
    public void process(Node externs, Node root, OptimizeCalls.ReferenceMap references) {
      this.references = references;
    }
  }
}
