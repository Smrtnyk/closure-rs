package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/** Captures the pinned registry, including member iteration order and unregistered public groups. */
public final class CompilerDiagnosticGroups {
  private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();

  private static List<String> type(DiagnosticType type) {
    return List.of(type.key, type.level.name(), type.format);
  }

  private static List<Object> group(String name, DiagnosticGroup group) {
    List<List<String>> types = new ArrayList<>();
    for (DiagnosticType type : group.getTypes()) {
      types.add(type(type));
    }
    return Arrays.asList(name, group.getName(), types);
  }

  public static void main(String[] args) throws Exception {
    List<List<Object>> registered = new ArrayList<>();
    for (var entry : DiagnosticGroups.getRegisteredGroups().entrySet()) {
      registered.add(group(entry.getKey(), entry.getValue()));
    }
    List<List<Object>> fields = new ArrayList<>();
    Field[] publicFields = DiagnosticGroups.class.getFields();
    Arrays.sort(publicFields, Comparator.comparing(Field::getName));
    for (Field field : publicFields) {
      if (Modifier.isStatic(field.getModifiers()) && field.getType() == DiagnosticGroup.class) {
        fields.add(group(field.getName(), (DiagnosticGroup) field.get(null)));
      }
    }
    // One compact JSON line per group keeps the fixture readable without repeating JSON field names.
    List<String> lines = new ArrayList<>();
    lines.add("[");
    for (List<List<Object>> section : List.of(registered, fields)) {
      lines.add("[");
      for (int i = 0; i < section.size(); i++) {
        lines.add(GSON.toJson(section.get(i)) + (i + 1 < section.size() ? "," : ""));
      }
      lines.add(section == registered ? "]," : "]");
    }
    lines.add("]");
    Files.write(Path.of(args[0]), lines, StandardCharsets.UTF_8);

    // Optional constant capture resolves Java concatenations/text blocks verbatim for transcription.
    if (args.length == 3) {
      List<Object> constants = new ArrayList<>();
      for (String selection : Files.readAllLines(Path.of(args[1]), StandardCharsets.UTF_8)) {
        String[] parts = selection.split(" ");
        Field field = Class.forName(parts[0]).getDeclaredField(parts[1]);
        field.setAccessible(true);
        constants.add(List.of(parts[0], parts[1], type((DiagnosticType) field.get(null))));
      }
      Files.writeString(Path.of(args[2]), GSON.toJson(constants) + "\n", StandardCharsets.UTF_8);
    }
  }
}
