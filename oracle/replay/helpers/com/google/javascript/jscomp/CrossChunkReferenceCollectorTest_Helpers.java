/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper", "testFieldsAfter"). The holder class
 * declaration `CrossChunkReferenceCollectorTest_Helpers` is generated: it stands in for the test
 * instance, so that the collector getProcessor builds is kept in the field `testedCollector`
 * exactly as in the test, and replay can compare that field with the record's testFieldsAfter
 * (post-call state, FORMAT.md "Post-call state"). Copied VERBATIM from
 * test/com/google/javascript/jscomp/CrossChunkReferenceCollectorTest.java (closure-compiler commit
 * bb8c8e7):
 *   - field testedCollector, line 35;
 *   - getProcessor, lines 42-47 (the @Override annotation is dropped: the holder does not extend
 *     CompilerTestCase; the descriptor calls getProcessor once per repetition, as the harness does).
 * DSL name: CrossChunkReferenceCollectorTest_Helpers (the holder itself).
 */
package com.google.javascript.jscomp;

final class CrossChunkReferenceCollectorTest_Helpers {

  // ---- verbatim, line 35 ----
  private CrossChunkReferenceCollector testedCollector;

  // ---- verbatim, lines 43-47 (line 42, @Override, dropped) ----
  protected CompilerPass getProcessor(final Compiler compiler) {
    ScopeCreator scopeCreator = new SyntacticScopeCreator(compiler);
    testedCollector = new CrossChunkReferenceCollector(compiler, scopeCreator);
    return testedCollector;
  }
}
