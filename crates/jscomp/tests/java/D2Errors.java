package com.google.javascript.jscomp;
import com.google.gson.*;
import com.google.common.collect.ImmutableList;
import com.google.debugging.sourcemap.proto.Mapping.OriginalMapping;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.logging.*;
public final class D2Errors {
  static final Gson GSON=new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
  static ByteArrayOutputStream stderr;
  static final class RecordingCompiler extends Compiler {
    Recorder recorder;
    RecordingCompiler(PrintStream stream){super(stream);}
    @Override public void setErrorManager(ErrorManager delegated){recorder=new Recorder(this,delegated);super.setErrorManager(recorder);}
  }
  static final class Runner extends CommandLineRunner {
    Runner(String[] args){super(args,System.in,System.out,System.err);}
    @Override protected Compiler createCompiler(){return new RecordingCompiler(System.err);}
    RecordingCompiler compiler(){return (RecordingCompiler)getCompiler();}
  }
  static final class Recorder implements ErrorManager {
    final RecordingCompiler compiler;final ErrorManager delegated;
    final List<JSError> errors=new ArrayList<>();final List<CheckLevel> levels=new ArrayList<>();
    JsonArray diagnostics=new JsonArray();JsonObject sources=new JsonObject();String reportStderr="";
    int errorCount,warningCount,summaryDetailLevel=1;double typedPercent;boolean generatedReport;
    Recorder(RecordingCompiler compiler,ErrorManager delegated){this.compiler=compiler;this.delegated=delegated;}
    @Override public void report(CheckLevel level,JSError error){errors.add(error);levels.add(level);delegated.report(level,error);}
    void source(String name){if(name==null||sources.has(name))return;SourceFile file=compiler.getSourceFileByName(name);if(file==null){sources.add(name,JsonNull.INSTANCE);return;}JsonObject s=new JsonObject();s.addProperty("stub",file.isStubSourceFileForAlreadyProvidedInput());try{String code=file.isStubSourceFileForAlreadyProvidedInput()?null:file.getCode();s.addProperty("code",code);if(code!=null && hasUnpairedSurrogate(code)){JsonArray units=new JsonArray();for(int i=0;i<code.length();i++)units.add((int)code.charAt(i));s.add("code_utf16",units);}}catch(IOException e){s.addProperty("code",(String)null);s.addProperty("io_exception",e.getMessage());}sources.add(name,s);}
    void snapshot(){
      diagnostics=new JsonArray();sources=new JsonObject();
      for(int i=0;i<errors.size();i++){JSError e=errors.get(i);JsonObject d=new JsonObject();d.addProperty("level",levels.get(i).toString());d.addProperty("key",e.type().key);d.addProperty("type_level",e.type().level.toString());d.addProperty("format",e.type().format);d.addProperty("description",e.description());d.addProperty("source_name",e.sourceName());d.addProperty("lineno",e.lineno());d.addProperty("charno",e.charno());d.addProperty("length",e.length());d.addProperty("default_level",e.defaultLevel().toString());d.addProperty("has_node",e.node()!=null);d.addProperty("node_length",e.node()==null?null:e.node().getLength());OriginalMapping mapping=compiler.getSourceMapping(e.sourceName(),e.lineno(),e.charno());if(mapping==null)d.add("mapping",JsonNull.INSTANCE);else{JsonObject m=new JsonObject();m.addProperty("original_file",mapping.getOriginalFile());m.addProperty("line_number",mapping.getLineNumber());m.addProperty("column_position",mapping.getColumnPosition());d.add("mapping",m);source(mapping.getOriginalFile());}source(e.sourceName());diagnostics.add(d);}
      summaryDetailLevel=compiler.getOptions().getSummaryDetailLevel();
      errorCount=getErrorCount();warningCount=getWarningCount();typedPercent=getTypedPercent();
    }
    @Override public void generateReport(){snapshot();generatedReport=true;int start=stderr.size();delegated.generateReport();reportStderr+=new String(stderr.toByteArray(),start,stderr.size()-start,StandardCharsets.UTF_8);
      errorCount=getErrorCount();warningCount=getWarningCount();typedPercent=getTypedPercent();
    }
    @Override public boolean hasHaltingErrors(){return delegated.hasHaltingErrors();}
    @Override public int getErrorCount(){return delegated.getErrorCount();}
    @Override public int getWarningCount(){return delegated.getWarningCount();}
    @Override public ImmutableList<JSError> getErrors(){return delegated.getErrors();}
    @Override public ImmutableList<JSError> getWarnings(){return delegated.getWarnings();}
    @Override public void setTypedPercent(double percent){delegated.setTypedPercent(percent);}
    @Override public double getTypedPercent(){return delegated.getTypedPercent();}
  }
  static boolean hasUnpairedSurrogate(String s){return !s.equals(replaceUnpairedSurrogates(s));}
  // JSON transport retains WTF-16 sources in code_utf16. Diagnostic text uses
  // PrintStream's UTF-8 replacement character '?' for unpaired surrogates.
  static String replaceUnpairedSurrogates(String s){StringBuilder b=new StringBuilder();for(int i=0;i<s.length();i++){char c=s.charAt(i);if(Character.isHighSurrogate(c)&&i+1<s.length()&&Character.isLowSurrogate(s.charAt(i+1))){b.append(c).append(s.charAt(++i));}else b.append(Character.isSurrogate(c)?'?':c);}return b.toString();}
  public static void main(String[] argv) throws Exception {
    PrintStream out=System.out,err=System.err;InputStream in=System.in;
    List<String> selection=Files.readAllLines(Path.of(argv[0]));
    try(BufferedWriter writer=Files.newBufferedWriter(Path.of(argv[1]),StandardCharsets.UTF_8,StandardOpenOption.CREATE,StandardOpenOption.APPEND)){
      int done=0;for(String filename:selection){
        JsonObject golden=JsonParser.parseString(Files.readString(Path.of(filename))).getAsJsonObject();
        String[] args=GSON.fromJson(golden.get("compiler_args"),String[].class);
        for(int i=0;i<args.length;i++){args[i]=args[i].replace("build/golden-tmp/","build/codex/diagnostics/tmp/");if(args[i].startsWith("--")&&args[i].contains("=build/codex/diagnostics/tmp/")){Path p=Path.of(args[i].substring(args[i].indexOf('=')+1));Files.createDirectories(p.getParent());}}
        stderr=new ByteArrayOutputStream();ByteArrayOutputStream stdout=new ByteArrayOutputStream();
        Runner runner=null;JsonObject row=new JsonObject();row.addProperty("case_id",golden.get("case_id").getAsString());row.addProperty("profile",golden.get("profile").getAsString());row.addProperty("source",golden.get("source").getAsString());row.addProperty("golden_path",filename);row.addProperty("golden_stderr",golden.get("stderr").getAsString());
        try{
          System.setOut(new PrintStream(stdout,true,StandardCharsets.UTF_8));System.setErr(new PrintStream(stderr,true,StandardCharsets.UTF_8));System.setIn(new ByteArrayInputStream(new byte[0]));Logger.getLogger(PhaseOptimizer.class.getName()).setLevel(Level.OFF);
          runner=new Runner(args);runner.setExitCodeReceiver(code->null);if(runner.shouldRunCompiler())runner.run();
        }catch(Throwable t){row.addProperty("exception",t.toString());System.err.print("Exception in thread \"main\" ");t.printStackTrace(System.err);}
        finally{System.out.flush();System.err.flush();System.setOut(out);System.setErr(err);System.setIn(in);}
        row.addProperty("stderr",stderr.toString(StandardCharsets.UTF_8));
        Recorder r=runner==null||runner.compiler()==null?null:runner.compiler().recorder;
        if(r==null){row.add("diagnostics",new JsonArray());row.add("sources",new JsonObject());row.addProperty("report_stderr","");row.addProperty("error_count",0);row.addProperty("warning_count",0);row.addProperty("typed_percent",0);row.addProperty("summary_detail_level",1);}
        else{if(!r.generatedReport)r.snapshot();row.addProperty("generated_report",r.generatedReport);row.add("diagnostics",r.diagnostics);row.add("sources",r.sources);row.addProperty("report_stderr",r.reportStderr);row.addProperty("error_count",r.errorCount);row.addProperty("warning_count",r.warningCount);row.addProperty("typed_percent",r.typedPercent);row.addProperty("summary_detail_level",r.summaryDetailLevel);}
        writer.write(replaceUnpairedSurrogates(GSON.toJson(row)));writer.newLine();writer.flush();if(++done%25==0)err.println("D2 captured "+done+"/"+selection.size());
      }
    }
  }
}
