/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor` are generated. Copied VERBATIM (re-indented only) from
 * test/com/google/javascript/jscomp/RewriteDynamicImportsTest.java
 * (closure-compiler commit bb8c8e7):
 *   - the holder fields dynamicImportAlias and chunkOutputType, lines 44 and 47 (restored from
 *     testFields by the DSL "outer" list; the lambda reads them at process time);
 *   - getProcessor, lines 74-99 (returns a lambda CompilerPass that runs TypeCheck, then
 *     Es6RewriteModules, then RewriteDynamicImports).
 * DSL name: RewriteDynamicImportsTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import static com.google.common.base.Preconditions.checkNotNull;

import com.google.javascript.jscomp.CompilerOptions.ChunkOutputType;
import com.google.javascript.jscomp.type.ReverseAbstractInterpreter;
import com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter;
import org.jspecify.annotations.Nullable;

final class RewriteDynamicImportsTest_Helpers {
  private @Nullable String dynamicImportAlias = "imprt_";
  private ChunkOutputType chunkOutputType = ChunkOutputType.GLOBAL_NAMESPACE;

  private final class GetProcessor {
    protected CompilerPass getProcessor(Compiler compiler) {
      return (externs, root) -> {
        // NOTE: we cannot just use enableTypeCheck(), because we need to be able
        // to retrieve globalTypedScope for use by Es6RewriteModules.
        ReverseAbstractInterpreter rai =
            new SemanticReverseAbstractInterpreter(compiler.getTypeRegistry());
        TypedScope globalTypedScope =
            checkNotNull(
                new TypeCheck(compiler, rai, compiler.getTypeRegistry())
                    .processForTesting(externs, root));
        compiler.setTypeCheckingHasRun(true);
        // We need to make sure modules are rewritten, because RewriteDynamicImports
        // expects to be able to see the module variables.
        new Es6RewriteModules(
                compiler,
                compiler.getModuleMetadataMap(),
                compiler.getModuleMap(),
                /* preprocessorSymbolTable= */ null,
                globalTypedScope)
            .process(externs, root);

        new RewriteDynamicImports(compiler, dynamicImportAlias, chunkOutputType)
            .process(externs, root);
      };
    }
  }
}
