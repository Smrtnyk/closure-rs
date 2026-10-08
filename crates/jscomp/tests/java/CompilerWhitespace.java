package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Uses reference-jar CLI options but returns Compiler.toSource and structured diagnostics. */
public final class CompilerWhitespace {
  private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
  private static final PrintStream SILENT = new PrintStream(OutputStream.nullOutputStream());
  private static final class Runner extends CommandLineRunner {
    Runner() {
      super(new String[] {"--compilation_level=WHITESPACE_ONLY", "--language_in=ECMASCRIPT_NEXT",
          "--language_out=ECMASCRIPT_NEXT"}, SILENT, SILENT);
    }
    CompilerOptions options() throws Exception {
      CompilerOptions options = createOptions();
      setRunOptions(options);
      return options;
    }
  }
  private static List<SourceFile> files(JsonObject row, String field) {
    List<SourceFile> files = new ArrayList<>();
    for (var item : row.getAsJsonArray(field)) {
      var source = item.getAsJsonObject();
      files.add(SourceFile.fromCode(source.get("name").getAsString(), source.get("code").getAsString()));
    }
    return files;
  }
  // Gson writes lone surrogates literally; retain them as UTF-16 units instead of
  // allowing a UTF-8 encoder to drop or replace diagnostic text.
  private static Object text(String value) {
    if (value == null) return null;
    for (int i = 0; i < value.length(); i++) {
      char ch = value.charAt(i);
      if (Character.isHighSurrogate(ch) && i + 1 < value.length()
          && Character.isLowSurrogate(value.charAt(i + 1))) { i++; continue; }
      if (Character.isSurrogate(ch)) {
        List<Integer> units = new ArrayList<>();
        for (int j = 0; j < value.length(); j++) units.add((int) value.charAt(j));
        return Map.of("utf16", units);
      }
    }
    return value;
  }
  private static List<Map<String, Object>> diagnostics(Iterable<JSError> errors) {
    List<Map<String, Object>> list = new ArrayList<>();
    for (JSError error : errors) {
      Map<String, Object> value = new LinkedHashMap<>();
      value.put("type", error.type().key);
      value.put("description", text(error.description()));
      value.put("source", error.sourceName());
      value.put("line", error.lineno());
      value.put("column", error.charno());
      value.put("length", error.length());
      list.add(value);
    }
    return list;
  }
  public static void main(String[] args) throws Exception {
    int skip = args.length > 2 ? Integer.parseInt(args[2]) : 0;
    try (var input = Files.newBufferedReader(Path.of(args[0]));
         var output = Files.newBufferedWriter(Path.of(args[1]),
             java.nio.file.StandardOpenOption.CREATE,
             skip == 0 ? java.nio.file.StandardOpenOption.TRUNCATE_EXISTING
                       : java.nio.file.StandardOpenOption.APPEND)) {
      for (int i = 0; i < skip; i++) input.readLine();
      int count = skip;
      for (String line; (line = input.readLine()) != null;) {
        JsonObject row = JsonParser.parseString(line).getAsJsonObject();
        Map<String, Object> result = new LinkedHashMap<>();
        result.put("id", row.get("id").getAsString());
        Compiler compiler = new Compiler(SILENT);
        try {
          compiler.compile(files(row, "externs"), files(row, "inputs"), new Runner().options());
          result.put("output", text(compiler.toSource()));
        } catch (Throwable error) {
          result.put("exception", text(error.toString()));
        }
        result.put("errors", diagnostics(compiler.getErrors()));
        result.put("warnings", diagnostics(compiler.getWarnings()));
        output.write(GSON.toJson(result));
        output.newLine();
        output.flush();
        if (++count % 50 == 0) System.err.println("Captured " + count);
      }
    }
  }
}
