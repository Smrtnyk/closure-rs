/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"), copied from
 *   test/com/google/javascript/jscomp/Es6RewriteRestParametersTest.java (closure-compiler commit
 *   bb8c8e7).
 *
 * - GetProcessorPass: the lambda CompilerPass `(externs, root) -> {...}` returned by getProcessor
 *   (lines 38-44, runtime name Es6RewriteRestParametersTest$$Lambda). The class declaration, the
 *   field `compiler` (the captured getProcessor parameter) and the constructor are generated; the
 *   body of process() is the lambda body copied VERBATIM from lines 41-42. A helper is needed
 *   instead of the DSL "sequence" because the lambda constructs each pass inside process(): the
 *   Es6RewriteRestParameters constructor calls compiler.getTranspilationNamespace(), which builds
 *   and caches a GlobalNamespace that must see the runtime libraries injected by
 *   InjectTranspilationRuntimeLibraries.process just before it (Compiler.java lines 2032-2048).
 *   DSL name: Es6RewriteRestParametersTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class Es6RewriteRestParametersTest_Helpers {
  private Es6RewriteRestParametersTest_Helpers() {}

  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      new InjectTranspilationRuntimeLibraries(compiler).process(externs, root);
      new Es6RewriteRestParameters(compiler).process(externs, root);
    }
  }
}
