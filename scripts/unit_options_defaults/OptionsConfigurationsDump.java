package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.javascript.jscomp.CompilerOptions.LanguageMode;
import com.google.javascript.jscomp.DependencyOptions.DependencyMode;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * Dumps what the option-setting APIs do to {@code new CompilerOptions()}: per configuration (every
 * CompilationLevel setter, every WarningLevel alone and after each CompilationLevel, browser
 * featureset years, language modes, defines of each type, and the cross product of
 * {@code DependencyOptions.fromFlags} arguments), the instance fields that differ from the
 * defaults, or the exception it throws. Fields are dumped with the recorder's own value encoder
 * ({@link UnitRecorder#fields}, depth 4, as OptionsDefaultsDump dumps the defaults). Output: a JSON
 * array of {"config":..., "diff"|"result":..., "error"?, "message"?} on stdout, the fixture
 * crates/jscomp/tests/data/compiler_options_configs.json (compiler_options_configs_test.rs).
 */
public final class OptionsConfigurationsDump {
  private static final JsonArray cases = new JsonArray();
  private static JsonObject defaults;

  private static JsonObject fields(Object o) {
    return UnitRecorder.fields(o, o.getClass(), Object.class, "options", new JsonArray(), 4);
  }

  private static JsonObject config(String kind, String value) {
    JsonObject c = new JsonObject();
    c.addProperty("kind", kind);
    c.addProperty("value", value);
    return c;
  }

  private static void options(JsonObject c, Consumer<CompilerOptions> action) {
    CompilerOptions o = new CompilerOptions();
    JsonObject row = new JsonObject();
    row.add("config", c);
    try {
      action.accept(o);
    } catch (RuntimeException e) {
      row.addProperty("error", e.getClass().getName());
      row.addProperty("message", e.getMessage());
    }
    JsonObject actual = fields(o);
    JsonObject diff = new JsonObject();
    for (Map.Entry<String, JsonElement> e : actual.entrySet()) {
      if (!e.getValue().equals(defaults.get(e.getKey()))) {
        diff.add(e.getKey(), e.getValue());
      }
    }
    row.add("diff", diff);
    cases.add(row);
  }

  public static void main(String[] args) {
    defaults = fields(new CompilerOptions());
    for (CompilationLevel l : CompilationLevel.values()) {
      for (String method : List.of("setOptionsForCompilationLevel", "setDebugOptionsForCompilationLevel",
          "setTypeBasedOptimizationOptions", "setWrappedOutputOptimizations", "cli")) {
        options(config(method, l.name()), o -> {
          switch (method) {
            case "setOptionsForCompilationLevel" -> l.setOptionsForCompilationLevel(o);
            case "setDebugOptionsForCompilationLevel" -> l.setDebugOptionsForCompilationLevel(o);
            case "setTypeBasedOptimizationOptions" -> l.setTypeBasedOptimizationOptions(o);
            case "setWrappedOutputOptimizations" -> l.setWrappedOutputOptimizations(o);
            case "cli" -> {
              l.setOptionsForCompilationLevel(o);
              l.setDebugOptionsForCompilationLevel(o);
              l.setTypeBasedOptimizationOptions(o);
              l.setWrappedOutputOptimizations(o);
            }
          }
        });
      }
    }
    for (WarningLevel w : WarningLevel.values()) {
      options(config("warning", w.name()), o -> w.setOptionsForWarningLevel(o));
      for (CompilationLevel l : CompilationLevel.values()) {
        JsonObject c = config("warning", w.name());
        c.addProperty("compilation", l.name());
        options(c, o -> {
          l.setOptionsForCompilationLevel(o);
          w.setOptionsForWarningLevel(o);
        });
      }
    }
    for (int y : new int[] {2012, 2018, 2019, 2020, 2021, 2022, 2023, 2024, 2025, 2026}) {
      options(config("browser", Integer.toString(y)), o -> o.setBrowserFeaturesetYear(y));
    }
    for (LanguageMode l : LanguageMode.values()) {
      options(config("setLanguage", l.name()), o -> o.setLanguage(l));
      options(config("setLanguageIn", l.name()), o -> o.setLanguageIn(l));
      options(config("setLanguageOut", l.name()), o -> o.setLanguageOut(l));
    }
    for (boolean b : new boolean[] {false, true}) {
      options(config("setEmitUseStrict", Boolean.toString(b)), o -> o.setEmitUseStrict(b));
      options(config("setTranspilePublicClassFields", Boolean.toString(b)),
          o -> o.setTranspilePublicClassFields(b));
      options(config("setDefineToBooleanLiteral", Boolean.toString(b)),
          o -> o.setDefineToBooleanLiteral("sample", b));
    }
    for (int n : new int[] {Integer.MIN_VALUE, -1, 0, 1, Integer.MAX_VALUE}) {
      options(config("setDefineToNumberLiteral", Integer.toString(n)),
          o -> o.setDefineToNumberLiteral("sample", n));
    }
    for (double n : new double[] {-0.0, 0.0, -1.5, 1.5, Double.MIN_VALUE, Double.MAX_VALUE, Double.NaN,
        Double.NEGATIVE_INFINITY, Double.POSITIVE_INFINITY}) {
      options(config("setDefineToDoubleLiteral", Double.toString(n)),
          o -> o.setDefineToDoubleLiteral("sample", n));
    }
    for (String s : List.of("", "string", "a\n\r\u0085  \"$", "\ud800")) {
      options(config("setDefineToStringLiteral", s), o -> o.setDefineToStringLiteral("sample", s));
    }
    // The full cross product of the mode, the entry point sources and the two legacy flags covers
    // fromFlags' precedence and validation.
    List<DependencyMode> modes = new ArrayList<>();
    modes.add(null);
    modes.addAll(Arrays.asList(DependencyMode.values()));
    for (DependencyMode mode : modes) {
      for (int entries = 0; entries < 8; entries++) {
        for (int legacy = 0; legacy < 4; legacy++) {
          List<String> ep = (entries & 1) != 0 ? List.of("goog:entry.ns", "src/../entry.js") : List.of();
          List<String> cep = (entries & 2) != 0 ? List.of("goog:chunk:entry.ns") : List.of();
          String common = (entries & 4) != 0 ? "common.js" : null;
          boolean manage = (legacy & 1) != 0;
          boolean only = (legacy & 2) != 0;
          JsonObject c = config("dependency", mode == null ? null : mode.name());
          c.addProperty("entries", entries);
          c.addProperty("legacy", legacy);
          JsonObject row = new JsonObject();
          row.add("config", c);
          try {
            DependencyOptions d = DependencyOptions.fromFlags(mode, ep, cep, common, manage, only);
            row.add("result", d == null ? JsonNull.INSTANCE : fields(d));
          } catch (RuntimeException e) {
            row.addProperty("error", e.getClass().getName());
            row.addProperty("message", e.getMessage());
          }
          cases.add(row);
        }
      }
    }
    Gson gson = new GsonBuilder().serializeNulls().disableHtmlEscaping()
        .serializeSpecialFloatingPointValues().create();
    System.out.println(UnitRecorder.escapeLoneSurrogates(gson.toJson(cases)));
  }
}
