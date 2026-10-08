import com.google.gson.*;
import java.nio.file.*;
import java.io.*;
import java.util.*;
import java.util.regex.PatternSyntaxException;
public final class CliGlob {
 public static void main(String[] args)throws Exception {
  Gson gson=new GsonBuilder().serializeNulls().disableHtmlEscaping().create();
  JsonArray cases=JsonParser.parseReader(new InputStreamReader(System.in,"UTF-8")).getAsJsonArray();
  List<Object> rows=new ArrayList<>();
  for(JsonElement value:cases){JsonObject c=value.getAsJsonObject();String pattern=c.get("pattern").getAsString();String input=c.get("input").getAsString();Map<String,Object> row=new LinkedHashMap<>();row.put("pattern",pattern);row.put("input",input);try{Path path=Path.of(input);row.put("normalized",path.toString());row.put("matches",FileSystems.getDefault().getPathMatcher("glob:"+pattern).matches(path));}catch(PatternSyntaxException e){row.put("description",e.getDescription());row.put("index",e.getIndex());row.put("message",e.getMessage());}rows.add(row);}
  System.out.println(gson.toJson(rows));
 }
}
