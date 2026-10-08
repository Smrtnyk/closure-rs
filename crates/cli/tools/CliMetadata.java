package com.google.javascript.jscomp;

import com.google.gson.GsonBuilder;
import java.lang.reflect.*;
import java.util.*;
import org.kohsuke.args4j.Option;

/** Reads the pinned annotations and defaults without changing the reference. */
public final class CliMetadata {
  public static void main(String[] args) throws Exception {
    Class<?> flagsClass = Class.forName("com.google.javascript.jscomp.CommandLineRunner$Flags");
    Constructor<?> ctor = flagsClass.getDeclaredConstructor();
    ctor.setAccessible(true);
    Object flags = ctor.newInstance();
    List<Object> rows = new ArrayList<>();
    for (Field field : flagsClass.getDeclaredFields()) {
      Option annotation = field.getAnnotation(Option.class);
      if (annotation == null) continue;
      field.setAccessible(true);
      Map<String, Object> row = new LinkedHashMap<>();
      row.put("name", annotation.name());
      row.put("aliases", annotation.aliases());
      row.put("usage", annotation.usage());
      row.put("metaVar", annotation.metaVar());
      row.put("hidden", annotation.hidden());
      row.put("required", annotation.required());
      row.put("help", annotation.help());
      row.put("depends", annotation.depends());
      row.put("forbids", annotation.forbids());
      row.put("handler", annotation.handler().getSimpleName());
      row.put("field", field.getName());
      row.put("type", field.getGenericType().getTypeName());
      Object value = field.get(flags);
      row.put("default", value);
      Class<?> type = field.getType();
      if (List.class.isAssignableFrom(type)) {
        Type element = ((ParameterizedType) field.getGenericType()).getActualTypeArguments()[0];
        type = (Class<?>) element;
      }
      if (type.isEnum()) row.put("enum", type.getEnumConstants());
      rows.add(row);
    }
    Map<String, Object> result = new LinkedHashMap<>();
    result.put("flags", rows);
    Field categories = flagsClass.getDeclaredField("categories");
    categories.setAccessible(true);
    Object cat = categories.get(null);
    result.put("categories", ((com.google.common.collect.Multimap<?, ?>) cat).asMap());
    result.put("diagnosticGroups", DiagnosticGroups.DIAGNOSTIC_GROUP_NAMES);
    result.put("registeredGroups", DiagnosticGroups.getRegisteredGroups().keySet());
    result.put("wildcardExcludedGroups", DiagnosticGroups.wildcardExcludedGroups);
    System.out.println(new GsonBuilder().serializeNulls().disableHtmlEscaping().setPrettyPrinting().create().toJson(result));
  }
}
