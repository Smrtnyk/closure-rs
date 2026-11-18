package com.google.javascript.jscomp;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.nio.charset.StandardCharsets;
import java.util.function.Consumer;

/**
 * Dumps {@code CompilerOptions#toString()} after each of 15 configurations of
 * {@code new CompilerOptions()}: the defaults, every compilation level, every warning level, the
 * three coding conventions, one define of each type, an output charset and a message bundle.
 * Identity hashes ({@code @1b2c3d}) are removed. Output: [{"kind":...,"expected":...},...] on
 * stdout, the fixture crates/jscomp/tests/data/compiler_options_strings.json
 * (compiler_options_strings_test.rs).
 */
public final class OptionsStringsDump {
  private static final JsonArray cases = new JsonArray();

  private static void add(String kind, Consumer<CompilerOptions> action) {
    CompilerOptions o = new CompilerOptions();
    action.accept(o);
    JsonObject row = new JsonObject();
    row.addProperty("kind", kind);
    row.addProperty("expected", o.toString().replaceAll("@[0-9a-f]+", ""));
    cases.add(row);
  }

  public static void main(String[] args) {
    add("default", o -> {});
    for (CompilationLevel l : CompilationLevel.values()) {
      add("level:" + l.name(), o -> l.setOptionsForCompilationLevel(o));
    }
    for (WarningLevel w : WarningLevel.values()) {
      add("warning:" + w.name(), o -> w.setOptionsForWarningLevel(o));
    }
    add("DefaultCodingConvention", o -> o.setCodingConvention(CodingConventions.getDefault()));
    add("ClosureCodingConvention", o -> o.setCodingConvention(new ClosureCodingConvention()));
    add("GoogleCodingConvention", o -> o.setCodingConvention(new GoogleCodingConvention()));
    add("defines", o -> {
      o.setDefineToBooleanLiteral("bool", true);
      o.setDefineToNumberLiteral("int", -3);
      o.setDefineToDoubleLiteral("double", 1.25);
      o.setDefineToStringLiteral("string", "abc");
    });
    add("charset", o -> o.setOutputCharset(StandardCharsets.UTF_8));
    add("message", o -> o.setMessageBundle(new EmptyMessageBundle()));
    System.out.println(new GsonBuilder().disableHtmlEscaping().create().toJson(cases));
  }
}
