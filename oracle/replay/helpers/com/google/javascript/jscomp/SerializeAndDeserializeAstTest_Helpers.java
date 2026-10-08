/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper") for SerializeAndDeserializeAstTest.
 * Source: test/com/google/javascript/jscomp/serialization/SerializeAndDeserializeAstTest.java at
 * closure-compiler commit bb8c8e7.
 *
 * This holder stands in for the test instance's `consumer` field at the moment the test calls the
 * hooked harness API. Both callers assign it immediately before that call:
 *   toInputStream (used by test/testSame/testAndReturnResult): local `result`, then the lambda
 *     `ast -> result[0] = ast` (lines 1080-1081), then super.test(...) (line 1084);
 *   runWithoutExpected (used by includesShadowedCode and shadowAsts_featuresSetOnMainAstScript):
 *     the same two statements (lines 955-956), then super.testNoWarning(...) (line 957).
 * The methods below run exactly those two statements, so the field holds a real lambda capturing
 * a one-element TypedAst array, like the recorded `consumer` value
 * ({"ref": "...serialization.SerializeAndDeserializeAstTest$$Lambda", "captures": {"arg$1": TypedAst[1]}}).
 * Both lambdas are declared in this one class, so they share the lambda class-name prefix
 * `com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers$$Lambda`, which the
 * descriptor's classMap maps to the single recorded name (the recorder drops the per-lambda
 * address suffix, so both recorded lambdas have the same name too). Replay applies classMap in
 * reverse to the testFieldsAfter dump; no recorded value is decoded through it.
 *
 * Copied VERBATIM (indentation unchanged):
 *   field consumer:                                   line 77
 *   the two statements of toInputStream(...):          lines 1080-1081
 *   the two statements of runWithoutExpected(...):     lines 955-956
 * Generated (not from the test): the package-private holder class declaration and the two
 * no-argument method declarations that wrap those statements (their names are the test methods
 * the statements come from; the hooked call and the follow-up deserialization are not copied).
 * (Fix-2 replaces the earlier class-based ToInputStreamConsumer/RunWithoutExpectedConsumer, whose
 * dump could not equal the recorded lambda dump.)
 * DSL: {"new": "com.google.javascript.jscomp.SerializeAndDeserializeAstTest_Helpers", "args": []},
 *      then {"call": ..., "method": "toInputStream" | "runWithoutExpected", "args": []}.
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.serialization.TypedAst;
import java.util.function.Consumer;
import org.jspecify.annotations.Nullable;

final class SerializeAndDeserializeAstTest_Helpers {
  private @Nullable Consumer<TypedAst> consumer = null;

  void toInputStream() {
    TypedAst[] result = new TypedAst[1];
    consumer = ast -> result[0] = ast;
  }

  void runWithoutExpected() {
    TypedAst[] result = new TypedAst[1];
    consumer = ast -> result[0] = ast;
  }
}
