package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableList;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import com.google.javascript.jscomp.parsing.Config;
import com.google.javascript.jscomp.parsing.ParserRunner;
import com.google.javascript.jscomp.parsing.parser.trees.Comment;
import com.google.javascript.jscomp.parsing.parser.util.SourcePosition;
import com.google.javascript.jscomp.parsing.parser.util.SourceRange;
import com.google.javascript.rhino.ErrorReporter;
import com.google.javascript.jscomp.parsing.parser.FeatureSet;
import com.google.javascript.rhino.InputId;
import com.google.javascript.rhino.JSDocInfo;
import com.google.javascript.rhino.JSTypeExpression;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.OracleNodeAccess;
import com.google.javascript.rhino.StaticSourceFile;
import java.io.ByteArrayInputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** parse_dump: parse one file through CompilerInput -> ParserRunner -> IRFactory; dump as JSON. */
final class ParseDump {
  private ParseDump() {}

  static JsonObject run(JsonObject req) throws Exception {
    String name;
    String code;
    if (req.has("content")) {
      name = req.has("name") ? req.get("name").getAsString() : "input.js";
      code = req.get("content").getAsString();
    } else {
      name = req.get("path").getAsString();
      code = Files.readString(Path.of(name), StandardCharsets.UTF_8);
      if (req.has("name")) {
        name = req.get("name").getAsString();
      }
    }
    List<String> argList = new ArrayList<>();
    if (req.has("language_in")) {
      argList.add("--language_in=" + req.get("language_in").getAsString());
    }
    if (req.has("args")) {
      argList.addAll(Arrays.asList(OracleWorker.argsOf(req)));
    }
    PrintStream nul = new PrintStream(OutputStream.nullOutputStream());
    OracleRunner runner =
        new OracleRunner(
            argList.toArray(new String[0]), new ByteArrayInputStream(new byte[0]), nul, nul);
    if (runner.hasErrors()) {
      throw new IllegalArgumentException("bad flags: " + argList);
    }
    // createOptions() + setRunOptions(): the same two steps AbstractCommandLineRunner.doRun
    // performs before parsing (warning guards, --hide_warnings_for, --print_tree,
    // --parse_inline_source_maps, ...).
    CompilerOptions options = runner.createOptionsForParseDump();
    if (req.has("jsdoc_parsing")) {
      options.setParseJsDocDocumentation(
          Config.JsDocParsing.valueOf(req.get("jsdoc_parsing").getAsString()));
    }
    boolean extern = req.has("kind") && req.get("kind").getAsString().equals("extern");
    SourceFile sf =
        SourceFile.builder()
            .withPath(name)
            .withKind(extern ? StaticSourceFile.SourceKind.EXTERN : StaticSourceFile.SourceKind.STRONG)
            .withContent(code)
            .build();
    RecordingCompiler compiler = new RecordingCompiler(nul);
    compiler.installLevelProbe(options);
    compiler.init(ImmutableList.of(), ImmutableList.of(), options);
    CompilerInput input = new CompilerInput(sf, extern);
    Config cfg =
        compiler.getParserConfig(
            extern ? AbstractCompiler.ConfigContext.EXTERNS : AbstractCompiler.ConfigContext.DEFAULT);
    // Parse on the compiler's own thread (CompilerExecutor, COMPILER_STACK_SIZE), exactly like
    // Compiler.parseInputs does during a real compile.
    Node root = compiler.runInCompilerThread(() -> input.getAstRoot(compiler));
    // A second, side-effect-free parse (diagnostics discarded) to read the parser outputs that
    // CompilerInput.JsAst consumes but does not keep: sourceMapURL, comments, statement ranges.
    ParserRunner.ParseResult pr =
        compiler.runInCompilerThread(
            () -> ParserRunner.parse(sf, sf.getCode(), cfg, SILENT_REPORTER));

    JsonObject o = new JsonObject();
    o.addProperty("schema", "closure-rs/parse_dump/2");
    o.addProperty("source_name", name);
    o.addProperty("language_in", String.valueOf(options.getLanguageIn()));
    o.addProperty("parser_config", String.valueOf(cfg));
    o.add("reports", compiler.reports);
    o.add("errors", diags(compiler.getErrors()));
    o.add("warnings", diags(compiler.getWarnings()));
    FeatureSet fs = input.getFeatures(compiler);
    o.add("features", fs == null ? JsonNull.INSTANCE : featureSet(fs));
    o.addProperty("source_map_url", pr.sourceMapURL);
    JsonArray comments = new JsonArray();
    for (Comment c : pr.comments) {
      JsonObject co = new JsonObject();
      co.addProperty("type", c.type.name());
      co.add("value", jsString(c.value));
      co.add("location", range(c.location));
      comments.add(co);
    }
    o.add("comments", comments);
    JsonArray ranges = new JsonArray();
    for (SourceRange r : pr.topLevelStatementRanges) {
      ranges.add(range(r));
    }
    o.add("top_level_statement_ranges", ranges);
    o.add("ast", node(root, 0));
    return o;
  }

  static JsonObject range(SourceRange r) {
    JsonObject o = new JsonObject();
    o.add("start", pos(r.start));
    o.add("end", pos(r.end));
    return o;
  }

  static JsonObject pos(SourcePosition p) {
    JsonObject o = new JsonObject();
    o.addProperty("line", p.line);
    o.addProperty("column", p.column);
    o.addProperty("offset", p.offset);
    return o;
  }

  private static final ErrorReporter SILENT_REPORTER =
      new ErrorReporter() {
        @Override
        public void error(String message, String sourceName, int line, int lineOffset) {}

        @Override
        public void warning(String message, String sourceName, int line, int lineOffset) {}
      };

  /**
   * Records every JSError reported to the compiler, before WarningsGuards apply (raw stream),
   * together with the level the guards assign (effective_level; "OFF" when suppressed).
   */
  static final class RecordingCompiler extends Compiler {
    final JsonArray reports = new JsonArray();

    RecordingCompiler(PrintStream out) {
      super(out);
    }

    @Override
    public void report(JSError error) {
      JsonObject d = diag(error);
      effective = null;
      super.report(error);
      // Compiler.report forwards to options.getErrorHandler() only when the guarded level is on.
      d.addProperty("effective_level", effective == null ? "OFF" : String.valueOf(effective));
      reports.add(d);
    }

    CheckLevel effective;

    void installLevelProbe(CompilerOptions options) {
      ErrorHandler prev = options.getErrorHandler();
      options.setErrorHandler(
          (level, error) -> {
            effective = level;
            if (prev != null) {
              prev.report(level, error);
            }
          });
    }
  }

  static JsonObject diag(JSError e) {
    JsonObject d = new JsonObject();
    d.addProperty("key", e.type().key);
    d.addProperty("level", String.valueOf(e.defaultLevel()));
    d.addProperty("description", e.description());
    d.addProperty("source_name", e.sourceName());
    d.addProperty("lineno", e.lineno());
    d.addProperty("charno", e.charno());
    return d;
  }

  static JsonArray diags(Iterable<JSError> errs) {
    JsonArray a = new JsonArray();
    for (JSError e : errs) {
      a.add(diag(e));
    }
    return a;
  }

  static JsonArray featureSet(FeatureSet fs) {
    JsonArray a = new JsonArray();
    List<String> names = new ArrayList<>();
    for (FeatureSet.Feature f : fs.getFeatures()) {
      names.add(f.name());
    }
    names.sort(Comparator.naturalOrder());
    names.forEach(a::add);
    return a;
  }

  static JsonObject threw(Throwable e) {
    JsonObject t = new JsonObject();
    t.addProperty("$threw", e.getClass().getName());
    return t;
  }

  /**
   * depth counts nesting of non-Node values (JSDoc beans, collections) only; AST depth is
   * unbounded (the dump runs on a thread with a large stack, see OracleWorker).
   */
  static JsonObject node(Node n, int depth) throws Exception {
    JsonObject o = new JsonObject();
    o.addProperty("token", n.getToken().name());
    o.addProperty("node_class", n.getClass().getSimpleName());
    o.addProperty("lineno", n.getLineno());
    o.addProperty("charno", n.getCharno());
    // source_offset is derived: SourceFile.getLineOffset counts only '\n', so it throws when the
    // parser counted other line terminators (CR, U+2028, U+2029). Emitted as {"$threw": class}.
    try {
      o.addProperty("source_offset", n.getSourceOffset());
    } catch (RuntimeException e) {
      o.add("source_offset", threw(e));
    }
    try {
      o.addProperty("source_position", n.getSourcePosition());
    } catch (RuntimeException e) {
      o.add("source_position", threw(e));
    }
    o.addProperty("length", n.getLength());
    o.addProperty("source_name", n.getSourceFileName());
    switch (n.getClass().getSimpleName()) {
      case "NumberNode" -> {
        double d = n.getDouble();
        o.addProperty("double_bits", String.format("0x%016x", Double.doubleToRawLongBits(d)));
        o.addProperty("double_java", Double.toString(d));
        String printed;
        try {
          printed = new CodePrinter.Builder(n).build();
        } catch (RuntimeException e) {
          printed = null;
        }
        o.addProperty("double_closure", printed);
      }
      case "BigIntNode" -> {
        BigInteger b = n.getBigInt();
        o.addProperty("bigint", b.toString());
      }
      case "StringNode" -> o.add("string", jsString(n.getString()));
      case "TemplateLiteralSubstringNode" -> {
        o.add("cooked", n.getCookedString() == null ? JsonNull.INSTANCE : jsString(n.getCookedString()));
        o.add("raw", jsString(n.getRawString()));
      }
      default -> {}
    }
    JsonObject props = new JsonObject();
    for (OracleNodeAccess.PropValue p : OracleNodeAccess.presentProps(n)) {
      if (p.kind.equals("int")) {
        props.addProperty(p.name, p.intValue);
      } else {
        props.add(p.name, value(p.objectValue, 0));
      }
    }
    o.add("props", props);
    JsonArray kids = new JsonArray();
    for (Node c = n.getFirstChild(); c != null; c = c.getNext()) {
      kids.add(node(c, 0));
    }
    o.add("children", kids);
    return o;
  }

  /**
   * JS strings are emitted as the Java String (UTF-16) plus, when it contains lone surrogates, an
   * explicit array of UTF-16 code units so nothing is lost by JSON/UTF-8 transcoding.
   */
  static JsonElement jsString(String s) {
    boolean lone = false;
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (Character.isHighSurrogate(c)) {
        if (i + 1 < s.length() && Character.isLowSurrogate(s.charAt(i + 1))) {
          i++;
        } else {
          lone = true;
        }
      } else if (Character.isLowSurrogate(c)) {
        lone = true;
      }
    }
    if (!lone) {
      return new JsonPrimitive(s);
    }
    JsonObject o = new JsonObject();
    JsonArray units = new JsonArray();
    for (int i = 0; i < s.length(); i++) {
      units.add((int) s.charAt(i));
    }
    o.add("utf16", units);
    return o;
  }

  static JsonElement value(Object v, int depth) throws Exception {
    if (v == null) {
      return JsonNull.INSTANCE;
    }
    if (depth > 200) {
      throw new IllegalStateException("value nesting too deep");
    }
    if (v instanceof String s) {
      return jsString(s);
    }
    if (v instanceof Boolean b) {
      return new JsonPrimitive(b);
    }
    if (v instanceof Number num) {
      return new JsonPrimitive(num);
    }
    if (v instanceof Enum<?> e) {
      return new JsonPrimitive(e.name());
    }
    if (v instanceof Node nd) {
      return node(nd, 0);
    }
    if (v instanceof JSTypeExpression te) {
      JsonObject o = new JsonObject();
      o.addProperty("$class", "JSTypeExpression");
      o.addProperty("source_name", te.getSourceName());
      o.add("root", node(te.getRoot(), 0));
      return o;
    }
    if (v instanceof JSDocInfo info) {
      return jsdoc(info, depth + 1);
    }
    if (v instanceof FeatureSet fs) {
      return featureSet(fs);
    }
    if (v instanceof StaticSourceFile sf) {
      JsonObject o = new JsonObject();
      o.addProperty("$class", "StaticSourceFile");
      o.addProperty("name", sf.getName());
      o.addProperty("kind", String.valueOf(sf.getKind()));
      return o;
    }
    if (v instanceof InputId id) {
      JsonObject o = new JsonObject();
      o.addProperty("$class", "InputId");
      o.addProperty("name", id.getIdName());
      return o;
    }
    if (v instanceof Map<?, ?> m) {
      JsonArray a = new JsonArray();
      for (Map.Entry<?, ?> e : m.entrySet()) {
        JsonArray pair = new JsonArray();
        pair.add(value(e.getKey(), depth + 1));
        pair.add(value(e.getValue(), depth + 1));
        a.add(pair);
      }
      JsonObject o = new JsonObject();
      o.add("$map", a);
      return o;
    }
    if (v instanceof Collection<?> c) {
      JsonArray a = new JsonArray();
      for (Object x : c) {
        a.add(value(x, depth + 1));
      }
      return a;
    }
    if (v.getClass().getName().startsWith("com.google.javascript.rhino.")) {
      return beanJson(v, depth + 1);
    }
    JsonObject o = new JsonObject();
    o.addProperty("$class", v.getClass().getName());
    o.addProperty("$toString", String.valueOf(v));
    return o;
  }

  private static final Set<String> SKIP =
      Set.of("toString", "toStringVerbose", "clone", "toBuilder", "hashCode", "getClass");

  /** Every public, non-static, no-arg, non-void getter of JSDocInfo, sorted by name. */
  static JsonObject jsdoc(JSDocInfo info, int depth) throws Exception {
    JsonObject o = beanJson(info, depth);
    // Parameterised getters, for each declared parameter, in declaration order.
    JsonArray params = new JsonArray();
    for (int i = 0; i < info.getParameterCount(); i++) {
      String pn = info.getParameterNameAt(i);
      JsonObject p = new JsonObject();
      p.addProperty("name", pn);
      p.add("type", value(info.getParameterType(pn), depth + 1));
      p.add("description", value(info.getDescriptionForParameter(pn), depth + 1));
      p.addProperty("has_parameter_type", info.hasParameterType(pn));
      params.add(p);
    }
    o.add("$parameters", params);
    return o;
  }

  static JsonObject beanJson(Object v, int depth) throws Exception {
    JsonObject o = new JsonObject();
    o.addProperty("$class", v.getClass().getName());
    List<Method> ms = new ArrayList<>();
    for (Method m : v.getClass().getMethods()) {
      if (m.getParameterCount() != 0
          || Modifier.isStatic(m.getModifiers())
          || m.getReturnType() == void.class
          || SKIP.contains(m.getName())
          || m.getDeclaringClass() == Object.class) {
        continue;
      }
      ms.add(m);
    }
    ms.sort(Comparator.comparing(Method::getName));
    for (Method m : ms) {
      Object r;
      try {
        m.setAccessible(true);
        r = m.invoke(v);
      } catch (java.lang.reflect.InvocationTargetException e) {
        JsonObject t = new JsonObject();
        t.addProperty("$threw", e.getCause().getClass().getName());
        o.add(m.getName(), t);
        continue;
      }
      o.add(m.getName(), value(r, depth + 1));
    }
    return o;
  }
}
