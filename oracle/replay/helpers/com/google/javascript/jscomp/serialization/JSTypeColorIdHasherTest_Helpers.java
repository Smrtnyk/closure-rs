/*
 * closure-rs unit-corpus helper for JSTypeColorIdHasherTest.
 * Source: test/com/google/javascript/jscomp/serialization/JSTypeColorIdHasherTest.java at
 * closure-compiler commit bb8c8e7. This holder stands in for the test instance: getProcessor
 * assigns the three fields and returns a lambda that traverses with the inner LabeledTypeFinder,
 * which reads and writes those fields of its enclosing instance.
 *
 * It lives in package com.google.javascript.jscomp.serialization (not com.google.javascript.jscomp
 * like the other helpers) because JSTypeColorIdHasher, its constructor and hashObjectType are
 * package-private there. The descriptor therefore builds it with {"new": FQCN} rather than
 * {"helper": ...}, which only resolves holders in com.google.javascript.jscomp.
 *
 * Copied VERBATIM (indentation unchanged, nothing else edited):
 *   imports actually used (subset of lines 19-35, same text)
 *   fields hasher, labelToColorId, colorIdToJSTypes: lines 44-46
 *   LabeledTypeFinder (inner class):                  lines 80-95
 *   getProcessor body:                                 lines 102-109 (the @Override on line 101 is
 *                                                      dropped: the holder has no superclass)
 * Generated (not from the test): the holder class declaration.
 */
package com.google.javascript.jscomp.serialization;

import static com.google.common.truth.Truth.assertThat;

import com.google.common.collect.LinkedHashMultimap;
import com.google.javascript.jscomp.Compiler;
import com.google.javascript.jscomp.CompilerPass;
import com.google.javascript.jscomp.NodeTraversal;
import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.colors.ColorId;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.jstype.JSType;
import com.google.javascript.rhino.jstype.ObjectType;
import java.util.LinkedHashMap;

final class JSTypeColorIdHasherTest_Helpers {
  private JSTypeColorIdHasher hasher;
  private LinkedHashMap<String, ColorId> labelToColorId;
  private LinkedHashMultimap<ColorId, JSType> colorIdToJSTypes; // Useful for debugging.

  private class LabeledTypeFinder extends AbstractPostOrderCallback {
    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      if (!n.isLabel()) {
        return;
      }

      String label = n.getFirstChild().getString();
      ObjectType type = n.getSecondChild().getFirstChild().getJSType().toMaybeObjectType();
      ColorId id = hasher.hashObjectType(type);

      assertThat(labelToColorId).doesNotContainKey(label);
      labelToColorId.put(label, id);
      colorIdToJSTypes.put(id, type);
    }
  }

  protected CompilerPass getProcessor(final Compiler compiler) {
    this.hasher = new JSTypeColorIdHasher(compiler.getTypeRegistry());
    this.labelToColorId = new LinkedHashMap<>();
    this.colorIdToJSTypes = LinkedHashMultimap.create();

    return (Node externs, Node root) ->
        NodeTraversal.traverseRoots(compiler, new LabeledTypeFinder(), externs, root);
  }
}
