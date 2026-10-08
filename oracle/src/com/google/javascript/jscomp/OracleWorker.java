package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.PrintStream;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.logging.Level;
import java.util.logging.Logger;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Executes one oracle request inside the classloader that holds the reference compiler. The
 * boundary with the launcher is a single static method taking and returning JSON text, so the
 * worker can be loaded into a fresh classloader per request.
 */
public final class OracleWorker {
  private OracleWorker() {}

  private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();

  /** Stack for parse_dump's recursive AST dump and its JSON serialization (reserved lazily). */
  static final long DUMP_STACK_SIZE = 1L << 30;

  /** Entry point used by the launcher (reflectively). */
  public static String handle(String requestJson) {
    if (requestJson.contains("\"parse_dump\"")) {
      JsonObject req = JsonParser.parseString(requestJson).getAsJsonObject();
      if (req.has("op") && req.get("op").getAsString().equals("parse_dump")) {
        String[] out = new String[1];
        Throwable[] thrown = new Throwable[1];
        Thread t =
            new Thread(
                null,
                () -> {
                  try {
                    out[0] = handleOnThisThread(requestJson);
                  } catch (Throwable e) {
                    thrown[0] = e;
                  }
                },
                "oracle-parse-dump",
                DUMP_STACK_SIZE);
        t.setContextClassLoader(Thread.currentThread().getContextClassLoader());
        t.start();
        try {
          t.join();
        } catch (InterruptedException e) {
          Thread.currentThread().interrupt();
          throw new IllegalStateException(e);
        }
        if (thrown[0] != null) {
          throw new IllegalStateException(thrown[0]);
        }
        return out[0];
      }
    }
    return handleOnThisThread(requestJson);
  }

  static String handleOnThisThread(String requestJson) {
    JsonObject req = JsonParser.parseString(requestJson).getAsJsonObject();
    JsonObject resp;
    try {
      String op = req.get("op").getAsString();
      resp =
          switch (op) {
            case "compile" -> compileOp(req, false);
            case "compile_with_pass_dumps" -> compileOp(req, true);
            case "checks_to_typedast" -> checksToTypedAst(req);
            case "optimize_from_typedast" -> optimizeFromTypedAst(req);
            case "parse_dump" -> ParseDump.run(req);
            case "ping" -> new JsonObject();
            default -> throw new IllegalArgumentException("unknown op: " + op);
          };
      resp.addProperty("ok", true);
    } catch (Throwable t) {
      resp = new JsonObject();
      resp.addProperty("ok", false);
      ByteArrayOutputStream b = new ByteArrayOutputStream();
      t.printStackTrace(new PrintStream(b, true, StandardCharsets.UTF_8));
      resp.addProperty("error", b.toString(StandardCharsets.UTF_8));
    }
    if (req.has("id")) {
      resp.add("id", req.get("id"));
    }
    return GSON.toJson(resp);
  }

  /** Result of one emulated `java -jar closure-compiler.jar argv` run. */
  static final class CliResult {
    byte[] stdout;
    byte[] stderr;
    int exitCode;
    List<String> outputFiles = new ArrayList<>();
    List<String> inputOrder;
    Object inputs;
    Object chunks;
  }

  static Charset streamCharset(String prop) {
    String enc = System.getProperty(prop);
    if (enc == null) {
      enc = System.getProperty("native.encoding", "UTF-8");
    }
    try {
      return Charset.forName(enc);
    } catch (RuntimeException e) {
      return Charset.defaultCharset();
    }
  }

  /**
   * Replicates CommandLineRunner.main(args) exactly (see CommandLineRunner.java:2249), with
   * System.in/out/err redirected and System.exit replaced by an exit-code receiver that applies
   * the same byte conversion as SystemExitCodeReceiver.
   */
  static CliResult runCli(String[] args, byte[] stdin, String typedAstListInput) {
    return runCli(args, stdin, typedAstListInput, List.of(), List.of());
  }

  static CliResult runCli(
      String[] args,
      byte[] stdin,
      String typedAstListInput,
      List<String> weakInputs,
      List<String> fillInputs) {
    PrintStream origOut = System.out;
    PrintStream origErr = System.err;
    InputStream origIn = System.in;
    ByteArrayOutputStream ob = new ByteArrayOutputStream();
    ByteArrayOutputStream eb = new ByteArrayOutputStream();
    PrintStream po = new PrintStream(ob, true, streamCharset("stdout.encoding"));
    PrintStream pe = new PrintStream(eb, true, streamCharset("stderr.encoding"));
    CliResult r = new CliResult();
    final Integer[] exit = new Integer[1];
    OracleRunner runner = null;
    try {
      System.setOut(po);
      System.setErr(pe);
      System.setIn(new ByteArrayInputStream(stdin == null ? new byte[0] : stdin));
      try {
        Logger phaseLogger = Logger.getLogger(PhaseOptimizer.class.getName());
        if (phaseLogger != null) {
          phaseLogger.setLevel(Level.OFF);
        }
        runner = new OracleRunner(args, System.in, System.out, System.err);
        runner.setExitCodeReceiver(
            code -> {
              if (exit[0] == null) {
                exit[0] = code;
              }
              return null;
            });
        if (typedAstListInput != null) {
          runner.setTypedAstListInput(typedAstListInput);
        }
        runner.weakInputs.addAll(weakInputs);
        runner.fillInputs.addAll(fillInputs);
        if (runner.shouldRunCompiler()) {
          runner.run();
        }
        if (exit[0] == null && runner.hasErrors()) {
          exit[0] = -1;
        }
        r.exitCode = exit[0] == null ? 0 : processStatus(exit[0]);
      } catch (Throwable t) {
        // Uncaught exception escaping main(): default handler output and JVM exit status 1.
        System.err.print("Exception in thread \"main\" ");
        t.printStackTrace(System.err);
        r.exitCode = exit[0] == null ? 1 : processStatus(exit[0]);
      }
      System.out.flush();
      System.err.flush();
    } finally {
      System.setOut(origOut);
      System.setErr(origErr);
      System.setIn(origIn);
    }
    r.stdout = ob.toByteArray();
    r.stderr = eb.toByteArray();
    if (runner != null) {
      r.outputFiles.addAll(runner.outputFilesOpened);
      r.inputOrder = runner.inputOrder();
      r.inputs = runner.inputs();
      r.chunks = runner.chunks();
    }
    return r;
  }

  /** Same conversion as AbstractCommandLineRunner.SystemExitCodeReceiver, then as seen by $?. */
  static int processStatus(int exitCodeValue) {
    byte b = (byte) exitCodeValue;
    if (b == 0 && exitCodeValue != 0) {
      b = (byte) -1;
    }
    return b & 0xff;
  }

  static String[] argsOf(JsonObject req) {
    JsonArray a = req.has("args") ? req.getAsJsonArray("args") : new JsonArray();
    String[] out = new String[a.size()];
    for (int i = 0; i < out.length; i++) {
      out[i] = a.get(i).getAsString();
    }
    return out;
  }

  static byte[] stdinOf(JsonObject req) {
    if (req.has("stdin_b64")) {
      return Base64.getDecoder().decode(req.get("stdin_b64").getAsString());
    }
    if (req.has("stdin")) {
      return req.get("stdin").getAsString().getBytes(StandardCharsets.UTF_8);
    }
    return new byte[0];
  }

  /** Flags (both --flag=v and --flag v forms) naming files written outside filenameToOutputStream. */
  private static final String[] EXTRA_OUTPUT_FLAGS = {
    "--typed_ast_output_file", "--json_warnings_file", "--tracer_output", "--save_state"
  };

  static Set<String> extraOutputPaths(String[] args) {
    Set<String> out = new LinkedHashSet<>();
    for (int i = 0; i < args.length; i++) {
      for (String f : EXTRA_OUTPUT_FLAGS) {
        if (args[i].startsWith(f + "=")) {
          out.add(args[i].substring(f.length() + 1));
        } else if (args[i].equals(f) && i + 1 < args.length) {
          out.add(args[i + 1]);
        }
      }
    }
    return out;
  }

  static JsonObject cliResultJson(CliResult r, String[] args, Set<String> skip) throws Exception {
    JsonObject o = new JsonObject();
    o.addProperty("exit_code", r.exitCode);
    o.addProperty("stdout_b64", Base64.getEncoder().encodeToString(r.stdout));
    o.addProperty("stderr_b64", Base64.getEncoder().encodeToString(r.stderr));
    JsonObject files = new JsonObject();
    Set<String> all = new LinkedHashSet<>(r.outputFiles);
    all.addAll(extraOutputPaths(args));
    for (String f : all) {
      if (skip != null && skip.contains(f)) {
        continue;
      }
      Path p = Path.of(f);
      if (Files.isRegularFile(p)) {
        files.addProperty(f, Base64.getEncoder().encodeToString(Files.readAllBytes(p)));
      }
    }
    o.add("output_files", files);
    o.add("input_order", r.inputOrder == null ? null : GSON.toJsonTree(r.inputOrder));
    o.add("inputs", r.inputs == null ? null : GSON.toJsonTree(r.inputs));
    o.add("chunks", r.chunks == null ? null : GSON.toJsonTree(r.chunks));
    return o;
  }

  static JsonObject compileOp(JsonObject req, boolean passDumps) throws Exception {
    String[] args = argsOf(req);
    List<String> stripped = new ArrayList<>();
    if (passDumps) {
      // Compiler.maybePrintSourceAfterEachPass -> getCurrentJsSource resets and re-fills the
      // source map; with --create_source_map active this throws "Incorrect source mappings
      // order" (exit 254). Source-map generation never changes the JS, so it is dropped here
      // and reported in "stripped_args".
      List<String> a2 = new ArrayList<>();
      for (int i = 0; i < args.length; i++) {
        if (args[i].startsWith("--create_source_map=")) {
          stripped.add(args[i]);
        } else if (args[i].equals("--create_source_map") && i + 1 < args.length) {
          stripped.add(args[i]);
          stripped.add(args[++i]);
        } else {
          a2.add(args[i]);
        }
      }
      a2.add("--print_source_after_each_pass");
      args = a2.toArray(new String[0]);
    }
    CliResult r = runCli(args, stdinOf(req), null);
    JsonObject o = cliResultJson(r, args, null);
    if (passDumps) {
      o.add("passes", parsePassDumps(new String(r.stderr, streamCharset("stderr.encoding"))));
      o.add("stripped_args", GSON.toJsonTree(stripped));
    }
    return o;
  }

  /**
   * Parses the blocks printed by Compiler.maybePrintSourceAfterEachPass (Compiler.java:1550):
   *
   * <pre>
   * "\n" ["// DEBUG: " msg "\n"] "// " passName " yields:\n"
   * "// ************************************\n" source "\n"
   * "// ************************************\n"
   * </pre>
   *
   * A block is printed only when the source differs from the previously printed source. The
   * source of a block ends at the LAST closing marker line before the next block header (or the
   * end of the stream), so sources containing the marker line are still delimited correctly
   * unless other stderr output containing the marker follows them.
   */
  static JsonArray parsePassDumps(String err) {
    String star = "// ************************************";
    Pattern header =
        Pattern.compile("(?m)^// (?:DEBUG: [^\\n]*\\n// )?(.+) yields:\\n" + Pattern.quote(star) + "\\n");
    Matcher m = header.matcher(err);
    List<int[]> spans = new ArrayList<>();
    List<String> names = new ArrayList<>();
    while (m.find()) {
      spans.add(new int[] {m.start(), m.end()});
      names.add(m.group(1));
    }
    JsonArray out = new JsonArray();
    for (int i = 0; i < spans.size(); i++) {
      int bodyStart = spans.get(i)[1];
      int limit = i + 1 < spans.size() ? spans.get(i + 1)[0] : err.length();
      String region = err.substring(bodyStart, limit);
      int close = region.lastIndexOf("\n" + star + "\n");
      String src;
      if (close >= 0) {
        src = region.substring(0, close);
      } else if (region.startsWith(star + "\n")) {
        src = ""; // empty source: "\n" printed then marker
      } else {
        src = region;
      }
      JsonObject e = new JsonObject();
      e.addProperty("pass", names.get(i));
      e.addProperty("source", src);
      out.add(e);
    }
    return out;
  }

  static String tmpPath(JsonObject req, String suffix) throws Exception {
    Path dir = Path.of("build", "oracle", "tmp");
    Files.createDirectories(dir);
    return Files.createTempFile(dir, "req", suffix).toString();
  }

  static JsonObject checksToTypedAst(JsonObject req) throws Exception {
    String[] base = argsOf(req);
    String out = tmpPath(req, ".typedast.gz");
    String[] args = new String[base.length + 2];
    System.arraycopy(base, 0, args, 0, base.length);
    args[base.length] = "--checks_only";
    args[base.length + 1] = "--typed_ast_output_file=" + out;
    Files.deleteIfExists(Path.of(out));
    CliResult r = runCli(args, stdinOf(req), null);
    JsonObject o = cliResultJson(r, args, Set.of(out));
    Path p = Path.of(out);
    if (Files.isRegularFile(p)) {
      o.addProperty("typedast_b64", Base64.getEncoder().encodeToString(Files.readAllBytes(p)));
      Files.delete(p);
    } else {
      o.add("typedast_b64", null);
    }
    return o;
  }

  static JsonObject optimizeFromTypedAst(JsonObject req) throws Exception {
    String[] args = argsOf(req);
    String in;
    boolean temp = false;
    if (req.has("typedast_path")) {
      in = req.get("typedast_path").getAsString();
    } else {
      in = tmpPath(req, ".typedast.gz");
      temp = true;
      Files.write(
          Path.of(in), Base64.getDecoder().decode(req.get("typedast_b64").getAsString()));
    }
    try {
      CliResult r =
          runCli(args, stdinOf(req), in, stringList(req, "weak_inputs"), stringList(req, "fill_inputs"));
      return cliResultJson(r, args, null);
    } finally {
      if (temp) {
        Files.deleteIfExists(Path.of(in));
      }
    }
  }

  static List<String> stringList(JsonObject req, String key) {
    List<String> out = new ArrayList<>();
    if (req.has(key)) {
      for (JsonElement e : req.getAsJsonArray(key)) {
        out.add(e.getAsString());
      }
    }
    return out;
  }

  static JsonElement toJsonTree(Object o) {
    return GSON.toJsonTree(o);
  }
}
