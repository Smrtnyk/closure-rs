package com.google.javascript.jscomp;
import com.google.gson.*;
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.Charset;
import java.nio.file.Path;
import java.util.*;

/** Captures setup without invoking compiler passes. */
public final class CliOptionSetup extends CommandLineRunner {
  CliOptionSetup(String[] args, PrintStream out, PrintStream err) { super(args, new ByteArrayInputStream(new byte[0]), out, err); }
  static Object normalize(Object value) throws Exception {
    if (value == null || value instanceof String || value instanceof Number || value instanceof Boolean) return value;
    if (value instanceof Enum<?>) return ((Enum<?>)value).name();
    if (value instanceof Charset) return ((Charset)value).name();
    if (value instanceof Path) return value.toString();
    if (value instanceof com.google.common.base.Optional<?>) return normalize(((com.google.common.base.Optional<?>)value).orNull());
    if (value instanceof com.google.javascript.jscomp.parsing.parser.FeatureSet) return value.toString();
    if (value instanceof CodingConvention) return value.getClass().getSimpleName();
    if (value instanceof DependencyOptions) {
      Map<String,Object> map = new LinkedHashMap<>();
      map.put("mode", ((DependencyOptions)value).mode().name());
      List<String> points = new ArrayList<>(); for (ModuleIdentifier point : ((DependencyOptions)value).entryPoints()) points.add(point.toString());
      map.put("entryPoints", points); return map;
    }
    if (value instanceof Map<?,?>) { Map<String,Object> map = new LinkedHashMap<>(); for (Map.Entry<?,?> e : ((Map<?,?>)value).entrySet()) map.put(e.getKey().toString(), normalize(e.getValue())); return map; }
    if (value instanceof Iterable<?>) { List<Object> list = new ArrayList<>(); for (Object e : ((Iterable<?>)value)) list.add(normalize(e)); return list; }
    if (value instanceof AbstractCommandLineRunner.FlagEntry<?>) { Map<String,Object> map = new LinkedHashMap<>(); map.put("flag", normalize(((AbstractCommandLineRunner.FlagEntry<?>)value).getFlag())); map.put("value", ((AbstractCommandLineRunner.FlagEntry<?>)value).getValue()); return map; }
    if (value instanceof SourceMap.PrefixLocationMapping) return fields(value, false);
    return Collections.singletonMap("class", value.getClass().getName());
  }
  static Map<String,Object> fields(Object value, boolean all) throws Exception {
    Map<String,Object> map = new LinkedHashMap<>();
    for (Field field : value.getClass().getDeclaredFields()) {
      if (Modifier.isStatic(field.getModifiers())) continue;
      field.setAccessible(true); map.put(field.getName(), normalize(field.get(value)));
    }
    return map;
  }
  static Object capture(String[] args) throws Exception {
    ByteArrayOutputStream out = new ByteArrayOutputStream(), err = new ByteArrayOutputStream();
    Map<String,Object> result = new LinkedHashMap<>();
    try {
      CliOptionSetup runner = new CliOptionSetup(args, new PrintStream(out), new PrintStream(err));
      result.put("shouldRunCompiler", runner.shouldRunCompiler()); result.put("hasErrors",runner.hasErrors());
      if (runner.shouldRunCompiler()) {
        result.put("config", fields(runner.getCommandLineConfig(),true));
        Field compiler = AbstractCommandLineRunner.class.getDeclaredField("compiler"); compiler.setAccessible(true); compiler.set(runner, runner.createCompiler());
        CompilerOptions options = runner.createOptions(); runner.setRunOptions(options);
        Map<String,Object> defaults = fields(new CompilerOptions(),true), values = fields(options,true), diff = new LinkedHashMap<>();
        for (Map.Entry<String,Object> e : values.entrySet()) if (!Objects.equals(e.getValue(), defaults.get(e.getKey()))) diff.put(e.getKey(),e.getValue());
        result.put("options", diff);
      }
    } catch (Throwable failure) { result.put("exception", failure.getClass().getName()); result.put("message", failure.getMessage()); }
    result.put("stdout",out.toString("UTF-8")); result.put("stderr",err.toString("UTF-8")); return result;
  }
  public static void main(String[] args) { try { execute(args); } catch (Exception e) { throw new RuntimeException(e); } }
  static void execute(String[] args) throws Exception {
    Gson gson = new GsonBuilder().serializeNulls().disableHtmlEscaping().setPrettyPrinting().create();
    if (args.length == 1 && args[0].equals("--metadata")) {
      List<Object> rows = new ArrayList<>(); CompilerOptions options = new CompilerOptions();
      for (Field field : CompilerOptions.class.getDeclaredFields()) {
        if (Modifier.isStatic(field.getModifiers())) continue;
        field.setAccessible(true); Map<String,Object> row = new LinkedHashMap<>();
        row.put("field",field.getName()); row.put("type",field.getGenericType().getTypeName()); row.put("default",normalize(field.get(options)));
        if (field.getType().isEnum()) row.put("enum", field.getType().getEnumConstants()); rows.add(row);
      }
      System.out.println(gson.toJson(rows));
    } else {
      JsonArray cases = JsonParser.parseReader(new InputStreamReader(System.in,"UTF-8")).getAsJsonArray();
      List<Object> output = new ArrayList<>();
      for (JsonElement element : cases) { JsonArray argv=element.getAsJsonArray(); String[] values=new String[argv.size()]; for(int i=0;i<values.length;i++)values[i]=argv.get(i).getAsString(); Map<String,Object> record=new LinkedHashMap<>(); record.put("argv",values); record.put("setup",capture(values)); output.add(record); }
      System.out.println(gson.toJson(output));
    }
  }
}
