/*
 * closure-rs unit-corpus helper for TypedAstSerializerTest.
 * Source: test/com/google/javascript/jscomp/serialization/TypedAstSerializerTest.java at
 * closure-compiler commit bb8c8e7. This holder stands in for the test instance: getProcessor
 * returns a lambda that clears the externs scripts, serializes the roots with TypedAstSerializer
 * and writes the result to the `testResult` field of its enclosing instance (read only by the
 * test's own follow-up assertions).
 *
 * It lives in package com.google.javascript.jscomp.serialization (not com.google.javascript.jscomp
 * like most helpers) because TypedAstSerializer and its constructor are package-private there.
 * The descriptor therefore builds it with {"new": FQCN} rather than {"helper": ...}, which only
 * resolves holders in com.google.javascript.jscomp (JSTypeColorIdHasherTest_Helpers precedent).
 *
 * Copied VERBATIM (indentation unchanged, nothing else edited):
 *   imports actually used (subset of lines 19-35, same text)
 *   field testResult (with its comment): lines 89-90
 *   getProcessor:                         lines 101-126 (the @Override on line 100 is dropped: the
 *                                         holder has no superclass)
 * Generated (not from the test): the holder class declaration.
 */
package com.google.javascript.jscomp.serialization;

import com.google.javascript.jscomp.Compiler;
import com.google.javascript.jscomp.CompilerPass;
import com.google.javascript.rhino.Node;
import org.jspecify.annotations.Nullable;

final class TypedAstSerializerTest_Helpers {
  /** Holds the serialized AST created by the last executed test method. */
  private @Nullable TypedAst testResult = null;

  protected CompilerPass getProcessor(final Compiler compiler) {
    return (externs, root) -> {
      // In general we avoid serializing types and properties of types that are not referenced by
      // AST nodes. In practice this avoids serializing properties and types that have been entirely
      // removed due to optimizations.
      //
      // We will simulate this behavior here by clearing the externs so only types and
      // properties that are referenced in the sources branch of the AST will be serialized.
      final Node externsRoot = compiler.getRoot().getFirstChild();
      for (Node externsScript = externsRoot.getFirstChild();
          externsScript != null;
          externsScript = externsScript.getNext()) {
        externsScript.removeChildren();
        compiler.reportChangeToChangeScope(externsScript);
      }

      final TypedAstSerializer typedAstSerializer =
          new TypedAstSerializer(
              compiler,
              SerializationOptions.builder()
                  .setIncludeDebugInfo(true)
                  .setRunValidation(true)
                  .build());
      testResult = typedAstSerializer.serializeRoots(externs, root);
    };
  }
}
