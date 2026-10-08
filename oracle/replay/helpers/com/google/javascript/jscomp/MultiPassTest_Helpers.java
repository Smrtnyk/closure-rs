/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Copied from
 * test/com/google/javascript/jscomp/MultiPassTest.java (closure-compiler commit bb8c8e7):
 *   field `passes`: line 35, VERBATIM.
 *   GetProcessor.getProcessor: getProcessor (lines 48-57), VERBATIM apart from one more
 *                     indentation level (2 spaces); the @Override (line 47) was dropped because
 *                     the method now lives in an inner helper class, not in CompilerTestCase.
 *   addNormalization ... addInjectTranspilationRuntimeLibraries: lines 430-549, VERBATIM.
 * Generated (not in the test):
 *   - the field `testName` and getName(), standing in for CompilerTestCase#getName() (which
 *     addPeephole calls); the descriptor passes the recorded harness.effective.getName.
 *   - the GetProcessor constructor, which replays the test method's setup of `passes`:
 *     `passes = new ArrayList<>();` followed by the add*() calls named in `adders`, in order.
 *     Every test method does exactly this (directly, or through
 *     setDestructuringArrowFunctionOptions, lines 416-428, whose other statements are harness
 *     setters already restored from harness.fields).
 * DSL names: {"call": {"helper": "MultiPassTest_Helpers.GetProcessor", "args": [name, adders]},
 *             "method": "getProcessor", "args": [compiler]}
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.Es6RewriteDestructuring.ObjectDestructuringRewriteMode;
import java.util.ArrayList;
import java.util.List;

final class MultiPassTest_Helpers {
  private List<PassFactory> passes;

  // Generated: the value CompilerTestCase#getName() returns for the test instance.
  private String testName;

  // Generated: stands in for CompilerTestCase#getName().
  private String getName() {
    return testName;
  }

  // Generated: the test method's `passes = new ArrayList<>(); add...();` setup, then the
  // test's getProcessor.
  private class GetProcessor {
    private GetProcessor(String testName, String[] adders) {
      MultiPassTest_Helpers.this.testName = testName;
      passes = new ArrayList<>();
      for (String adder : adders) {
        switch (adder) {
          case "addNormalization" -> addNormalization();
          case "addCollapseObjectLiterals" -> addCollapseObjectLiterals();
          case "addInlineFunctions" -> addInlineFunctions();
          case "addInlineVariables" -> addInlineVariables();
          case "addPeephole" -> addPeephole();
          case "addRemoveUnusedClassProperties" -> addRemoveUnusedClassProperties();
          case "addRemoveUnusedVars" -> addRemoveUnusedVars();
          case "addDestructuringPass" -> addDestructuringPass();
          case "addArrowFunctionPass" -> addArrowFunctionPass();
          case "addInjectTranspilationRuntimeLibraries" -> addInjectTranspilationRuntimeLibraries();
          default -> throw new IllegalArgumentException("unknown MultiPassTest adder " + adder);
        }
      }
    }

    protected CompilerPass getProcessor(Compiler compiler) {
      PhaseOptimizer phaseopt = new PhaseOptimizer(compiler, null);
      phaseopt.consume(passes);
      phaseopt.setValidityCheck(
          PassFactory.builder()
              .setName("validityCheck")
              .setRunInFixedPointLoop(true)
              .setInternalFactory(ValidityCheck::new)
              .build());
      return phaseopt;
    }
  }

  private void addNormalization() {
    passes.add(
        PassFactory.builder()
            .setName("normalization")
            .setInternalFactory(Normalize::createNormalizeForOptimizations)
            .build());
  }

  private void addCollapseObjectLiterals() {
    passes.add(
        PassFactory.builder()
            .setName("collapseObjectLiterals")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) ->
                    new InlineObjectLiterals(compiler, compiler.getUniqueNameIdSupplier()))
            .build());
  }

  private void addInlineFunctions() {
    passes.add(
        PassFactory.builder()
            .setName("inlineFunctions")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) ->
                    new InlineFunctions(
                        compiler,
                        compiler.getUniqueNameIdSupplier(),
                        CompilerOptions.Reach.ALL,
                        true,
                        true,
                        CompilerOptions.UNLIMITED_FUN_SIZE_AFTER_INLINING))
            .build());
  }

  private void addInlineVariables() {
    passes.add(
        PassFactory.builder()
            .setName("inlineVariables")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) -> new InlineVariables(compiler, InlineVariables.Mode.ALL))
            .build());
  }

  private void addPeephole() {
    passes.add(
        PassFactory.builder()
            .setName("peepholeOptimizations")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) -> {
                  final boolean late = false;
                  return new PeepholeOptimizationsPass(
                      compiler,
                      getName(),
                      new PeepholeMinimizeConditions(late),
                      new PeepholeSubstituteAlternateSyntax(late),
                      new PeepholeReplaceKnownMethods(late, /* useTypes= */ false),
                      new PeepholeRemoveDeadCode(),
                      new PeepholeFoldConstants(late, false /* useTypes */),
                      new PeepholeCollectPropertyAssignments());
                })
            .build());
  }

  private void addRemoveUnusedClassProperties() {
    passes.add(
        PassFactory.builder()
            .setName("removeUnusedClassProperties")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) ->
                    new RemoveUnusedCode.Builder(compiler)
                        .removeUnusedThisProperties(true)
                        .removeUnusedObjectDefinePropertiesDefinitions(true)
                        .build())
            .build());
  }

  private void addRemoveUnusedVars() {
    passes.add(
        PassFactory.builder()
            .setName("removeUnusedVars")
            .setRunInFixedPointLoop(true)
            .setInternalFactory(
                (compiler) -> new RemoveUnusedCode.Builder(compiler).removeLocalVars(true).build())
            .build());
  }

  private void addDestructuringPass() {
    passes.add(
        PassFactory.builder()
            .setName("destructuringPass")
            .setInternalFactory(
                (compiler) ->
                    new Es6RewriteDestructuring.Builder(compiler)
                        .setDestructuringRewriteMode(
                            ObjectDestructuringRewriteMode.REWRITE_ALL_OBJECT_PATTERNS)
                        .build())
            .build());
  }

  private void addArrowFunctionPass() {
    passes.add(
        PassFactory.builder()
            .setName("arrowFunctionPass")
            .setInternalFactory(Es6RewriteArrowFunction::new)
            .build());
  }

  private void addInjectTranspilationRuntimeLibraries() {
    passes.add(
        PassFactory.builder()
            .setName("injectTranspilationRuntimeLibraries")
            .setInternalFactory(InjectTranspilationRuntimeLibraries::new)
            .build());
  }
}
