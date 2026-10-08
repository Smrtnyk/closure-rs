/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md). It lives in package
 * com.google.javascript.jscomp.disambiguate, like the test, because ColorFindPropertyReferences,
 * ColorGraphNodeFactory and ColorGraphNode are package-private there; the descriptor therefore
 * builds it with {"new": FQCN} (an inner class gets its holder instance as first argument).
 * Copied from test/com/google/javascript/jscomp/disambiguate/ColorFindPropertyReferencesTest.java
 * (closure-compiler commit bb8c8e7):
 *   fields externsCallback, processor (lines 70-71), labeledStatementMap (line 79), finder,
 *     flattener (lines 602-603): VERBATIM.
 *   getProcessor: lines 106-109, VERBATIM (@Override, line 105, dropped: no superclass here).
 *   clearSourceFileRecursive: lines 473-478, VERBATIM.
 *   createColorGraphNode: lines 598-600, VERBATIM.
 *   StubColorGraphNodeFactory: lines 632-653, VERBATIM apart from the qualified outer reference
 *     `ColorFindPropertyReferencesTest.this` renamed to `ColorFindPropertyReferencesTest_Helpers.this`.
 *   SilenceChecksWarningsGuard: the anonymous WarningsGuard of SILENCE_CHECKS_WARNINGS_GUARD
 *     (lines 655-666; recorded in options.warningsGuard as ColorFindPropertyReferencesTest$1);
 *     its two methods (lines 657-665) are VERBATIM apart from indentation, the class is named.
 *   LabelledStatementCollector: lines 668-683, VERBATIM.
 * Generated (not in the test):
 *   - the field `compiler`: the test's `private final Compiler compiler = new Compiler();`
 *     (line 68) is what createCompiler() returns, so it is the Compiler handed to getProcessor;
 *     here it is that Compiler, passed to the constructor.
 *   - the constructor, which runs, in order, setUp's `this.externsCallback = (Node unused) -> {};`
 *     (line 90, VERBATIM), the externsCallback assignment of
 *     externProps_areClusteredTogether_evenIfSourceInformationIsMissingWithinExterns (lines
 *     487-493, VERBATIM) when `stripExternsSourceInfo` is set, and the body of
 *     collectProperties(propertyReflectorNames, externs, src) up to its test(...) call (lines
 *     612-624, VERBATIM); propertyReflectorNames is the constructor parameter of that name.
 */
package com.google.javascript.jscomp.disambiguate;

import static com.google.common.base.Preconditions.checkNotNull;
import static com.google.common.base.Preconditions.checkState;
import static com.google.common.truth.Truth.assertWithMessage;
import static com.google.javascript.rhino.testing.NodeSubject.assertNode;

import com.google.common.collect.HashBiMap;
import com.google.common.collect.ImmutableSet;
import com.google.javascript.jscomp.CheckLevel;
import com.google.javascript.jscomp.Compiler;
import com.google.javascript.jscomp.CompilerPass;
import com.google.javascript.jscomp.JSError;
import com.google.javascript.jscomp.NodeTraversal;
import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.WarningsGuard;
import com.google.javascript.jscomp.colors.Color;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.Token;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;

final class ColorFindPropertyReferencesTest_Helpers {
  // Generated: the test's `compiler` field (line 68), i.e. the Compiler createCompiler() returns.
  private final Compiler compiler;

  private Consumer<Node> externsCallback;
  private @Nullable CompilerPass processor;

  /**
   * Maps a label name to information about the color of the labeled statement node.
   *
   * <p>This map is recreated each time collectProperties() is run.
   */
  private HashBiMap<String, Color> labeledStatementMap;

  private ColorFindPropertyReferences finder;
  private StubColorGraphNodeFactory flattener;

  // Generated: setUp's externsCallback assignment, the test method's own externsCallback
  // assignment (if any), then collectProperties up to its test(...) call.
  ColorFindPropertyReferencesTest_Helpers(
      Compiler compiler,
      boolean stripExternsSourceInfo,
      ImmutableSet<String> propertyReflectorNames) {
    this.compiler = compiler;
    this.externsCallback = (Node unused) -> {};
    if (stripExternsSourceInfo) {
      this.externsCallback =
          (Node externs) -> {
            Node externsScript = this.compiler.getRoot().getFirstChild().getLastChild();
            for (Node externsNode : externsScript.children()) {
              clearSourceFileRecursive(externsNode);
            }
          };
    }

    // Create a fresh statement map for each test case.
    labeledStatementMap = HashBiMap.create();
    this.processor =
        (e, s) -> {
          this.externsCallback.accept(e);
          NodeTraversal.traverse(compiler, s, new LabelledStatementCollector());
          this.flattener = new StubColorGraphNodeFactory();
          this.finder =
              new ColorFindPropertyReferences(
                  this.flattener,
                  (node) -> propertyReflectorNames.contains(node.getQualifiedName()));
          NodeTraversal.traverse(this.compiler, e.getParent(), checkNotNull(this.finder));
        };
  }

  protected CompilerPass getProcessor(Compiler compiler) {
    checkState(compiler == this.compiler);
    return checkNotNull(this.processor);
  }

  private void clearSourceFileRecursive(Node root) {
    root.setStaticSourceFile(null);
    for (Node child : root.children()) {
      clearSourceFileRecursive(child);
    }
  }

  private ColorGraphNode createColorGraphNode() {
    return ColorGraphNode.createForTesting(-1);
  }

  private final class StubColorGraphNodeFactory extends ColorGraphNodeFactory {

    private final LinkedHashSet<Color> created = new LinkedHashSet<>();
    private final HashBiMap<Color, ColorGraphNode> createdNodes = HashBiMap.create();

    StubColorGraphNodeFactory() {
      super(
          new LinkedHashMap<>(), ColorFindPropertyReferencesTest_Helpers.this.compiler.getColorRegistry());
    }

    @Override
    public ColorGraphNode createNode(@Nullable Color color) {
      this.created.add(color);
      return createdNodes.computeIfAbsent(
          color, (c) -> ColorFindPropertyReferencesTest_Helpers.this.createColorGraphNode());
    }

    @Override
    public ImmutableSet<ColorGraphNode> getAllKnownTypes() {
      throw new UnsupportedOperationException();
    }
  }

  // Named: the anonymous `new WarningsGuard() {...}` of SILENCE_CHECKS_WARNINGS_GUARD.
  static final class SilenceChecksWarningsGuard extends WarningsGuard {
    @Override
    protected int getPriority() {
      return WarningsGuard.Priority.MAX.getValue();
    }

    @Override
    public CheckLevel level(JSError error) {
      return error.description().contains("Parse") ? null : CheckLevel.OFF;
    }
  }

  private class LabelledStatementCollector extends AbstractPostOrderCallback {
    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      if (parent != null && parent.isLabel() && !n.isLabelName()) {
        // First child of a LABEL is a LABEL_NAME, n is the second child.
        Node labelNameNode = checkNotNull(n.getPrevious(), n);
        checkState(labelNameNode.isLabelName(), labelNameNode);
        String labelName = labelNameNode.getString();
        assertWithMessage("Duplicate label name: %s", labelName)
            .that(labeledStatementMap)
            .doesNotContainKey(labelName);
        assertNode(n).hasToken(Token.EXPR_RESULT);
        labeledStatementMap.put(labelName, n.getOnlyChild().getColor());
      }
    }
  }
}
