/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper") for SerializeTypedAstPassTest.
 * Source: test/com/google/javascript/jscomp/serialization/SerializeTypedAstPassTest.java at
 * closure-compiler commit bb8c8e7.
 *
 * This holder stands in for the test instance's `astConsumer` field at the moment the private
 * method compile(TestPart...) calls test(parts): compile first creates a local
 * `TypedAst[] resultAst = new TypedAst[1]` and assigns the lambda `(ast) -> resultAst[0] = ast`
 * to the field. Here the same two statements run in the constructor, so the field holds a real
 * lambda capturing a one-element array, exactly like the recorded `astConsumer` value
 * ({"ref": "...SerializeTypedAstPassTest$$Lambda", "captures": {"arg$1": TypedAst[1]}}). The
 * descriptor's classMap maps that recorded lambda name to this holder's lambda name, so the
 * testFieldsAfter check (which applies classMap in reverse) compares like with like.
 *
 * Copied VERBATIM (indentation unchanged):
 *   import of Consumer:                         line 42
 *   field astConsumer:                          line 53
 *   the two statements of compile(TestPart...): lines 489-490
 * Generated (not from the test): the package-private holder class declaration and the
 * constructor that wraps lines 489-490.
 * It lives in package com.google.javascript.jscomp.serialization (TypedAst's package, the test's
 * own package) and is built with {"new": FQCN}.
 */
package com.google.javascript.jscomp.serialization;

import java.util.function.Consumer;

final class SerializeTypedAstPassTest_Compile {
  private Consumer<TypedAst> astConsumer;

  SerializeTypedAstPassTest_Compile() {
    TypedAst[] resultAst = new TypedAst[1];
    astConsumer = (ast) -> resultAst[0] = ast;
  }
}
