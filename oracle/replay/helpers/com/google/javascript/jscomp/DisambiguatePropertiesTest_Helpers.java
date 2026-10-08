/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class is generated; the
 * nested class SilenceNoiseGuard is copied VERBATIM from
 *   test/com/google/javascript/jscomp/disambiguate/DisambiguatePropertiesTest.java lines 1302-1321
 * (closure-compiler commit bb8c8e7). Only the imports are added, because the holder lives in
 * package com.google.javascript.jscomp instead of ...jscomp.disambiguate.
 *
 * The original (DisambiguatePropertiesTest$SilenceNoiseGuard) is not a pass: the test's
 * getOptions() adds it with options.addWarningsGuard(new SilenceNoiseGuard()) (lines 83-88), so
 * every DisambiguatePropertiesTest record stores it inside options.warningsGuard. The descriptor's
 * classMap maps the recorded class name to this copy, so the recorded (field-less) guard object is
 * rebuilt on it while the options are decoded.
 *
 * classMap target: com.google.javascript.jscomp.DisambiguatePropertiesTest_Helpers$SilenceNoiseGuard
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableSet;
import com.google.javascript.jscomp.disambiguate.DisambiguateProperties;

final class DisambiguatePropertiesTest_Helpers {
  private DisambiguatePropertiesTest_Helpers() {}

  private static final class SilenceNoiseGuard extends WarningsGuard {
    private static final ImmutableSet<DiagnosticType> RELEVANT_DIAGNOSTICS =
        ImmutableSet.of(DisambiguateProperties.PROPERTY_INVALIDATION);

    @Override
    protected int getPriority() {
      return WarningsGuard.Priority.MAX.getValue();
    }

    @Override
    public CheckLevel level(JSError error) {
      if (error.description().contains("Parse")) {
        return null;
      } else if (RELEVANT_DIAGNOSTICS.contains(error.type())) {
        return null;
      } else {
        return CheckLevel.OFF;
      }
    }
  }
}
