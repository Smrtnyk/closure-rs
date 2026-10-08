/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM
 * (re-indented only) from test/com/google/javascript/jscomp/Es6RewriteDestructuringTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder field destructuringRewriteMode, lines 40-41 (restored from testFields by the
 *     DSL "outer" list);
 *   - getProcessor, lines 77-91 (makePassFactory is the inherited CompilerTestCase helper).
 * The test's getOptions override (lines 70-75) and tearDown (lines 63-68) affect only the
 * options and field values, which come from the record.
 * DSL name: Es6RewriteDestructuringTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.Es6RewriteDestructuring.ObjectDestructuringRewriteMode;

final class Es6RewriteDestructuringTest_Helpers {
  private ObjectDestructuringRewriteMode destructuringRewriteMode =
      ObjectDestructuringRewriteMode.REWRITE_ALL_OBJECT_PATTERNS;

  private final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory(
              "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
      optimizer.addOneTimePass(
          makePassFactory(
              "es6RewriteDestructuring",
              (c) ->
                  new Es6RewriteDestructuring.Builder(c)
                      .setDestructuringRewriteMode(destructuringRewriteMode)
                      .build()));
      return optimizer;
    }
  }
}
