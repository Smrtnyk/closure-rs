/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration `GetProcessor extends CompilerTestCase` are generated; the getProcessor
 * method is copied VERBATIM (re-indented only) from
 *   test/com/google/javascript/jscomp/Es6TranspilationIntegrationTest.java lines 99-147
 * (closure-compiler commit 48f4107ca). getProcessor reads no test-instance field, so the nested
 * class is static. The descriptor calls getProcessor(compiler) on a fresh instance.
 * DSL name: Es6TranspilationIntegrationTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.serialization.ConvertTypesToColors;
import com.google.javascript.jscomp.serialization.SerializationOptions;

final class Es6TranspilationIntegrationTest_Helpers {
  private Es6TranspilationIntegrationTest_Helpers() {}

  private static final class GetProcessor extends CompilerTestCase {
    @Override
    protected CompilerPass getProcessor(final Compiler compiler) {
      PhaseOptimizer optimizer = new PhaseOptimizer(compiler, null);

      CompilerOptions compilerOptions = compiler.getOptions();
      PassListBuilder passes = new PassListBuilder(compilerOptions);

      passes.maybeAdd(
          PassFactory.builder()
              .setName("es6InjectRuntimeLibraries")
              .setInternalFactory(InjectTranspilationRuntimeLibraries::new)
              .build());

      passes.maybeAdd(
          PassFactory.builder()
              .setName("rewritePolyfills")
              .setInternalFactory(
                  c ->
                      new RewritePolyfills(
                          c, /* injectPolyfills= */ true, /* isolatePolyfills= */ false, null))
              .build());

      passes.maybeAdd(
          PassFactory.builder()
              .setName("convertTypesToColors")
              .setInternalFactory(
                  (c) ->
                      new ConvertTypesToColors(
                          c, SerializationOptions.builder().setIncludeDebugInfo(true).build()))
              .build());

      passes.maybeAdd(
          PassFactory.builder()
              .setName(PassNames.NORMALIZE)
              .setInternalFactory((abstractCompiler) -> Normalize.builder(abstractCompiler).build())
              .build());
      TranspilationPasses.addTranspilationPasses(passes, compilerOptions);
      // Since we're testing the transpile-only case, we need to put back the original variable names
      // where possible once transpilation is complete. This matches the behavior in
      // DefaultPassConfig. See comments there for further explanation.
      passes.maybeAdd(
          PassFactory.builder()
              .setName("invertContextualRenaming")
              .setInternalFactory(MakeDeclaredNamesUnique::getContextualRenameInverter)
              .build());
      optimizer.consume(passes.build());

      return optimizer;
    }
  }
}
