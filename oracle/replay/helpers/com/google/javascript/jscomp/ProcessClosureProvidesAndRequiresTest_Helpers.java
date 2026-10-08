/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Copied VERBATIM from
 * test/com/google/javascript/jscomp/ProcessClosureProvidesAndRequiresTest.java (closure-compiler
 * commit bb8c8e7):
 *   fields preserveGoogProvidesAndRequires, lastProcessor: lines 41-42
 *   createClosureProcessor:                               lines 44-46
 *   verifyCollectProvidedNamesDoesntChangeAst (with its javadoc): lines 1043-1061
 *   the getProcessor lambda body:                         lines 63-65 (lambda at lines 62-66)
 * Generated: the holder class (standing in for the test instance that the lambda and the private
 * methods use) and the declaration, field and constructor of the inner class GetProcessorLambda,
 * which captures `compiler` and the enclosing instance like the original lambda. The DSL "outer"
 * list restores preserveGoogProvidesAndRequires from the record; lastProcessor is assigned before
 * it is read, so it needs no restoring.
 * DSL name: ProcessClosureProvidesAndRequiresTest_Helpers.GetProcessorLambda
 */
package com.google.javascript.jscomp;

import static com.google.javascript.rhino.testing.NodeSubject.assertNode;

import com.google.javascript.rhino.Node;

final class ProcessClosureProvidesAndRequiresTest_Helpers {
  private boolean preserveGoogProvidesAndRequires;
  private ProcessClosureProvidesAndRequires lastProcessor;

  private ProcessClosureProvidesAndRequires createClosureProcessor(Compiler compiler) {
    return new ProcessClosureProvidesAndRequires(compiler, preserveGoogProvidesAndRequires);
  }

  /** The lambda {@code (Node externs, Node root) -> {...}} returned by getProcessor. */
  private final class GetProcessorLambda implements CompilerPass {
    private final Compiler compiler;

    private GetProcessorLambda(Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      verifyCollectProvidedNamesDoesntChangeAst(externs, root, compiler);
      lastProcessor = createClosureProcessor(compiler);
      lastProcessor.rewriteProvidesAndRequires(externs, root);
    }
  }

  /**
   * Validates that running {@link ProcessClosureProvidesAndRequires#collectProvidedNames(Node,
   * Node)} does not modify the AST.
   *
   * <p>This is important because we want to call this method to gain information about
   * goog.provides but preserve the original AST structure for future checks.
   */
  private void verifyCollectProvidedNamesDoesntChangeAst(
      Node externs, Node root, Compiler compiler) {
    // Validate that this does not modify the AST at all!
    Node originalExterns = externs.cloneTree();
    Node originalRoot = root.cloneTree();
    ProcessClosureProvidesAndRequires processor = createClosureProcessor(compiler);
    processor.collectProvidedNames(externs, root);

    assertNode(externs).isEqualIncludingJsDocTo(originalExterns);
    assertNode(root).isEqualIncludingJsDocTo(originalRoot);
  }
}
