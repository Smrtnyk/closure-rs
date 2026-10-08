/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and its three
 * fields (mirroring RenameVarsTest's fields of the same names, restored from the record's
 * testFields by the DSL "outer" list) are generated; the two nested classes are copied VERBATIM
 * from test/com/google/javascript/jscomp/RenameVarsTest.java (closure-compiler commit bb8c8e7):
 *   ClosurePassAndRenameVars: lines 1221-1249
 *   NormalizePassWrapper:     lines 1251-1267
 * DSL names: RenameVarsTestHelpers.ClosurePassAndRenameVars, RenameVarsTestHelpers.NormalizePassWrapper
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableSet;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;
import com.google.javascript.rhino.Node;

final class RenameVarsTestHelpers {
  private String prefix;
  private VariableMap previouslyUsedMap;
  private RenameVars renameVars;

  private class ClosurePassAndRenameVars implements CompilerPass {
    private final Compiler compiler;

    private ClosurePassAndRenameVars(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      new GatherModuleMetadata(
              compiler, /* processCommonJsModules= */ false, ResolutionMode.BROWSER)
          .process(externs, root);
      ProcessClosureProvidesAndRequires closurePass =
          new ProcessClosureProvidesAndRequires(compiler, true);
      closurePass.process(externs, root);
      renameVars =
          new RenameVars(
              compiler,
              prefix,
              false,
              false,
              false,
              previouslyUsedMap,
              ImmutableSet.<Character>of(),
              closurePass.getExportedVariableNames(),
              new DefaultNameGenerator());
      renameVars.process(externs, root);
    }
  }

  private static class NormalizePassWrapper implements CompilerPass {
    private final Compiler compiler;
    private final CompilerPass wrappedPass;

    private NormalizePassWrapper(Compiler compiler, CompilerPass wrappedPass) {
      this.compiler = compiler;
      this.wrappedPass = wrappedPass;
    }

    @Override
    public void process(Node externs, Node root) {
      Normalize normalize = Normalize.createNormalizeForOptimizations(compiler);
      normalize.process(externs, root);

      wrappedPass.process(externs, root);
    }
  }
}
