/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper", invoked through the DSL "static"
 * construct). The holder class is generated; the body of getProcessor is copied VERBATIM from
 * test/com/google/javascript/jscomp/InlineAndCollapsePropertiesTest.java (closure-compiler commit
 * bb8c8e7), lines 53-68. Only the modifiers changed: "@Override protected" became "static",
 * because the holder is not a CompilerTestCase. makePassFactory is CompilerTestCase's
 * package-private static helper (compiler_tests_lib), imported statically.
 *
 * DSL: {"static": "com.google.javascript.jscomp.InlineAndCollapsePropertiesTest_Helpers",
 *       "method": "getProcessor", "args": [{"compiler": true}]}
 */
package com.google.javascript.jscomp;

import static com.google.javascript.jscomp.CompilerTestCase.makePassFactory;

import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.CompilerOptions.PropertyCollapseLevel;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;

final class InlineAndCollapsePropertiesTest_Helpers {
  private InlineAndCollapsePropertiesTest_Helpers() {}

  static CompilerPass getProcessor(final Compiler compiler) {
    PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
    optimizer.addOneTimePass(makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new));
    optimizer.addOneTimePass(
        makePassFactory(
            "inlineAndCollapseProperties",
            (comp) ->
                InlineAndCollapseProperties.builder(comp)
                    .setPropertyCollapseLevel(PropertyCollapseLevel.ALL)
                    .setChunkOutputType(ChunkOutputType.GLOBAL_NAMESPACE)
                    .setHaveModulesBeenRewritten(false)
                    .setModuleResolutionMode(ResolutionMode.BROWSER)
                    .build()));
    return optimizer;
  }
}
