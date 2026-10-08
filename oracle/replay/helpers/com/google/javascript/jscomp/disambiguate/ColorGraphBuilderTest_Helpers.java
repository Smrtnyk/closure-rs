/*
 * closure-rs unit-corpus helper for ColorGraphBuilderTest (corpus/unit/DSL.md "helper").
 * Source: test/com/google/javascript/jscomp/disambiguate/ColorGraphBuilderTest.java at
 * closure-compiler commit bb8c8e7. This holder stands in for the test instance: in every recorded
 * test, createBuilderIncludingCode sets the `processor` field to a lambda (traversing with an
 * anonymous AbstractPostOrderCallback that fills the local `testTypes` map and the `labelToId`
 * field) and then calls testSame, whose getProcessor returns that field.
 *
 * It lives in package com.google.javascript.jscomp.disambiguate (not com.google.javascript.jscomp
 * like most helpers) because ColorGraphNodeFactory and createFactory are package-private there.
 * The descriptor therefore builds it with {"new": FQCN} rather than {"helper": ...}, which only
 * resolves holders in com.google.javascript.jscomp.
 *
 * Copied VERBATIM (indentation unchanged, nothing else edited):
 *   imports actually used (subset of lines 19-56, same text)
 *   graphNodeFactory field (with its javadoc): lines 65-71
 *   processor and labelToId fields:            lines 73 and 75
 *   getProcessor body:                         lines 83-86 (the @Override on line 82 is dropped:
 *                                              the holder has no superclass)
 *   the statements of createBuilderIncludingCode that run before testSame: lines 108-134
 *     (in the generated method prepareProcessor)
 * Generated (not from the test): the holder class declaration; the `compiler` field, which in the
 * test is `private final Compiler compiler = new Compiler();` (line 63) and is returned by the
 * createCompiler override (lines 77-80), so getProcessor's argument is that same instance (as
 * getProcessor itself asserts); here it is set from the constructor to getProcessor's compiler;
 * the constructor; the declaration of prepareProcessor and its `return this;`.
 */
package com.google.javascript.jscomp.disambiguate;

import static com.google.common.truth.Truth.assertThat;

import com.google.javascript.jscomp.Compiler;
import com.google.javascript.jscomp.CompilerPass;
import com.google.javascript.jscomp.NodeTraversal;
import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.colors.ColorId;
import com.google.javascript.jscomp.colors.ColorRegistry;
import com.google.javascript.rhino.Node;
import java.util.LinkedHashMap;
import org.jspecify.annotations.Nullable;

final class ColorGraphBuilderTest_Helpers {

  // Generated: stands in for the test's `compiler` field (line 63), see the header.
  private final Compiler compiler;

  /**
   * This registry is only used to lookup box colors so it doesn't have to be the same one used by
   * the graph builder.
   */
  private final ColorGraphNodeFactory graphNodeFactory =
      ColorGraphNodeFactory.createFactory(
          ColorRegistry.builder().setDefaultNativeColorsForTesting().build());

  private @Nullable CompilerPass processor;
  private LinkedHashMap<String, ColorId> labelToId;

  // Generated.
  ColorGraphBuilderTest_Helpers(Compiler compiler) {
    this.compiler = compiler;
  }

  // Generated declaration; the body up to `return this;` is lines 108-134 of
  // createBuilderIncludingCode, VERBATIM.
  ColorGraphBuilderTest_Helpers prepareProcessor() {
    ColorGraphNodeFactory graphNodeFactory = this.graphNodeFactory;
    LinkedHashMap<String, ColorGraphNode> testTypes = new LinkedHashMap<>();
    this.labelToId = new LinkedHashMap<>();

    /* Flatten and collect the types of all NAMEs that start with "test". */
    this.processor =
        (externs, main) ->
            NodeTraversal.traverse(
                this.compiler,
                main,
                new AbstractPostOrderCallback() {
                  @Override
                  public void visit(NodeTraversal t, Node n, Node unused) {
                    if (n.isName() && n.getString().startsWith("test")) {
                      testTypes.put(n.getString(), graphNodeFactory.createNode(n.getColor()));
                    }

                    if (n.isLabel()) {
                      String labelName = n.getFirstChild().getString();
                      Node labeledExpr =
                          n.getSecondChild().isExprResult()
                              ? n.getSecondChild().getOnlyChild()
                              : n.getSecondChild();
                      labelToId.put(labelName, labeledExpr.getColor().getId());
                    }
                  }
                });
    return this;
  }

  protected CompilerPass getProcessor(Compiler compiler) {
    assertThat(compiler).isSameInstanceAs(this.compiler);
    return this.processor;
  }
}
