/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper", "postconditions"). The holder class
 * declaration `ExternExportsPassTest_Helpers extends CompilerTestCase` is generated: it stands in
 * for the test instance, so that the postcondition lambdas and the nested test(...) call of
 * useExportsAsExternsWithClass bind exactly as in the test. Copied VERBATIM from
 * test/com/google/javascript/jscomp/ExternExportsPassTest.java (closure-compiler commit bb8c8e7):
 *   - setUp, lines 32-37;
 *   - getOptions, lines 39-46;
 *   - getProcessor, lines 48-51;
 *   - compileAndCheck (both overloads), lines 1716-1738;
 *   - compileAndExportExterns (all three overloads, with their javadoc), lines 1758-1805.
 * Generated factory methods (their signatures are generated; each returned lambda expression is
 * copied verbatim) rebuild the recorded lambdas from their captures
 * (record expected.postconditionValues, FORMAT.md):
 *   - postcondition(consumer): the Postcondition lambda of compileAndExportExterns, lines 1798-1803
 *     (capture arg$1 = consumer, possibly null);
 *   - compileAndCheckConsumer(expected): the Consumer lambda of compileAndCheck, lines 1724-1737
 *     (capture arg$1 = expected);
 *   - useExportsAsExternsWithClassConsumer(clientSource): the Consumer lambda of the test method
 *     useExportsAsExternsWithClass, lines 1378-1379 (captures arg$1 = the test instance, which is
 *     this holder, and arg$2 = clientSource).
 * DSL name: ExternExportsPassTest_Helpers (the holder itself).
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertThat;

import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;
import org.junit.Before;

final class ExternExportsPassTest_Helpers extends CompilerTestCase {

  // ---- verbatim, lines 32-51 ----

  @Override
  @Before
  public void setUp() throws Exception {
    super.setUp();
    enableTypeCheck();
  }

  @Override
  public CompilerOptions getOptions() {
    CompilerOptions options = super.getOptions();
    options.setExternExportsPath("exports.js");
    // Check types so we can make sure our exported externs have type information.
    options.setCheckSymbols(true);
    return options;
  }

  @Override
  protected CompilerPass getProcessor(Compiler compiler) {
    return new ExternExportsPass(compiler);
  }

  // ---- generated factories; the returned lambda expressions are verbatim ----

  /** Lines 1798-1803 (the Postcondition passed to test(...) by compileAndExportExterns). */
  Postcondition postcondition(final @Nullable Consumer<String> consumer) {
    return (Postcondition)
        compiler -> {
          if (consumer != null) {
            consumer.accept(compiler.getResult().externExport);
          }
        };
  }

  /** Lines 1724-1737 (the Consumer passed to compileAndExportExterns by compileAndCheck). */
  Consumer<String> compileAndCheckConsumer(final String expected) {
    return generatedExterns -> {
      String fileoverview =
          """
          /**
           * @fileoverview Generated externs.
           * @externs
           */
          """;
      // NOTE(sdh): The type checker just produces {?}.
      // For now we will not worry about this distinction and just normalize it.
      generatedExterns = generatedExterns.replace("?=", "?");

      assertThat(generatedExterns).isEqualTo(fileoverview + expected);
    };
  }

  /** Lines 1378-1379 (the Consumer of the test method useExportsAsExternsWithClass). */
  Consumer<String> useExportsAsExternsWithClassConsumer(String clientSource) {
    return generatedExterns ->
        compileAndExportExterns(clientSource, MINIMAL_EXTERNS + generatedExterns);
  }

  // ---- verbatim, lines 1716-1738 and 1758-1805 ----

  private void compileAndCheck(String js, final String expected) {
    compileAndCheck(MINIMAL_EXTERNS, js, expected);
  }

  private void compileAndCheck(String externs, String js, final String expected) {
    compileAndExportExterns(
        js,
        externs,
        generatedExterns -> {
          String fileoverview =
              """
              /**
               * @fileoverview Generated externs.
               * @externs
               */
              """;
          // NOTE(sdh): The type checker just produces {?}.
          // For now we will not worry about this distinction and just normalize it.
          generatedExterns = generatedExterns.replace("?=", "?");

          assertThat(generatedExterns).isEqualTo(fileoverview + expected);
        });
  }

  /**
   * Compiles the passed in JavaScript and returns the new externs exported by the this pass.
   *
   * @param js the source to be compiled
   */
  private void compileAndExportExterns(String js) {
    compileAndExportExterns(js, MINIMAL_EXTERNS);
  }

  /**
   * Compiles the passed in JavaScript with the passed in externs and returns the new externs
   * exported by the this pass.
   *
   * @param js the source to be compiled
   * @param externs the externs the {@code js} source needs
   */
  private void compileAndExportExterns(String js, String externs) {
    compileAndExportExterns(js, externs, null);
  }

  /**
   * Compiles the passed in JavaScript with the passed in externs and returns the new externs
   * exported by the this pass.
   *
   * @param js the source to be compiled
   * @param externs the externs the {@code js} source needs
   * @param consumer consumer for the externs generated from {@code js}
   */
  private void compileAndExportExterns(
      String js, String externs, final @Nullable Consumer<String> consumer) {
    js =
        """
        /** @const */ var goog = {};
        goog.exportSymbol = function(a, b) {};
        goog.exportProperty = function(a, b, c) {};
        """
            + js;

    test(
        externs(externs),
        srcs(js),
        (Postcondition)
            compiler -> {
              if (consumer != null) {
                consumer.accept(compiler.getResult().externExport);
              }
            });
  }
}
