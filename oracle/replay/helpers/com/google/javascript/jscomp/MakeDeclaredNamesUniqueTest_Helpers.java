/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, its fields
 * (mirroring MakeDeclaredNamesUniqueTest's fields of the same names and types, lines 36-46;
 * the instance fields are restored from the record's testFields by the DSL "outer" list) and
 * the nested class declaration, `compiler` field and constructor (standing in for the anonymous
 * class's captured getProcessor parameter `final Compiler compiler`) are generated. The process
 * method is copied VERBATIM from the anonymous CompilerPass in getProcessor,
 *   test/com/google/javascript/jscomp/MakeDeclaredNamesUniqueTest.java lines 51-67
 * (closure-compiler commit bb8c8e7). DSL name: MakeDeclaredNamesUniqueTest_Helpers.Processor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.MakeDeclaredNamesUnique.InlineRenamer;
import com.google.javascript.rhino.Node;

final class MakeDeclaredNamesUniqueTest_Helpers {
  private boolean useDefaultRenamer = false;
  private boolean invert = false;
  private boolean removeConst = false;
  private boolean assertOnChange;
  private static final String LOCAL_NAME_PREFIX = "unique_";

  private class Processor implements CompilerPass {
    private final Compiler compiler;

    private Processor(final Compiler compiler) {
      this.compiler = compiler;
    }

    @Override
    public void process(Node externs, Node root) {
      MakeDeclaredNamesUnique.Builder renamer =
          MakeDeclaredNamesUnique.builder().withAssertOnChange(assertOnChange);
      if (!useDefaultRenamer) {
        renamer =
            renamer.withRenamer(
                new InlineRenamer(
                    compiler.getCodingConvention(),
                    compiler.getUniqueNameIdSupplier(),
                    LOCAL_NAME_PREFIX,
                    removeConst,
                    true,
                    null));
      }
      NodeTraversal.traverseRoots(compiler, renamer.build(), externs, root);
    }
  }
}
