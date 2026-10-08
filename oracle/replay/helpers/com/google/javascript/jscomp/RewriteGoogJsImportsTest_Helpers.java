/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class wrapper (its name, the captured `compiler` field and the constructor) are generated; the
 * body of process() is the lambda returned by getProcessor, copied VERBATIM from
 *   test/com/google/javascript/jscomp/RewriteGoogJsImportsTest.java lines 61-70
 * (closure-compiler commit bb8c8e7). The lambda captures getProcessor's `compiler` parameter,
 * which the wrapper receives as its constructor argument. The passes must be constructed inside
 * process(), after the earlier passes ran: ModuleMapCreator reads compiler.getModuleMetadataMap()
 * and RewriteGoogJsImports reads compiler.getModuleMap() at construction time.
 * DSL name: RewriteGoogJsImportsTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.RewriteGoogJsImports.Mode;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;
import com.google.javascript.jscomp.modules.ModuleMapCreator;
import com.google.javascript.rhino.Node;

final class RewriteGoogJsImportsTest_Helpers {
  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorPass(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      GatherModuleMetadata gmm =
          new GatherModuleMetadata(
              compiler, /* processCommonJsModules= */ false, ResolutionMode.BROWSER);
      gmm.process(externs, root);
      ModuleMapCreator mmc = new ModuleMapCreator(compiler, compiler.getModuleMetadataMap());
      mmc.process(externs, root);
      new RewriteGoogJsImports(compiler, Mode.LINT_AND_REWRITE, compiler.getModuleMap())
          .process(externs, root);
    }
  }
}
