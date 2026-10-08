/*
 * closure-rs unit-corpus replay (docs/PORTING.md §4.2, gate 0.2(a)): replays one CompilerTestCase
 * record (kind=compiler_test_case, api=testInternal). Not an original closure *Test class: it is
 * built only from the harness base class (compiler_tests_lib) and oracle/replay sources.
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableList;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Restores harness state + options from a record, builds the processor from the descriptor. */
public final class ReplayCompilerTest extends CompilerTestCase {
  private static final Set<String> SKIP_FIELDS =
      Set.of("lastCompiler", "setUpRan", "__currentRec", "__recCompiler");

  private final JsonObject record;
  private final JsonObject descCase;
  private final ReplayDsl.Ctx ctx = new ReplayDsl.Ctx();

  public ReplayCompilerTest(JsonObject record, JsonObject descCase) {
    this.record = record;
    this.descCase = descCase;
    ctx.record = record;
  }

  private JsonObject effective() {
    return record.getAsJsonObject("harness").getAsJsonObject("effective");
  }

  /**
   * Gate 0.2 no-op sanity run (--noop-processor): the whole processor is replaced by a pass that
   * does nothing, without evaluating the descriptor. A record that still passes is vacuous unless
   * its post-call state is checked.
   */
  static boolean noopProcessor = false;

  @Override
  protected CompilerPass getProcessor(Compiler compiler) {
    ctx.compiler = compiler;
    if (noopProcessor) {
      return (externs, root) -> {};
    }
    CompilerPass p = (CompilerPass) ReplayDsl.eval(descCase.get("processor"), ctx);
    if (firstProcessor == null) {
      firstProcessor = p;
    }
    if (dumpProcessor && replayedProcessor == null) {
      // Processor identity: dump what the descriptor built exactly as the recorder
      // dumped what getProcessor returned (first repetition, right before it runs, depth 2).
      JsonObject d = new JsonObject();
      try {
        d.addProperty("class", UnitRecorder.className(p.getClass()));
        d.add("fields", UnitRecorder.refFields(p, p.getClass(), Object.class, "processor", 2));
      } catch (Throwable t) {
        d.addProperty("error", t.toString());
      }
      replayedProcessor = d;
    }
    return p;
  }

  /** The processor of the first repetition (post-call snapshot producer root, FORMAT.md). */
  private Object firstProcessor;

  /** --proc-out: dump of the first processor the descriptor built for the current record. */
  static boolean dumpProcessor = false;

  static JsonObject replayedProcessor = null;

  /** Recorded options plus the case's options override; then getOptions() harness side effects. */
  @Override
  protected CompilerOptions getOptions() {
    CompilerOptions o =
        ReplayOptions.build(record.getAsJsonObject("options"), descCase.getAsJsonObject("options"), ctx);
    JsonElement after = record.getAsJsonObject("harness").get("fieldsAfterGetOptions");
    if (after != null && after.isJsonObject()) {
      for (Map.Entry<String, JsonElement> e : after.getAsJsonObject().entrySet()) {
        if (!SKIP_FIELDS.contains(e.getKey())) {
          ReplayValues.setField(this, CompilerTestCase.class, Object.class, e.getKey(), e.getValue());
        }
      }
    }
    return o;
  }

  @Override
  protected int getNumRepetitions() {
    return effective().get("getNumRepetitions").getAsInt();
  }

  @Override
  public String getName() {
    JsonElement n = effective().get("getName");
    return n == null || n.isJsonNull() ? super.getName() : n.getAsString();
  }

  /** The first compiler createCompiler returned during the call (testExternChanges never sets lastCompiler). */
  private Compiler created;

  @Override
  protected Compiler createCompiler() {
    JsonElement c = effective().get("createCompiler");
    if (c == null || c.isJsonNull() || c.getAsString().equals(Compiler.class.getName())) {
      Compiler compiler = super.createCompiler();
      if (created == null) {
        created = compiler; // the first compiler of the call is the one the recorder observes
      }
      Compiler saved = ctx.compiler;
      ctx.compiler = compiler;
      try {
        ReplayDsl.evalAll(descCase.get("compilerSetup"), ctx);
      } finally {
        ctx.compiler = saved;
      }
      return compiler;
    }
    throw new ReplayValues.Undecodable("custom compiler class " + c.getAsString());
  }

  /** Restores the reflective harness-field dump onto this instance (after setUp). */
  void restoreHarness() throws Exception {
    setUp();
    JsonObject fields = record.getAsJsonObject("harness").getAsJsonObject("fields");
    for (Map.Entry<String, JsonElement> e : fields.entrySet()) {
      if (SKIP_FIELDS.contains(e.getKey())) {
        continue;
      }
      ReplayValues.setField(this, CompilerTestCase.class, Object.class, e.getKey(), e.getValue());
    }
  }

  /** Runs the record; returns the observed outcome in record form. */
  JsonObject run() throws Exception {
    restoreHarness();
    JsonObject in = record.getAsJsonObject("inputs");
    Externs externs =
        in.get("externs") == null || in.get("externs").isJsonNull()
            ? null
            : externs(ReplayValues.sourceFiles(in.get("externs")));
    Sources srcs =
        in.has("sources")
            ? srcs(ReplayValues.sourceFiles(in.get("sources")))
            : srcs(ReplayValues.chunks(in.get("chunks")).toArray(new JSChunk[0]));
    JsonObject ex = record.getAsJsonObject("expected");
    JsonElement out = ex.get("output");
    Expected expected;
    if (out == null || out.isJsonNull()) {
      expected = null;
    } else if (out.isJsonPrimitive() && out.getAsString().equals("SAME")) {
      Field f = CompilerTestCase.class.getDeclaredField("EXPECTED_SAME");
      f.setAccessible(true);
      expected = (Expected) f.get(null);
    } else {
      expected = expected(ReplayValues.sourceFiles(out));
    }
    List<Diagnostic> diags = new ArrayList<>();
    for (JsonElement de : ex.getAsJsonArray("diagnostics")) {
      JsonObject d = de.getAsJsonObject();
      DiagnosticType t = DiagnosticType.error(d.get("key").getAsString(), "{0}");
      Diagnostic diag = d.get("level").getAsString().equals("ERROR") ? error(t) : warning(t);
      if (d.has("messageMode")) {
        String mode = d.get("messageMode").getAsString();
        String msg = d.get("message").getAsString();
        if (mode.equals("contains")) {
          diag = diag.withMessageContaining(msg);
        } else if (mode.equals("exact_trimmed")) {
          diag = diag.withMessage(msg);
        } else {
          throw new ReplayValues.Undecodable("message predicate " + msg);
        }
      }
      if (d.has("line")) {
        diag = diag.withLocation(d.get("line").getAsInt(), d.get("charno").getAsInt(), d.get("length").getAsInt());
      }
      diags.add(diag);
    }
    List<Postcondition> posts = new ArrayList<>();
    JsonElement pe = descCase.get("postconditions");
    if (pe != null && !pe.isJsonNull()) {
      for (JsonElement x : pe.getAsJsonArray()) {
        posts.add((Postcondition) ReplayDsl.eval(x, ctx));
      }
    }
    String api = record.get("api").getAsString();
    created = null;
    JsonObject res = new JsonObject();
    JsonObject outcome = new JsonObject();
    try {
      switch (api) {
        case "testInternal" -> testInternal(externs, srcs, expected, diags, ImmutableList.copyOf(posts));
        case "testExternChanges" -> {
          if (!posts.isEmpty()) {
            throw new IllegalStateException("testExternChanges takes no postconditions");
          }
          testExternChanges(externs, srcs, expected, diags.toArray(new Diagnostic[0]));
        }
        default -> throw new UnsupportedOperationException("api " + api);
      }
      outcome.addProperty("status", "normal");
    } catch (UnsupportedOperationException u) {
      throw u;
    } catch (Throwable t) {
      outcome.addProperty("status", "exception");
      outcome.addProperty("exceptionClass", t.getClass().getName());
      outcome.addProperty("message", t.getMessage());
    }
    res.add("outcome", outcome);
    JsonObject obs = new JsonObject();
    // As recorded: testInternal observes lastCompiler; testExternChanges the compiler it created.
    Compiler lc = api.equals("testExternChanges") ? created : getLastCompiler();
    if (lc != null) {
      obs.add("errors", UnitRecorder.errors(lc.getErrors()));
      obs.add("warnings", UnitRecorder.errors(lc.getWarnings()));
    }
    JsonElement tfa = descCase.get("testFieldsAfter");
    if (tfa != null && !tfa.isJsonNull()) {
      // Opt-in check (DSL.md "testFieldsAfter"): the named object's fields, dumped like the
      // recorder dumps test fields, must equal the recorded testFieldsAfter.
      Object holder = ReplayDsl.eval(tfa, ctx);
      JsonObject want = expectedTestFieldsAfter(record, descCase);
      JsonObject got = new JsonObject();
      if (want != null) {
        for (String k : want.keySet()) {
          java.lang.reflect.Field f = ReplayValues.findField(holder.getClass(), Object.class, k);
          if (f == null) {
            throw new IllegalStateException("testFieldsAfter: no field " + k + " in " + holder.getClass().getName());
          }
          f.setAccessible(true);
          got.add(k, canonicalClassNames(UnitRecorder.refValue(f.get(holder), "test." + k, 2)));
        }
      }
      obs.add("testFieldsAfter", got);
      res.add("testFieldsAfterWant", want);
    }
    if (record.has("postCall")) {
      // FORMAT.md "Post-call snapshot": recomputed with the recorder's own code on the replayed
      // compiler, the replayed processor and the descriptor's helper objects (DSL "once" cache and
      // variables), class names mapped back through the descriptor's classMap.
      List<Object> roots = new java.util.ArrayList<>();
      roots.addAll(ctx.once.values());
      roots.addAll(ctx.vars.values());
      try {
        // D-017 item 1: postcondition data exactly when the original call ran postconditions.
        JsonObject pcEx = record.getAsJsonObject("expected");
        JsonElement pcs = pcEx == null ? null : pcEx.get("postconditions");
        boolean pcData = pcs != null && !pcs.isJsonNull() && pcs.getAsInt() > 0;
        res.add("postCall", canonicalClassNames(UnitRecorder.postCallSnapshot(lc, firstProcessor, roots, pcData ? lc : null)));
      } catch (RuntimeException e) {
        JsonObject err = new JsonObject();
        err.addProperty("error", e.toString());
        res.add("postCall", err);
      }
    }
    res.add("observed", obs);
    return res;
  }

  /**
   * The post-call field values a testFieldsAfter check compares (DSL.md "testFieldsAfter"): every
   * key of the record's testFieldsAfter (fields that changed during the call), plus each field
   * named in the case's "testFieldsAfterAlso" list, whose post-call value is its testFieldsAfter
   * value if it changed and otherwise its unchanged entry value from testFields. The latter makes
   * a field the test asserts on checked even when the call left it unchanged (e.g. an empty
   * lastCheckViolationMessages that stays empty).
   */
  /**
   * The replay dumps helper classes where the recording dumped test classes; the descriptor's
   * classMap (recorded FQCN -> helper FQCN) is applied in reverse to every string of the dump, so
   * the comparison sees the recorded names. Any other difference fails.
   */
  static JsonElement canonicalClassNames(JsonElement e) {
    if (ReplayValues.classMap == null || ReplayValues.classMap.isEmpty() || e == null) {
      return e;
    }
    Map<String, String> inv = new java.util.HashMap<>();
    for (Map.Entry<String, String> m : ReplayValues.classMap.entrySet()) {
      inv.put(m.getValue(), m.getKey());
    }
    return rename(e, inv);
  }

  private static JsonElement rename(JsonElement e, Map<String, String> inv) {
    if (e.isJsonPrimitive() && e.getAsJsonPrimitive().isString()) {
      String r = inv.get(e.getAsString());
      return r == null ? e : new com.google.gson.JsonPrimitive(r);
    }
    if (e.isJsonArray()) {
      com.google.gson.JsonArray a = new com.google.gson.JsonArray();
      for (JsonElement x : e.getAsJsonArray()) {
        a.add(rename(x, inv));
      }
      return a;
    }
    if (e.isJsonObject()) {
      JsonObject o = new JsonObject();
      for (Map.Entry<String, JsonElement> m : e.getAsJsonObject().entrySet()) {
        o.add(m.getKey(), rename(m.getValue(), inv));
      }
      return o;
    }
    return e;
  }

  static JsonObject expectedTestFieldsAfter(JsonObject record, JsonObject descCase) {
    JsonObject after = record.getAsJsonObject("testFieldsAfter");
    JsonObject want = new JsonObject();
    if (after != null) {
      for (String k : after.keySet()) {
        want.add(k, after.get(k));
      }
    }
    JsonElement skip = descCase.get("testFieldsAfterSkip");
    if (skip != null && skip.isJsonObject()) {
      for (Map.Entry<String, JsonElement> e : skip.getAsJsonObject().entrySet()) {
        if (e.getValue() == null || !e.getValue().isJsonPrimitive() || e.getValue().getAsString().isBlank()) {
          throw new IllegalStateException("testFieldsAfterSkip." + e.getKey() + " needs a reason");
        }
        want.remove(e.getKey());
      }
    }
    JsonElement also = descCase.get("testFieldsAfterAlso");
    if (also != null && also.isJsonArray()) {
      JsonObject before = record.getAsJsonObject("testFields");
      for (JsonElement e : also.getAsJsonArray()) {
        String k = e.getAsString();
        if (!want.has(k)) {
          JsonElement v = before == null ? null : before.get(k);
          if (v == null) {
            throw new IllegalStateException("testFieldsAfterAlso: no recorded field " + k);
          }
          want.add(k, v);
        }
      }
    }
    return want;
  }
}
