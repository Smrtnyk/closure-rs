/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder; its two fields
 * mirror CollapsePropertiesAndModuleRewritingTest's fields of the same names and are restored from
 * the record's testFields by the DSL "outer" list. The method getProcessor below is copied
 * VERBATIM (without its @Override line, since the holder has no superclass) from
 * test/com/google/javascript/jscomp/CollapsePropertiesAndModuleRewritingTest.java
 * (closure-compiler commit bb8c8e7):
 *   getProcessor (anonymous CompilerPass): lines 41-95
 * The generated inner class Processor only delegates to the pass that getProcessor returns, so
 * the anonymous class keeps its captured `compiler` and its outer-instance field reads.
 * DSL name: CollapsePropertiesAndModuleRewritingTestHelpers.Processor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.CompilerOptions.PropertyCollapseLevel;
import com.google.javascript.jscomp.modules.ModuleMapCreator;
import com.google.javascript.rhino.Node;

final class CollapsePropertiesAndModuleRewritingTestHelpers {
  private PropertyCollapseLevel collapseLevel = PropertyCollapseLevel.ALL;
  private ChunkOutputType chunkOutputType = ChunkOutputType.ES_MODULES;

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

  // ---- verbatim from CollapsePropertiesAndModuleRewritingTest.java lines 41-95 ----
  protected CompilerPass getProcessor(final Compiler compiler) {
    return new CompilerPass() {
      final CompilerOptions options = compiler.getOptions();

      @Override
      public void process(Node externs, Node root) {
        PassListBuilder factories = new PassListBuilder(options);
        GatherModuleMetadata gatherModuleMetadata =
            new GatherModuleMetadata(compiler, false, options.getModuleResolutionMode());
        factories.maybeAdd(
            PassFactory.builder()
                .setName(PassNames.GATHER_MODULE_METADATA)
                .setRunInFixedPointLoop(true)
                .setInternalFactory((x) -> gatherModuleMetadata)
                .build());
        factories.maybeAdd(
            PassFactory.builder()
                .setName(PassNames.CREATE_MODULE_MAP)
                .setRunInFixedPointLoop(true)
                .setInternalFactory(
                    (x) -> new ModuleMapCreator(compiler, compiler.getModuleMetadataMap()))
                .build());
        TranspilationPasses.addEs6ModulePass(
            factories, new PreprocessorSymbolTable.CachedInstanceFactory());
        factories.maybeAdd(
            PassFactory.builder()
                .setName("REWRITE_DYNAMIC_IMPORT")
                .setInternalFactory(
                    (x) -> new RewriteDynamicImports(compiler, null, ChunkOutputType.ES_MODULES))
                .build());
        factories.maybeAdd(
            PassFactory.builder()
                .setName(PassNames.ES6_NORMALIZE_CLASSES)
                .setInternalFactory(Es6NormalizeClasses::new)
                .build());
        factories.maybeAdd(
            PassFactory.builder()
                .setName(PassNames.COLLAPSE_PROPERTIES)
                .setRunInFixedPointLoop(true)
                .setInternalFactory(
                    (x) ->
                        InlineAndCollapseProperties.builder(compiler)
                            .setPropertyCollapseLevel(collapseLevel)
                            .setChunkOutputType(chunkOutputType)
                            .setHaveModulesBeenRewritten(true)
                            .setModuleResolutionMode(options.getModuleResolutionMode())
                            .build())
                .build());

        for (PassFactory factory : factories.build()) {
          factory.create(compiler).process(externs, root);
        }
      }
    };
  }
}
