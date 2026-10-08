/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the inner
 * class CreateRuntimeJsLibManager are generated. Copied VERBATIM from
 * test/com/google/javascript/jscomp/RewritePolyfillsTest.java (closure-compiler commit bb8c8e7):
 *   - the holder fields injectableLibraries and injectBeforePass, lines 45-46 (declared with the
 *     test's own names, types and initializers; restored from testFields by the DSL "outer" list);
 *   - createRuntimeJsLibManager, lines 91-103 (its resource-provider lambda reads
 *     injectableLibraries at injection time, so it must see the holder field, as it sees the test
 *     field).
 * The generated inner class only exposes that private method to the descriptor, which builds the
 * rest of getProcessor (lines 72-81) itself:
 *   new RewritePolyfills(compiler, createRuntimeJsLibManager(compiler),
 *       Polyfills.fromTable(Joiner.on("\n").join(polyfillTable)), injectPolyfills,
 *       isolatePolyfills, injectPolyfillsNewerThan)
 * DSL name: RewritePolyfillsTest_Helpers.CreateRuntimeJsLibManager (method "create").
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.js.RuntimeJsLibManager;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;

final class RewritePolyfillsTest_Helpers {
  // ---- verbatim from RewritePolyfillsTest.java lines 45-46 ----
  private final Map<String, String> injectableLibraries = new LinkedHashMap<>();
  private final Set<String> injectBeforePass = new LinkedHashSet<>();

  /** Generated: exposes the verbatim createRuntimeJsLibManager to the descriptor. */
  private final class CreateRuntimeJsLibManager {
    RuntimeJsLibManager create(Compiler compiler) {
      return createRuntimeJsLibManager(compiler);
    }
  }

  // ---- verbatim from RewritePolyfillsTest.java lines 91-103 ----
  private RuntimeJsLibManager createRuntimeJsLibManager(Compiler compiler) {
    RuntimeJsLibManager runtimeLibs =
        RuntimeJsLibManager.create(
            RuntimeJsLibManager.RuntimeLibraryMode.INJECT,
            // stub out the resource parsing
            (resource, path) -> compiler.parseTestCode(injectableLibraries.get(resource)),
            compiler.getChangeTracker(),
            () -> compiler.getNodeForCodeInsertion(null));
    for (String toInject : injectBeforePass) {
      runtimeLibs.ensureLibraryInjected(toInject, /* force= */ false);
    }
    return runtimeLibs;
  }
}
