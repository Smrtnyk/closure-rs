package closurers.oracle;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.BufferedReader;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TimeZone;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Launcher for the closure-rs oracle. See oracle/PROTOCOL.md.
 *
 * <pre>
 *   Main compile ARGV...        one compile, behaves like `java -jar closure-compiler.jar ARGV`
 *   Main request                one JSON request on stdin, one JSON response on stdout
 *   Main server [--isolate=MODE] JSON-lines server; MODE = none (default) | request
 * </pre>
 */
public final class Main {
  private Main() {}

  static Path oracleJar;
  static Path referenceJar;
  static Path projectRoot;
  static String isolate = "none";
  static final AtomicLong counter = new AtomicLong();
  static Method sharedHandle;

  public static void main(String[] argv) throws Exception {
    oracleJar = Path.of(Main.class.getProtectionDomain().getCodeSource().getLocation().toURI());
    projectRoot = oracleJar.getParent().getParent().getParent();
    referenceJar =
        Path.of(
            System.getProperty(
                "oracle.reference_jar",
                projectRoot.resolve("build/reference/closure-compiler.jar").toString()));
    if (argv.length == 0) {
      System.err.println("usage: Main compile ARGV... | request | server [--isolate=request|none]");
      System.exit(2);
    }
    switch (argv[0]) {
      case "compile" -> {
        JsonObject req = new JsonObject();
        req.addProperty("op", "compile");
        com.google.gson.JsonArray a = new com.google.gson.JsonArray();
        for (int i = 1; i < argv.length; i++) {
          a.add(argv[i]);
        }
        req.add("args", a);
        // stdin is passed through, as java -jar would see it.
        byte[] in = System.in.readAllBytes();
        req.addProperty("stdin_b64", Base64.getEncoder().encodeToString(in));
        isolate = "none";
        JsonObject resp = JsonParser.parseString(dispatch(req.toString())).getAsJsonObject();
        if (!resp.get("ok").getAsBoolean()) {
          System.err.println(resp.get("error").getAsString());
          System.exit(3);
        }
        OutputStream out = new FileOutputStream(FileDescriptor.out);
        OutputStream err = new FileOutputStream(FileDescriptor.err);
        out.write(Base64.getDecoder().decode(resp.get("stdout_b64").getAsString()));
        out.flush();
        err.write(Base64.getDecoder().decode(resp.get("stderr_b64").getAsString()));
        err.flush();
        Runtime.getRuntime().halt(resp.get("exit_code").getAsInt());
      }
      case "request" -> {
        isolate = "none";
        String req = new String(System.in.readAllBytes(), StandardCharsets.UTF_8);
        String resp = handleRequest(req.trim());
        PrintStream proto = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        proto.println(resp);
        proto.flush();
        Runtime.getRuntime().halt(0);
      }
      case "server" -> {
        for (int i = 1; i < argv.length; i++) {
          if (argv[i].startsWith("--isolate=")) {
            isolate = argv[i].substring("--isolate=".length());
          }
        }
        serve();
      }
      default -> {
        System.err.println("unknown mode " + argv[0]);
        System.exit(2);
      }
    }
  }

  static void serve() throws Exception {
    PrintStream proto =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    BufferedReader r =
        new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8), 1 << 20);
    JsonObject hello = new JsonObject();
    hello.addProperty("ready", true);
    hello.addProperty("isolate", isolate);
    hello.addProperty("protocol", "closure-rs-oracle/1");
    hello.add("env", jvmEnv());
    proto.println(hello);
    String line;
    while ((line = r.readLine()) != null) {
      if (line.isBlank()) {
        continue;
      }
      String resp;
      try {
        resp = handleRequest(line);
      } catch (Throwable t) {
        JsonObject e = new JsonObject();
        e.addProperty("ok", false);
        // Unwrap the reflective call (dispatch) so the real failure, e.g. an
        // OutOfMemoryError, is named; the full stack trace includes every "Caused by:".
        Throwable c = t;
        while (c instanceof InvocationTargetException && c.getCause() != null) {
          c = c.getCause();
        }
        e.addProperty("error_class", c.getClass().getName());
        e.addProperty("error", stackTrace(c));
        resp = e.toString();
      }
      proto.println(resp);
      proto.flush();
    }
    Runtime.getRuntime().halt(0);
  }

  static String stackTrace(Throwable t) {
    try {
      StringWriter w = new StringWriter();
      t.printStackTrace(new PrintWriter(w, true));
      return w.toString();
    } catch (Throwable again) {
      return String.valueOf(t);
    }
  }

  /**
   * The JVM settings that the compiler's output depends on (see PROTOCOL.md, "Environment").
   * Closure formats the summary with String.format and the default FORMAT locale
   * ("%.1f%% typed"), and writes stdout/stderr in stdout.encoding/stderr.encoding, both
   * derived from LANG/LC_ALL. Clients compare these with the golden environment.
   */
  static JsonObject jvmEnv() {
    JsonObject env = new JsonObject();
    for (String k :
        new String[] {
          "user.language", "user.country", "user.variant", "native.encoding", "stdout.encoding",
          "stderr.encoding", "sun.jnu.encoding", "file.encoding", "java.version", "java.vm.version"
        }) {
      env.addProperty(k, System.getProperty(k));
    }
    env.addProperty("locale.default", Locale.getDefault().toLanguageTag());
    env.addProperty("locale.format", Locale.getDefault(Locale.Category.FORMAT).toLanguageTag());
    env.addProperty("locale.display", Locale.getDefault(Locale.Category.DISPLAY).toLanguageTag());
    env.addProperty("timezone", TimeZone.getDefault().getID());
    for (String k : new String[] {"LANG", "LC_ALL", "LC_NUMERIC", "LC_CTYPE", "TZ"}) {
      env.addProperty("getenv." + k, System.getenv(k));
    }
    return env;
  }

  /** Handles inline files (child JVM with cwd = materialized dir); else dispatches in-process. */
  static String handleRequest(String reqJson) throws Exception {
    JsonObject req = JsonParser.parseString(reqJson).getAsJsonObject();
    if (req.has("files")) {
      return runInline(req);
    }
    return dispatch(reqJson);
  }

  static String dispatch(String reqJson) throws Exception {
    if (isolate.equals("none")) {
      if (sharedHandle == null) {
        sharedHandle =
            Class.forName("com.google.javascript.jscomp.OracleWorker")
                .getMethod("handle", String.class);
      }
      return (String) sharedHandle.invoke(null, reqJson);
    }
    try (URLClassLoader cl =
        // Unnamed on purpose: a named loader prefixes every stack frame with "name//", which
        // would make printed stack traces differ from java -jar.
        new URLClassLoader(
            new URL[] {oracleJar.toUri().toURL(), referenceJar.toUri().toURL()},
            ClassLoader.getPlatformClassLoader())) {
      Thread t = Thread.currentThread();
      ClassLoader prev = t.getContextClassLoader();
      t.setContextClassLoader(cl);
      try {
        Method h =
            Class.forName("com.google.javascript.jscomp.OracleWorker", true, cl)
                .getMethod("handle", String.class);
        return (String) h.invoke(null, reqJson);
      } finally {
        t.setContextClassLoader(prev);
      }
    }
  }

  static String runInline(JsonObject req) throws Exception {
    Path dir =
        projectRoot
            .resolve("build/oracle/tmp")
            .resolve("inline-" + ProcessHandle.current().pid() + "-" + counter.incrementAndGet());
    Files.createDirectories(dir);
    for (Map.Entry<String, com.google.gson.JsonElement> e :
        req.getAsJsonObject("files").entrySet()) {
      Path rel = Path.of(e.getKey());
      if (rel.isAbsolute() || rel.normalize().startsWith("..")) {
        throw new IllegalArgumentException("inline file path must be relative: " + e.getKey());
      }
      Path p = dir.resolve(rel).normalize();
      Files.createDirectories(p.getParent());
      Files.writeString(p, e.getValue().getAsString(), StandardCharsets.UTF_8);
    }
    req.remove("files");
    List<String> cmd = new ArrayList<>();
    cmd.add(Path.of(System.getProperty("java.home"), "bin", "java").toString());
    cmd.addAll(java.lang.management.ManagementFactory.getRuntimeMXBean().getInputArguments());
    cmd.add("-cp");
    cmd.add(oracleJar + java.io.File.pathSeparator + referenceJar);
    cmd.add("closurers.oracle.Main");
    cmd.add("request");
    ProcessBuilder pb = new ProcessBuilder(cmd).directory(dir.toFile());
    pb.redirectError(ProcessBuilder.Redirect.INHERIT);
    Process p = pb.start();
    try (OutputStream o = p.getOutputStream()) {
      o.write(req.toString().getBytes(StandardCharsets.UTF_8));
    }
    String out = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8).trim();
    p.waitFor();
    JsonObject resp = JsonParser.parseString(out).getAsJsonObject();
    resp.addProperty("inline_dir", projectRoot.relativize(dir).toString());
    return resp.toString();
  }
}
