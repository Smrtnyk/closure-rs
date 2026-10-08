/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"), copied from
 *   test/com/google/javascript/jscomp/ReplaceStringsTest.java (closure-compiler commit bb8c8e7).
 *
 * - Renamer: the private static nested class at lines 104-114, copied VERBATIM.
 *   DSL name: ReplaceStringsTest_Helpers.Renamer (used only by GetProcessorPass).
 * - GetProcessorPass: the anonymous `new CompilerPass() {...}` returned by getProcessor (lines
 *   120-142, runtime name ReplaceStringsTest$1). The class declaration, the four fields and the
 *   constructor are generated: the anonymous class captures the parameter `compiler` and reads the
 *   test-instance fields `rename`, `runDisambiguateProperties` and `pass` (getProcessor assigns
 *   `pass = new ReplaceStrings(compiler, "`", functionsToInspect)` at line 118, just before it
 *   returns the anonymous class; the descriptor builds that ReplaceStrings and passes it in). No
 *   test code changes `rename` or `runDisambiguateProperties` between getProcessor and process, so
 *   holding their values in fields is equivalent. The body of process() is copied VERBATIM from
 *   lines 121-141. DSL name: ReplaceStringsTest_Helpers.GetProcessorPass
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableSet;
import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.CompilerOptions.PropertyCollapseLevel;
import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;
import com.google.javascript.jscomp.disambiguate.DisambiguateProperties;
import com.google.javascript.rhino.Node;

final class ReplaceStringsTest_Helpers {
  private ReplaceStringsTest_Helpers() {}

  private static class Renamer extends AbstractPostOrderCallback {
    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      if (n.isName() || n.isGetProp()) {
        String originalName = n.getString();
        n.setOriginalName(originalName);
        n.setString("renamed_" + originalName);
        t.reportCodeChange();
      }
    }
  }

  /** The anonymous CompilerPass returned by getProcessor (ReplaceStringsTest$1). */
  private static final class GetProcessorPass implements CompilerPass {
    private final Compiler compiler;
    private final ReplaceStrings pass;
    private final boolean rename;
    private final boolean runDisambiguateProperties;

    private GetProcessorPass(
        Compiler compiler,
        ReplaceStrings pass,
        boolean rename,
        boolean runDisambiguateProperties) {
      this.compiler = compiler;
      this.pass = pass;
      this.rename = rename;
      this.runDisambiguateProperties = runDisambiguateProperties;
    }

      @Override
      public void process(Node externs, Node js) {
        if (rename) {
          NodeTraversal.traverse(compiler, js, new Renamer());
        }
        new Es6NormalizeClasses(compiler).process(externs, js);
        InlineAndCollapseProperties.builder(compiler)
            .setPropertyCollapseLevel(PropertyCollapseLevel.ALL)
            .setChunkOutputType(ChunkOutputType.GLOBAL_NAMESPACE)
            .setHaveModulesBeenRewritten(false)
            .setModuleResolutionMode(ResolutionMode.BROWSER)
            .build()
            .process(externs, js);
        if (runDisambiguateProperties) {
          SourceInformationAnnotator sia = SourceInformationAnnotator.create();
          NodeTraversal.traverse(compiler, js, sia);

          new DisambiguateProperties(compiler, ImmutableSet.of("foobar")).process(externs, js);
        }
        pass.process(externs, js);
      }
  }
}
