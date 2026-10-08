/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM
 * (re-indented only) from test/com/google/javascript/jscomp/Es6RewriteClassTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder field es6SubclassTranspilation, line 50 (restored from testFields by the DSL
 *     "outer" list);
 *   - getProcessor, lines 69-81 (makePassFactory is the inherited CompilerTestCase helper).
 * DSL name: Es6RewriteClassTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.CompilerOptions.Es6SubclassTranspilation;

final class Es6RewriteClassTest_Helpers {
  private Es6SubclassTranspilation es6SubclassTranspilation;

  private final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory(
              "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
      optimizer.addOneTimePass(makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new));
      optimizer.addOneTimePass(makePassFactory("es6ConvertSuper", Es6ConvertSuper::new));
      optimizer.addOneTimePass(
          makePassFactory(
              "es6RewriteClass", (c) -> new Es6RewriteClass(compiler, es6SubclassTranspilation)));
      return optimizer;
    }
  }
}
