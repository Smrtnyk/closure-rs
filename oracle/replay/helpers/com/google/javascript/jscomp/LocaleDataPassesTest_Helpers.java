/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its no-arg
 * constructor are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/LocaleDataPassesTest.java (closure-compiler commit bb8c8e7):
 *   TestMode:     lines 30-35 (the enum, with its javadoc)
 *   testMode:     lines 37-38 (the field, with its comment and initializer)
 *   getProcessor: lines 63-87 (the switch over testMode returning one of two anonymous
 *                 CompilerPasses; the @Override annotation on line 63 is dropped, because the
 *                 holder does not extend CompilerTestCase)
 * The test's other field, `locale`, is not copied: getProcessor never reads it (the test's
 * getOptions copies it into CompilerOptions.setLocale, which the replay restores from the
 * record's options diff, and the REPLACE_PROTECTED_DATA pass reads compiler.getOptions()).
 * DSL (descriptor classMap maps LocaleDataPassesTest$TestMode to LocaleDataPassesTest_Helpers$TestMode):
 *   {"mutationPoint": "com.google.javascript.jscomp.LocaleDataPasses",
 *    "value": {"call": {"withFields": {"helper": "LocaleDataPassesTest_Helpers"},
 *                       "fields": {"testMode": {"field": "testMode"}}},
 *              "method": "getProcessor", "args": [{"compiler": true}]}}
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.LocaleDataPasses.ProtectGoogLocale;
import com.google.javascript.rhino.Node;

final class LocaleDataPassesTest_Helpers {

  /** Indicates which part of the replacement we're currently testing */
  enum TestMode {
    PROTECT_DATA,
    // Test replacement of the protected function call form with the final message values.
    REPLACE_PROTECTED_DATA
  }

  // Messages returned from fake bundle, keyed by `JsMessage.id`.
  private TestMode testMode = TestMode.PROTECT_DATA;

  LocaleDataPassesTest_Helpers() {}

  protected CompilerPass getProcessor(Compiler compiler) {
    return switch (testMode) {
      case PROTECT_DATA ->
          new CompilerPass() {
            @Override
            public void process(Node externs, Node root) {
              final ProtectGoogLocale extract = new ProtectGoogLocale(compiler);
              extract.process(externs, root);
            }
          };
      case REPLACE_PROTECTED_DATA ->
          new CompilerPass() {
            @Override
            public void process(Node externs, Node root) {
              final ProtectGoogLocale extract = new ProtectGoogLocale(compiler);
              extract.process(externs, root);
              final LocaleDataPasses.LocaleSubstitutions subs =
                  new LocaleDataPasses.LocaleSubstitutions(
                      compiler, compiler.getOptions().getLocale());
              subs.process(externs, root);
            }
          };
    };
  }
}
