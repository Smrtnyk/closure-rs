/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `IdTestMap implements RenamingMap` are generated: the original is the
 * anonymous class `new RenamingMap() {...}` (runtime name ReplaceIdGeneratorsTest$1) assigned to
 * the local `idTestMap` in getProcessor. The class body (field `map` and method `get`) is copied
 * VERBATIM from
 *   test/com/google/javascript/jscomp/ReplaceIdGeneratorsTest.java lines 44-53 (anonymous class at
 *   lines 43-54)
 * (closure-compiler commit bb8c8e7). DSL name: ReplaceIdGeneratorsTest_Helpers.IdTestMap
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableMap;

final class ReplaceIdGeneratorsTest_Helpers {
  private ReplaceIdGeneratorsTest_Helpers() {}

  /** The anonymous RenamingMap {@code idTestMap} built in getProcessor. */
  private static final class IdTestMap implements RenamingMap {
    private IdTestMap() {}

          private final ImmutableMap<String, String> map =
              ImmutableMap.of(
                  "foo", ":foo:",
                  "bar", ":bar:");

          @Override
          public String get(String value) {
            String replacement = map.get(value);
            return replacement != null ? replacement : "unknown:" + value;
          }
  }
}
