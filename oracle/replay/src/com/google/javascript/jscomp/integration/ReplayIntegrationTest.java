/*
 * closure-rs unit-corpus replay: replays one IntegrationTestCase record (kind=integration).
 * Lives in the harness package because IntegrationTestCase is package-private.
 */
package com.google.javascript.jscomp.integration;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.javascript.jscomp.CompilerOptions;
import com.google.javascript.jscomp.DiagnosticGroup;
import com.google.javascript.jscomp.ReplayBridge;
import java.util.Map;
import java.util.Set;

/** Restores IntegrationTestCase fields and the recorded options, then re-runs the hooked API. */
public final class ReplayIntegrationTest extends IntegrationTestCase {
  private static final Set<String> SKIP_FIELDS = Set.of("lastCompiler");
  private final JsonObject record;

  private ReplayIntegrationTest(JsonObject record) {
    this.record = record;
  }

  private JsonObject descCase;

  private CompilerOptions createCompilerOptions() {
    return ReplayBridge.options(record.getAsJsonObject("options"), descCase, record);
  }

  public static JsonObject replay(JsonObject record) throws Exception {
    return replay(record, null);
  }

  /** descCase: the matching descriptor case or null (DSL.md "Options override"). */
  public static JsonObject replay(JsonObject record, JsonObject descCase) throws Exception {
    ReplayIntegrationTest t = new ReplayIntegrationTest(record);
    t.descCase = descCase;
    t.setUp();
    JsonObject fields = record.getAsJsonObject("harness").getAsJsonObject("fields");
    for (Map.Entry<String, JsonElement> e : fields.entrySet()) {
      if (!SKIP_FIELDS.contains(e.getKey())) {
        ReplayBridge.setField(t, IntegrationTestCase.class, e.getKey(), e.getValue());
      }
    }
    CompilerOptions options = t.createCompilerOptions();
    JsonObject in = record.getAsJsonObject("inputs");
    JsonObject ex = record.getAsJsonObject("expected");
    String api = record.get("api").getAsString();
    String[] originals = strings(in.get("originals"));
    String[] compiled = strings(ex.get("output"));
    JsonObject outcome = new JsonObject();
    try {
      switch (api) {
        case "test" -> t.test(options, originals, compiled);
        case "test_warning" -> t.test(options, originals, compiled, groups(ex)[0]);
        case "test_warnings" -> t.test(options, originals, compiled, groups(ex));
        case "testNoWarnings" -> t.testNoWarnings(options, originals);
        case "testParseError" -> t.testParseError(options, originals[0], compiled == null ? null : compiled[0]);
        case "compile" ->
            t.compile(options, com.google.common.collect.ImmutableList.copyOf(ReplayBridge.chunks(in.get("chunks"))));
        default -> throw new UnsupportedOperationException("integration api " + api);
      }
      outcome.addProperty("status", "normal");
    } catch (UnsupportedOperationException u) {
      throw u;
    } catch (Throwable th) {
      outcome.addProperty("status", "exception");
      outcome.addProperty("exceptionClass", th.getClass().getName());
      outcome.addProperty("message", th.getMessage());
    }
    JsonObject res = new JsonObject();
    res.add("outcome", outcome);
    JsonObject obs = new JsonObject();
    if (t.lastCompiler != null) {
      obs.add("errors", ReplayBridge.errors(t.lastCompiler.getErrors()));
      obs.add("warnings", ReplayBridge.errors(t.lastCompiler.getWarnings()));
      obs.addProperty("output", t.lastCompiler.getRoot() == null ? null : t.lastCompiler.toSource());
    }
    res.add("observed", obs);
    return res;
  }

  private static DiagnosticGroup[] groups(JsonObject ex) {
    JsonArray a = ex.getAsJsonArray("diagnosticGroups");
    DiagnosticGroup[] out = new DiagnosticGroup[a.size()];
    for (int i = 0; i < a.size(); i++) {
      out[i] = ReplayBridge.diagnosticGroup(a.get(i).getAsJsonObject());
    }
    return out;
  }

  private static String[] strings(JsonElement e) {
    if (e == null || e.isJsonNull()) {
      return null;
    }
    JsonArray a = e.getAsJsonArray();
    String[] out = new String[a.size()];
    for (int i = 0; i < a.size(); i++) {
      out[i] = a.get(i).getAsString();
    }
    return out;
  }
}
