package com.google.javascript.jscomp;
import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.text.MessageFormat;
import java.util.*;
import java.util.jar.*;
public final class MessageFormatCases {
  public static void main(String[] argv) throws Exception {
    Gson gson = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
    List<List<String>> vectors = new ArrayList<>();
    for(int n=0;n<=4;n++){ List<String> a=new ArrayList<>();for(int i=0;i<n;i++)a.add("a"+i);vectors.add(a); }
    vectors.add(List.of("'", "{", "}", "é😀"));vectors.add(List.of("", "a'b{c}", "λ", ""));
    Map<String,Integer> patterns=new LinkedHashMap<>(); List<Object> diagnostics=new ArrayList<>();
    int failures=0; List<String> classes=new ArrayList<>();
    try(JarFile jar=new JarFile(argv[0])) { jar.stream().map(JarEntry::getName).filter(n->n.startsWith("com/google/javascript/")&&n.endsWith(".class")).sorted().forEach(n->classes.add(n.substring(0,n.length()-6).replace('/','.'))); }
    for(String name:classes){
      try {Class<?> c=Class.forName(name,false,MessageFormatCases.class.getClassLoader());
        for(Field f:c.getDeclaredFields()) if(Modifier.isStatic(f.getModifiers()) && f.getType()==DiagnosticType.class){
          f.setAccessible(true); DiagnosticType d=(DiagnosticType)f.get(null); if(d==null) continue;
          int index=patterns.computeIfAbsent(d.format,k->patterns.size());
          diagnostics.add(List.of(name+"#"+f.getName(),d.key,d.level.toString(),index));
        }
      } catch(Throwable t) { failures++;System.err.println(name+": "+t); }
    }
    String[] base={"", "plain } tail", "{0}", "{1}{0}{4}", "'{0}'", "''{0}''", "'''{0}'''", "x'{0}", "'{''}' {0}", "{", "{{", "{{0}}", "{0{", "{0}", "{ 0 }", "{0 }", "{+0}", "{-0}", "{-1}", "{9999}", "{10000}", "{2147483647}", "{2147483648}", "{}", "{x}", "{0,}", "{0,,}", "{0,,#}", "{0,unknown}", "{0,number}", "{0,number,integer}", "{0,date}", "{0,time}", "{0,choice,0#zero|1#one}", "{0,'}'", "{0,{foo}", "{0}''{'0'}", "{0002}", "{0,   }", "a\n{0}\t{2}", "{٠}", "{+٠}", "{-١}", "{１２}", "{९९९९}", "{೧೦೦೦೦}"};
    for(String p:base) patterns.computeIfAbsent(p,k->patterns.size());
    // 240 additional deterministic combinations stress adjacent quoted and malformed elements.
    for(int i=0;i<240;i++){String p=(i%3==0?"'x'":i%3==1?"''":"α ")+base[i%base.length]+(i%2==0?" {2}":" '{3}'")+(i%5==0?"'":"");patterns.computeIfAbsent(p,k->patterns.size());}
    List<Object> cases=new ArrayList<>();
    for(String p:patterns.keySet()){List<Object> results=new ArrayList<>();for(List<String> a:vectors){try{results.add(Map.of("output",DiagnosticType.error("fixture",p).format(a.toArray(String[]::new))));}catch(IllegalArgumentException e){results.add(Map.of("exception",e.getMessage()));}}cases.add(Map.of("pattern",p,"results",results));}
    List<Object> floats=new ArrayList<>();for(double v:new double[]{0.05,0.15,0.25,0.35,99.95,100.0/3,200.0/3,1e-7,100.0,Double.NaN,Double.POSITIVE_INFINITY,Double.NEGATIVE_INFINITY,-0.0,-0.05,Double.MIN_VALUE,Double.MAX_VALUE,9.95,0.049999999999999996})floats.add(List.of(Double.toString(v),String.format(Locale.US,"%.1f",v)));
    Map<String,Object> table=new LinkedHashMap<>();table.put("arguments",vectors);table.put("diagnostics",diagnostics);table.put("cases",cases);table.put("floats",floats);table.put("reflection_failures",failures);
    Files.writeString(Path.of(argv[1]),gson.toJson(table));System.err.println("fields="+diagnostics.size()+" patterns="+patterns.size()+" rows="+patterns.size()*vectors.size()+" reflection_failures="+failures+" bytes="+Files.size(Path.of(argv[1])));
  }
}
