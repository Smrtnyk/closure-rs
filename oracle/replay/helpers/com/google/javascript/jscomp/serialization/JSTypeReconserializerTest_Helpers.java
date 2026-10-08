/*
 * closure-rs unit-corpus helper for JSTypeReconserializerTest.
 * Source: test/com/google/javascript/jscomp/serialization/JSTypeReconserializerTest.java at
 * closure-compiler commit bb8c8e7. This holder stands in for the test instance: getProcessor
 * assigns stringPoolBuilder and labelToPointer and returns a lambda that reads
 * shouldSerializeProperty and writes typePool and labelToPointer of its enclosing instance.
 *
 * It lives in package com.google.javascript.jscomp.serialization (not com.google.javascript.jscomp
 * like most helpers) because JSTypeReconserializer and its serializeType/generateTypePool are
 * package-private there. The descriptor builds it with {"helper": "JSTypeReconserializerTest_Helpers",
 * "package": "com.google.javascript.jscomp.serialization"} inside {"once": "test"}, so one holder
 * plays the test instance for the whole record (every getProcessor call and the testFieldsAfter
 * check of typePool, stringPoolBuilder and labelToPointer).
 *
 * Copied VERBATIM (indentation unchanged, nothing else edited):
 *   imports actually used (subset of lines 19-47, same text)
 *   fields shouldSerializeProperty .. labelToPointer: lines 54-58 (typesToForwardDeclare,
 *                                                    lines 52-53, is read only by the test's
 *                                                    createCompiler override and is omitted;
 *                                                    the descriptor's compilerSetup models that
 *                                                    override from the recorded field)
 *   getProcessor:                                    lines 82-120 (the @Override on line 81 is
 *                                                    dropped and "protected" becomes
 *                                                    package-private: the holder has no
 *                                                    superclass)
 * Generated (not from the test): the holder class declaration. shouldSerializeProperty is set by
 * the descriptor ("withFields"), see corpus/unit/descriptors/JSTypeReconserializerTest.json.
 */
package com.google.javascript.jscomp.serialization;

import com.google.javascript.jscomp.Compiler;
import com.google.javascript.jscomp.CompilerPass;
import com.google.javascript.jscomp.InvalidatingTypes;
import com.google.javascript.jscomp.NodeTraversal;
import com.google.javascript.rhino.Node;
import java.util.LinkedHashMap;
import java.util.function.Predicate;

final class JSTypeReconserializerTest_Helpers {

  private Predicate<String> shouldSerializeProperty;

  private TypePool typePool;
  private StringPool.Builder stringPoolBuilder;
  private LinkedHashMap<String, Integer> labelToPointer;

  CompilerPass getProcessor(Compiler compiler) {
    this.stringPoolBuilder = StringPool.builder();
    this.labelToPointer = new LinkedHashMap<>();

    return (externs, root) -> {
      JSTypeReconserializer serializer =
          JSTypeReconserializer.create(
              compiler.getTypeRegistry(),
              new InvalidatingTypes.Builder(compiler.getTypeRegistry())
                  .addAllTypeMismatches(compiler.getTypeMismatches())
                  .build(),
              this.stringPoolBuilder,
              this.shouldSerializeProperty,
              SerializationOptions.builder()
                  .setIncludeDebugInfo(true)
                  .setRunValidation(true)
                  .build());

      NodeTraversal.traverseRoots(
          compiler,
          new NodeTraversal.AbstractPostOrderCallback() {
            @Override
            public void visit(NodeTraversal t, Node n, Node parent) {
              if (n.getJSType() != null) {
                int pointer = serializer.serializeType(n.getJSType());

                if (n.getParent().isExprResult() && n.getGrandparent().isLabel()) {
                  String labelName = n.getParent().getPrevious().getString();
                  labelToPointer.put(labelName, pointer);
                }
              }
            }
          },
          externs,
          root);

      this.typePool = serializer.generateTypePool();
    };
  }
}
