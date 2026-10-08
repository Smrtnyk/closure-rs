/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated;
 * the nested class below is copied VERBATIM from
 *   test/com/google/javascript/jscomp/InlineFunctionsTest.java lines 76-83
 * (closure-compiler commit bb8c8e7). DSL name: InlineFunctionsTestHelpers.UniqueIdSupplier
 */
package com.google.javascript.jscomp;

import com.google.common.base.Supplier;

final class InlineFunctionsTestHelpers {
  private static class UniqueIdSupplier implements Supplier<String> {
    private int nextId = 0;

    @Override
    public String get() {
      return "v" + nextId++; // prefix with "v" to distinguish from Compiler's unique id supplier.
    }
  }
}
