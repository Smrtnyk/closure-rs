/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"), copied from
 *   test/com/google/javascript/jscomp/serialization/SerializeTypedAstPassTest.java
 *   (closure-compiler commit bb8c8e7).
 *
 * - AstConsumer: the lambda `(ast) -> resultAst[0] = ast` that the private method
 *   compile(TestPart...) assigns to the test field astConsumer (line 490) immediately before it
 *   calls test(parts); it captures the local `TypedAst[] resultAst = new TypedAst[1]` (line 489).
 *   The holder class, the nested class declaration and the `resultAst` field (initialised exactly
 *   as the captured local) are generated; the accept() body is the lambda body copied VERBATIM.
 *   (Same pattern as SerializeAndDeserializeAstTest_Helpers.)
 *   DSL name: SerializeTypedAstPassTest_Helpers.AstConsumer
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.serialization.TypedAst;
import java.util.function.Consumer;

final class SerializeTypedAstPassTest_Helpers {
  private SerializeTypedAstPassTest_Helpers() {}

  private static final class AstConsumer implements Consumer<TypedAst> {
    private final TypedAst[] resultAst = new TypedAst[1];

    private AstConsumer() {}

    @Override
    public void accept(TypedAst ast) {
      resultAst[0] = ast;
    }
  }
}
