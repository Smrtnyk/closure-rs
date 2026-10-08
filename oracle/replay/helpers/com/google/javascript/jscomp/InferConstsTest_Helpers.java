/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class, its two fields
 * (mirroring InferConstsTest's fields of the same names and types; the DSL restores `names` from
 * the record's testFields with "withFields") and the no-arg constructor are generated. Copied
 * VERBATIM from test/com/google/javascript/jscomp/InferConstsTest.java (closure-compiler commit
 * bb8c8e7):
 *   getProcessor (with its anonymous CompilerPass): lines 37-47 (the @Override annotation is
 *     dropped, because the holder does not extend CompilerTestCase)
 *   FindConstants:                                  lines 339-361
 * DSL: {"call": {"withFields": {"helper": "InferConstsTest_Helpers"}, "fields": {"names": ...}},
 *       "method": "getProcessor", "args": [{"compiler": true}]}
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableList;
import com.google.javascript.rhino.Node;
import java.util.HashSet;
import java.util.Set;

final class InferConstsTest_Helpers {
  private FindConstants constFinder;

  private ImmutableList<String> names;

  InferConstsTest_Helpers() {}

  public CompilerPass getProcessor(final Compiler compiler) {
    constFinder = new FindConstants(names);
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        new InferConsts(compiler).process(externs, root);
        NodeTraversal.traverse(compiler, root, constFinder);
      }
    };
  }

  private static class FindConstants extends NodeTraversal.AbstractPostOrderCallback {
    final ImmutableList<String> names;
    final Set<String> declaredNodes = new HashSet<>();
    final Set<String> inferredNodes = new HashSet<>();

    FindConstants(ImmutableList<String> names) {
      this.names = names;
    }

    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      for (String name : names) {
        if ((n.isName() || n.isImportStar()) && n.matchesQualifiedName(name)) {
          if (n.isDeclaredConstantVar()) {
            declaredNodes.add(name);
          }
          if (n.isInferredConstantVar()) {
            inferredNodes.add(name);
          }
        }
      }
    }
  }
}
