/*
 * closure-rs unit-corpus replay runner (docs/PORTING.md §4.2, gate 0.2(a)/(d)). Reads
 * corpus/unit/records/<Class>.jsonl.gz and corpus/unit/descriptors/<Class>.json, replays every
 * record through ReplayCompilerTest / ReplayIntegrationTest / ReplayTypeCheckTest, and passes a
 * record iff its observed outcome (status, exception class + message) and observed diagnostics
 * are identical to the recorded ones.
 */
package com.google.javascript.jscomp;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.zip.GZIPInputStream;
import com.google.javascript.jscomp.integration.ReplayIntegrationTest;

/** CLI: --records DIR --descriptors DIR --classes A,B [--mutate-noop Cls] [--out FILE]. */
public final class ReplayMain {
  private static final Gson GSON = new GsonBuilder().serializeNulls().disableHtmlEscaping().create();

  public static void main(String[] argv) throws Exception {
    String records = null;
    String descriptors = null;
    String classes = null;
    String out = null;
    String postOut = null;
    String procOut = null;
    String sigOut = null;
    String traceOut = null;
    for (int i = 0; i < argv.length; i++) {
      switch (argv[i]) {
        case "--records" -> records = argv[++i];
        case "--descriptors" -> descriptors = argv[++i];
        case "--classes" -> classes = argv[++i];
        case "--mutate-noop" -> ReplayDsl.mutateNoop = argv[++i];
        case "--out" -> out = argv[++i];
        case "--post-out" -> postOut = argv[++i];
        case "--proc-out" -> procOut = argv[++i];
        case "--sig-out" -> sigOut = argv[++i];
        case "--trace-out" -> traceOut = argv[++i];
        case "--noop-processor" -> ReplayCompilerTest.noopProcessor = true;
        default -> throw new IllegalArgumentException("unknown arg " + argv[i]);
      }
    }
    List<String> names = new ArrayList<>();
    for (String c : classes.split(",")) {
      if (!c.isBlank()) {
        names.add(c.trim());
      }
    }
    PrintStream sink = out == null ? null : new PrintStream(out, StandardCharsets.UTF_8);
    PrintStream postSink = postOut == null ? null : new PrintStream(postOut, StandardCharsets.UTF_8);
    PrintStream procSink = procOut == null ? null : new PrintStream(procOut, StandardCharsets.UTF_8);
    ReplayCompilerTest.dumpProcessor = procSink != null;
    PrintStream sigSink = sigOut == null ? null : new PrintStream(sigOut, StandardCharsets.UTF_8);
    PrintStream traceSink = traceOut == null ? null : new PrintStream(traceOut, StandardCharsets.UTF_8);
    if (traceSink != null && TRACE == null) {
      throw new IllegalStateException("--trace-out needs -javaagent:noop-agent.jar=trace=<scope file>");
    }
    int totalPass = 0;
    int totalFail = 0;
    PrintStream stdout = System.out;
    for (String name : names) {
      JsonObject desc = null;
      Path dp = Path.of(descriptors, name + ".json");
      if (Files.exists(dp)) {
        desc = JsonParser.parseString(Files.readString(dp)).getAsJsonObject();
      }
      java.util.Map<String, String> cm = new java.util.LinkedHashMap<>();
      if (desc != null && desc.has("classMap")) {
        for (java.util.Map.Entry<String, JsonElement> e : desc.getAsJsonObject("classMap").entrySet()) {
          cm.put(e.getKey(), e.getValue().getAsString());
        }
      }
      ReplayValues.classMap = cm;
      ReplayDsl.sigLog = sigSink == null ? null : new java.util.TreeSet<>();
      int pass = 0;
      int fail = 0;
      int idx = 0;
      try (BufferedReader r =
          new BufferedReader(
              new InputStreamReader(
                  new GZIPInputStream(new FileInputStream(records + "/" + name + ".jsonl.gz")),
                  StandardCharsets.UTF_8))) {
        String line;
        while ((line = r.readLine()) != null) {
          JsonObject rec = JsonParser.parseString(line).getAsJsonObject();
          JsonObject res;
          String why;
          ReplayCompilerTest.replayedProcessor = null;
          List<String> trace = null;
          try {
            traceBegin();
            try {
              res = replayOne(rec, desc);
            } finally {
              trace = traceEnd();
            }
            why = compare(rec, res);
          } catch (Throwable t) {
            res = null;
            why = "harness error: " + t;
          }
          // D-017 item 8 (FORMAT.md "passTrace", HARNESS.md "Pass trace"): with the agent in TRACE
          // mode the set of in-scope pass classes whose entry points ran during the replayed call
          // must equal the recorded passTrace; a difference fails the record. A record without a
          // recorded passTrace is unverified (gate (a) counts it as excluded).
          if (TRACE != null) {
            String ts;
            JsonElement wt = rec.get("passTrace");
            JsonArray got = new JsonArray();
            if (trace != null) {
              for (String t : trace) {
                got.add(t);
              }
            }
            if (wt == null || wt.isJsonNull()) {
              ts = "unverified";
            } else if (trace != null && wt.equals(got)) {
              ts = "equal";
            } else {
              ts = "mismatch";
              if (why == null) {
                why = "passTrace differs: got " + truncate(GSON.toJson(got), 600) + " want " + truncate(GSON.toJson(wt), 600);
              }
            }
            if (traceSink != null) {
              JsonObject to = new JsonObject();
              to.addProperty("index", idx);
              to.addProperty("method", rec.get("method").getAsString());
              to.addProperty("status", ts);
              if (ts.equals("mismatch")) {
                to.add("want", wt);
                to.add("got", got);
              }
              if (!ts.equals("unverified")) {
                to.add("passTrace", wt);
              }
              traceSink.println(GSON.toJson(to));
            }
          }
          if (procSink != null && rec.get("kind").getAsString().equals("compiler_test_case")) {
            JsonObject pi = ProcessorIdentity.compare(rec, ReplayCompilerTest.replayedProcessor, cm);
            pi.addProperty("index", idx);
            pi.addProperty("method", rec.get("method").getAsString());
            procSink.println(GSON.toJson(pi));
          }
          if (postSink != null) {
            JsonObject ps = postState(rec, desc);
            int[] pc = postconditionCounts(rec, desc);
            if (ps == null && pc != null) {
              ps = new JsonObject();
            }
            if (ps != null) {
              ps.addProperty("index", idx);
              ps.addProperty("method", rec.get("method").getAsString());
              if (pc != null) {
                ps.addProperty("postconditionsRecorded", pc[0]);
                ps.addProperty("postconditionsReplayed", pc[1]);
              }
              postSink.println(GSON.toJson(ps));
            }
          }
          if (why == null) {
            pass++;
          } else {
            fail++;
            if (sink != null) {
              JsonObject f = new JsonObject();
              f.addProperty("class", name);
              f.addProperty("index", idx);
              f.addProperty("method", rec.get("method").getAsString());
              f.addProperty("call", rec.get("call").getAsInt());
              f.addProperty("why", why);
              sink.println(GSON.toJson(f));
            }
          }
          idx++;
        }
      }
      totalPass += pass;
      totalFail += fail;
      stdout.println("REPLAY " + name + " records=" + idx + " pass=" + pass + " fail=" + fail);
      if (sigSink != null) {
        for (String sig : ReplayDsl.sigLog) {
          sigSink.println(name + "\t" + sig);
        }
        ReplayDsl.sigLog = null;
      }
    }
    stdout.println("REPLAY_TOTAL pass=" + totalPass + " fail=" + totalFail);
    if (sink != null) {
      sink.close();
    }
    if (postSink != null) {
      postSink.close();
    }
    if (procSink != null) {
      procSink.close();
    }
    if (sigSink != null) {
      sigSink.close();
    }
    if (traceSink != null) {
      traceSink.close();
    }
    System.exit(totalFail == 0 ? 0 : 1);
  }

  // ---- pass trace (D-017 item 8): closurers.agent.Trace of the NoopAgent in TRACE mode, by
  // reflection (the agent jar is on the system class path only when -javaagent is given). ----
  private static final java.lang.reflect.Method[] TRACE = traceMethods();

  private static java.lang.reflect.Method[] traceMethods() {
    try {
      Class<?> t = Class.forName("closurers.agent.Trace", true, ClassLoader.getSystemClassLoader());
      if (!(Boolean) t.getMethod("active").invoke(null)) {
        return null;
      }
      return new java.lang.reflect.Method[] {t.getMethod("begin"), t.getMethod("end")};
    } catch (ReflectiveOperationException | LinkageError e) {
      return null;
    }
  }

  private static void traceBegin() throws ReflectiveOperationException {
    if (TRACE != null) {
      TRACE[0].invoke(null);
    }
  }

  @SuppressWarnings("unchecked")
  private static List<String> traceEnd() throws ReflectiveOperationException {
    return TRACE == null ? null : (List<String>) TRACE[1].invoke(null);
  }

  static JsonObject replayOne(JsonObject rec, JsonObject desc) throws Exception {
    String kind = rec.get("kind").getAsString();
    String api = rec.get("api").getAsString();
    switch (kind) {
      case "compiler_test_case":
        if (!api.equals("testInternal") && !api.equals("testExternChanges")) {
          throw new UnsupportedOperationException("api " + api);
        }
        if (desc == null) {
          throw new IllegalStateException("no descriptor");
        }
        return new ReplayCompilerTest(rec, ReplayDsl.selectCase(desc, rec)).run();
      case "integration":
        return ReplayIntegrationTest.replay(rec, ReplayDsl.selectCaseOrNull(desc, rec));
      case "type_check":
        return ReplayTypeCheckTest.replay(rec, ReplayDsl.selectCaseOrNull(desc, rec));
      default:
        throw new UnsupportedOperationException("kind " + kind);
    }
  }

  /**
   * Post-call state classification (FORMAT.md "Post-call state"), or null when the record has no
   * post-call state to check. "checked": the selected case compares testFieldsAfter.
   * "notCaptured": the descriptor declares, with a reason, that the test's post-call assertions
   * are not represented (counted in gate (e), never green in (a)); also every type_check
   * observed_only and integration compile record, whose follow-up assertions the hook does not
   * see. "unclassified": post-call state was recorded but neither checked nor classified (fails
   * gate (a)).
   */
  static JsonObject postState(JsonObject rec, JsonObject desc) {
    String kind = rec.get("kind").getAsString();
    String api = rec.get("api").getAsString();
    JsonObject o = new JsonObject();
    if (kind.equals("type_check")) {
      JsonObject cmp = rec.getAsJsonObject("comparison");
      if (cmp != null && cmp.has("mode") && cmp.get("mode").getAsString().equals("observed_only")) {
        o.addProperty("postState", "notCaptured");
        o.addProperty("reason", "type_check observed_only: the test's assertions on the returned TypeCheckResult are not recorded");
        return o;
      }
      return null;
    }
    if (kind.equals("integration")) {
      if (api.equals("compile")) {
        // notCaptured unless gate (a) resolves it (FORMAT.md "Post-call state"): the method has no
        // post-call assertion per the static scan, or its postcall entry is a verified captured
        // claim via observed.errors / observed.warnings / observed.output.
        o.addProperty("postState", "notCaptured");
        o.addProperty("integrationCompile", true);
        o.addProperty("reason", "integration compile: the test's assertions on the returned compiler are not recorded");
        return o;
      }
      return null;
    }
    JsonObject after = rec.getAsJsonObject("testFieldsAfter");
    JsonObject c = null;
    try {
      c = desc == null ? null : ReplayDsl.selectCase(desc, rec);
    } catch (RuntimeException e) {
      c = null;
    }
    // FORMAT.md "Postconditions": a record whose selected case replays fewer (or more)
    // postconditions than the original call ran has its in-call assertions not represented; it is
    // notCaptured (excluded from gate (a), counted in gate (e)), never green.
    int[] pc = postconditionCounts(rec, desc);
    if (pc != null && pc[0] != pc[1]) {
      o.addProperty("postState", "notCaptured");
      o.addProperty("reason", "postconditions: the original call ran " + pc[0] + ", the selected case replays " + pc[1]);
      return o;
    }
    // "checked" means replay compares a non-empty field set: the fields
    // ReplayCompilerTest.expectedTestFieldsAfter compares (record testFieldsAfter keys, plus
    // testFieldsAfterAlso, minus testFieldsAfterSkip). The set is written as comparedFields so gate
    // (a) checks testFieldsAfter.<f> claims against what replay really compared.
    boolean optIn = c != null && c.has("testFieldsAfter") && !c.get("testFieldsAfter").isJsonNull();
    if (optIn) {
      JsonArray compared = new JsonArray();
      String err = null;
      try {
        java.util.TreeSet<String> ks = new java.util.TreeSet<>(ReplayCompilerTest.expectedTestFieldsAfter(rec, c).keySet());
        for (String k : ks) {
          compared.add(k);
        }
      } catch (RuntimeException e) {
        err = String.valueOf(e);
      }
      if (err == null && compared.size() > 0) {
        o.addProperty("postState", "checked");
        o.add("comparedFields", compared);
        return o;
      }
      if (after != null && after.size() > 0) {
        o.addProperty("postState", "unclassified");
        o.addProperty("reason", "the case opts in to testFieldsAfter but compares no field"
            + (err == null ? "" : " (" + err + ")") + "; testFieldsAfter changed " + after.keySet());
        return o;
      }
      return null;
    }
    if (after == null || after.size() == 0) {
      return null;
    }
    JsonElement why = c == null ? null : c.get("testFieldsAfterUnchecked");
    if ((why == null || why.isJsonNull()) && desc != null) {
      why = desc.get("testFieldsAfterUnchecked");
    }
    if (why != null && !why.isJsonNull() && !why.getAsString().isBlank()) {
      o.addProperty("postState", "notCaptured");
      o.addProperty("reason", why.getAsString());
    } else {
      o.addProperty("postState", "unclassified");
      o.addProperty("reason", "testFieldsAfter changed " + after.keySet() + " but the case neither checks it nor gives testFieldsAfterUnchecked");
    }
    return o;
  }

  /**
   * For a compiler_test_case record: {expected.postconditions, number of postcondition Exprs in
   * the selected case}; the replayed count is -1 when no case can be selected. Null otherwise.
   */
  static int[] postconditionCounts(JsonObject rec, JsonObject desc) {
    if (!rec.get("kind").getAsString().equals("compiler_test_case")) {
      return null;
    }
    JsonObject e = rec.getAsJsonObject("expected");
    JsonElement n = e == null ? null : e.get("postconditions");
    int recorded = n == null || n.isJsonNull() ? 0 : n.getAsInt();
    int replayed;
    try {
      JsonObject c = desc == null ? null : ReplayDsl.selectCase(desc, rec);
      JsonElement pe = c == null ? null : c.get("postconditions");
      replayed = c == null ? -1 : (pe == null || pe.isJsonNull() ? 0 : pe.getAsJsonArray().size());
    } catch (RuntimeException ex) {
      replayed = -1;
    }
    return new int[] {recorded, replayed};
  }

  private static String truncate(String s, int n) {
    return s.length() <= n ? s : s.substring(0, n) + "...<truncated " + (s.length() - n) + " chars>";
  }

  /** Returns null when identical, else a reason. */
  static String compare(JsonObject rec, JsonObject res) {
    JsonObject want = rec.getAsJsonObject("outcome");
    JsonObject got = res.getAsJsonObject("outcome");
    String ws = want.get("status").getAsString();
    String gs = got.get("status").getAsString();
    if (!ws.equals(gs)) {
      return "outcome " + gs + " (" + str(got.get("exceptionClass")) + ": " + str(got.get("message")) + ") != recorded " + ws
          + " (" + str(want.get("exceptionClass")) + ": " + str(want.get("message")) + ")";
    }
    if (ws.equals("exception")) {
      if (!str(want.get("exceptionClass")).equals(str(got.get("exceptionClass")))) {
        return "exception class " + str(got.get("exceptionClass")) + " != " + str(want.get("exceptionClass"));
      }
      String gm = got.get("message").isJsonNull() ? null : truncate(got.get("message").getAsString(), 4000);
      String wm = want.get("message") == null || want.get("message").isJsonNull() ? null : want.get("message").getAsString();
      if (!java.util.Objects.equals(gm, wm)) {
        return "exception message differs: got <" + gm + "> want <" + wm + ">";
      }
    }
    if (rec.has("postCall")) {
      // FORMAT.md "Post-call snapshot": every recorded key must be reproduced exactly (Java JSON
      // equality; a non-Java harness uses the neutral equality). Keys the replay produces but the
      // record lacks also fail: the snapshot is a fixed set, read the same way on both sides.
      JsonElement wp = rec.get("postCall");
      JsonElement gp = res.get("postCall");
      if (gp == null || !wp.equals(gp)) {
        return "postCall differs: got " + truncate(String.valueOf(gp), 600) + " want " + truncate(GSON.toJson(wp), 600);
      }
    }
    JsonObject wo = rec.getAsJsonObject("observed");
    JsonObject go = res.getAsJsonObject("observed");
    if (go != null && go.has("testFieldsAfter")) {
      // The case's expected post-call values (record testFieldsAfter, plus testFieldsAfterAlso
      // fields, minus testFieldsAfterSkip fields), as computed by ReplayCompilerTest.
      JsonElement wantTfa = res.get("testFieldsAfterWant");
      if (wantTfa == null || !wantTfa.equals(go.get("testFieldsAfter"))) {
        return "testFieldsAfter differs: got " + truncate(GSON.toJson(go.get("testFieldsAfter")), 600) + " want "
            + truncate(String.valueOf(wantTfa), 600);
      }
    }
    if (wo != null && go != null) {
      for (String k : wo.keySet()) {
        if (k.equals("compilerClass")) {
          continue;
        }
        JsonElement a = wo.get(k);
        JsonElement b = go.get(k);
        if (b == null) {
          return "observed." + k + " missing in replay";
        }
        if (!a.equals(b)) {
          return "observed." + k + " differs: got " + truncate(GSON.toJson(b), 600) + " want " + truncate(GSON.toJson(a), 600);
        }
      }
    }
    return null;
  }

  private static String str(JsonElement e) {
    return e == null || e.isJsonNull() ? "null" : e.getAsString();
  }
}
