package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

/**
 * Dumps every instance field of {@code new CompilerOptions()} with the recorder's own value
 * encoder ({@link UnitRecorder#fields}, the same call and depth {@code optionsDiff} uses for the
 * defaults side), so a record's {@code options} diff can be applied to these defaults without Java.
 * Output: {"v":2,"class":...,"depth":4,"fields":{...},"unrepresentable":[...]} on stdout.
 */
public final class OptionsDefaultsDump {
  public static void main(String[] args) {
    JsonArray unrep = new JsonArray();
    CompilerOptions def = new CompilerOptions();
    JsonObject fields = UnitRecorder.fields(def, def.getClass(), Object.class, "default", unrep, 4);
    JsonObject out = new JsonObject();
    out.addProperty("v", 2);
    out.addProperty("class", CompilerOptions.class.getName());
    out.addProperty("depth", 4);
    out.add("fields", fields);
    // Declared Java type of every dumped field (same naming as UnitRecorder.fields), so a JSON number
    // can be decoded to its Java numeric type (int/short/byte/long/float/double) without Java.
    JsonObject types = new JsonObject();
    for (Class<?> k = def.getClass(); k != null && k != Object.class; k = k.getSuperclass()) {
      for (java.lang.reflect.Field f : k.getDeclaredFields()) {
        if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) || f.isSynthetic()) {
          continue;
        }
        String name = (k == def.getClass() ? "" : k.getSimpleName() + ".") + f.getName();
        if (types.has(name)) {
          name = k.getName() + "." + f.getName();
        }
        types.addProperty(name, f.getGenericType().getTypeName());
      }
    }
    out.add("fieldTypes", types);
    out.add("unrepresentable", unrep);
    Gson gson = new GsonBuilder().serializeNulls().disableHtmlEscaping().serializeSpecialFloatingPointValues().setPrettyPrinting().create();
    System.out.println(UnitRecorder.escapeLoneSurrogates(gson.toJson(out)));
  }
}
