/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class declaration
 * `ReplaceCssNamesTest_Helpers extends CompilerTestCase` is generated (it stands in for the test
 * instance, so that getProcessor's @Override and the anonymous renaming maps bind exactly as in the
 * test). Copied VERBATIM from test/com/google/javascript/jscomp/ReplaceCssNamesTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the instance fields useReplacementMap, replacementMap, replacementMapFull, renamingMap,
 *     skiplist and cssNames, lines 41-77 (the descriptor restores useReplacementMap, skiplist and
 *     cssNames from testFields with "withFields"; replacementMap and replacementMapFull are never
 *     reassigned by the test, so their initializers are the recorded values);
 *   - getProcessor, lines 89-93 (its `cssNames::add` method reference is the CssNameCollector);
 *   - getPartialMap (anonymous ReplaceCssNamesTest$1) and getFullMap (anonymous
 *     ReplaceCssNamesTest$2), lines 95-111. The descriptor calls the one whose anonymous class the
 *     record's testFields.renamingMap names, just as setUp (line 121) or the test body did.
 * DSL name: ReplaceCssNamesTest_Helpers (the holder itself).
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableMap;
import java.util.Map;
import java.util.Set;

final class ReplaceCssNamesTest_Helpers extends CompilerTestCase {
  /** Whether to pass the map of replacements as opposed to null */
  boolean useReplacementMap;

  /** Map of replacements to use during the test. */
  Map<String, String> replacementMap =
      new ImmutableMap.Builder<String, String>()
          .put("active", "a")
          .put("buttonbar", "b")
          .put("colorswatch", "c")
          .put("disabled", "d")
          .put("elephant", "e")
          .put("footer", "f")
          .put("goog", "g")
          .put("fooStylesBar", "fsr")
          .put("fooStylesBaz", "fsz")
          .put("--foo-bar", "--fb")
          .put("---foo-baz", "--fbz")
          .put("--foo-bar-baz--qux", "--fbzq")
          .buildOrThrow();

  Map<String, String> replacementMapFull =
      new ImmutableMap.Builder<String, String>()
          .put("long-prefix", "h")
          .put("suffix1", "i")
          .put("unrelated-word", "k")
          .put("unrelated", "l")
          .put("long-suffix", "m")
          .put("long-prefix-suffix1", "h-i")
          .put("--foo-bar", "--fb")
          .put("---foo-baz", "--fbz")
          .put("--foo-bar-baz--qux", "--fbzq")
          .buildOrThrow();

  CssRenamingMap renamingMap;
  Set<String> skiplist;

  Set<String> cssNames;

  @Override
  protected CompilerPass getProcessor(Compiler compiler) {
    return new ReplaceCssNames(
        compiler, useReplacementMap ? renamingMap : null, cssNames::add, skiplist);
  }

  CssRenamingMap getPartialMap() {
    return new CssRenamingMap.ByPart() {
      @Override
      public String get(String value) {
        return replacementMap.get(value);
      }
    };
  }

  CssRenamingMap getFullMap() {
    return new CssRenamingMap.ByWhole() {
      @Override
      public String get(String value) {
        return replacementMapFull.get(value);
      }
    };
  }
}
