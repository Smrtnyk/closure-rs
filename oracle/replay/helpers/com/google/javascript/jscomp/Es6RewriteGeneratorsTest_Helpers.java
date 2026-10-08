/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/Es6RewriteGeneratorsTest.java (closure-compiler commit
 * bb8c8e7):
 *   - the holder field extraRuntimeLibFields, line 64 (restored from testFields by the DSL "outer"
 *     list);
 *   - getProcessor, lines 112-130 (makePassFactory is the inherited CompilerTestCase helper).
 * The test's setUp (lines 91-103) and getOptions (lines 105-110) affect only harness fields,
 * options and extraRuntimeLibFields, which come from the record.
 * DSL name: Es6RewriteGeneratorsTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableList;

final class Es6RewriteGeneratorsTest_Helpers {
  private ImmutableList<String> extraRuntimeLibFields = ImmutableList.of();

  private final class GetProcessor extends CompilerTestCase {
  @Override
  protected CompilerPass getProcessor(final Compiler compiler) {
    PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
    optimizer.addOneTimePass(
        makePassFactory(
            "extraRuntimeLibraries",
            (AbstractCompiler c) -> {
              return (externs, js) -> {
                for (String field : extraRuntimeLibFields) {
                  c.getRuntimeJsLibManager().injectLibForField(field);
                }
              };
            }));
    optimizer.addOneTimePass(
        makePassFactory(
            "injectTranspilationRuntimeLibraries", InjectTranspilationRuntimeLibraries::new));
    optimizer.addOneTimePass(makePassFactory("es6RewriteGenerators", Es6RewriteGenerators::new));
    return optimizer;
  }
  }
}
