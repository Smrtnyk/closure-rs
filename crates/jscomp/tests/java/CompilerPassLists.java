package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Dumps metadata only: no pass is instantiated or executed. */
public final class CompilerPassLists {
  private static final class Runner extends CommandLineRunner {
    Runner(String[] args) { super(args, System.out, System.err); }
    CompilerOptions options() throws Exception {
      CompilerOptions options = createOptions();
      setRunOptions(options);
      return options;
    }
  }
  private static List<Map<String, Object>> list(PassListBuilder builder) {
    List<Map<String, Object>> result = new ArrayList<>();
    for (PassFactory factory : builder.build()) {
      Map<String, Object> row = new LinkedHashMap<>();
      row.put("name", factory.getName());
      row.put("loop", factory.isRunInFixedPointLoop());
      result.add(row);
    }
    return result;
  }
  private static Object checkedList(java.util.function.Supplier<PassListBuilder> supplier) {
    try { return list(supplier.get()); }
    catch (IllegalStateException error) { return Map.of("error", error.getMessage()); }
  }
  private static Map<String, Object> capture(String name, CompilerOptions options, List<String> flags) {
    DefaultPassConfig config = new DefaultPassConfig(options);
    Map<String, Object> result = new LinkedHashMap<>();
    result.put("configuration", name);
    result.put("flags", flags);
    result.put("whitespace", checkedList(config::getWhitespaceOnlyPasses));
    result.put("transpile", checkedList(config::getTranspileOnlyPasses));
    result.put("checks", checkedList(config::getChecks));
    result.put("optimizations", checkedList(config::getOptimizations));
    result.put("finalizations", checkedList(config::getFinalizations));
    return result;
  }
  public static void main(String[] args) throws Exception {
    Gson gson = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
    var profiles = com.google.gson.JsonParser.parseString(Files.readString(Path.of(args[0])))
        .getAsJsonObject().getAsJsonObject("profiles");
    List<Map<String, Object>> rows = new ArrayList<>();
    for (var entry : profiles.entrySet()) {
      List<String> flags = new ArrayList<>();
      for (var flag : entry.getValue().getAsJsonObject().getAsJsonArray("flags")) {
        flags.add(flag.getAsString().replace("{out_dir}", args[2]));
      }
      // The profiles' chunk specs become runner flags; empty inputs suffice for option building.
      if (entry.getValue().getAsJsonObject().has("chunks")) {
        var chunks = entry.getValue().getAsJsonObject().getAsJsonObject("chunks");
        var names = chunks.getAsJsonArray("names");
        for (int i = 0; i < names.size(); i++) {
          String name = names.get(i).getAsString();
          flags.add("--js=" + args[2] + "/" + name + ".js");
          flags.add("--chunk=" + name + ":1" + (i == 0 ? "" : ":c0"));
        }
        flags.add("--chunk_output_path_prefix=" + args[2] + "/");
      }
      rows.add(capture(entry.getKey(), new Runner(flags.toArray(String[]::new)).options(), flags));
    }
    for (CompilationLevel level : new CompilationLevel[] {
        CompilationLevel.WHITESPACE_ONLY, CompilationLevel.SIMPLE_OPTIMIZATIONS,
        CompilationLevel.ADVANCED_OPTIMIZATIONS}) {
      CompilerOptions options = new CompilerOptions();
      level.setOptionsForCompilationLevel(options);
      rows.add(capture("default_" + level, options, List.of()));
    }
    Files.writeString(Path.of(args[1]), gson.toJson(rows) + "\n");
  }
}
