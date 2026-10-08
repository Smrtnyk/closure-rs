/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration line are generated. Copied VERBATIM (re-indented by two spaces) from
 *   test/com/google/javascript/jscomp/ProcessDefinesTest.java (closure-compiler commit bb8c8e7):
 *   - the test-instance fields, lines 41-47;
 *   - getProcessor(Compiler), lines 62-68 (including @Override);
 *   - the inner class ProcessDefinesWithInjectedNamespace, lines 1159-1179.
 * The nested class extends CompilerTestCase (as the test does) only so that the verbatim @Override
 * compiles; no CompilerTestCase state is used. The descriptor builds a fresh GetProcessorHost,
 * restores the fields getProcessor and the inner class read (overrides, mode,
 * recognizeClosureDefines, enableJ2clPasses) with "withFields", then calls getProcessor.
 * `namespace` is written by ProcessDefinesWithInjectedNamespace.process before it is read, so it
 * is not restored. enableDefineWithoutGoogDefineCheck is read only by getOptions (its effect is in
 * the record's options).
 * DSL name: ProcessDefinesTest_Helpers.GetProcessorHost
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;
import java.util.HashMap;
import java.util.Map;

final class ProcessDefinesTest_Helpers {
  private static final class GetProcessorHost extends CompilerTestCase {

    private final Map<String, Node> overrides = new HashMap<>();
    private GlobalNamespace namespace;
    private ProcessDefines.Mode mode;
    private boolean recognizeClosureDefines = true;

    private boolean enableJ2clPasses = false;
    private boolean enableDefineWithoutGoogDefineCheck = false;

    @Override
    protected CompilerPass getProcessor(Compiler compiler) {
      if (enableJ2clPasses) {
        J2clSourceFileChecker.markToRunJ2clPasses(compiler);
      }
      return new ProcessDefinesWithInjectedNamespace(compiler);
    }

    private class ProcessDefinesWithInjectedNamespace implements CompilerPass {
      private final Compiler compiler;

      ProcessDefinesWithInjectedNamespace(Compiler compiler) {
        this.compiler = compiler;
      }

      @Override
      public void process(Node externs, Node js) {
        namespace = new GlobalNamespace(compiler, externs, js);
        new ProcessDefines.Builder(compiler)
            .putReplacements(overrides)
            .setMode(mode)
            .injectNamespace(() -> namespace)
            .setRecognizeClosureDefines(recognizeClosureDefines)
            .setEnableZonesDefineName(compiler.getOptions().getEnableZonesDefineName())
            .setZoneInputPattern(compiler.getOptions().getZoneInputPattern())
            .build()
            .process(externs, js);
      }
    }
  }
}
