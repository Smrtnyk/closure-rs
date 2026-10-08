// Generates Java-side fixtures for crates/resources (task key 'resources').
// Run: javac -cp closure-compiler.jar GenResourceFixtures.java && java -cp closure-compiler.jar:. GenResourceFixtures <outdir>
import com.google.javascript.jscomp.AbstractCommandLineRunner;
import com.google.javascript.jscomp.CompilerOptions;
import com.google.javascript.jscomp.SourceFile;
import com.google.javascript.jscomp.js.RuntimeJsLibManager;
import com.google.javascript.jscomp.resources.ResourceLoader;
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;

public class GenResourceFixtures {
  static String sha(String s) throws Exception {
    MessageDigest md = MessageDigest.getInstance("SHA-256");
    byte[] d = md.digest(s.getBytes(StandardCharsets.UTF_8));
    StringBuilder sb = new StringBuilder();
    for (byte b : d) sb.append(String.format("%02x", b));
    return sb.toString();
  }

  static String esc(String s) {
    StringBuilder sb = new StringBuilder();
    for (char c : s.toCharArray()) {
      if (c == '\\') sb.append("\\\\");
      else if (c == '\n') sb.append("\\n");
      else if (c == '\r') sb.append("\\r");
      else if (c == '\t') sb.append("\\t");
      else sb.append(c);
    }
    return sb.toString();
  }

  public static void main(String[] args) throws Exception {
    Path out = Paths.get(args[0]);
    Files.createDirectories(out);
    for (CompilerOptions.Environment env : CompilerOptions.Environment.values()) {
      List<SourceFile> files = AbstractCommandLineRunner.getBuiltinExterns(env);
      StringBuilder sb = new StringBuilder();
      sb.append("# index\tname\tutf8_len\tsha256  (AbstractCommandLineRunner.getBuiltinExterns(" + env + "))\n");
      int i = 0;
      for (SourceFile f : files) {
        String code = f.getCode();
        sb.append(i++).append('\t').append(f.getName()).append('\t')
            .append(code.getBytes(StandardCharsets.UTF_8).length).append('\t')
            .append(sha(code)).append('\n');
      }
      Files.writeString(out.resolve("builtin_externs_" + env + ".tsv"), sb.toString());
    }

    // RuntimeJsLibManager.FieldsTable.INSTANCE (private nested class) via reflection.
    Class<?> ft = Class.forName("com.google.javascript.jscomp.js.RuntimeJsLibManager$FieldsTable");
    Field inst = ft.getDeclaredField("INSTANCE");
    inst.setAccessible(true);
    Map<?, ?> m = (Map<?, ?>) inst.get(null);
    StringBuilder sb = new StringBuilder();
    sb.append("# fieldName\tresourceName  (RuntimeJsLibManager.FieldsTable.INSTANCE, iteration order)\n");
    for (Map.Entry<?, ?> e : m.entrySet()) {
      sb.append(e.getKey()).append('\t').append(e.getValue()).append('\n');
    }
    Files.writeString(out.resolve("fields_table.tsv"), sb.toString());

    // ResourceLoader.loadTextResource on the class/path pairs the compiler uses.
    String[][] loads = {
      {"com.google.javascript.jscomp.RewritePolyfills", "js/polyfills.txt"},
      {"com.google.javascript.jscomp.IsolatePolyfills", "js/polyfills.txt"},
      {"com.google.javascript.jscomp.RemoveUnusedCode", "js/polyfills.txt"},
      {"com.google.javascript.jscomp.js.RuntimeJsLibManager", "transpilation_libs.txt"},
      {"com.google.javascript.jscomp.Compiler", "js/base.js"},
      {"com.google.javascript.jscomp.Compiler", "js/es6/set.js"},
      {"com.google.javascript.jscomp.Compiler", "js/util/global.js"},
      {"com.google.javascript.jscomp.Compiler", "/com/google/javascript/jscomp/js/es6_runtime.js"},
    };
    sb = new StringBuilder();
    sb.append("# class\tpath\tutf8_len\tsha256  (ResourceLoader.loadTextResource)\n");
    for (String[] l : loads) {
      String s = ResourceLoader.loadTextResource(Class.forName(l[0]), l[1]);
      sb.append(l[0]).append('\t').append(l[1]).append('\t')
          .append(s.getBytes(StandardCharsets.UTF_8).length).append('\t').append(sha(s)).append('\n');
    }
    Files.writeString(out.resolve("load_text_resource.tsv"), sb.toString());

    // ResourceLoader.loadPropertiesMap on the text resources the compiler loads.
    String[][] props = {
      {"com.google.javascript.jscomp.RewritePolyfills", "js/polyfills.txt"},
      {"com.google.javascript.jscomp.js.RuntimeJsLibManager", "transpilation_libs.txt"},
      {"com.google.javascript.jscomp.Compiler", "js/base.js"},
    };
    sb = new StringBuilder();
    sb.append("# class\tpath\tsize\tsha256 of entries joined as key=>value\\n, or EXC:class:message  (ResourceLoader.loadPropertiesMap)\n");
    for (String[] l : props) {
      sb.append(l[0]).append('\t').append(l[1]).append('\t');
      try {
        Map<String, String> r = ResourceLoader.loadPropertiesMap(Class.forName(l[0]), l[1]);
        StringBuilder j = new StringBuilder();
        for (Map.Entry<String, String> e : r.entrySet()) j.append(e.getKey()).append("=>").append(e.getValue()).append('\n');
        sb.append(r.size()).append('\t').append(sha(j.toString()));
      } catch (RuntimeException e) {
        sb.append("EXC:").append(e.getClass().getName()).append(':').append(esc(String.valueOf(e.getMessage())));
      }
      sb.append('\n');
    }
    Files.writeString(out.resolve("load_properties_map.tsv"), sb.toString());

    // ResourceLoader.resourceExists
    String[][] exists = {
      {"com.google.javascript.jscomp.Compiler", "js/base.js"},
      {"com.google.javascript.jscomp.Compiler", "js/nonexistent.js"},
      {"com.google.javascript.jscomp.Compiler", "/runtime_libs.typedast"},
      {"com.google.javascript.jscomp.Compiler", "runtime_libs.typedast"},
      {"com.google.javascript.jscomp.AbstractCommandLineRunner", "/externs.zip"},
      {"com.google.javascript.jscomp.AbstractCommandLineRunner", "externs.zip"},
      {"com.google.javascript.jscomp.js.RuntimeJsLibManager", "transpilation_libs.txt"},
      {"com.google.javascript.jscomp.js.RuntimeJsLibManager", "es6/set.js"},
      {"com.google.javascript.jscomp.js.RuntimeJsLibManager", "js/es6/set.js"},
      {"com.google.javascript.jscomp.Compiler", "js/build_metadata_table.js"},
      {"com.google.javascript.jscomp.Compiler", "js/es6/"},
      {"com.google.javascript.jscomp.Compiler", "js/es6"},
    };
    sb = new StringBuilder();
    sb.append("# class\tpath\texists  (ResourceLoader.resourceExists)\n");
    for (String[] l : exists) {
      sb.append(l[0]).append('\t').append(l[1]).append('\t')
          .append(ResourceLoader.resourceExists(Class.forName(l[0]), l[1])).append('\n');
    }
    Files.writeString(out.resolve("resource_exists.tsv"), sb.toString());

    // ResourceLoader.loadTextResource on a missing resource: exception class + message.
    sb = new StringBuilder();
    try {
      ResourceLoader.loadTextResource(Class.forName("com.google.javascript.jscomp.Compiler"), "js/nonexistent.js");
      sb.append("no exception\n");
    } catch (RuntimeException e) {
      sb.append(e.getClass().getName()).append('\t').append(e.getMessage()).append('\n');
    }
    Files.writeString(out.resolve("load_missing.tsv"), sb.toString());

    // PropertiesParser.parse (package-private) via reflection on a set of inputs.
    Class<?> pp = Class.forName("com.google.javascript.jscomp.resources.PropertiesParser");
    Method parse = pp.getDeclaredMethod("parse", String.class);
    parse.setAccessible(true);
    String[] inputs = {
      "",
      "a=b",
      "a = b",
      "  a  =   b  ",
      "a:b",
      "a b",
      "a   b c",
      "novalue",
      "# comment\n! bang\n\nk=v",
      "k=v1 \\\n   v2\\\n v3",
      "k=v\\",
      "k=line1\r\nj=line2\r\n",
      "a=1\nb=2\nc=3",
      "a=b=c",
      "a:b=c",
      "a=b:c",
      "x=\ty",
      "\tk=v",
      " # notcomment=1",
      "k=\\",
      "k = trailing   ",
      "jsdoc.annotations =\\\n    a,\\\n    b,\\\n    c",
      "\n",
      "\n\n",
      "\nk=v",
      "k=v\n\n\n",
      "a\r\r\nb=c",
      "k=v\r",
      "a=1\na=2",
      "a=1\nb=2\na=3",
      "k=x\\\n# c\n",
      "=v",
      ":",
      " k",
      "k\u00e9=\u00fc",
      // Two duplicate pairs among keys with equal String.hashCode ({Aa,BB}^4): with few
      // colliding keys Guava reports the pair found scanning backwards; with enough of them a
      // hash bucket overflows first and JdkBackedImmutableMap reports the pair found forwards.
      "AaAaAaAa=1\nAaAaAaBB=1\nAaAaBBAa=x\nAaAaBBBB=x\nAaAaAaAa=2\nAaAaAaBB=2",
      "AaAaAaAa=1\nAaAaAaBB=1\nAaAaBBAa=x\nAaAaBBBB=x\nAaBBAaAa=x\nAaBBAaBB=x\nAaBBBBAa=x"
          + "\nAaBBBBBB=x\nBBAaAaAa=x\nBBAaAaBB=x\nBBAaBBAa=x\nBBAaBBBB=x\nAaAaAaAa=2\nAaAaAaBB=2",
      "AaAaAaAa=1\nAaAaAaBB=2\nAaAaBBAa=3\nAaAaBBBB=4\nAaBBAaAa=5\nAaBBAaBB=6\nAaBBBBAa=7"
          + "\nAaBBBBBB=8\nBBAaAaAa=9\nBBAaAaBB=10\nBBAaBBAa=11\nBBAaBBBB=12",
    };
    sb = new StringBuilder();
    sb.append("# input\tresult (key=>value pairs joined by \\u0001, or EXC:class:message)\n");
    for (String in : inputs) {
      sb.append(esc(in)).append('\t');
      try {
        Map<?, ?> r = (Map<?, ?>) parse.invoke(null, in);
        StringJoiner j = new StringJoiner("\u0001");
        for (Map.Entry<?, ?> e : r.entrySet()) j.add(esc(e.getKey() + "=>" + e.getValue()));
        sb.append(j.toString());
      } catch (InvocationTargetException e) {
        sb.append("EXC:").append(e.getCause().getClass().getName()).append(':').append(esc(String.valueOf(e.getCause().getMessage())));
      }
      sb.append('\n');
    }
    Files.writeString(out.resolve("properties_parser.tsv"), sb.toString(), StandardCharsets.UTF_8);
  }
}
