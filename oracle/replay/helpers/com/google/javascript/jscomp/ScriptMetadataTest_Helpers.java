/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, its no-arg
 * constructor and the import of com.google.javascript.refactoring.ScriptMetadata (the original
 * test lives in that package; the holder must live in com.google.javascript.jscomp) are
 * generated. Copied VERBATIM from
 * test/com/google/javascript/refactoring/ScriptMetadataTest.java (closure-compiler commit
 * bb8c8e7):
 *   lastMetadata: line 31 (the field the lambda writes)
 *   getProcessor: lines 33-37 (returns a lambda CompilerPass; the @Override annotation on line 33
 *                 is dropped, because the holder does not extend CompilerTestCase)
 * DSL: processor {"call": {"once": "test", "value": {"helper": "ScriptMetadataTest_Helpers"}},
 *                  "method": "getProcessor", "args": [{"compiler": true}]} (inside a mutationPoint);
 *      testFieldsAfter {"once": "test", "value": {"helper": "ScriptMetadataTest_Helpers"}}, which
 *      checks this holder's lastMetadata against the record.
 */
package com.google.javascript.jscomp;

import com.google.javascript.refactoring.ScriptMetadata;

final class ScriptMetadataTest_Helpers {

  private ScriptMetadata lastMetadata;

  ScriptMetadataTest_Helpers() {}

  public CompilerPass getProcessor(Compiler compiler) {
    return (externs, root) ->
        this.lastMetadata = ScriptMetadata.create(root.getOnlyChild(), compiler);
  }
}
