/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder; its field
 * propertyCollapseLevel mirrors CollapsePropertiesTest's field of the same name and type and is
 * restored from the record's testFields by the DSL "outer" list. The method getProcessor below is
 * copied VERBATIM (without its @Override line, since the holder has no superclass) from
 * test/com/google/javascript/jscomp/CollapsePropertiesTest.java (closure-compiler commit bb8c8e7):
 *   getProcessor (PhaseOptimizer with two makePassFactory lambdas): lines 54-68
 * makePassFactory is CompilerTestCase's package-private static helper (CompilerTestCase.java
 * lines 2132-2134, harness base class), statically imported so the body stays verbatim.
 * The generated inner class Processor only delegates to the pass that getProcessor returns, so the
 * lambdas keep their outer-instance read of propertyCollapseLevel (at process time, as in the test).
 * DSL name: CollapsePropertiesTestHelpers.Processor
 */
package com.google.javascript.jscomp;

import static com.google.javascript.jscomp.CompilerTestCase.makePassFactory;

import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.CompilerOptions.PropertyCollapseLevel;
import com.google.javascript.jscomp.deps.ModuleLoader.ResolutionMode;
import com.google.javascript.rhino.Node;

final class CollapsePropertiesTestHelpers {
  private PropertyCollapseLevel propertyCollapseLevel = PropertyCollapseLevel.ALL;

  /** Generated: the CompilerPass returned by the verbatim getProcessor, for a given compiler. */
  private class Processor implements CompilerPass {
    private final CompilerPass delegate;

    private Processor(Compiler compiler) {
      this.delegate = getProcessor(compiler);
    }

    @Override
    public void process(Node externs, Node root) {
      delegate.process(externs, root);
    }
  }

  // ---- verbatim from CollapsePropertiesTest.java lines 54-68 ----
  protected CompilerPass getProcessor(final Compiler compiler) {
    PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);
    optimizer.addOneTimePass(makePassFactory("es6NormalizeClasses", Es6NormalizeClasses::new));
    optimizer.addOneTimePass(
        makePassFactory(
            "inlineAndCollapseProperties",
            (comp) ->
                InlineAndCollapseProperties.builder(comp)
                    .setPropertyCollapseLevel(propertyCollapseLevel)
                    .setChunkOutputType(ChunkOutputType.GLOBAL_NAMESPACE)
                    .setHaveModulesBeenRewritten(false)
                    .setModuleResolutionMode(ResolutionMode.BROWSER)
                    .build()));
    return optimizer;
  }
}
