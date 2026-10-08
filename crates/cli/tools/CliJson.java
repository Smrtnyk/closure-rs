package com.google.javascript.jscomp;
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import com.google.gson.*;
import com.google.gson.stream.*;
/** Exercise precisely the reader/writer configuration used by the CLI. */
public final class CliJson {
 static Object units(String value) {if(value==null)return null; List<Integer> out=new ArrayList<>(); for(char c:value.toCharArray())out.add((int)c); return out;}
 static Object capture(String input) {
  Map<String,Object> row = new LinkedHashMap<>();
  try(JsonReader reader = new JsonReader(new StringReader(input))) {
   List<AbstractCommandLineRunner.JsonFileSpec> files = new ArrayList<>();
   Gson gson = new Gson(); reader.beginArray();
   while(reader.hasNext())files.add(gson.fromJson(reader,AbstractCommandLineRunner.JsonFileSpec.class));
   reader.endArray();
   List<Object> values=new ArrayList<>();
   for(AbstractCommandLineRunner.JsonFileSpec f:files) {
    if(f==null){values.add(null);continue;}
    Map<String,Object> value=new LinkedHashMap<>(); value.put("src",units(f.getSrc()));value.put("path",units(f.getPath()));value.put("source_map",units(f.getSourceMap()));value.put("webpack_id",units(f.getWebpackId()));values.add(value);
   }
   row.put("files", values);
   ByteArrayOutputStream out=new ByteArrayOutputStream();
   try(JsonWriter writer = new JsonWriter(new BufferedWriter(new OutputStreamWriter(out,StandardCharsets.UTF_8)))) {
    new GsonBuilder().disableHtmlEscaping().setPrettyPrinting().create().toJson(files,new com.google.gson.reflect.TypeToken<List<AbstractCommandLineRunner.JsChunkSpec>>(){}.getType(),writer);
   }
   row.put("output", Base64.getEncoder().encodeToString(out.toByteArray()));
  } catch(Throwable e) {row.put("error_class",e.getClass().getName());row.put("error",e.getMessage());}
  return row;
 }
 public static void main(String[] args)throws Exception {
  Gson gson = new GsonBuilder().serializeNulls().disableHtmlEscaping().setPrettyPrinting().create();
  JsonArray inputs=JsonParser.parseReader(new InputStreamReader(System.in,StandardCharsets.UTF_8)).getAsJsonArray();
  List<Object> rows=new ArrayList<>();for(JsonElement input:inputs){Map<String,Object> row=new LinkedHashMap<>();row.put("input",input.getAsString());row.put("result",capture(input.getAsString()));rows.add(row);}
  System.out.println(gson.toJson(rows));
 }
}
