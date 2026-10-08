/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated. Copied VERBATIM
 * (re-indented only) from test/com/google/javascript/jscomp/InstrumentAsyncContextTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder field instrumentAwait, line 49 (restored from testFields by the DSL "outer"
 *     list; the lambda reads it when the pass factory runs);
 *   - getProcessor, lines 102-116 (a PhaseOptimizer of two makePassFactory lambdas;
 *     makePassFactory is the inherited CompilerTestCase helper).
 * The test's setUp (lines 93-100) and getOptions (lines 118-123) affect only harness fields,
 * options and instrumentAwait, which come from the record.
 * DSL name: InstrumentAsyncContextTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

final class InstrumentAsyncContextTest_Helpers {
  private boolean instrumentAwait = true;

  private final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory(
              "injectTranspilationRuntimeLibraries",
              (c) ->
                  new InjectTranspilationRuntimeLibraries(
                      compiler, /* shouldInstrumentAsyncContext= */ true)));
      optimizer.addOneTimePass(
          makePassFactory(
              "instrumentAsyncContext",
              (c) -> new InstrumentAsyncContext(compiler, instrumentAwait)));
      return optimizer;
    }
  }
}
