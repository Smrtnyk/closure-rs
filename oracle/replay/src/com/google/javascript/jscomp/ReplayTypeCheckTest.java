/*
 * closure-rs unit-corpus replay: replays one TypeCheckTestCase record (kind=type_check):
 * TypeTestBuilder.run or a direct parseAndTypeCheckWithScope call.
 */
package com.google.javascript.jscomp;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.javascript.jscomp.parsing.parser.FeatureSet.Feature;
import java.lang.reflect.Constructor;

/** TypeCheckTestCase replay: rebuilds the builder (or direct call) from the record. */
public final class ReplayTypeCheckTest extends TypeCheckTestCase {
  private ReplayTypeCheckTest() {}

  /** Compiler as built by TypeTestBuilder.newTest / CompilerTypeTestCase.setUp, recorded options. */
  private static Compiler compiler(JsonObject record, JsonObject descCase) {
    ReplayDsl.Ctx ctx = new ReplayDsl.Ctx();
    ctx.record = record;
    Compiler compiler = new Compiler();
    compiler.initOptions(
        ReplayOptions.build(
            record.getAsJsonObject("options"), descCase == null ? null : descCase.getAsJsonObject("options"), ctx));
    compiler.markFeatureNotAllowed(Feature.MODULES);
    if (descCase != null) {
      ctx.compiler = compiler;
      ReplayDsl.evalAll(descCase.get("compilerSetup"), ctx);
    }
    return compiler;
  }

  public static JsonObject replay(JsonObject record) throws Exception {
    return replay(record, null);
  }

  /** descCase: the matching descriptor case or null (DSL.md: compilerSetup, options). */
  public static JsonObject replay(JsonObject record, JsonObject descCase) throws Exception {
    String api = record.get("api").getAsString();
    JsonObject in = record.getAsJsonObject("inputs");
    JsonObject h = record.getAsJsonObject("harness");
    Compiler compiler = compiler(record, descCase);
    JsonObject outcome = new JsonObject();
    try {
      if (api.equals("TypeTestBuilder.run")) {
        Constructor<TypeTestBuilder> k = TypeTestBuilder.class.getDeclaredConstructor(Compiler.class);
        k.setAccessible(true);
        TypeTestBuilder b = k.newInstance(compiler);
        for (SourceFile f : ReplayValues.sourceFiles(in.get("sources"))) {
          b.addSource(f.getName(), f.getCode());
        }
        for (JsonElement e : in.getAsJsonArray("externStrings")) {
          b.addExterns(e.getAsString());
        }
        if (in.get("includeDefaultExterns").getAsBoolean()) {
          b.includeDefaultExterns();
        }
        if (h.get("diagnosticsAreErrors").getAsBoolean()) {
          b.diagnosticsAreErrors();
        }
        if (h.get("reportUnknownTypes").getAsBoolean()) {
          b.enableReportUnknownTypes();
        }
        for (JsonElement g : h.getAsJsonArray("suppress")) {
          b.suppress(ReplayValues.diagnosticGroup(g.getAsJsonObject()));
        }
        JsonObject ex = record.getAsJsonObject("expected");
        for (JsonElement d : ex.getAsJsonArray("diagnosticTypes")) {
          JsonObject t = d.getAsJsonObject();
          b.addDiagnostic(DiagnosticType.make(
              t.get("key").getAsString(), CheckLevel.valueOf(t.get("level").getAsString()), "{0}"));
        }
        for (JsonElement d : ex.getAsJsonArray("diagnosticDescriptions")) {
          b.addDiagnostic(d.getAsString());
        }
        b.run();
      } else if (api.equals("parseAndTypeCheckWithScope")) {
        String externs = in.getAsJsonArray("externs").get(0).getAsJsonObject().get("code").getAsString();
        parseAndTypeCheckWithScope(
            compiler, externs, ReplayValues.sourceFiles(in.get("sources")),
            h.get("reportUnknownTypes").getAsBoolean());
      } else {
        throw new UnsupportedOperationException("type_check api " + api);
      }
      outcome.addProperty("status", "normal");
    } catch (UnsupportedOperationException u) {
      throw u;
    } catch (Throwable t) {
      outcome.addProperty("status", "exception");
      outcome.addProperty("exceptionClass", t.getClass().getName());
      outcome.addProperty("message", t.getMessage());
    }
    JsonObject res = new JsonObject();
    res.add("outcome", outcome);
    JsonObject obs = new JsonObject();
    obs.add("errors", UnitRecorder.errors(compiler.getErrors()));
    obs.add("warnings", UnitRecorder.errors(compiler.getWarnings()));
    res.add("observed", obs);
    return res;
  }
}
