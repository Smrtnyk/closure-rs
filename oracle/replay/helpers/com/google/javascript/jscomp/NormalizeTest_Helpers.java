/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration line are generated; the nested class body is the body of the anonymous
 * CompilerTestCase subclass in testRenamingConstantProperties, copied VERBATIM from
 *   test/com/google/javascript/jscomp/NormalizeTest.java lines 1637-1654
 * (closure-compiler commit bb8c8e7). The records of that anonymous tester (instanceClass
 * NormalizeTest$1) replay its getProcessor through a DSL "call" on this class.
 * DSL name: NormalizeTest_Helpers.RenamingConstantPropertiesTester
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.CompilerOptions.PropertyCollapseLevel;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;

final class NormalizeTest_Helpers {
  private static final class RenamingConstantPropertiesTester extends CompilerTestCase {

    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
      optimizer.addOneTimePass(
          makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new));
      optimizer.addOneTimePass(
          makePassFactory(
              "inlineAndCollapseProperties",
              (comp) ->
                  InlineAndCollapseProperties.builder(compiler)
                      .setPropertyCollapseLevel(PropertyCollapseLevel.ALL)
                      .setChunkOutputType(ChunkOutputType.GLOBAL_NAMESPACE)
                      .setHaveModulesBeenRewritten(false)
                      .setModuleResolutionMode(ResolutionMode.BROWSER)
                      .build()));
      return optimizer;
    }
  }
}
