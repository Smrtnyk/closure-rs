/*
 * closure-rs unit-corpus replay: rebuilds CompilerOptions from a record's options diff
 * (FORMAT.md "options": field -> value, diff against new CompilerOptions()).
 */
package com.google.javascript.jscomp;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.Map;

/** Applies a recorded options diff onto a fresh CompilerOptions. */
public final class ReplayOptions {
  private ReplayOptions() {}

  public static CompilerOptions build(JsonObject diff) {
    return build(diff, null, null);
  }

  /**
   * DSL.md "Options override": fields named in override.skip are not decoded from the diff; then
   * every override.then expression runs with {"options":true} bound to the options being built.
   */
  public static CompilerOptions build(JsonObject diff, JsonObject override, ReplayDsl.Ctx ctx) {
    java.util.Set<String> skip = new java.util.HashSet<>();
    if (override != null && override.has("skip")) {
      for (JsonElement k : override.getAsJsonArray("skip")) {
        skip.add(k.getAsString());
      }
    }
    CompilerOptions o = buildSkipping(diff, skip);
    if (override != null && override.has("then")) {
      ReplayDsl.Ctx c = ctx != null ? ctx : new ReplayDsl.Ctx();
      CompilerOptions saved = c.options;
      c.options = o;
      try {
        ReplayDsl.evalAll(override.get("then"), c);
      } finally {
        c.options = saved;
      }
    }
    return o;
  }

  private static CompilerOptions buildSkipping(JsonObject diff, java.util.Set<String> skip) {
    if (diff == null) {
      return new CompilerOptions();
    }
    CompilerOptions o;
    if (diff.has("@class")) {
      Class<?> c = ReplayValues.classForName(diff.get("@class").getAsString());
      o = (CompilerOptions) ReplayValues.instantiate(c);
    } else {
      o = new CompilerOptions();
    }
    for (Map.Entry<String, JsonElement> e : diff.entrySet()) {
      if (e.getKey().equals("@class") || skip.contains(e.getKey())) {
        continue;
      }
      ReplayValues.setField(o, o.getClass(), Object.class, e.getKey(), e.getValue());
    }
    return o;
  }
}
