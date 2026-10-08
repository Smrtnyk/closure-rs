package com.google.javascript.jscomp;
import com.google.gson.*;
import java.nio.file.*;
public final class DiagnosticGroupFixtures {
  public static void main(String[] args) throws Exception {
    JsonObject result=new JsonObject();
    for(String name:new String[]{"DEPRECATED","VISIBILITY","ACCESS_CONTROLS","CHECK_TYPES","MESSAGE_DESCRIPTIONS"}){
      DiagnosticGroup group=(DiagnosticGroup)DiagnosticGroups.class.getField(name).get(null);JsonObject g=new JsonObject();g.addProperty("name",group.getName());JsonArray types=new JsonArray();
      for(DiagnosticType type:group.getTypes()){JsonObject t=new JsonObject();t.addProperty("key",type.key);t.addProperty("level",type.level.toString());t.addProperty("format",type.format);types.add(t);}g.add("types",types);result.add(name,g);
    }
    Files.writeString(Path.of(args[0]),new GsonBuilder().disableHtmlEscaping().create().toJson(result));
  }
}
