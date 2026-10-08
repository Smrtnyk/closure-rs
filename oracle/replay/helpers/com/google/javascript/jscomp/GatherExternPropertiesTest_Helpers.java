/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated lines are the holder and
 * class declarations, the constructors and the field that stands for the lambda capture; the
 * verify body, the field mode and the getProcessor body are copied VERBATIM (closure-compiler
 * commit bb8c8e7) from:
 *   test/com/google/javascript/jscomp/GatherExternPropertiesTest.java
 *     ExpectExterns.verify: body of the Postcondition lambda returned by expectExterns(final
 *       String... properties), lines 342-345 (captures: properties, recorded as
 *       expected.postconditionValues.0.captures.arg$1)
 *   field mode: line 57
 *   getProcessor body: lines 64-67 (the @Override on line 63 is dropped: the holder has no
 *     superclass); the holder stands in for the test instance, so getProcessor's write-back of the
 *     defaulted mode into the field is observable by the descriptor's testFieldsAfter check
 * DSL name: GatherExternPropertiesTest_Helpers.ExpectExterns (postcondition) and the holder
 *   GatherExternPropertiesTest_Helpers itself (getProcessor, testFieldsAfter.mode)
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertThat;

import com.google.javascript.jscomp.CompilerTestCase.Postcondition;

final class GatherExternPropertiesTest_Helpers {
  private GatherExternPropertiesTest_Helpers() {}

  private GatherExternProperties.Mode mode;

  protected CompilerPass getProcessor(Compiler compiler) {
    mode = this.mode == null ? GatherExternProperties.Mode.OPTIMIZE : mode;
    return new GatherExternProperties(compiler, mode);
  }

  // Generated: the postcondition lambda of expectExterns; the constructor parameter is its
  // captured `properties`.
  static final class ExpectExterns implements Postcondition {
    private final String[] properties;

    ExpectExterns(String[] properties) {
      this.properties = properties;
    }

    @Override
    public void verify(Compiler compiler) {
        assertThat(compiler.getExternProperties()).containsExactlyElementsIn(properties);
    }
  }
}
