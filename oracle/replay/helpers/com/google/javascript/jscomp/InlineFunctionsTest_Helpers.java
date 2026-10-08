/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class declaration is
 * generated; its contents are copied VERBATIM from
 *   test/com/google/javascript/jscomp/InlineFunctionsTest.java (closure-compiler commit bb8c8e7):
 *   - the test-instance field uniqueIdSupplier, line 46;
 *   - the nested class UniqueIdSupplier, lines 76-83.
 * The holder stands in for the test instance as the owner of `uniqueIdSupplier`: the descriptor
 * builds one holder per record (DSL "once"), restores the field from the record's testFields
 * (classMap maps InlineFunctionsTest$UniqueIdSupplier to this file's UniqueIdSupplier), hands the
 * same supplier to every InlineFunctions repetition, and names the holder in testFieldsAfter so
 * replay compares the supplier's post-call state (nextId) with the record.
 * DSL name: InlineFunctionsTest_Helpers (the holder itself).
 */
package com.google.javascript.jscomp;

import com.google.common.base.Supplier;

final class InlineFunctionsTest_Helpers {
  private UniqueIdSupplier uniqueIdSupplier = new UniqueIdSupplier();

  private static class UniqueIdSupplier implements Supplier<String> {
    private int nextId = 0;

    @Override
    public String get() {
      return "v" + nextId++; // prefix with "v" to distinguish from Compiler's unique id supplier.
    }
  }
}
