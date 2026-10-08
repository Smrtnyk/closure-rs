/*
 * closure-rs unit-corpus replay: public bridge to the package-private decoder, for replay
 * classes that must live in other harness packages.
 */
package com.google.javascript.jscomp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.List;

/** Public entry points for ReplayValues/ReplayOptions. */
public final class ReplayBridge {
  private ReplayBridge() {}

  public static CompilerOptions options(JsonObject diff) {
    return ReplayOptions.build(diff);
  }

  /** Options with a descriptor case's options override applied (case may be null). */
  public static CompilerOptions options(JsonObject diff, JsonObject descCase, JsonObject record) {
    ReplayDsl.Ctx ctx = new ReplayDsl.Ctx();
    ctx.record = record;
    return ReplayOptions.build(diff, descCase == null ? null : descCase.getAsJsonObject("options"), ctx);
  }

  public static void setField(Object inst, Class<?> c, String name, JsonElement v) {
    ReplayValues.setField(inst, c, Object.class, name, v);
  }

  public static DiagnosticGroup diagnosticGroup(JsonObject g) {
    return ReplayValues.diagnosticGroup(g);
  }

  public static List<JSChunk> chunks(JsonElement e) {
    return ReplayValues.chunks(e);
  }

  public static List<SourceFile> sourceFiles(JsonElement e) {
    return ReplayValues.sourceFiles(e);
  }

  public static JsonArray errors(List<JSError> errs) {
    return UnitRecorder.errors(errs);
  }
}
