/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its no-arg
 * constructor are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/CheckRegExpTest.java (closure-compiler commit bb8c8e7):
 *   last, reportErrors, assumeAllGlobalRegexpUsagesVisible: lines 31-33 (the test's fields)
 *   getProcessor: lines 55-59 (the @Override annotation on line 55 is dropped, because the holder
 *                 does not extend CompilerTestCase)
 * getProcessor stores the pass it returns in `last`; the test's own follow-up assertions read
 * last.isGlobalRegExpPropertiesUsed(). The recorder captures that state as testFieldsAfter.last.
 * DSL: processor {"call": {"once": "test", "value": {"withFields": {"helper":
 *        "CheckRegExpTest_Helpers"}, "fields": {...}}}, "method": "getProcessor",
 *        "args": [{"compiler": true}]} (inside a mutationPoint on CheckRegExp);
 *      testFieldsAfter {"once": "test", ...}, which checks this holder's `last` against the record.
 */
package com.google.javascript.jscomp;

import org.jspecify.annotations.Nullable;

final class CheckRegExpTest_Helpers {
  private @Nullable CheckRegExp last = null;
  private boolean reportErrors;
  private boolean assumeAllGlobalRegexpUsagesVisible;

  CheckRegExpTest_Helpers() {}

  protected CompilerPass getProcessor(Compiler compiler) {
    last = new CheckRegExp(compiler, assumeAllGlobalRegexpUsagesVisible, reportErrors);
    return last;
  }
}
