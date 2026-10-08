/*
 * closure-rs unit-corpus replay (docs/PORTING.md §4.2): evaluator for the processor-descriptor DSL
 * specified in corpus/unit/DSL.md.
 */
package com.google.javascript.jscomp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.javascript.rhino.Node;
import java.lang.reflect.Constructor;
import java.lang.reflect.Executable;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;

/** Evaluates DSL expressions (DSL.md) against a record. */
public final class ReplayDsl {
  /** Evaluation context. */
  public static final class Ctx {
    public Compiler compiler;
    public JsonObject record;
    public final Map<String, Object> fieldOverrides = new LinkedHashMap<>();
    /** Per-record cache for {"once":...}: one object shared by every getProcessor call. */
    public final Map<String, Object> once = new LinkedHashMap<>();
    /** Lexical variables bound by "let" and "lambda" (DSL.md "Variables"). */
    public Map<String, Object> vars = new LinkedHashMap<>();
    /** Bound while an options override's "then" list runs ({"options":true}). */
    public CompilerOptions options;
  }

  /** Every no-op stand-in produced by --mutate-noop (calls on them are transparent). */
  private static final Map<Object, Boolean> NOOPS = new java.util.IdentityHashMap<>();

  private ReplayDsl() {}

  /** --mutate-noop: simple or fully qualified class name whose construction becomes a no-op. */
  public static volatile String mutateNoop = null;

  /** DSL.md "No-op mutation": c, or a class that encloses c, has the mutated name. */
  private static boolean isMutated(Class<?> c) {
    String m = mutateNoop;
    if (m == null) {
      return false;
    }
    for (Class<?> k = c; k != null; k = k.getEnclosingClass()) {
      if (k.getName().equals(m) || k.getSimpleName().equals(m)) {
        return true;
      }
    }
    return false;
  }

  private static boolean nameMutated(String n) {
    String m = mutateNoop;
    if (m == null) {
      return false;
    }
    String simple = n.substring(Math.max(n.lastIndexOf('.'), n.lastIndexOf('$')) + 1);
    return n.equals(m) || simple.equals(m);
  }

  /** The pass roles a no-op stand-in can take (DSL.md "No-op mutation"). */
  private static final List<Class<?>> ROLE_INTERFACES =
      List.of(
          CompilerPass.class,
          NodeTraversal.ScopedCallback.class,
          NodeTraversal.Callback.class,
          OptimizeCalls.CallGraphCompilerPass.class);

  private static boolean hasPassRole(Class<?> c) {
    if (AbstractPeepholeOptimization.class.isAssignableFrom(c)
        || AbstractPeepholeTranspilation.class.isAssignableFrom(c)) {
      return true;
    }
    for (Class<?> i : ROLE_INTERFACES) {
      if (i.isAssignableFrom(c)) {
        return true;
      }
    }
    return false;
  }

  /** No-op stand-in with the same role(s) as c. */
  private static Object noop(Class<?> c) {
    Object n;
    if (AbstractPeepholeOptimization.class.isAssignableFrom(c)) {
      n = new AbstractPeepholeOptimization() {
        @Override
        Node optimizeSubtree(Node subtree) {
          return subtree;
        }
      };
    } else if (AbstractPeepholeTranspilation.class.isAssignableFrom(c)) {
      n = new AbstractPeepholeTranspilation() {
        @Override
        com.google.javascript.jscomp.parsing.parser.FeatureSet getTranspiledAwayFeatures() {
          return com.google.javascript.jscomp.parsing.parser.FeatureSet.BARE_MINIMUM;
        }

        @Override
        Node transpileSubtree(Node subtree) {
          return subtree;
        }
      };
    } else {
      List<Class<?>> roles = new ArrayList<>();
      for (Class<?> i : ROLE_INTERFACES) {
        if (i.isAssignableFrom(c)) {
          roles.add(i);
        }
      }
      if (roles.isEmpty()) {
        roles.add(CompilerPass.class);
      }
      n = java.lang.reflect.Proxy.newProxyInstance(
          ReplayDsl.class.getClassLoader(),
          roles.toArray(new Class<?>[0]),
          (proxy, method, margs) -> {
            switch (method.getName()) {
              case "shouldTraverse":
                return true;
              case "hashCode":
                return System.identityHashCode(proxy);
              case "equals":
                return proxy == margs[0];
              case "toString":
                return "noop:" + c.getName();
              default:
                return null;
            }
          });
    }
    synchronized (NOOPS) {
      NOOPS.put(n, true);
    }
    return n;
  }

  private static boolean isNoop(Object o) {
    synchronized (NOOPS) {
      return o != null && NOOPS.containsKey(o);
    }
  }

  /** Replaces a static/instance call result that is a mutated pass (DSL.md "No-op mutation"). */
  private static Object mutateResult(Object v) {
    if (v != null && !isNoop(v) && isMutated(v.getClass()) && hasPassRole(v.getClass())) {
      return noop(v.getClass());
    }
    return v;
  }

  /** Selects the descriptor case for a record (DSL.md "Descriptor file"). */
  public static JsonObject selectCase(JsonObject descriptor, JsonObject record) {
    JsonObject c = selectCaseOrNull(descriptor, record);
    if (c == null) {
      throw new IllegalStateException("no descriptor case matches the record");
    }
    return c;
  }

  /** As selectCase, but null when no case matches (integration and type_check records). */
  public static JsonObject selectCaseOrNull(JsonObject descriptor, JsonObject record) {
    if (descriptor == null || !descriptor.has("cases")) {
      return null;
    }
    for (JsonElement ce : descriptor.getAsJsonArray("cases")) {
      JsonObject c = ce.getAsJsonObject();
      JsonObject when = c.has("when") ? c.getAsJsonObject("when") : new JsonObject();
      boolean ok = true;
      for (Map.Entry<String, JsonElement> w : when.entrySet()) {
        JsonElement actual = path(record, w.getKey());
        if (!matches(actual, w.getValue())) {
          ok = false;
          break;
        }
      }
      if (ok) {
        return c;
      }
    }
    return null;
  }

  /** Evaluates each expression of a JSON array in order (DSL.md effect lists). */
  public static void evalAll(JsonElement list, Ctx ctx) {
    if (list == null || list.isJsonNull()) {
      return;
    }
    for (JsonElement x : list.getAsJsonArray()) {
      eval(x, ctx);
    }
  }

  private static boolean matches(JsonElement actual, JsonElement want) {
    if (want.isJsonObject() && want.getAsJsonObject().has("isNull")) {
      boolean isNull = actual == null || actual.isJsonNull();
      return isNull == want.getAsJsonObject().get("isNull").getAsBoolean();
    }
    if (actual == null) {
      return want.isJsonNull();
    }
    return actual.equals(want);
  }

  /** Dotted path into a JSON object ("testFields.late"). */
  public static JsonElement path(JsonObject root, String p) {
    JsonElement cur = root;
    for (String part : p.split("\\.")) {
      if (cur != null && cur.isJsonArray() && part.matches("[0-9]+")) {
        int i = Integer.parseInt(part);
        cur = i < cur.getAsJsonArray().size() ? cur.getAsJsonArray().get(i) : null;
        continue;
      }
      if (cur == null || !cur.isJsonObject()) {
        return null;
      }
      cur = cur.getAsJsonObject().get(part);
    }
    return cur;
  }

  public static Object eval(JsonElement e, Ctx ctx) {
    if (e == null || e.isJsonNull()) {
      return null;
    }
    JsonObject o = e.getAsJsonObject();
    if (o.has("int")) {
      return o.get("int").getAsInt();
    }
    if (o.has("long")) {
      return Long.parseLong(o.get("long").getAsString());
    }
    if (o.has("double")) {
      return Double.parseDouble(o.get("double").getAsString());
    }
    if (o.has("bool")) {
      return o.get("bool").getAsBoolean();
    }
    if (o.has("string")) {
      return o.get("string").getAsString();
    }
    if (o.has("enum")) {
      return ReplayValues.enumValue(o.get("enum").getAsString(), o.get("name").getAsString());
    }
    if (o.has("null")) {
      return null;
    }
    if (o.has("list")) {
      List<Object> l = new ArrayList<>();
      for (JsonElement x : o.getAsJsonArray("list")) {
        l.add(eval(x, ctx));
      }
      return l;
    }
    if (o.has("set")) {
      LinkedHashSet<Object> s = new LinkedHashSet<>();
      for (JsonElement x : o.getAsJsonArray("set")) {
        s.add(eval(x, ctx));
      }
      return s;
    }
    if (o.has("map")) {
      Map<Object, Object> m = new LinkedHashMap<>();
      for (JsonElement kv : o.getAsJsonArray("map")) {
        JsonArray a = kv.getAsJsonArray();
        m.put(eval(a.get(0), ctx), eval(a.get(1), ctx));
      }
      return m;
    }
    if (o.has("array")) {
      Class<?> comp = ReplayValues.classForName(o.get("array").getAsString());
      JsonArray items = o.getAsJsonArray("items");
      Object arr = java.lang.reflect.Array.newInstance(comp, items.size());
      for (int i = 0; i < items.size(); i++) {
        java.lang.reflect.Array.set(arr, i, eval(items.get(i), ctx));
      }
      return arr;
    }
    if (o.has("compiler")) {
      return ctx.compiler;
    }
    if (o.has("options")) {
      return ctx.options != null ? ctx.options : ctx.compiler.getOptions();
    }
    if (o.has("class")) {
      return ReplayValues.classForName(o.get("class").getAsString());
    }
    if (o.has("var")) {
      String n = o.get("var").getAsString();
      if (!ctx.vars.containsKey(n)) {
        throw new IllegalStateException("unbound DSL variable " + n);
      }
      return ctx.vars.get(n);
    }
    if (o.has("let")) {
      Map<String, Object> saved = ctx.vars;
      try {
        for (JsonElement b : o.getAsJsonArray("let")) {
          JsonArray pair = b.getAsJsonArray();
          Object v = eval(pair.get(1), ctx);
          Map<String, Object> next = new LinkedHashMap<>(ctx.vars);
          next.put(pair.get(0).getAsString(), v);
          ctx.vars = next;
        }
        return eval(o.get("in"), ctx);
      } finally {
        ctx.vars = saved;
      }
    }
    if (o.has("do")) {
      evalAll(o.get("do"), ctx);
      return eval(o.get("value"), ctx);
    }
    if (o.has("lambda")) {
      return lambda(o, ctx);
    }
    if (o.has("getField")) {
      Object target = eval(o.get("getField"), ctx);
      try {
        Field f = ReplayValues.findField(target.getClass(), Object.class, o.get("name").getAsString());
        if (f == null) {
          throw new IllegalArgumentException("no field " + o.get("name") + " in " + target.getClass().getName());
        }
        f.setAccessible(true);
        return f.get(target);
      } catch (ReflectiveOperationException ex) {
        throw new RuntimeException(ex);
      }
    }
    if (o.has("mutationPoint")) {
      Object v = eval(o.get("value"), ctx);
      if (v != null && nameMutated(o.get("mutationPoint").getAsString())) {
        return noop(v.getClass());
      }
      return v;
    }
    if (o.has("field")) {
      String name = o.get("field").getAsString();
      if (ctx.fieldOverrides.containsKey(name)) {
        return ctx.fieldOverrides.get(name);
      }
      JsonObject tf = ctx.record.getAsJsonObject("testFields");
      JsonElement v = tf == null ? null : tf.get(name);
      if (v == null && tf != null) {
        // Field declared in a superclass of the instance class: "Declaring.name".
        for (Map.Entry<String, JsonElement> fe : tf.entrySet()) {
          if (fe.getKey().endsWith("." + name)) {
            v = fe.getValue();
            break;
          }
        }
      }
      if (v == null) {
        throw new IllegalStateException("record has no testFields." + name);
      }
      if (o.has("path")) {
        for (String part : o.get("path").getAsString().split("\\.")) {
          v = v.getAsJsonObject().get(part);
        }
      }
      return ReplayValues.decode(v, Object.class);
    }
    if (o.has("record")) {
      return ReplayValues.decode(path(ctx.record, o.get("record").getAsString()), Object.class);
    }
    if (o.has("new")) {
      Class<?> c = ReplayValues.classForName(o.get("new").getAsString());
      Object[] args = args(o, ctx);
      if (isMutated(c) && hasPassRole(c)) {
        return noop(c);
      }
      Constructor<?> k = (Constructor<?>) pick(c.getDeclaredConstructors(), args, c.getName());
      try {
        k.setAccessible(true);
        return k.newInstance(coerce(k, args));
      } catch (ReflectiveOperationException ex) {
        throw invocationFailure(ex);
      }
    }
    if (o.has("static")) {
      Class<?> c = ReplayValues.classForName(o.get("static").getAsString());
      return mutateResult(invoke(c, null, o.get("method").getAsString(), args(o, ctx), true));
    }
    if (o.has("call")) {
      Object target = eval(o.get("call"), ctx);
      Object[] args = args(o, ctx);
      if (isNoop(target)) {
        return target;
      }
      if (target == null) {
        throw new NullPointerException("DSL call of " + o.get("method").getAsString() + " on null");
      }
      return mutateResult(invoke(target.getClass(), target, o.get("method").getAsString(), args, false));
    }
    if (o.has("once")) {
      String key = o.get("once").getAsString();
      if (!ctx.once.containsKey(key)) {
        ctx.once.put(key, eval(o.get("value"), ctx));
      }
      return ctx.once.get(key);
    }
    if (o.has("withFields")) {
      Object target = eval(o.get("withFields"), ctx);
      for (Map.Entry<String, JsonElement> fe : o.getAsJsonObject("fields").entrySet()) {
        Object v = eval(fe.getValue(), ctx);
        if (isNoop(target)) {
          continue;
        }
        try {
          Field f = ReplayValues.findField(target.getClass(), Object.class, fe.getKey());
          f.setAccessible(true);
          f.set(target, v);
        } catch (ReflectiveOperationException ex) {
          throw new RuntimeException(ex);
        }
      }
      return target;
    }
    if (o.has("sequence")) {
      List<CompilerPass> passes = new ArrayList<>();
      for (JsonElement x : o.getAsJsonArray("sequence")) {
        passes.add((CompilerPass) eval(x, ctx));
      }
      return new SequencePass(passes);
    }
    if (o.has("helper")) {
      return helper(o, ctx);
    }
    throw new IllegalArgumentException("unknown DSL expression " + o);
  }

  /**
   * {"lambda":["p",...],"iface":"FQCN","body":E}: an instance of the functional interface whose
   * single abstract method binds its arguments to the parameter names (on top of the variables
   * visible where the lambda was created) and returns the value of body, evaluated at call time.
   */
  private static Object lambda(JsonObject o, Ctx ctx) {
    Class<?> iface = ReplayValues.classForName(o.get("iface").getAsString());
    if (!iface.isInterface()) {
      throw new IllegalArgumentException("lambda iface is not an interface: " + iface.getName());
    }
    List<String> params = new ArrayList<>();
    for (JsonElement p : o.getAsJsonArray("lambda")) {
      params.add(p.getAsString());
    }
    JsonElement body = o.get("body");
    Map<String, Object> captured = ctx.vars;
    return java.lang.reflect.Proxy.newProxyInstance(
        iface.getClassLoader(),
        new Class<?>[] {iface},
        (proxy, method, margs) -> {
          if (method.getDeclaringClass() == Object.class) {
            switch (method.getName()) {
              case "hashCode":
                return System.identityHashCode(proxy);
              case "equals":
                return proxy == margs[0];
              default:
                return "lambda:" + iface.getName();
            }
          }
          if (method.isDefault()) {
            return java.lang.reflect.InvocationHandler.invokeDefault(proxy, method, margs);
          }
          int n = margs == null ? 0 : margs.length;
          if (n != params.size()) {
            throw new IllegalArgumentException(
                "lambda for " + iface.getName() + "." + method.getName() + " takes " + n + " args, has "
                    + params.size() + " params");
          }
          Map<String, Object> saved = ctx.vars;
          Map<String, Object> env = new LinkedHashMap<>(captured);
          for (int i = 0; i < n; i++) {
            env.put(params.get(i), margs[i]);
          }
          ctx.vars = env;
          try {
            Object r = eval(body, ctx);
            Class<?> rt = method.getReturnType();
            if (rt == void.class) {
              return null;
            }
            return ReplayValues.adapt(r, box(rt));
          } finally {
            ctx.vars = saved;
          }
        });
  }

  /** Runs passes in order on (externs, root). */
  static final class SequencePass implements CompilerPass {
    final List<CompilerPass> passes;

    SequencePass(List<CompilerPass> passes) {
      this.passes = passes;
    }

    @Override
    public void process(Node externs, Node root) {
      for (CompilerPass p : passes) {
        p.process(externs, root);
      }
    }
  }

  /**
   * {"helper":"Holder.Inner","outer":["f1","f2"],"args":[...]}: instantiates the verbatim inner
   * class Inner of helper holder Holder (package com.google.javascript.jscomp). For a non-static
   * inner class, a Holder instance is the outer instance; each name in "outer" is a Holder field
   * restored from the record's testFields (or a fieldOverrides entry).
   */
  private static Object helper(JsonObject o, Ctx ctx) {
    String name = o.get("helper").getAsString();
    int dot = name.indexOf('.');
    String pkg = o.has("package") ? o.get("package").getAsString() : "com.google.javascript.jscomp";
    String holderName = pkg + "." + (dot < 0 ? name : name.substring(0, dot));
    Class<?> holder = ReplayValues.classForName(holderName);
    Class<?> c = dot < 0 ? holder : ReplayValues.classForName(holderName + "$" + name.substring(dot + 1));
    Object[] args = args(o, ctx);
    boolean inner = c != holder && !Modifier.isStatic(c.getModifiers());
    Object outer = null;
    if (inner && o.has("outerInstance")) {
      outer = eval(o.get("outerInstance"), ctx);
      if (!holder.isInstance(outer)) {
        throw new IllegalArgumentException("outerInstance is not a " + holder.getName());
      }
      Object[] full = new Object[args.length + 1];
      full[0] = outer;
      System.arraycopy(args, 0, full, 1, args.length);
      args = full;
    } else if (inner) {
      outer = ReplayValues.instantiate(holder);
      if (o.has("outer")) {
        for (JsonElement f : o.getAsJsonArray("outer")) {
          String fn = f.getAsString();
          Object v = eval(fieldRef(fn), ctx);
          try {
            Field hf = holder.getDeclaredField(fn);
            hf.setAccessible(true);
            hf.set(outer, ReplayValues.adapt(v, hf.getType()));
          } catch (ReflectiveOperationException ex) {
            throw new RuntimeException(ex);
          }
        }
      }
      Object[] full = new Object[args.length + 1];
      full[0] = outer;
      System.arraycopy(args, 0, full, 1, args.length);
      args = full;
    }
    Constructor<?> k = (Constructor<?>) pick(c.getDeclaredConstructors(), args, c.getName());
    try {
      k.setAccessible(true);
      return k.newInstance(coerce(k, args));
    } catch (ReflectiveOperationException ex) {
      throw invocationFailure(ex);
    }
  }

  /**
   * Exception fidelity (HARNESS.md "DSL exception fidelity"): code the DSL
   * invokes (a constructor, a helper or pass method, a DSL-lambda body) may throw. An unchecked
   * throwable (Error, including AssertionError, or RuntimeException) is rethrown unchanged, exactly as
   * a direct Java call would propagate it, so the outcome class, message and assertion flag match the
   * original. Only a checked cause, or a reflection failure without a cause, is wrapped.
   */
  static RuntimeException invocationFailure(ReflectiveOperationException ex) {
    Throwable cause = ex instanceof java.lang.reflect.InvocationTargetException ? ex.getCause() : null;
    if (cause instanceof Error) {
      throw (Error) cause;
    }
    if (cause instanceof RuntimeException) {
      return (RuntimeException) cause;
    }
    return new RuntimeException(ex.getCause() != null ? ex.getCause() : ex);
  }

  private static JsonObject fieldRef(String n) {
    JsonObject r = new JsonObject();
    r.addProperty("field", n);
    return r;
  }

  private static Object[] args(JsonObject o, Ctx ctx) {
    if (!o.has("args")) {
      return new Object[0];
    }
    JsonArray a = o.getAsJsonArray("args");
    Object[] out = new Object[a.size()];
    for (int i = 0; i < a.size(); i++) {
      out[i] = eval(a.get(i), ctx);
    }
    return out;
  }

  private static Object invoke(Class<?> c, Object target, String name, Object[] args, boolean isStatic) {
    List<Method> cands = new ArrayList<>();
    for (Class<?> k = c; k != null; k = k.getSuperclass()) {
      for (Method m : k.getDeclaredMethods()) {
        if (m.getName().equals(name) && Modifier.isStatic(m.getModifiers()) == isStatic) {
          cands.add(m);
        }
      }
      for (Class<?> i : k.getInterfaces()) {
        for (Method m : i.getMethods()) {
          if (m.getName().equals(name) && Modifier.isStatic(m.getModifiers()) == isStatic) {
            cands.add(m);
          }
        }
      }
    }
    Method m = (Method) pick(cands.toArray(new Executable[0]), args, c.getName() + "." + name);
    try {
      m.setAccessible(true);
      return m.invoke(target, coerce(m, args));
    } catch (ReflectiveOperationException ex) {
      throw invocationFailure(ex);
    }
  }

  /**
   * DSL.md overload rule: arity; applicable without widening, else with int widening; then the
   * most specific applicable candidate (Java JLS 15.12.2.5 style); ambiguity is an error.
   */
  /** When non-null, every overload resolution adds its resolved signature here (--sig-out). */
  static java.util.Set<String> sigLog = null;

  private static Executable pick(Executable[] cands, Object[] args, String what) {
    for (int pass = 0; pass < 2; pass++) {
      List<Executable> ok = new ArrayList<>();
      for (Executable ex : cands) {
        if (ex.getParameterCount() != args.length || (ex instanceof Method m && m.isBridge())) {
          continue;
        }
        if (applicable(ex.getParameterTypes(), args, pass == 1)) {
          ok.add(ex);
        }
      }
      if (ok.isEmpty()) {
        continue;
      }
      List<Executable> best = new ArrayList<>();
      for (Executable a : ok) {
        boolean dominated = false;
        for (Executable b : ok) {
          if (a != b && moreSpecific(b, a) && !moreSpecific(a, b)) {
            dominated = true;
            break;
          }
        }
        if (!dominated) {
          boolean dup = false;
          for (Executable x : best) {
            if (java.util.Arrays.equals(x.getParameterTypes(), a.getParameterTypes())) {
              dup = true; // an override of a method already chosen (subclass first)
              break;
            }
          }
          if (!dup) {
            best.add(a);
          }
        }
      }
      if (best.size() > 1) {
        throw new IllegalArgumentException("ambiguous overload for " + what + ": " + best);
      }
      Executable chosen = best.get(0);
      if (sigLog != null) {
        // --sig-out: the resolved Java signature at every DSL call site.
        StringBuilder sb = new StringBuilder();
        sb.append(what).append('\t').append(chosen.getDeclaringClass().getName()).append('\t')
            .append(chosen instanceof Method ? chosen.getName() : "<init>").append('(');
        Class<?>[] ps = chosen.getParameterTypes();
        for (int i = 0; i < ps.length; i++) {
          sb.append(i == 0 ? "" : ",").append(ps[i].getTypeName());
        }
        sb.append(')').append(pass == 1 ? "\twidened" : "");
        sigLog.add(sb.toString());
      }
      return chosen;
    }
    throw new IllegalArgumentException("no applicable overload for " + what + " with " + args.length + " args");
  }

  /** Every parameter type of a is a subtype of (or the same as) b's. */
  private static boolean moreSpecific(Executable a, Executable b) {
    Class<?>[] pa = a.getParameterTypes();
    Class<?>[] pb = b.getParameterTypes();
    for (int i = 0; i < pa.length; i++) {
      if (!box(pb[i]).isAssignableFrom(box(pa[i]))) {
        return false;
      }
    }
    return true;
  }

  private static boolean applicable(Class<?>[] ps, Object[] args, boolean widen) {
    for (int i = 0; i < ps.length; i++) {
      Class<?> p = box(ps[i]);
      Object a = args[i];
      if (a == null) {
        if (ps[i].isPrimitive()) {
          return false;
        }
        continue;
      }
      if (p.isInstance(a)) {
        continue;
      }
      if (widen && a instanceof Integer && (p == Long.class || p == Double.class)) {
        continue;
      }
      if (a instanceof java.util.Collection
          && (p.isAssignableFrom(com.google.common.collect.ImmutableList.class)
              || p.isAssignableFrom(com.google.common.collect.ImmutableSet.class))) {
        continue;
      }
      if (a instanceof Map && p.isAssignableFrom(com.google.common.collect.ImmutableMap.class)) {
        continue;
      }
      return false;
    }
    return true;
  }

  private static Object[] coerce(Executable ex, Object[] args) {
    Class<?>[] ps = ex.getParameterTypes();
    Object[] out = new Object[args.length];
    for (int i = 0; i < args.length; i++) {
      Object a = args[i];
      Class<?> p = box(ps[i]);
      if (a instanceof Integer n && p == Long.class) {
        a = (long) n;
      } else if (a instanceof Integer n && p == Double.class) {
        a = (double) n;
      } else if (a != null && !p.isInstance(a)) {
        a = ReplayValues.adapt(a, p);
        if (!p.isInstance(a) && a instanceof java.util.Collection<?> col) {
          if (p.isAssignableFrom(com.google.common.collect.ImmutableSet.class)) {
            a = com.google.common.collect.ImmutableSet.copyOf(col);
          } else if (p.isAssignableFrom(com.google.common.collect.ImmutableList.class)) {
            a = com.google.common.collect.ImmutableList.copyOf(col);
          }
        }
      }
      out[i] = a;
    }
    return out;
  }

  private static Class<?> box(Class<?> c) {
    if (!c.isPrimitive()) {
      return c;
    }
    if (c == int.class) {
      return Integer.class;
    }
    if (c == long.class) {
      return Long.class;
    }
    if (c == boolean.class) {
      return Boolean.class;
    }
    if (c == double.class) {
      return Double.class;
    }
    if (c == char.class) {
      return Character.class;
    }
    if (c == float.class) {
      return Float.class;
    }
    if (c == short.class) {
      return Short.class;
    }
    return Byte.class;
  }
}
