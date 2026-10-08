#!/usr/bin/env python3
"""Gate 0.2 ((a)-(e), D-015/D-017; docs/PORTING.md §4.2) driver.  Used by gates/gate_0_2.sh.

Subcommands (each resumable: a finished unit leaves <unit>.done.json and is skipped):
  prep        compile the replay harness once, prove rule 6, compute the pass mapping
  replay      (a) replay every record-bearing class, no agent
  cov-orig    (c) original suite (all *Test classes, recording hook OFF) under the JaCoCo agent
  cov-replay  (c) full replay under the same JaCoCo agent
  mutate      (d) class-level NoopAgent replays (D-015, D-017 items 10 and 11): the 30 in-scope passes
              (type-check unit {TypedScopeCreator, TypeInferencePass, TypeCheck} as one) with the most
              change-expecting records, each group of two or more ranked passes a record's passTrace shows
              (joint no-op, agent form A;B;C), and TypeCheck alone for information
  noop        (a) vacuity sanity: every compiler_test_case class replayed with the whole
              processor replaced by a no-op (--noop-processor); a class whose records all still
              pass (no assertion-type failure) is vacuous; its records without replay-checked post-call
              state or postconditions become notCaptured (mechanical rule, no free-text escape)
  report      (a)-(e) verdicts -> gates/reports/gate_0_2.{md,json}

Heap: at most PARALLEL (8) JVMs at -Xmx2g run at once (16 GB < 20 GB host budget); phases are
sequential.  Work dir: build/gate02/.
"""
import concurrent.futures as cf
import gzip
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import time
import zipfile
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "scripts"))
from paths import ROOT  # noqa: E402  the main checkout (scripts/paths.py)
W = f"{ROOT}/build/gate02"
REC = f"{ROOT}/corpus/unit/records"
DESC = f"{ROOT}/corpus/unit/descriptors"
STATS = f"{ROOT}/build/unit/recording-v5/stats"  # stats of the current recording
JARS = f"{ROOT}/build/unit/jars"
SUPPORT = f"{JARS}/unit_support_deploy.jar"
ALLTESTS = f"{JARS}/unit_all_tests.jar"
WS = f"{ROOT}/reference/closure-compiler-recording"
PRISTINE_SRC = f"{ROOT}/reference/closure-compiler/src"
JAVA_HOME = f"{ROOT}/tools/jdk-21"
JAVA = f"{JAVA_HOME}/bin/java"
JAVAC = f"{JAVA_HOME}/bin/javac"
JACOCO_VERSION = "0.8.15"
AGENT = f"{W}/tools/org.jacoco.agent-{JACOCO_VERSION}-runtime.jar"
CLI = f"{W}/tools/org.jacoco.cli-{JACOCO_VERSION}-nodeps.jar"
AGENT_SHA256 = "fb5b0036a0899ea97edfa0fc2c7985b55f3f7c5695028163e5e60d4f3cf6075d"
CLI_SHA256 = "d2b74b20b415163c1f53261e7c5ef4445b788327094208ff821e0c2baf9bc8f1"
AGENT_URL = f"https://repo1.maven.org/maven2/org/jacoco/org.jacoco.agent/{JACOCO_VERSION}/org.jacoco.agent-{JACOCO_VERSION}-runtime.jar"
CLI_URL = f"https://repo1.maven.org/maven2/org/jacoco/org.jacoco.cli/{JACOCO_VERSION}/org.jacoco.cli-{JACOCO_VERSION}-nodeps.jar"
# Identical agent options for both coverage runs (same agent, same mechanism).
AGENT_OPTS = "includes=com.google.javascript.*:com.google.debugging.*,output=file,append=false"
PARALLEL = int(os.environ.get("GATE02_PARALLEL", "8"))  # 8 x -Xmx2g = 16 GB (host budget < 20 GB)
XMX = "-Xmx2g"
# Same JVM flags as scripts/unit_replay.sh and the recording run.
JOPTS = [XMX, "-Xss8m", "-XX:+UseParallelGC", "-Duser.language=en", "-Duser.country=US",
         "-Duser.timezone=UTC", "-Dfile.encoding=UTF-8"]
ENV = {"LC_ALL": "C.UTF-8", "LANG": "C.UTF-8", "TZ": "UTC", "PATH": f"{JAVA_HOME}/bin:/usr/bin:/bin",
       "JAVA_HOME": JAVA_HOME, "HOME": os.environ.get("HOME", "/tmp")}
REPLAY_CLASSES = f"{W}/replay-classes"
# Harness infrastructure, not a pass under test (only reached through the name convention).
WRAPPERS = {"com.google.javascript.jscomp.PeepholeOptimizationsPass",
            "com.google.javascript.jscomp.PhaseOptimizer",
            "com.google.javascript.jscomp.CombinedCompilerPass"}
NOOP_AGENT_SRC = f"{ROOT}/oracle/replay/agent/src/closurers/agent/NoopAgent.java"
NOOP_AGENT_BUILD = f"{ROOT}/scripts/unit_noop_agent_build.sh"
NOOP_AGENT_JAR = f"{W}/tools/noop-agent.jar"
# D-017 item 8 (HARNESS.md "Pass trace"): NoopAgent TRACE mode, scope = in-scope src top-level classes.
TRACE_SRC = f"{ROOT}/oracle/replay/agent/src/closurers/agent/Trace.java"
TRACE_SCOPE_SCRIPT = f"{ROOT}/scripts/unit_trace_scope.py"
TRACE_SCOPE = f"{W}/tools/trace_scope.txt"
ASM_EXPORT = "--add-exports=java.base/jdk.internal.org.objectweb.asm=ALL-UNNAMED"
POSTCALL = f"{ROOT}/corpus/unit/postcall"
RUST_FRAG = f"{ROOT}/corpus/unit/rust_unit_tests"
EXCLUSION_CAP = 0.03
# docs/PORTING.md §2: out-of-scope source directories (gate (d) ranks only in-scope passes; (c) lists only
# in-scope non-harness test classes).
OUT_OF_SCOPE_PKGS = ("com.google.javascript.refactoring.", "com.google.javascript.jscomp.ant.",
                     "com.google.javascript.jscomp.instrumentation.", "com.google.javascript.jscomp.lint.")
# docs/PORTING.md §2 also puts "debugger" and the J2CL-, Polymer- and Chrome-specific passes out
# of scope. Source files / classes whose simple name starts with one of these prefixes are out of scope
# for both (c) (file scope of the verdict) and (d) (pass ranking).
OUT_OF_SCOPE_NAME_PREFIXES = ("J2cl", "Polymer", "Chrome", "Debugger")


def in_scope_src(name):
    """docs/PORTING.md §2 scope test for a src class FQCN (a.b.C, a.b.C$D) or a source path (a/b/C.java)."""
    if "/" in name:
        fq = name[:-5].replace("/", ".") if name.endswith(".java") else name.replace("/", ".")
    else:
        fq = name
    if fq.startswith(OUT_OF_SCOPE_PKGS):
        return False
    simple = fq.rsplit(".", 1)[-1]
    return not simple.startswith(OUT_OF_SCOPE_NAME_PREFIXES)
ALLOWED_TEST_NAMED = re.compile(r"^com/google/javascript/jscomp/(integration/)?Replay(Compiler|Integration|TypeCheck)Test(\$.*)?\.class$")


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def repo_relative(o):
    """o with every string path under ROOT made relative to it (recursively)."""
    if isinstance(o, str):
        return o[len(ROOT) + 1:] if o.startswith(ROOT + "/") else o
    if isinstance(o, list):
        return [repo_relative(v) for v in o]
    if isinstance(o, dict):
        return {k: repo_relative(v) for k, v in o.items()}
    return o


def jdump(path, obj):
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(obj, f, indent=1, sort_keys=True)
    os.replace(tmp, path)


def jload(path):
    with open(path) as f:
        return json.load(f)


def record_counts():
    """Independent line count of every corpus/unit/records/*.jsonl.gz."""
    out = {}
    for fn in sorted(os.listdir(REC)):
        if fn.endswith(".jsonl.gz"):
            with gzip.open(os.path.join(REC, fn), "rt", encoding="utf-8") as f:
                out[fn[:-len(".jsonl.gz")]] = sum(1 for _ in f)
    return out


def iter_records(name):
    with gzip.open(f"{REC}/{name}.jsonl.gz", "rt", encoding="utf-8") as f:
        for line in f:
            yield json.loads(line)


# ---------------------------------------------------------------- class-file SourceFile parse
def class_source_file(data):
    if data[:4] != b"\xca\xfe\xba\xbe":
        return None
    n = struct.unpack(">H", data[8:10])[0]
    pos = 10
    utf8 = {}
    i = 1
    while i < n:
        tag = data[pos]
        pos += 1
        if tag == 1:
            ln = struct.unpack(">H", data[pos:pos + 2])[0]
            utf8[i] = data[pos + 2:pos + 2 + ln].decode("utf-8", "replace")
            pos += 2 + ln
        elif tag in (3, 4, 9, 10, 11, 12, 17, 18):
            pos += 4
        elif tag in (5, 6):
            pos += 8
            i += 1
        elif tag in (7, 8, 16, 19, 20):
            pos += 2
        elif tag == 15:
            pos += 3
        else:
            raise ValueError(f"bad cp tag {tag}")
        i += 1
    pos += 6
    ic = struct.unpack(">H", data[pos:pos + 2])[0]
    pos += 2 + 2 * ic
    for _ in range(2):  # fields, methods
        cnt = struct.unpack(">H", data[pos:pos + 2])[0]
        pos += 2
        for _ in range(cnt):
            pos += 6
            ac = struct.unpack(">H", data[pos:pos + 2])[0]
            pos += 2
            for _ in range(ac):
                ln = struct.unpack(">I", data[pos + 2:pos + 6])[0]
                pos += 6 + ln
    ac = struct.unpack(">H", data[pos:pos + 2])[0]
    pos += 2
    for _ in range(ac):
        nm = struct.unpack(">H", data[pos:pos + 2])[0]
        ln = struct.unpack(">I", data[pos + 2:pos + 6])[0]
        if utf8.get(nm) == "SourceFile":
            return utf8.get(struct.unpack(">H", data[pos + 6:pos + 8])[0])
        pos += 6 + ln
    return None


def original_test_binary_name(n):
    """True when class-file path n names an original *Test class or a class nested in one: its
    top-level binary name (before the first '$') ends in Test and the pristine reference has
    test/<that path>.java."""
    top = n[:-len(".class")].split("$", 1)[0]
    return top.endswith("Test") and os.path.exists(f"{ROOT}/reference/closure-compiler/test/{top}.java")


def rule6_scan():
    """Lists every class entry of every replay classpath element; reports any class whose
    name ends in Test (or Test$...) or whose SourceFile attribute ends in Test.java, if it is in
    Closure's namespaces (com/google/javascript, com/google/debugging) or its source file exists
    under the reference's test/ tree. Test-named third-party library classes (junit.framework.Test,
    guava-testlib) are not original *Test classes; they are listed separately."""
    entries = []
    with zipfile.ZipFile(SUPPORT) as z:
        for n in z.namelist():
            if n.endswith(".class"):
                entries.append(("unit_support_deploy.jar", n, z.read(n)))
    for dp, _, fns in os.walk(REPLAY_CLASSES):
        for fn in fns:
            if fn.endswith(".class"):
                p = os.path.join(dp, fn)
                with open(p, "rb") as f:
                    entries.append(("replay-classes", os.path.relpath(p, REPLAY_CLASSES), f.read()))
    name_hits, src_hits, allowed, parse_err, third_party, helper_named = [], [], [], [], [], []
    for where, n, data in entries:
        try:
            sf = class_source_file(data)
        except Exception as e:  # noqa
            sf = None
            parse_err.append(f"{where}:{n}: {e}")
        name_bad = re.search(r"Test(\$[^/]*)?\.class$", n) is not None
        src_bad = sf is not None and sf.endswith("Test.java")
        if (name_bad or src_bad) and where == "replay-classes" and ALLOWED_TEST_NAMED.match(n):
            allowed.append(f"{where}:{n} (SourceFile {sf})")
            continue
        pkgdir = os.path.dirname(n)
        closure_ns = n.startswith("com/google/javascript/") or n.startswith("com/google/debugging/")
        in_ref_test = sf is not None and os.path.exists(f"{ROOT}/reference/closure-compiler/test/{pkgdir}/{sf}")
        if (name_bad or src_bad) and not closure_ns and not in_ref_test:
            third_party.append(f"{where}:{n} (SourceFile {sf})")
            continue
        # No allowance for helper-compiled classes. Any class whose binary name
        # equals, or is nested under, an original *Test class (a reference test/<pkg>/<Top>.java
        # with Top ending in Test) is a violation, whatever its SourceFile.
        if original_test_binary_name(n):
            helper_named.append(f"{where}:{n} (SourceFile {sf})")
        if name_bad:
            name_hits.append(f"{where}:{n}")
        if src_bad:
            src_hits.append(f"{where}:{n} (SourceFile {sf})")
    with open(f"{W}/rule6_classpath_entries.txt", "w") as f:
        for where, n, _ in entries:
            f.write(f"{where}\t{n}\n")
    return {"classpath": [SUPPORT, REPLAY_CLASSES], "classEntries": len(entries),
            "entriesListing": "build/gate02/rule6_classpath_entries.txt",
            "nameEndsInTest": name_hits, "sourceFileEndsInTestJava": src_hits,
            "allowedReplayHarnessClasses": allowed, "parseErrors": parse_err,
            "thirdPartyTestNamedNotOriginal": third_party,
            "helperClassesWithTestLikeBinaryNames": helper_named,
            "supportJarSha256": sha256(SUPPORT),
            "ok": not name_hits and not src_hits and not parse_err and not helper_named}


# ---------------------------------------------------------------- prep
def ensure_tools():
    for url, path, want in ((AGENT_URL, AGENT, AGENT_SHA256), (CLI_URL, CLI, CLI_SHA256)):
        if not os.path.exists(path):
            os.makedirs(os.path.dirname(path), exist_ok=True)
            subprocess.run(["curl", "-fsSL", "-o", path, url], check=True)
        got = sha256(path)
        if got != want:
            raise SystemExit(f"sha256 mismatch for {path}: {got} != {want}")


def src_top_classes():
    out = []
    for dp, _, fns in os.walk(PRISTINE_SRC):
        for fn in fns:
            if fn.endswith(".java") and fn != "package-info.java":
                rel = os.path.relpath(os.path.join(dp, fn[:-5]), PRISTINE_SRC)
                out.append(rel.replace("/", "."))
    return sorted(out)


ROLE_JAVA = r"""
import java.io.*;
import java.util.*;
/** Gate 0.2(d) helper: prints FQCN TAB role-flag for each FQCN on stdin (DSL.md pass role). */
public final class Gate02PassRole {
  public static void main(String[] a) throws Exception {
    ClassLoader cl = Gate02PassRole.class.getClassLoader();
    String[] roles = {"com.google.javascript.jscomp.AbstractPeepholeOptimization",
        "com.google.javascript.jscomp.AbstractPeepholeTranspilation",
        "com.google.javascript.jscomp.CompilerPass",
        "com.google.javascript.jscomp.NodeTraversal$Callback",
        "com.google.javascript.jscomp.OptimizeCalls$CallGraphCompilerPass"};
    List<Class<?>> rc = new ArrayList<>();
    for (String r : roles) rc.add(Class.forName(r, false, cl));
    BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
    String l;
    while ((l = in.readLine()) != null) {
      l = l.trim();
      if (l.isEmpty()) continue;
      String res;
      try {
        Class<?> c = Class.forName(l, false, cl);
        boolean p = false;
        for (Class<?> r : rc) p |= r.isAssignableFrom(c);
        res = p ? "pass" : "nopass";
      } catch (Throwable t) {
        res = "missing";
      }
      System.out.println(l + "\t" + res);
    }
  }
}
"""


# Gate (d) "loaded pass classes not rewritten": for each loaded class, the
# executable entry points it has (the NoopAgent's isPassEntry/isCallbackEntry name+descriptor set),
# resolved like the agent's inheritedEntries: the nearest declaration along the superclass chain
# (the class itself first; static, private, bridge and synthetic methods skipped; a nearest abstract
# declaration means no executable entry point). Prints class TAB name+desc TAB declaring class, or
# class TAB - TAB - when it has none.
ENTRY_JAVA = r"""
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
public final class Gate02EntryOwners {
  static final String NODE = "Lcom/google/javascript/rhino/Node;";
  static final String NT = "Lcom/google/javascript/jscomp/NodeTraversal;";
  static boolean entry(String n, String d) {
    String p2 = "(" + NODE + NODE + ")V";
    String p3 = "(" + NODE + NODE + "Lcom/google/javascript/jscomp/OptimizeCalls$ReferenceMap;)V";
    return (n.equals("process") && (d.equals(p2) || d.equals(p3)))
        || (n.equals("hotSwapScript") && d.equals(p2))
        || ((n.equals("optimizeSubtree") || n.equals("transpileSubtree")) && d.equals("(" + NODE + ")" + NODE))
        || (n.equals("visit") && d.equals("(" + NT + NODE + NODE + ")V"))
        || (n.equals("shouldTraverse") && d.equals("(" + NT + NODE + NODE + ")Z"))
        || ((n.equals("enterScope") || n.equals("exitScope") || n.equals("enterScopeWithCfg")
            || n.equals("exitScopeWithCfg")) && d.equals("(" + NT + ")V"))
        || (n.equals("enterChangedScopeRoot")
            && d.equals("(Lcom/google/javascript/jscomp/AbstractCompiler;" + NODE + ")V"));
  }
  static String desc(Method m) {
    StringBuilder b = new StringBuilder("(");
    for (Class<?> t : m.getParameterTypes()) b.append(t.descriptorString());
    return b.append(")").append(m.getReturnType().descriptorString()).toString();
  }
  public static void main(String[] a) throws Exception {
    ClassLoader cl = Gate02EntryOwners.class.getClassLoader();
    BufferedReader in = new BufferedReader(new InputStreamReader(System.in));
    String l;
    while ((l = in.readLine()) != null) {
      l = l.trim();
      if (l.isEmpty()) continue;
      Map<String, String> owner = new LinkedHashMap<>();
      Set<String> seen = new HashSet<>();
      try {
        for (Class<?> c = Class.forName(l, false, cl); c != null && c != Object.class; c = c.getSuperclass()) {
          for (Method m : c.getDeclaredMethods()) {
            int acc = m.getModifiers();
            if (Modifier.isStatic(acc) || Modifier.isPrivate(acc) || m.isBridge() || m.isSynthetic()) continue;
            String k = m.getName() + desc(m);
            if (!entry(m.getName(), desc(m)) || !seen.add(k)) continue;
            if (!Modifier.isAbstract(acc)) owner.put(k, c.getName());
          }
        }
      } catch (Throwable t) {
        System.out.println(l + "\tERROR\t" + t);
        continue;
      }
      if (owner.isEmpty()) System.out.println(l + "\t-\t-");
      for (Map.Entry<String, String> e : owner.entrySet()) System.out.println(l + "\t" + e.getKey() + "\t" + e.getValue());
    }
  }
}
"""


def entry_owners(names):
    """{class: [(name+desc, declaring class)]} via ENTRY_JAVA; an unloadable class maps to None."""
    jd = f"{W}/java-entry"
    os.makedirs(jd, exist_ok=True)
    jf = f"{jd}/Gate02EntryOwners.java"
    with open(jf, "w") as f:
        f.write(ENTRY_JAVA)
    subprocess.run([JAVAC, "-nowarn", "-d", jd, "-cp", SUPPORT, jf], check=True, env=ENV)
    p = subprocess.run([JAVA, "-cp", f"{jd}:{SUPPORT}", "Gate02EntryOwners"], input="\n".join(names) + "\n",
                       capture_output=True, text=True, check=True, env=ENV)
    out = {}
    for line in p.stdout.splitlines():
        c, k, o = line.split("\t", 2)
        if k == "ERROR":
            out[c] = None
        elif k == "-":
            out.setdefault(c, [])
        elif out.get(c, []) is not None:
            out.setdefault(c, []).append((k, o))
    return out


# D-017 item 10 helpers (compiled into build/gate02/java against the replay classpath; not on the replay
# classpath itself, and neither name matches the rule-6 *Test pattern).
EQUIV_JAVA = r"""package com.google.javascript.jscomp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.javascript.rhino.Node;
import java.io.BufferedReader;
import java.io.FileInputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.zip.GZIPInputStream;

/**
 * Gate 0.2(d) helper, D-017 item 10: for each compiler_test_case record whose expected output is a
 * list of files, parse the record's input sources and the expected output in the record's language
 * mode (options.languageIn; each file parsed on its own, parser only) and compare the scripts,
 * file by file, with Node.isEquivalentTo, without normalization, including JSDoc when the record's
 * comparison.compareJsDoc is set. Prints class TAB index TAB (equivalent|different|error: ...) per such record.
 */
public final class Gate02AstEquiv {
  /**
   * Parses each file on its own (CompilerInput.getAstRoot: the parser only, no module loading or dependency processing)
   * in the given language mode. A parser error makes the comparison impossible (exception).
   */
  static List<Node> parse(List<JsonObject> files, String mode) {
    CompilerOptions o = new CompilerOptions();
    if (mode != null) {
      o.setLanguageIn(CompilerOptions.LanguageMode.valueOf(mode));
    }
    Compiler c = new Compiler(new PrintStream(OutputStream.nullOutputStream()));
    c.initOptions(o);
    List<Node> out = new ArrayList<>();
    for (JsonObject f : files) {
      out.add(new CompilerInput(SourceFile.fromCode(f.get("name").getAsString(), f.get("code").getAsString())).getAstRoot(c));
    }
    if (c.hasErrors()) {
      throw new IllegalStateException("parse error: " + c.getErrors().get(0));
    }
    return out;
  }

  /**
   * The harness comparison includes JSDoc when compareJsDoc is set (docs/PORTING.md §4.2): with
   * jsDoc the scripts are compared with JsDocComparison.COMPARE, so a JSDoc difference the
   * record's own comparison checks is not a text-only difference.
   */
  static boolean equivalent(List<Node> a, List<Node> b, boolean jsDoc) {
    if (a.size() != b.size()) {
      return false;
    }
    for (int i = 0; i < a.size(); i++) {
      boolean eq = jsDoc
          ? a.get(i).isEquivalentTo(b.get(i), Node.RecursionMode.DEEP, Node.TypeComparison.IGNORE,
              Node.JsDocComparison.COMPARE, Node.SideEffectComparison.IGNORE)
          : a.get(i).isEquivalentTo(b.get(i));
      if (!eq) {
        return false;
      }
    }
    return true;
  }

  static List<JsonObject> files(JsonElement e) {
    List<JsonObject> out = new ArrayList<>();
    if (e != null && e.isJsonArray()) {
      for (JsonElement x : e.getAsJsonArray()) {
        out.add(x.getAsJsonObject());
      }
    }
    return out;
  }

  public static void main(String[] a) throws Exception {
    String records = a[0];
    PrintStream out = new PrintStream(a[1], StandardCharsets.UTF_8);
    for (int k = 2; k < a.length; k++) {
      String name = a[k];
      int idx = 0;
      try (BufferedReader r = new BufferedReader(new InputStreamReader(
          new GZIPInputStream(new FileInputStream(records + "/" + name + ".jsonl.gz")), StandardCharsets.UTF_8))) {
        String line;
        while ((line = r.readLine()) != null) {
          JsonObject rec = JsonParser.parseString(line).getAsJsonObject();
          int i = idx++;
          if (!rec.get("kind").getAsString().equals("compiler_test_case")) {
            continue;
          }
          JsonObject e = rec.getAsJsonObject("expected");
          JsonElement o = e == null ? null : e.get("output");
          if (o == null || !o.isJsonArray()) {
            continue;
          }
          String res;
          try {
            String mode = null;
            JsonObject opts = rec.getAsJsonObject("options");
            JsonElement li = opts == null ? null : opts.get("languageIn");
            if (li != null && li.isJsonObject() && li.getAsJsonObject().has("name")) {
              mode = li.getAsJsonObject().get("name").getAsString();
            }
            JsonObject inp = rec.getAsJsonObject("inputs");
            List<JsonObject> src = files(inp == null ? null : inp.get("sources"));
            if ((inp == null || !inp.has("sources") || inp.get("sources").isJsonNull())
                && inp != null && inp.has("chunks") && inp.get("chunks").isJsonArray()) {
              for (JsonElement ch : inp.getAsJsonArray("chunks")) {
                src.addAll(files(ch.getAsJsonObject().get("inputs")));
              }
            }
            JsonObject cmp = rec.getAsJsonObject("comparison");
            boolean jsDoc = cmp != null && cmp.has("compareJsDoc") && cmp.get("compareJsDoc").getAsBoolean();
            res = equivalent(parse(src, mode), parse(files(o), mode), jsDoc) ? "equivalent" : "different";
            if (mode == null) {
              res += " (no languageIn: CompilerOptions default)";
            }
            if (jsDoc) {
              res += " (JSDoc compared)";
            }
          } catch (Throwable t) {
            res = "error: " + String.valueOf(t).replace('\t', ' ').replace('\n', ' ');
          }
          out.println(name + "\t" + i + "\t" + res);
        }
      }
    }
    out.close();
  }
}
"""

PROBE_JAVA = r"""package com.google.javascript.jscomp;

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
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.zip.GZIPInputStream;

/**
 * Gate 0.2(d) helper, D-017 item 10 (postcondition): for each compiler_test_case record with
 * expected.postconditions > 0, replay the record with the whole processor a no-op
 * (ReplayCompilerTest.noopProcessor, the parsed and unprocessed input) and compare the observed
 * postCall.postcondition state (D-017 item 1) with the recorded one. Prints class TAB index TAB
 * (equal|differs|noRecordedValue|noObservedValue|error: ...) per such record.
 */
public final class Gate02PostcondProbe {
  public static void main(String[] a) throws Exception {
    String records = a[0];
    String descriptors = a[1];
    PrintStream out = new PrintStream(a[2], StandardCharsets.UTF_8);
    PrintStream stdout = System.out;
    ReplayCompilerTest.noopProcessor = true;
    for (int k = 3; k < a.length; k++) {
      String name = a[k];
      JsonObject desc = null;
      Path dp = Path.of(descriptors, name + ".json");
      if (Files.exists(dp)) {
        desc = JsonParser.parseString(Files.readString(dp)).getAsJsonObject();
      }
      Map<String, String> cm = new LinkedHashMap<>();
      if (desc != null && desc.has("classMap")) {
        for (Map.Entry<String, JsonElement> e : desc.getAsJsonObject("classMap").entrySet()) {
          cm.put(e.getKey(), e.getValue().getAsString());
        }
      }
      ReplayValues.classMap = cm;
      int idx = 0;
      try (BufferedReader r = new BufferedReader(new InputStreamReader(
          new GZIPInputStream(new FileInputStream(records + "/" + name + ".jsonl.gz")), StandardCharsets.UTF_8))) {
        String line;
        while ((line = r.readLine()) != null) {
          JsonObject rec = JsonParser.parseString(line).getAsJsonObject();
          int i = idx++;
          if (!rec.get("kind").getAsString().equals("compiler_test_case")) {
            continue;
          }
          JsonObject e = rec.getAsJsonObject("expected");
          JsonElement n = e == null ? null : e.get("postconditions");
          if (n == null || n.isJsonNull() || n.getAsInt() <= 0) {
            continue;
          }
          JsonElement want = null;
          if (rec.has("postCall") && rec.get("postCall").isJsonObject()) {
            want = rec.getAsJsonObject("postCall").get("postcondition");
          }
          String st;
          try {
            JsonObject res = ReplayMain.replayOne(rec, desc);
            JsonElement pc = res == null ? null : res.get("postCall");
            JsonElement got = pc != null && pc.isJsonObject() ? pc.getAsJsonObject().get("postcondition") : null;
            if (want == null || want.isJsonNull()) {
              st = "noRecordedValue";
            } else if (got == null || got.isJsonNull()) {
              st = "noObservedValue";
            } else {
              st = want.equals(got) ? "equal" : "differs";
            }
          } catch (Throwable t) {
            st = "error: " + String.valueOf(t).replace('\t', ' ').replace('\n', ' ');
          } finally {
            System.setOut(stdout);
          }
          out.println(name + "\t" + i + "\t" + st);
        }
      }
    }
    out.close();
  }
}
"""


def replay_sources():
    srcs = []
    for d in (f"{ROOT}/oracle/replay/src", f"{ROOT}/oracle/replay/helpers"):
        for dp, _, fns in os.walk(d):
            srcs += [os.path.join(dp, f) for f in fns if f.endswith(".java")]
    srcs.sort()
    h = hashlib.sha256()
    for s in srcs:
        h.update(s.encode())
        with open(s, "rb") as f:
            h.update(f.read())
    return srcs, h.hexdigest()


def compile_replay():
    marker = f"{REPLAY_CLASSES}.done.json"
    srcs, digest = replay_sources()
    if os.path.exists(marker) and jload(marker).get("sourcesSha256") == digest:
        return digest
    bad = [s for s in srcs if s.endswith("Test.java") and not re.search(r"/Replay(Compiler|Integration|TypeCheck)Test\.java$", s)]
    if bad:
        raise SystemExit(f"oracle/replay contains *Test.java that is not a replay harness: {bad}")
    shutil.rmtree(REPLAY_CLASSES, ignore_errors=True)
    os.makedirs(REPLAY_CLASSES)
    with open(f"{W}/replay-sources.txt", "w") as f:
        f.write("\n".join(srcs) + "\n")
    subprocess.run([JAVAC, "-nowarn", "-encoding", "UTF-8", "-proc:none", "-d", REPLAY_CLASSES, "-cp", SUPPORT,
                    f"@{W}/replay-sources.txt"], check=True, env=ENV)
    jdump(marker, {"sourcesSha256": digest, "files": len(srcs)})
    return digest


def pass_roles(names):
    jd = f"{W}/java"
    os.makedirs(jd, exist_ok=True)
    jf = f"{jd}/Gate02PassRole.java"
    if not os.path.exists(f"{jd}/Gate02PassRole.class"):
        with open(jf, "w") as f:
            f.write(ROLE_JAVA)
        subprocess.run([JAVAC, "-nowarn", "-d", jd, "-cp", SUPPORT, jf], check=True, env=ENV)
    p = subprocess.run([JAVA, "-cp", f"{jd}:{SUPPORT}", "Gate02PassRole"], input="\n".join(names) + "\n",
                       capture_output=True, text=True, check=True, env=ENV)
    out = {}
    for line in p.stdout.splitlines():
        k, v = line.split("\t")
        out[k] = v
    return out


def fqcns_in(obj, acc):
    if isinstance(obj, dict):
        for k, v in obj.items():
            if k in ("class", "object", "unrepresentable", "classRef") and isinstance(v, str):
                acc.add(v)
            fqcns_in(v, acc)
    elif isinstance(obj, list):
        for v in obj:
            fqcns_in(v, acc)


def outer(fq):
    return fq.split("$", 1)[0]


TYPECHECK_KEY = "com.google.javascript.jscomp.TypeCheck"
# D-017 item 11: TypeCheck is ranked (and no-op'd) as the type-check unit.
TYPECHECK_UNIT = ("com.google.javascript.jscomp.TypeCheck", "com.google.javascript.jscomp.TypeInferencePass",
                  "com.google.javascript.jscomp.TypedScopeCreator")


def unit_key(p):
    """D-017 item 11: a member of the type-check unit {TypedScopeCreator, TypeInferencePass, TypeCheck}
    is ranked and attributed as the unit (key TYPECHECK_KEY)."""
    return TYPECHECK_KEY if p in TYPECHECK_UNIT else p


def nested_src_classes(srcset):
    """Nested classes (binary names with '$') of the top-level src classes, from the support jar."""
    out = []
    with zipfile.ZipFile(SUPPORT) as z:
        for n in z.namelist():
            if n.endswith(".class") and "$" in n:
                b = n[:-6].replace("/", ".")
                if outer(b) in srcset:
                    out.append(b)
    return sorted(out)


def compute_pass_map(counts):
    """Ranking candidates per record, for gate (d) (D-015, D-017 item 11). A record of test class T (kind
    compiler_test_case or type_check) is a record of src pass P when:
      1. name convention: P's simple name S is the longest src pass-role class name with
         T == S + 'Test', or T starting with S followed by an upper-case letter when the record's
         processor dump names P or no src pass-role class at all; or
      2. P is the outermost class of a src class named anywhere in the record's processor dump
         (class/object/unrepresentable/classRef) where that class or P has a pass role,
         excluding harness wrappers (PeepholeOptimizationsPass, PhaseOptimizer, CombinedCompilerPass); or
      3. kind type_check: P = TypeCheck (TypeCheckTestCase runs TypeCheck).
    D-017 item 11: when rule 1 applies (the test class is named after P, prefix form included, fixed per
    item 5) the record is a record of P only; members of the type-check unit {TypedScopeCreator,
    TypeInferencePass, TypeCheck} count as the unit (unit_key). Per record the map also keeps whether
    rule 1 applied ("named") and the record's passTrace (D-017 item 8) mapped through unit_key ("trace"),
    which d_units uses to attribute records to the ranked passes and groups.
    Integration records test whole pipelines and are not attributed to a pass."""
    src = src_top_classes()
    srcset = set(src)
    referenced = set()
    per_record = {}
    for name, n in counts.items():
        if n == 0:
            continue
        lst = []
        for r in iter_records(name):
            acc = set()
            if isinstance(r.get("processor"), dict):
                fqcns_in(r["processor"], acc)
            acc = {a for a in acc if outer(a) in srcset}
            referenced |= acc
            lst.append((r["kind"], sorted(acc), r.get("passTrace")))
        per_record[name] = lst
    roles = pass_roles(sorted(srcset | referenced))
    # D-017 item 11 name rule: a top-level src class whose pass role comes from
    # a nested pass-role class (ReplaceMessages: MsgProtectionPass/FullReplacementPass) is a pass for
    # rule 1 as it already is for rule 2 (outer() of a pass-role class), so the two notions agree.
    nested = nested_src_classes(srcset)
    nroles = pass_roles(nested) if nested else {}
    nested_pass_tops = {outer(n) for n in nested if nroles.get(n) == "pass"}
    passes_by_simple = {}
    for fq in src:
        if roles.get(fq) == "pass" or fq in nested_pass_tops:
            passes_by_simple.setdefault(fq.rsplit(".", 1)[1], []).append(fq)
    mapping = {}  # test class -> list of per-record pass lists
    convention = {}
    named_map = {}
    trace_map = {}
    for name, lst in per_record.items():
        conv, prefix_only = None, False
        if name.endswith("Test"):
            best = None
            for s, fqs in passes_by_simple.items():
                if name == s + "Test" or (name.startswith(s) and len(name) > len(s) and name[len(s)].isupper()):
                    if best is None or len(s) > len(best):
                        best = s
            if best is not None and len(passes_by_simple[best]) == 1:
                conv = passes_by_simple[best][0]
                prefix_only = name != best + "Test"
        convention[name] = conv
        recs, nameds, traces = [], [], []
        for kind, acc, ptrace in lst:
            ps = set()
            if kind == "integration":
                recs.append([])
                nameds.append(False)
                traces.append(None)
                continue
            traces.append(sorted({unit_key(t) for t in ptrace}) if isinstance(ptrace, list) else None)
            # The prefix form of rule 1 (T starts with S + upper-case letter, T != S + 'Test')
            # applies only when the record's processor dump names P, or names no src pass-role class at all;
            # a test class whose processor is a different pass (Es6RewriteClassExtendsExpressionsTest runs
            # Es6NormalizeClasses) is not attributed to P by its name.
            named_passes = {outer(c) for c in acc if outer(c) not in WRAPPERS
                            and (roles.get(c) == "pass" or roles.get(outer(c)) == "pass")}
            if conv and (not prefix_only or conv in named_passes or not named_passes):
                # D-017 item 11: named after a pass -> that pass only.
                recs.append([unit_key(conv)])
                nameds.append(True)
                continue
            nameds.append(False)
            for c in acc:
                o = outer(c)
                if o in WRAPPERS:
                    continue
                if roles.get(c) == "pass" or roles.get(o) == "pass":
                    ps.add(o)
            if kind == "type_check":
                ps.add("com.google.javascript.jscomp.TypeCheck")
            recs.append(sorted({unit_key(x) for x in ps}))
        mapping[name] = recs
        named_map[name] = nameds
        trace_map[name] = traces
    return mapping, convention, named_map, trace_map


# Gate code that determines phase results (not the report): a change to any of these
# invalidates every cached phase result, exactly like a change to the records or descriptors.
PHASE_CODE = ("class_source_file", "rule6_scan", "ensure_tools", "src_top_classes", "nested_src_classes", "replay_sources",
              "compile_replay", "pass_roles", "fqcns_in", "outer", "compute_pass_map", "run_units",
              "replay_argv", "record_classes", "cmd_replay", "ctc_classes", "cmd_noop", "cmd_cov_replay",
              "cmd_cov_orig", "selected_passes", "cmd_mutate", "change_expecting", "noop_passable",
              "in_scope_pass", "in_scope_src", "build_noop_agent", "vacuity_noop_classes", "vnc_stem",
              "unit_key", "compile_gate_java", "change_facts", "d_units", "unit_members")


def build_noop_agent():
    """Class-level no-op mutation agent (HARNESS.md "Class-level no-op mutation", D-015 d)."""
    marker = f"{NOOP_AGENT_JAR}.done.json"
    digest = sha256(NOOP_AGENT_SRC) + sha256(TRACE_SRC) + sha256(NOOP_AGENT_BUILD) + sha256(TRACE_SCOPE_SCRIPT)
    if (os.path.exists(marker) and os.path.exists(NOOP_AGENT_JAR) and os.path.exists(TRACE_SCOPE)
            and jload(marker).get("sha") == digest):
        return
    os.makedirs(os.path.dirname(NOOP_AGENT_JAR), exist_ok=True)
    subprocess.run(["bash", NOOP_AGENT_BUILD, NOOP_AGENT_JAR], check=True, env=ENV)
    subprocess.run(["python3", TRACE_SCOPE_SCRIPT, TRACE_SCOPE], check=True, env=ENV)
    jdump(marker, {"sha": digest})


def compute_stamp():
    """sha256 over everything the phase results depend on: records, descriptors, hook stats, jars,
    JaCoCo jars, JVM flags and agent options, replay harness and helper sources, and the gate's own
    phase code (the source of PHASE_CODE plus ROLE_JAVA and WRAPPERS)."""
    import inspect
    h = hashlib.sha256()
    parts = {}
    for d in (REC, DESC, STATS):
        for fn in sorted(os.listdir(d)):
            h.update(fn.encode())
            h.update(sha256(os.path.join(d, fn)).encode())
    for f in (SUPPORT, ALLTESTS, AGENT, CLI):
        h.update(sha256(f).encode())
    h.update(json.dumps([JOPTS, AGENT_OPTS, PARALLEL > 0]).encode())
    _, rdigest = replay_sources()
    h.update(rdigest.encode())
    parts["replaySourcesSha256"] = rdigest
    parts["noopAgentSha256"] = sha256(NOOP_AGENT_SRC) + sha256(NOOP_AGENT_BUILD)
    h.update(parts["noopAgentSha256"].encode())
    g = hashlib.sha256()
    for name in PHASE_CODE:
        g.update(inspect.getsource(globals()[name]).encode())
    g.update(ROLE_JAVA.encode())
    g.update(EQUIV_JAVA.encode())
    g.update(PROBE_JAVA.encode())
    g.update(json.dumps([TYPECHECK_KEY, TYPECHECK_UNIT]).encode())
    g.update(json.dumps(sorted(WRAPPERS)).encode())
    with open(f"{ROOT}/build/unit/all_test_classes.txt", "rb") as f:
        g.update(f.read())
    parts["phaseCodeSha256"] = g.hexdigest()
    h.update(parts["phaseCodeSha256"].encode())
    return {"inputsSha256": h.hexdigest(), **parts}


def cmd_prep():
    os.makedirs(W, exist_ok=True)
    ensure_tools()
    digest = compile_replay()
    build_noop_agent()
    # Staleness: results are valid only for these exact inputs; otherwise archive them.
    stamp = compute_stamp()
    sp = f"{W}/stamp.json"
    if os.path.exists(sp) and jload(sp) != stamp:
        arch = f"{W}/stale-{time.strftime('%Y%m%d-%H%M%S')}"
        os.makedirs(arch)
        for x in ("replay", "cov", "mut", "noop", "passmap.json", "passrank.json", "passunits.json", "changefacts"):
            if os.path.exists(f"{W}/{x}"):
                shutil.move(f"{W}/{x}", f"{arch}/{x}")
        log("inputs changed; previous results archived to", arch)
    jdump(sp, stamp)
    counts = record_counts()
    r6 = rule6_scan()
    jdump(f"{W}/rule6.json", r6)
    log("rule6", "ok" if r6["ok"] else "VIOLATION", r6["classEntries"], "class entries")
    if not os.path.exists(f"{W}/passmap.json"):
        mapping, conv, named_map, trace_map = compute_pass_map(counts)
        jdump(f"{W}/passmap.json", {"records": mapping, "convention": conv, "named": named_map, "trace": trace_map})
    jdump(f"{W}/prep.json", {"recordCounts": counts, "replaySourcesSha256": digest,
                            "agent": {"version": JACOCO_VERSION, "url": AGENT_URL, "sha256": sha256(AGENT)},
                            "cli": {"version": JACOCO_VERSION, "url": CLI_URL, "sha256": sha256(CLI)}})
    log("prep done")


# ---------------------------------------------------------------- unit runner
def run_units(units, label):
    """units: list of (key, done_path, argv, cwd, log_path). Runs missing ones PARALLEL-wide."""
    todo = [u for u in units if not os.path.exists(u[1])]
    log(label, f"{len(units) - len(todo)} done, {len(todo)} to run")

    def one(u):
        key, done, argv, cwd, lp = u
        # A NoopAgent report (report=<path>, see replay_argv) is removed only when its unit actually
        # runs, so a stale report can never be read as proof; a finished unit keeps the report its own
        # run wrote (deleting it while building argv lost every report on a resumed run).
        for a in argv:
            if a.startswith(f"-javaagent:{NOOP_AGENT_JAR}=") and ",report=" in a:
                rpt = a.split(",report=", 1)[1]
                if os.path.exists(rpt):
                    os.remove(rpt)
        t0 = time.time()
        with open(lp, "w") as lf:
            rc = subprocess.run(argv, cwd=cwd, stdout=lf, stderr=subprocess.STDOUT, env=ENV).returncode
        res = {"key": key, "rc": rc, "secs": round(time.time() - t0, 1)}
        with open(lp, errors="replace") as lf:
            txt = lf.read()
        m = re.findall(r"^REPLAY (\S+) records=(\d+) pass=(\d+) fail=(\d+)$", txt, re.M)
        if m:
            res["replay"] = {a: {"records": int(b), "pass": int(c), "fail": int(d)} for a, b, c, d in m}
        jdump(done, res)
        return key, rc, res["secs"]

    with cf.ThreadPoolExecutor(PARALLEL) as ex:
        for i, (key, rc, secs) in enumerate(ex.map(one, todo)):
            log(label, f"[{i + 1}/{len(todo)}]", key, "rc", rc, f"{secs}s")


def replay_argv(name, out, mutate=None, exec_file=None, post_out=None, noop=False, proc_out=None, sig_out=None,
                trace_out=None):
    a = [JAVA] + JOPTS
    if exec_file:
        a.append(f"-javaagent:{AGENT}=destfile={exec_file},{AGENT_OPTS}")
    if trace_out:
        # D-017 item 8: NoopAgent TRACE mode (no class is no-op'd); ReplayMain compares each record's
        # passTrace with the replayed one (a difference fails the record) and writes the statuses.
        assert not mutate, "trace mode and no-op mutation are separate runs"
        a += [ASM_EXPORT, f"-javaagent:{NOOP_AGENT_JAR}=trace={TRACE_SCOPE}"]
    if mutate:
        # D-015 (d): class-level no-op of the pass's entry points wherever it is constructed
        # (NoopAgent rewrites the class bytes at load time; src/ is untouched). The agent writes
        # LOADED/REWROTE lines to <out stem>.agent.txt as proof that the mutation took effect.
        # The report is removed by run_units right before the unit runs, never here: argv is built
        # for finished units too.
        rpt = re.sub(r"\.failures\.jsonl$", "", out) + ".agent.txt"
        a += [ASM_EXPORT, f"-javaagent:{NOOP_AGENT_JAR}={mutate},report={rpt}"]
    a += ["-cp", f"{REPLAY_CLASSES}:{SUPPORT}", "com.google.javascript.jscomp.ReplayMain",
          "--records", REC, "--descriptors", DESC, "--classes", name, "--out", out]
    if post_out:
        a += ["--post-out", post_out]
    if noop:
        a += ["--noop-processor"]
    if proc_out:
        a += ["--proc-out", proc_out]
    if sig_out:
        a += ["--sig-out", sig_out]
    if trace_out:
        a += ["--trace-out", trace_out]
    return a


def record_classes():
    counts = jload(f"{W}/prep.json")["recordCounts"]
    return sorted([c for c, n in counts.items() if n > 0], key=lambda c: -counts[c])


def cmd_replay():
    d = f"{W}/replay"
    os.makedirs(d, exist_ok=True)
    units = [(c, f"{d}/{c}.done.json",
              replay_argv(c, f"{d}/{c}.failures.jsonl", post_out=f"{d}/{c}.post.jsonl",
                          proc_out=f"{d}/{c}.proc.jsonl", sig_out=f"{d}/{c}.sig.txt",
                          trace_out=f"{d}/{c}.trace.jsonl"), WS, f"{d}/{c}.log")
             for c in record_classes()]
    build_noop_agent()
    run_units(units, "replay")


def ctc_classes():
    """Record-bearing classes with at least one compiler_test_case record."""
    out = []
    for c in record_classes():
        for r in iter_records(c):
            if r["kind"] == "compiler_test_case":
                out.append(c)
                break
    return out


def vacuity_noop_classes(c):
    """Descriptor key `vacuityNoopClass` (FORMAT.md "No-op vacuity"): the src
    pass class(es) the harness itself runs and the class's records assert through (for example
    TypeCheck under enableTypeCheck), for a class whose recorded processor has no effect of its own.
    A string or a list of strings; anything else is returned as-is so the report fails it."""
    v = load_desc(c).get("vacuityNoopClass")
    if v is None:
        return []
    return [v] if isinstance(v, str) else (v if isinstance(v, list) else [v])


def vnc_stem(c, p):
    return f"{W}/noop/{c}.vnc.{str(p).rsplit('.', 1)[-1]}"


def cmd_noop():
    d = f"{W}/noop"
    os.makedirs(d, exist_ok=True)
    units = [(c, f"{d}/{c}.done.json", replay_argv(c, f"{d}/{c}.failures.jsonl", noop=True), WS, f"{d}/{c}.log")
             for c in ctc_classes()]
    # Harness-pass no-op (vacuityNoopClass): the class replayed with the class-level NoopAgent for each
    # named pass (the same agent and the same JVM flags as gate (d)).
    for c in ctc_classes():
        for p in vacuity_noop_classes(c):
            if isinstance(p, str) and re.fullmatch(r"[A-Za-z_][\w.$]*", p):
                st = vnc_stem(c, p)
                units.append((f"{c}@{p}", f"{st}.done.json", replay_argv(c, f"{st}.failures.jsonl", mutate=p),
                              WS, f"{st}.log"))
    run_units(units, "noop")


def cmd_cov_replay():
    d = f"{W}/cov/replay"
    os.makedirs(d, exist_ok=True)
    units = [(c, f"{d}/{c}.done.json",
              replay_argv(c, f"{d}/{c}.failures.jsonl", exec_file=f"{d}/{c}.exec"), WS, f"{d}/{c}.log")
             for c in record_classes()]
    run_units(units, "cov-replay")


def cmd_cov_orig():
    d = f"{W}/cov/orig"
    os.makedirs(f"{d}/rec", exist_ok=True)
    os.makedirs(f"{d}/stats", exist_ok=True)
    with open(f"{ROOT}/build/unit/all_test_classes.txt") as f:
        classes = [l.strip() for l in f if l.strip()]
    # biggest test sources first
    def size(c):
        try:
            return os.path.getsize(f"{WS}/test/{c.replace('.', '/')}.java")
        except OSError:
            return 0
    classes.sort(key=lambda c: -size(c))
    units = []
    for c in classes:
        n = c.rsplit(".", 1)[1]
        # Recording hook OFF (no -Dclosurers.unit.record): this is the original suite.
        argv = [JAVA] + JOPTS + [f"-javaagent:{AGENT}=destfile={d}/{n}.exec,{AGENT_OPTS}",
                                 "-cp", f"{SUPPORT}:{ALLTESTS}", "com.google.javascript.jscomp.UnitRecordingMain",
                                 f"{d}/rec", f"{d}/stats", n, c]
        units.append((n, f"{d}/{n}.done.json", ["timeout", "2400"] + argv, WS, f"{d}/{n}.log"))
    run_units(units, "cov-orig")


def compile_gate_java():
    """Compiles EQUIV_JAVA and PROBE_JAVA into build/gate02/java (package com.google.javascript.jscomp,
    against the replay classes and the support jar). Returns the class directory."""
    jd = f"{W}/java"
    pd = f"{jd}/com/google/javascript/jscomp"
    os.makedirs(pd, exist_ok=True)
    srcs = []
    for n, body in (("Gate02AstEquiv", EQUIV_JAVA), ("Gate02PostcondProbe", PROBE_JAVA)):
        p = f"{pd}/{n}.java"
        with open(p, "w") as f:
            f.write(body)
        srcs.append(p)
    subprocess.run([JAVAC, "-nowarn", "-d", jd, "-cp", f"{REPLAY_CLASSES}:{SUPPORT}"] + srcs, check=True, env=ENV)
    return jd


def change_facts():
    """D-017 item 10 inputs, computed once per stamp (build/gate02/changefacts/):
      code: per compiler_test_case record whose expected output is a file list, Gate02AstEquiv parses
        the input sources and the expected output in the record's language mode (options.languageIn;
        each file with the parser only, CompilerInput.getAstRoot, no module loading) and compares the
        scripts file by file with Node.isEquivalentTo, without normalization, with JSDoc compared when
        the record's comparison.compareJsDoc is set ("equivalent" / "different" / "error: ...", optionally
        annotated " (no languageIn: CompilerOptions default)" and " (JSDoc compared)");
      postcondition: per compiler_test_case record with expected.postconditions > 0, Gate02PostcondProbe
        replays it with the whole processor a no-op (the parsed, unprocessed input) and compares the
        observed postCall.postcondition state (D-017 item 1) with the recorded expected value
        ("equal" / "differs" / "noRecordedValue" / "noObservedValue" / "error: ...")."""
    d = f"{W}/changefacts"
    cache = f"{d}/changefacts.json"
    if os.path.exists(cache):
        return jload(cache)
    os.makedirs(d, exist_ok=True)
    jd = compile_gate_java()
    cp = f"{jd}:{REPLAY_CLASSES}:{SUPPORT}"
    classes = record_classes()

    def read_tsv(path, acc):
        with open(path, encoding="utf-8") as f:
            for line in f:
                parts = line.rstrip("\n").split("\t")
                if len(parts) == 3:
                    acc.setdefault(parts[0], {})[parts[1]] = parts[2]
    code, post = {}, {}
    eq_out = f"{d}/equiv.tsv"
    with open(f"{d}/equiv.log", "w") as lf:
        subprocess.run([JAVA] + JOPTS + ["-cp", cp, "com.google.javascript.jscomp.Gate02AstEquiv", REC, eq_out]
                       + classes, check=True, env=ENV, cwd=WS, stdout=lf, stderr=subprocess.STDOUT)
    read_tsv(eq_out, code)
    pc_classes = [c for c in classes if any(r["kind"] == "compiler_test_case"
                                            and ((r.get("expected") or {}).get("postconditions") or 0) > 0
                                            for r in iter_records(c))]
    for c in pc_classes:
        out = f"{d}/post.{c}.tsv"
        with open(f"{d}/post.{c}.log", "w") as lf:
            rc = subprocess.run(["timeout", "1800", JAVA] + JOPTS + ["-cp", cp, "com.google.javascript.jscomp.Gate02PostcondProbe",
                                REC, DESC, out, c], env=ENV, cwd=WS, stdout=lf, stderr=subprocess.STDOUT).returncode
        if os.path.exists(out):
            read_tsv(out, post)
        log("changefacts postcondition", c, "rc", rc)
    facts = {"code": code, "postcondition": post}
    jdump(cache, facts)
    return facts


def change_expecting(cls, i, r, facts):
    """D-017 item 10 (gate 0.2 (d)): the record expects
      - an output AST different from its input: the input and the expected output, parsed in the
        record's language mode, are not isEquivalentTo (no normalization; a text-only difference is
        no-change; a record whose comparison could not be made counts as change-expecting); or
      - a diagnostic (unchanged); or
      - a postcondition whose expected value differs from the value observed on the parsed,
        unprocessed input (whole-processor no-op replay; a record whose comparison could not be made
        counts as change-expecting).
    Integration records (never attributed to a pass) count as change-expecting, as before."""
    e = r.get("expected") or {}
    if r["kind"] == "type_check":
        return bool(e.get("diagnosticTypes") or e.get("diagnosticDescriptions"))
    if r["kind"] != "compiler_test_case" or (e.get("diagnostics") or []):
        return True
    o = e.get("output")
    if o is not None and o != "SAME":
        # The status may carry annotations: " (no languageIn: CompilerOptions default)" (the options dump
        # holds only non-default fields, so a missing languageIn is the CompilerOptions default, the mode
        # Gate02AstEquiv parses with: still "the record's language mode") and " (JSDoc compared)".
        st = (facts["code"].get(cls) or {}).get(str(i))
        if not isinstance(o, list) or st is None or st.split(" (", 1)[0] != "equivalent":
            return True
    if (e.get("postconditions") or 0) > 0:
        return (facts["postcondition"].get(cls) or {}).get(str(i)) != "equal"
    return False


def in_scope_pass(fq):
    return in_scope_src(fq)


def unit_members(key):
    """The classes the NoopAgent no-ops jointly for a ranked pass key (D-017 item 11)."""
    return list(TYPECHECK_UNIT) if key == TYPECHECK_KEY else [key]


def d_units(top, pm):
    """D-017 item 11 measurement units over the 30 ranked passes R:
      - a record whose test class is named after a pass (compute_pass_map rule 1) belongs to that pass
        only (when it is ranked);
      - otherwise the record belongs to the ranked passes whose entry points its passTrace shows
        executing; if its trace shows none, to its ranked candidates (no record leaves the measurement);
      - one ranked pass -> that pass; two or more -> that group, no-op'd jointly (NoopAgent A;B;C);
      - the type-check unit {TypedScopeCreator, TypeInferencePass, TypeCheck} is one ranked pass, no-op'd
        jointly; TypeCheck alone is measured on the unit's records for information only.
    Returns a list of {key, kind (pass|group|info), label, dir, members, records {class: [index]}}."""
    rset = {p for p, _ in top}
    single = {p: {} for p, _ in top}
    groups = {}
    for cls, recs in sorted(pm["records"].items()):
        nameds = pm["named"][cls]
        traces = pm["trace"][cls]
        for i, ps in enumerate(recs):
            if traces[i] is None and not ps:
                continue
            if nameds[i]:
                u = set(ps) & rset
            else:
                u = set(traces[i] or []) & rset
                if not u:
                    u = set(ps) & rset
            if len(u) == 1:
                single[next(iter(u))].setdefault(cls, []).append(i)
            elif len(u) > 1:
                groups.setdefault(tuple(sorted(u)), {}).setdefault(cls, []).append(i)
    units = []
    for p, _ in top:
        simple = p.rsplit(".", 1)[1]
        units.append({"key": p, "kind": "pass", "label": simple + (" (unit: TypedScopeCreator, TypeInferencePass, TypeCheck)" if p == TYPECHECK_KEY else ""),
                      "dir": simple + (".unit" if p == TYPECHECK_KEY else ""), "members": unit_members(p),
                      "records": single[p]})
    for g, recs in sorted(groups.items()):
        members = sorted({m for p in g for m in unit_members(p)})
        label = "{" + ", ".join(p.rsplit(".", 1)[1] for p in g) + "}"
        units.append({"key": ";".join(g), "kind": "group", "label": label,
                      "dir": "group-" + hashlib.sha256(";".join(g).encode()).hexdigest()[:12],
                      "members": members, "records": recs})
    if TYPECHECK_KEY in rset:
        units.append({"key": TYPECHECK_KEY + "#alone", "kind": "info", "label": "TypeCheck alone (information, D-017 item 11)",
                      "dir": "TypeCheck.alone", "members": [TYPECHECK_KEY], "records": single[TYPECHECK_KEY]})
    return units


def selected_passes():
    """D-015 (d), D-017 items 10 and 11: the 30 in-scope passes (type-check unit as one) with the most
    change-expecting records (ranking candidates from compute_pass_map), and the measurement units."""
    pm = jload(f"{W}/passmap.json")
    facts = change_facts()
    cache = f"{W}/passrank.json"
    if os.path.exists(cache):
        cnt = jload(cache)
    else:
        cnt = {}
        for cls, recs in pm["records"].items():
            if not any(recs):
                continue
            for i, (r, ps) in enumerate(zip(iter_records(cls), recs)):
                if not change_expecting(cls, i, r, facts):
                    continue
                for p in ps:
                    if in_scope_pass(p):
                        cnt[p] = cnt.get(p, 0) + 1
        jdump(cache, cnt)
    ranked = sorted(cnt.items(), key=lambda kv: (-kv[1], kv[0]))
    top = ranked[:30]
    ucache = f"{W}/passunits.json"
    if os.path.exists(ucache):
        units = jload(ucache)
    else:
        units = d_units(top, pm)
        jdump(ucache, units)
    return top, ranked, pm, units, facts


def cmd_mutate():
    top, _, pm, units, _ = selected_passes()
    out = []
    for u in units:
        d = f"{W}/mut/{u['dir']}"
        os.makedirs(d, exist_ok=True)
        for cls in sorted(u["records"]):
            out.append((f"{u['dir']}/{cls}", f"{d}/{cls}.done.json",
                        replay_argv(cls, f"{d}/{cls}.failures.jsonl", mutate=";".join(u["members"])), WS, f"{d}/{cls}.log"))
    # longest first
    counts = jload(f"{W}/prep.json")["recordCounts"]
    out.sort(key=lambda u: -counts[u[0].split("/")[1]])
    run_units(out, "mutate")


# ---------------------------------------------------------------- report
def read_failures(path):
    out = {}
    if os.path.exists(path):
        with open(path, encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    j = json.loads(line)
                    out[j["index"]] = j
    return out


def read_post(path):
    """Post-call state lines written by ReplayMain --post-out: index -> {postState, reason}."""
    return read_failures(path)


def load_desc(c):
    fn = f"{DESC}/{c}.json"
    return jload(fn) if os.path.exists(fn) else {}


def captured_postcondition_indices(r):
    """D-017 item 1 (FORMAT.md "Postcondition data"): indices i of expected.postconditionValues whose
    lambda's asserted state the recorder captured as replay-compared post-call data
    (expected.postconditionsCaptured, valid only when postCall.postcondition is present)."""
    ex = r.get("expected") or {}
    pc = r.get("postCall")
    if not isinstance(pc, dict) or not isinstance(pc.get("postcondition"), dict):
        return set()
    return {i for i in (ex.get("postconditionsCaptured") or []) if isinstance(i, int)}


def placeholder_sections(r):
    """Top-level record sections that hold a {"unrepresentable": ...} placeholder anywhere, except the
    lambda placeholders of postconditions whose asserted state is captured (D-017 item 1)."""
    found = set()
    skip = captured_postcondition_indices(r)

    def walk(x, sec):
        if isinstance(x, dict):
            if isinstance(x.get("unrepresentable"), str):
                found.add(sec)
            for v in x.values():
                walk(v, sec)
        elif isinstance(x, list):
            for v in x:
                walk(v, sec)

    for k, v in r.items():
        if k == "unrepresentable":
            continue
        if k == "expected" and skip and isinstance(v, dict):
            for k2, v2 in v.items():
                if k2 == "postconditionValues" and isinstance(v2, list):
                    for i, x in enumerate(v2):
                        if i not in skip:
                            walk(x, k)
                else:
                    walk(v2, k)
            continue
        walk(v, k)
    return found


def _postcall_absent_key(rec, via):
    """D-017 item 3 (FORMAT.md "postCall key-set equality"): postCallAbsent.<compiler|pass>.<key> claims
    that the asserted state is the absence of that key. Valid because ReplayMain compares postCall by
    JsonObject equality (exact key sets): the record must have a postCall snapshot whose section lacks
    the key (pass keys also without the "pass." prefix)."""
    parts = via.split(".", 2)
    if len(parts) != 3 or parts[0] != "postCallAbsent" or parts[1] not in ("compiler", "pass"):
        return False
    pc = rec.get("postCall")
    if not isinstance(pc, dict) or not isinstance(pc.get(parts[1]), dict):
        return False
    sec = pc[parts[1]]
    return parts[2] not in sec and not (parts[1] == "pass" and f"pass.{parts[2]}" in sec)


def _postcall_key(rec, via):
    """FORMAT.md postcall schema: postCall.<compiler|pass|postcondition>.<key>; pass keys may omit
    "pass.". The postcondition section (D-017 item 1, FORMAT.md "Postcondition data") is part of the
    postCall object replay compares by JsonObject equality, like the other two sections."""
    parts = via.split(".", 2)
    if len(parts) != 3 or parts[0] != "postCall" or parts[1] not in ("compiler", "pass", "postcondition"):
        return False
    sec = (rec.get("postCall") or {}).get(parts[1]) or {}
    return parts[2] in sec or (parts[1] == "pass" and f"pass.{parts[2]}" in sec)


# Integration records may claim the observed state replay compares JSON-equal
# (FORMAT.md "corpus/unit/postcall/<Class>.json").
OBSERVED_VIA = ("observed.errors", "observed.warnings", "observed.output")
# A rust_unit_test reason containing one of these is rejected (lower-case match).
RUT_REJECT_MARKERS = ("not java-internal", "not a java-internal", "interim", "provisional",
                      # Reasons that say the value is capturable or already replayed, or
                      # that the label is temporary, are not Java-internal assertions either.
                      "capturable", "until then", "harness request", "deferred", "in fact replayed",
                      "until the gate accepts")
# A rust_unit_test reason with category graph or exception (or none) that names a
# value replay compares (observed.*, the outcome) contradicts its category: such a method is captured
# (via observed.*/outcome), not a Rust unit test.
RUT_REPLAYED_VALUE_MARKERS = ("observed.", "outcome")
# Closed list of Java-internal categories a rust_unit_test entry must name
# (FORMAT.md "corpus/unit/postcall/<Class>.json"). Neutral values (strings, numbers, sets/maps of
# them, diagnostics, output code) are never in this list: they must be captured by a snapshot key.
RUT_CATEGORIES = {
    "jstype",            # JSType / FunctionType / ObjectType / TemplateType objects and their registry
    "jsdocinfo",         # JSDocInfo / comment objects attached to nodes
    "color",             # Color / ColorRegistry objects
    "node-identity",     # Node object identity, Node props/flags, source positions of Node objects
    "graph",             # scope / var / reference / module-metadata / call-graph object graphs
    "exception",         # Java exception objects thrown by the hooked call (type/message)
    "test-local-object", # objects built by test-local Java code (callbacks, custom passes)
}


def load_postcall(c):
    """corpus/unit/postcall/<c>.json -> ({(recordClass, method): entry}, [schema errors])."""
    fn = f"{POSTCALL}/{c}.json"
    if not os.path.exists(fn):
        return {}, []
    errs, out = [], {}
    try:
        d = jload(fn)
    except Exception as e:  # noqa
        return {}, [f"{c}: unreadable postcall file: {e!r}"]
    if d.get("v") != 1 or not isinstance(d.get("methods"), list):
        errs.append(f"{c}: postcall file needs v=1 and a methods list")
        return {}, errs
    for x in d["methods"]:
        m = x.get("method")
        fq = x.get("recordClass") or d.get("class")
        if not m or not fq:
            errs.append(f"{c}: entry without method/class: {str(x)[:120]}")
            continue
        if (fq, m) in out:
            errs.append(f"{c}: duplicate entry {fq}#{m}")
            continue
        st = x.get("status")
        if st == "captured":
            via = x.get("via")
            if not isinstance(via, list) or not via or not all(
                    isinstance(v, str) and (v in ("postconditions", "outcome") or v.startswith("postCall.")
                                            or v.startswith("postCallAbsent.")
                                            or v.startswith("testFieldsAfter.") or v in OBSERVED_VIA) for v in via):
                errs.append(f"{c}#{m}: captured needs a non-empty via list of postCall.*, postCallAbsent.*, testFieldsAfter.*, "
                            f"postconditions, outcome or (integration records) {', '.join(sorted(OBSERVED_VIA))}")
                continue
        elif st == "rust_unit_test":
            frag = x.get("fragment") or ""
            fp = os.path.join(ROOT, frag)
            if not (x.get("reason") or "").strip():
                errs.append(f"{c}#{m}: rust_unit_test needs a reason")
                continue
            # D-015 a: rust_unit_test means "the assertion inspects Java-internal
            # objects". A reason that itself says the asserted state is not Java-internal, or that
            # the label is interim/provisional (pending a snapshot key), is not that class: the method
            # stays unclassified until it is captured.
            low = x["reason"].lower()
            bad_marker = next((k for k in RUT_REJECT_MARKERS if k in low), None)
            if bad_marker:
                errs.append(f"{c}#{m}: rust_unit_test reason contains {bad_marker!r}; not a Java-internal "
                            f"assertion (D-015 a): capture it or leave it unclassified")
                continue
            rv = next((k for k in RUT_REPLAYED_VALUE_MARKERS if k in low), None)
            # Applies to the categories whose asserted value could itself be the replayed value
            # (a diagnostic or the thrown exception); a node-identity/jsdocinfo reason may name
            # observed.output to say the asserted node property is not in the printed code.
            if rv and x.get("category") in ("graph", "exception", None):
                errs.append(f"{c}#{m}: rust_unit_test reason names {rv!r}, a value replay compares; category "
                            f"{x.get('category')!r} contradicts it: use status captured (via observed.*/outcome)")
                continue
            if x.get("category") not in RUT_CATEGORIES:
                errs.append(f"{c}#{m}: rust_unit_test needs a category from the closed Java-internal list "
                            f"{sorted(RUT_CATEGORIES)}, got {x.get('category')!r}")
                continue
            held = [k for k in (x.get("postCallKeysChecked") or [])]
            if held:
                errs.append(f"{c}#{m}: rust_unit_test lists postCall keys {held}; use status captured")
                continue
            if not (frag.startswith("corpus/unit/rust_unit_tests/") and os.path.exists(fp)):
                errs.append(f"{c}#{m}: rust_unit_test fragment {frag!r} does not exist under corpus/unit/rust_unit_tests/")
                continue
            with open(fp, encoding="utf-8") as fh:
                if not any(l.startswith(f"- {c}#{m}:") for l in fh):
                    errs.append(f"{c}#{m}: fragment {frag} has no line '- {c}#{m}: ...'")
                    continue
        elif st == "unclassified":
            # An explicit entry for a method that is neither captured nor a Rust unit
            # test yet (the D-017 item 2 neutral key it needs is not produced). It counts as unclassified;
            # `pending` says what would capture it.
            if not (x.get("pending") or "").strip():
                errs.append(f"{c}#{m}: an explicit unclassified entry needs a pending text")
                continue
        else:
            errs.append(f"{c}#{m}: status must be captured, rust_unit_test or unclassified, got {st!r}")
            continue
        out[(fq, m)] = x
    return out, errs


def post_call_audit(post_by_class, fails_by_class, marked):
    """D-015 (a): the static post-call-assertion scan (gates/lib/unit_postassert_scan.py) joined with
    the replay's post-call state and corpus/unit/postcall/<Class>.json (FORMAT.md). A flagged method is
    classified when (1) its postcall entry is `captured` and every claim in `via` holds for every
    record of the method (there is no mechanical rule), or (2) its postcall
    entry is `rust_unit_test` (every record of the method is excluded) and the
    reason names every postCall key its records carry. Records with replay post state
    "unclassified" (testFieldsAfter changed, not compared) are resolved by the same entries.
    Descriptor `postCallAssertions` no longer classify anything (D-015 replaced them)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import unit_postassert_scan
    flagged = unit_postassert_scan.scan(REC)  # raises UnresolvedSource (fails the report loudly)
    unit_postassert_scan.self_test(flagged)
    rows, unclassified, rut_recs, errors, legacy_desc = [], [], {}, [], []
    entries_by_file, recs_by_file, used = {}, {}, set()

    def entries(c):
        if c not in entries_by_file:
            e, er = load_postcall(c)
            entries_by_file[c] = e
            errors.extend(er)
        return entries_by_file[c]

    def recs(c):
        if c not in recs_by_file:
            recs_by_file[c] = list(iter_records(c))
        return recs_by_file[c]

    def verify(c, fq, m, entry, idx):
        post, fails = post_by_class.get(c, {}), fails_by_class.get(c, {})
        rl = recs(c)
        bad = []
        for i in idx:
            r, x = rl[i], (post.get(i) or {})
            if i in fails or i in marked.get(c, {}):
                bad.append(f"record {i} does not pass in (a)")
                continue
            for v in entry["via"]:
                if v.startswith("postCall.") and not _postcall_key(r, v):
                    bad.append(f"record {i}: {v} not in its postCall snapshot")
                elif v.startswith("postCallAbsent.") and not _postcall_absent_key(r, v):
                    bad.append(f"record {i}: {v} is present in (or has no) postCall snapshot section")
                elif v.startswith("testFieldsAfter."):
                    # The compared field set replay reports per record (ReplayMain --post-out
                    # comparedFields: testFieldsAfter keys plus testFieldsAfterAlso, minus Skip).
                    f_ = v.split(".", 1)[1]
                    if x.get("postState") != "checked" or f_ not in (x.get("comparedFields") or []):
                        bad.append(f"record {i}: {v} not compared by replay")
                elif v in OBSERVED_VIA:
                    # Replay's pass rule compares every recorded observed key JSON-equal, and the record
                    # passes (checked above); the claim is valid only for integration records.
                    k = v.split(".", 1)[1]
                    if r.get("kind") != "integration" or k not in (r.get("observed") or {}):
                        bad.append(f"record {i}: {v} needs an integration record whose observed holds {k}")
                elif v == "outcome":
                    # Replay compares the outcome's status, exception class and message
                    # exactly (ReplayMain.compare); the claim covers an asserted compiler exception only.
                    o = r.get("outcome") or {}
                    if o.get("status") != "exception" or o.get("assertion") or o.get("message") in (None, ""):
                        bad.append(f"record {i}: outcome claim needs a non-assertion exception with a message")
                elif v == "postconditions":
                    want = (r.get("expected") or {}).get("postconditions") or 0
                    if want <= 0 or x.get("postconditionsReplayed") != want:
                        bad.append(f"record {i}: postconditions {want} recorded, {x.get('postconditionsReplayed')} replayed")
        # A testFieldsAfter.<f> claim must name state the hooked call produced: f must
        # have changed during the call (be a key of testFieldsAfter) in at least one record of the
        # method. A field only compared through testFieldsAfterAlso with its entry value (input state
        # the test set itself, e.g. GatherExternPropertiesTest.mode) is not captured post-call state.
        for v in entry["via"]:
            if v.startswith("testFieldsAfter.") and idx:
                f_ = v.split(".", 1)[1]
                if not any(f_ in (rl[i].get("testFieldsAfter") or {}) for i in idx):
                    bad.append(f"{v}: the field never changed during the hooked call (input state, not post-call state)")
        return bad

    # Cross-check: a rust_unit_test reason must name each post-call snapshot key of its
    # records that is a producer for the asserted kind of object, and so say why that key does not hold
    # the asserted state: every pass-level key (postCall.pass.*, pass-specific producers such as
    # referenceMap or crossChunkReferences), and the compiler-level producers of the entry's category.
    CATEGORY_COMPILER_KEYS = {"graph": {"moduleMetadataByPath"}, "jstype": {"typeMismatches", "typeMismatchesError"}}

    def postcall_keys(c, idx, category=None):
        ks = set()
        for i in idx:
            for sec, d_ in ((recs(c)[i].get("postCall") or {}).items()):
                if not isinstance(d_, dict):
                    continue
                for k in d_:
                    last = k.split(".")[-1]
                    if sec == "pass" or last in CATEGORY_COMPILER_KEYS.get(category, ()):
                        ks.add(last)
        return ks

    for fq, ms in sorted(flagged.items()):
        c = unit_postassert_scan.RECFILE[fq]
        d = load_desc(c)
        rl = recs(c)
        post = post_by_class.get(c, {})
        for m, info in sorted(ms.items()):
            idx = [i for i, r in enumerate(rl) if r["method"].split("[")[0] == m and r["class"] == fq]
            checked = bool(idx) and all((post.get(i) or {}).get("postState") == "checked" for i in idx)
            entry = entries(c).get((fq, m))
            if entry is not None:
                used.add((c, fq, m))
            if any(x.get("methods") == "*" or m in (x.get("methods") or []) for x in (d.get("postCallAssertions") or [])):
                legacy_desc.append(f"{c}#{m}")
            # The former "mechanical" rule (every record's testFieldsAfter compared ->
            # captured, no entry) is removed: it never checked that a compared field holds the asserted
            # state. Every flagged method needs an explicit corpus/unit/postcall entry.
            if entry and entry["status"] == "captured":
                bad = verify(c, fq, m, entry, idx)
                if bad:
                    status, reason = "claim_invalid", "; ".join(bad[:3])
                    unclassified.append(f"{c}#{m} (captured claim does not hold: {bad[0]})")
                    for i in idx:
                        rut_recs.setdefault(c, {})[i] = "post-call assertions not captured: captured claim does not hold: " + bad[0]
                else:
                    status, reason = "captured", "via " + ", ".join(entry["via"])
            elif entry and entry["status"] == "rust_unit_test" and [
                    k for k in sorted(postcall_keys(c, idx, entry.get("category"))) if k not in entry["reason"]]:
                miss = [k for k in sorted(postcall_keys(c, idx, entry.get("category"))) if k not in entry["reason"]]
                status, reason = "claim_invalid", f"rust_unit_test reason does not name the records' postCall keys {miss}"
                unclassified.append(f"{c}#{m} (rust_unit_test, records carry postCall keys {miss} the reason does not address)")
                for i in idx:
                    rut_recs.setdefault(c, {})[i] = "post-call assertions not captured: " + reason
            elif entry and entry["status"] == "rust_unit_test":
                status, reason = "rust_unit_test", entry["reason"]
                for i in idx:
                    rut_recs.setdefault(c, {})[i] = "post-call assertions ported as a Rust unit test: " + entry["reason"]
            elif entry and entry["status"] == "unclassified":
                status, reason = "unclassified", "explicit: " + entry["pending"]
                unclassified.append(f"{c}#{m} (explicit unclassified entry: {entry['pending'][:120]})")
                for i in idx:
                    rut_recs.setdefault(c, {})[i] = "post-call assertions not captured (method unclassified): " + entry["pending"]
            else:
                status, reason = "unclassified", ""
                unclassified.append(f"{c}#{m}")
                for i in idx:
                    rut_recs.setdefault(c, {})[i] = "post-call assertions not captured (method unclassified)"
            rows.append({"class": c, "recordClass": fq, "method": m, "records": len(idx), "status": status,
                         "reason": reason[:200], "allRecordsPostStateChecked": checked, **info})
    # Integration api=compile records (replay post state notCaptured with integrationCompile): resolved
    # when the static scan flags no post-call assertion in their method (the outcome and every observed
    # key are compared by replay), or when their method's captured claim was verified above.
    flagged_keys = {(fq, m) for fq, ms in flagged.items() for m in ms}
    row_status = {(r["recordClass"], r["method"]): r["status"] for r in rows}
    ic_resolved = {}
    for c, post in post_by_class.items():
        for i, x in post.items():
            if x.get("postState") != "notCaptured" or not x.get("integrationCompile"):
                continue
            r = recs(c)[i]
            k = (r["class"], r["method"].split("[")[0])
            if k not in flagged_keys:
                ic_resolved[(c, i)] = "noPostCallAssertion"
            elif row_status.get(k) == "captured":
                ic_resolved[(c, i)] = "capturedClaimVerified"
    # Records whose replay post state is "unclassified" (testFieldsAfter changed but not compared):
    # resolved by a postcall entry of their method (also allowed for methods the scan did not flag).
    post_resolved, post_left = {}, []
    for c, post in post_by_class.items():
        for i, x in post.items():
            if x.get("postState") != "unclassified":
                continue
            r = recs(c)[i]
            m, fq = r["method"].split("[")[0], r["class"]
            entry = entries(c).get((fq, m))
            if entry is not None:
                used.add((c, fq, m))
            if entry and entry["status"] == "captured" and not verify(c, fq, m, entry, [i]):
                post_resolved[(c, i)] = "captured"
            elif entry and entry["status"] == "rust_unit_test":
                post_resolved[(c, i)] = "rust_unit_test"
                rut_recs.setdefault(c, {})[i] = "post-call state ported as a Rust unit test: " + entry["reason"]
            else:
                post_left.append({"class": c, "index": i, "method": x.get("method"), "why": x.get("reason", "")[:200]})
                rut_recs.setdefault(c, {}).setdefault(i, "post-call state not captured (unclassified post state)")
    # Entries naming a method that is neither flagged nor has unclassified post state: extra.
    for c, es in entries_by_file.items():
        for (fq, m) in es:
            if (c, fq, m) not in used:
                errors.append(f"{c}: entry {fq}#{m} names a method that is neither flagged by the scan nor has unclassified post state")
    if os.path.isdir(POSTCALL):
        for fn in sorted(os.listdir(POSTCALL)):
            if fn.endswith(".json"):
                entries(fn[:-5])
                if not os.path.exists(f"{REC}/{fn[:-5]}.jsonl.gz"):
                    errors.append(f"{fn}: no record file {fn[:-5]}.jsonl.gz")
    st = collections_counter([r["status"] for r in rows])
    return {"methods": len(rows), "unclassified": len(unclassified), "unclassifiedList": unclassified[:300],
            "captured": st.get("captured", 0), "rustUnitTest": st.get("rust_unit_test", 0),
            "claimInvalid": st.get("claim_invalid", 0), "schemaErrors": errors[:100], "schemaErrorCount": len(errors),
            "legacyDescriptorPostCallAssertionsIgnored": legacy_desc[:100], "legacyDescriptorCount": len(legacy_desc),
            "postStateResolvedByPostcall": collections_counter(post_resolved.values()),
            "integrationCompileResolved": collections_counter(ic_resolved.values()),
            "integrationCompileResolvedByClass": collections_counter([c for (c, _) in ic_resolved]),
            "perClass": {c: collections_counter([r["status"] for r in rows if r["class"] == c])
                         for c in sorted({r["class"] for r in rows})},
            "perMethod": rows}, rut_recs, post_left, ic_resolved


# FORMAT.md "Post-call state", rule 1a: post-call values whose JSON dump holds the
# private state of a Java library object cannot be reproduced by a non-Java harness from Closure
# semantics, so neutral equality on them is no capture. An `object` dump is library-internal when its
# class is in one of LIB_PREFIXES, or when it is a protobuf message (its fields include a protobuf
# memo cache or the unknown-field set).
LIB_PREFIXES = ("com.google.protobuf.", "com.google.common.")
PROTO_INTERNAL_FIELDS = ("memoizedIsInitialized", "memoizedSize", "memoizedHashCode", "unknownFields")


def library_internal_objects(x, acc, path=""):
    if isinstance(x, dict):
        o = x.get("object")
        if isinstance(o, str) and isinstance(x.get("fields"), dict):
            if o.startswith(LIB_PREFIXES) or any(f in x["fields"] for f in PROTO_INTERNAL_FIELDS):
                acc.append(f"{path}: {o}")
                return
        for k, v in x.items():
            library_internal_objects(v, acc, f"{path}.{k}" if path else k)
    elif isinstance(x, list):
        for j, v in enumerate(x):
            library_internal_objects(v, acc, f"{path}[{j}]")


def demote_library_internal_post_state(post_by_class):
    """Records whose replay post state is "checked" but whose compared testFieldsAfter values hold a
    library-internal object dump become "unclassified" (resolved only by a corpus/unit/postcall entry,
    like any other unclassified post state). Returns {class: count}."""
    out = {}
    for c, post in post_by_class.items():
        todo = [i for i, x in post.items() if x.get("postState") == "checked"]
        if not todo:
            continue
        want = set(todo)
        for i, r in enumerate(iter_records(c)):
            if i not in want:
                continue
            x = post[i]
            vals = dict(r.get("testFields") or {})
            vals.update(r.get("testFieldsAfter") or {})
            acc = []
            for f in x.get("comparedFields") or []:
                library_internal_objects(vals.get(f), acc, f)
            if acc:
                x["postState"] = "unclassified"
                x["checkedByReplay"] = True
                x["reason"] = ("compared post-call value holds library-internal object dumps (FORMAT.md rule 1a): "
                               + "; ".join(acc[:3]))
                out[c] = out.get(c, 0) + 1
    return out


def collections_counter(xs):
    out = {}
    for x in xs:
        out[x] = out.get(x, 0) + 1
    return out


def desc_marked_unrep():
    out = {}
    for fn in sorted(os.listdir(DESC)):
        d = jload(f"{DESC}/{fn}")
        for u in d.get("unrepresentable", []) or []:
            if isinstance(u, dict) and "index" in u:
                out.setdefault(fn[:-5], {})[u["index"]] = u.get("reason", "")
    return out


HARNESS_BASES = ("CompilerTestCase", "IntegrationTestCase", "CompilerTypeTestCase")


def harness_hierarchy_classes():
    """Simple names of concrete test classes in the pristine test/ tree whose superclass chain reaches
    one of HARNESS_BASES (gate (c) denominator)."""
    root = f"{ROOT}/reference/closure-compiler/test/"
    parent, abstract = {}, set()
    pat = re.compile(r"^\s*(?:public\s+|protected\s+|private\s+)?((?:abstract|final|static|\s)*)class\s+(\w+)"
                     r"(?:<[^{]*?>)?\s+extends\s+([\w.]+)", re.M)
    for dp, _, fns in os.walk(root):
        for fn in fns:
            if not fn.endswith(".java"):
                continue
            with open(os.path.join(dp, fn), encoding="utf-8", errors="replace") as f:
                src = f.read()
            for m in pat.finditer(src):
                if m.group(2) != fn[:-5]:
                    continue  # top-level class only
                parent[m.group(2)] = m.group(3).rsplit(".", 1)[-1]
                if "abstract" in m.group(1):
                    abstract.add(m.group(2))
    out = set()
    for c in parent:
        seen, x = set(), c
        while x in parent and x not in seen and x not in HARNESS_BASES:
            seen.add(x)
            x = parent[x]
        if x in HARNESS_BASES and c not in abstract and c not in HARNESS_BASES and c.endswith("Test"):
            out.add(c)
    return out, {"bases": list(HARNESS_BASES), "topLevelClassesParsed": len(parent), "concreteHarnessClasses": len(out)}


def _find_key(obj, key):
    """First non-blank string value of `key` anywhere in a descriptor (or None)."""
    if isinstance(obj, dict):
        v = obj.get(key)
        if isinstance(v, str) and v.strip():
            return v
        for x in obj.values():
            r = _find_key(x, key)
            if r:
                return r
    elif isinstance(obj, list):
        for x in obj:
            r = _find_key(x, key)
            if r:
                return r
    return None


def coverage_xml(execs, tag):
    """Merges exec files and writes a JaCoCo XML report over the src/ classes of the support jar."""
    cdir = f"{W}/cov/classfiles"
    if not os.path.exists(f"{cdir}.done"):
        shutil.rmtree(cdir, ignore_errors=True)
        os.makedirs(cdir)
        with zipfile.ZipFile(SUPPORT) as z:
            for n in z.namelist():
                if n.endswith(".class") and (n.startswith("com/google/javascript/") or n.startswith("com/google/debugging/")):
                    z.extract(n, cdir)
        open(f"{cdir}.done", "w").close()
    merged = f"{W}/cov/{tag}.merged.exec"
    xml = f"{W}/cov/{tag}.xml"
    if not os.path.exists(xml):
        subprocess.run([JAVA, "-Xmx3g", "-jar", CLI, "merge"] + execs + ["--destfile", merged], check=True, env=ENV,
                       stdout=subprocess.DEVNULL)
        subprocess.run([JAVA, "-Xmx3g", "-jar", CLI, "report", merged, "--classfiles", cdir, "--xml", xml],
                       check=True, env=ENV, stdout=subprocess.DEVNULL)
    per = {}
    root = ET.parse(xml).getroot()
    for pkg in root.iter("package"):
        pn = pkg.get("name")
        for sf in pkg.findall("sourcefile"):
            path = f"{pn}/{sf.get('name')}"
            if not os.path.exists(f"{PRISTINE_SRC}/{path}"):
                continue
            for c in sf.findall("counter"):
                if c.get("type") == "LINE":
                    per[path] = (int(c.get("covered")), int(c.get("missed")))
    return per


def _codes(files):
    return [f.get("code") for f in files or []]


ASSERTION_CLASSES = ("com.google.common.truth.AssertionErrorWithFacts", "com.google.common.truth.ComparisonFailureWithFacts",
                     "java.lang.AssertionError", "org.junit.ComparisonFailure", "junit.framework.AssertionFailedError",
                     "junit.framework.ComparisonFailure", "org.opentest4j.AssertionFailedError")


def _is_assert_cls(name):
    return bool(name) and (name in ASSERTION_CLASSES or name.startswith("com.google.common.truth.")
                           or name.endswith("AssertionError") or name.endswith("ComparisonFailure"))


def assertion_failure(fail, rec):
    """A replay failure counts as a detected change (gate (d), and the (a) whole-
    processor no-op check) only when it is assertion-type: the replay threw an assertion error or
    comparison failure where the original did not (or the reverse: an expected assertion failure
    vanished), the post-call state differs (testFieldsAfter), or an observed output/diagnostic
    differs. NullPointerExceptions, other runtime exceptions and harness errors are reported
    separately and never count as caught."""
    why = fail.get("why", "")
    if (why.startswith("testFieldsAfter differs") or why.startswith("observed.")
            or why.startswith("postCall differs")):
        return True
    m = re.match(r"^outcome (\w+) \(([^:]*):.*?\) != recorded (\w+) \(([^:]*):", why, re.S)
    if m:
        got_status, got_cls, want_status, want_cls = m.groups()
        if got_status == "exception":
            return _is_assert_cls(got_cls)
        return want_status == "exception" and _is_assert_cls(want_cls)
    m = re.match(r"^exception class (\S+) != (\S+)", why)
    if m:
        return _is_assert_cls(m.group(1)) or _is_assert_cls(m.group(2))
    if why.startswith("exception message differs"):
        return _is_assert_cls(((rec or {}).get("outcome") or {}).get("exceptionClass"))
    return False


def noop_passable(r):
    """True when the record expects no change and no diagnostic, so a no-op pass passes it by
    construction (informational split for (d) only)."""
    e = r.get("expected") or {}
    if r["kind"] == "type_check":
        return not (e.get("diagnosticTypes") or e.get("diagnosticDescriptions"))
    if r["kind"] != "compiler_test_case" or (e.get("diagnostics") or []):
        return False
    o = e.get("output")
    if o is None or o == "SAME":
        return True
    inp = r.get("inputs") or {}
    src = inp.get("sources")
    if src is None and inp.get("chunks") is not None:
        src = [f for ch in inp["chunks"] for f in ch.get("inputs", [])]
    return isinstance(o, list) and _codes(o) == _codes(src)


def rule6_unzip_check():
    """Independent cross-check of rule 6 (report only): list the support jar's entries with
    `unzip -Z1` and the compiled replay classes with a directory walk, then grep the names for
    `Test($...)?.class`. Any hit in Closure's namespaces inside the support jar is an original test
    class on the replay classpath and fails rule 6. Hits in replay-classes must be the replay
    harness's own classes or classes compiled from oracle/replay/helpers (no *Test.java source)."""
    lst = subprocess.run(["unzip", "-Z1", SUPPORT], capture_output=True, text=True, check=True, env=ENV).stdout.split("\n")
    lst = [x for x in lst if x]
    rc = sorted(os.path.relpath(os.path.join(dp, fn), REPLAY_CLASSES)
                for dp, _, fns in os.walk(REPLAY_CLASSES) for fn in fns if fn.endswith(".class"))
    with open(f"{W}/rule6_unzip_listing.txt", "w") as f:
        f.write("".join(f"unit_support_deploy.jar\t{x}\n" for x in lst))
        f.write("".join(f"replay-classes\t{x}\n" for x in rc))
    pat = re.compile(r"Test(\$[^/]*)?\.class$")
    ns = re.compile(r"^com/google/(javascript|debugging)/")
    jar_hits = [x for x in lst if pat.search(x)]
    jar_closure = [x for x in jar_hits if ns.match(x)]
    rc_hits = [x for x in rc if pat.search(x)]
    rc_bad = []
    orig_named = [x for x in lst + rc if original_test_binary_name(x)]
    for x in rc_hits:
        if ALLOWED_TEST_NAMED.match(x):
            continue
        if original_test_binary_name(x):
            rc_bad.append(f"{x} (binary name of, or nested in, an original *Test class)")
            continue
        # explained only if compiled from a file under oracle/replay/helpers (SourceFile attribute)
        with open(os.path.join(REPLAY_CLASSES, x), "rb") as f:
            sf = class_source_file(f.read())
        if not (sf and not sf.endswith("Test.java")
                and os.path.exists(f"{ROOT}/oracle/replay/helpers/{os.path.dirname(x)}/{sf}")):
            rc_bad.append(f"{x} (SourceFile {sf})")
    return {"listing": "build/gate02/rule6_unzip_listing.txt", "jarEntries": len(lst), "replayClassFiles": len(rc),
            "grep": "Test(\\$[^/]*)?\\.class$", "jarHits": jar_hits, "jarHitsInClosureNamespaces": jar_closure,
            "replayClassesHits": rc_hits, "replayClassesUnexplained": rc_bad,
            "originalTestBinaryNames": orig_named,
            "ok": not jar_closure and not rc_bad and not orig_named}


def find_new(obj, fq):
    if isinstance(obj, dict):
        if obj.get("new") == fq:
            return True
        return any(find_new(v, fq) for v in obj.values())
    if isinstance(obj, list):
        return any(find_new(v, fq) for v in obj)
    return False


def find_helpers(obj, acc):
    if isinstance(obj, dict):
        if isinstance(obj.get("helper"), str):
            acc.add(obj["helper"])
        for v in obj.values():
            find_helpers(v, acc)
    elif isinstance(obj, list):
        for v in obj:
            find_helpers(v, acc)
    return acc


def mutation_points(obj, acc):
    if isinstance(obj, dict):
        if isinstance(obj.get("mutationPoint"), str):
            acc.add(obj["mutationPoint"])
        for v in obj.values():
            mutation_points(v, acc)
    elif isinstance(obj, list):
        for v in obj:
            mutation_points(v, acc)
    return acc


def pass_construction(cls, fq, failed, distinguishable=1):
    """Where --mutate-noop reaches pass fq in class cls's replay (report only, explains (d)).
    ReplayDsl replaces a DSL `new` of the pass, a `static`/`call` result whose runtime class is the
    pass, and a `mutationPoint` naming it (DSL.md "No-op mutation"). The verdict per class is the
    measured one: the mutation took effect if any record that passed in (a) fails under it."""
    d = load_desc(cls)
    simple = fq.rsplit(".", 1)[1]
    static = []
    if find_new(d.get("cases"), fq):
        static.append("DSL new")
    if {fq, simple} & mutation_points(d.get("cases"), set()):
        static.append("mutationPoint")
    if failed:
        return "reached" + (f" ({', '.join(static)})" if static else " (static/call result)")
    if not distinguishable:
        return "undetermined: every record of the pass in this class is no-op-passable"
    hs = sorted(find_helpers(d.get("cases"), set()))
    where = []
    if hs:
        where.append("built inside helper " + ", ".join(hs))
    if any(r["kind"] == "type_check" for r in iter_records(cls)):
        where.append("built by the type_check harness")
    if not where:
        where.append("built by a wrapper or the harness")
    return "NOT reached: " + "; ".join(where) + (f" (descriptor has {', '.join(static)})" if static else "")


D_SURVIVOR_CAUSES = {
    "crash-not-counted": "the record failed under mutation, but not with an assertion-type failure (never counted as caught)",
    "pass-not-reached-in-class": "no record of the pass in this class fails under mutation (the class's replay never reaches the pass)",
    "postcondition-not-replayed": "the record has postconditions but replay did not run as many as were recorded (the postcondition-count rule does not hold for it)",
    "postcondition-asserts-empty": "change-expecting only through its postcondition: the output is unchanged, no diagnostic is expected, the replayed postcondition still holds under the no-op, and the postcondition's captured expected value is an empty list (a no-op passes it by construction)",
    "noop-passable-postcondition": "change-expecting only through its postcondition: the output is unchanged, no diagnostic is expected, and the replayed postcondition still holds under the no-op (no empty-list capture found)",
    "co-attributed-multi-pass": "the record's processor runs more than one attributed pass; the other pass(es) still produce the expected result",
    "diagnostic-expected": "the expected diagnostic is still reported with the pass a no-op (it comes from code other than the pass class)",
    "output-only": "the expected output differs textually from the input, yet the no-op output still compares equal (includes text-only differences)",
}


def d_survivor_cause(r, passes, failed_any, construction, post=None):
    """Informational (gate (d) report only, never part of a verdict): first matching cause from
    D_SURVIVOR_CAUSES for a change-expecting record that passed in (a) and did not fail with an
    assertion-type failure under the class-level no-op of its pass."""
    e = r.get("expected") or {}
    if failed_any:
        return "crash-not-counted"
    if construction.startswith("NOT reached"):
        return "pass-not-reached-in-class"
    # The cause is decided by whether replay ran the record's postconditions (the
    # postcondition-count rule, from ReplayMain --post-out), not by the recorder's lambda placeholder.
    want = e.get("postconditions") or 0
    if want > 0:
        if (post or {}).get("postconditionsReplayed") != want:
            return "postcondition-not-replayed"
        if noop_passable({**r, "expected": {k: v for k, v in e.items() if k not in ("postconditions", "postconditionValues")}}):
            def empty_list_capture(x):
                if isinstance(x, dict):
                    if x.get("list") == [] or x.get("set") == [] or x.get("arrayOf") and x.get("items") == []:
                        return True
                    return any(empty_list_capture(v) for v in x.values())
                if isinstance(x, list):
                    return any(empty_list_capture(v) for v in x)
                return False
            caps = [pv.get("captures") for pv in (e.get("postconditionValues") or []) if isinstance(pv, dict)]
            return "postcondition-asserts-empty" if empty_list_capture(caps) else "noop-passable-postcondition"
    if len(passes) > 1:
        return "co-attributed-multi-pass"
    if e.get("diagnostics") or e.get("diagnosticTypes") or e.get("diagnosticDescriptions"):
        return "diagnostic-expected"
    return "output-only"


def cmd_report():
    prep = jload(f"{W}/prep.json")
    counts = prep["recordCounts"]
    r6 = jload(f"{W}/rule6.json")
    r6["unzipGrepCrossCheck"] = rule6_unzip_check()
    r6["ok"] = r6["ok"] and r6["unzipGrepCrossCheck"]["ok"]
    # The mutate phase also puts the NoopAgent jar on the class path (-javaagent appends it).
    with zipfile.ZipFile(NOOP_AGENT_JAR) as zj:
        ag = sorted(n for n in zj.namelist() if n.endswith(".class"))
    ag_bad = [n for n in ag if re.search(r"Test(\$.*)?\.class$", n)]
    r6["noopAgentJar"] = {"jar": NOOP_AGENT_JAR, "sha256": sha256(NOOP_AGENT_JAR), "classEntries": ag, "testNamed": ag_bad}
    r6["ok"] = r6["ok"] and not ag_bad
    marked = desc_marked_unrep()
    rep = {"gate": "0.2", "generated": time.strftime("%Y-%m-%dT%H:%M:%S%z"), "criteria": {},
           "inputs": {"recordsDir": REC, "descriptorsDir": DESC, "statsDir": STATS, "supportJar": SUPPORT,
                      "allTestsJar": ALLTESTS, "supportJarSha256": sha256(SUPPORT), "allTestsJarSha256": sha256(ALLTESTS),
                      "replaySourcesSha256": prep["replaySourcesSha256"],
                      "recordingWorkspaceSrcIdenticalToPristine": subprocess.run(
                          ["diff", "-rq", PRISTINE_SRC, f"{WS}/src"], capture_output=True).returncode == 0}}
    # ---------------- post-call state (replay --post-out) and the static post-call assertion audit
    post_by_class = {c: read_post(f"{W}/replay/{c}.post.jsonl") for c in record_classes()}
    lib_demoted = demote_library_internal_post_state(post_by_class)
    fails_by_class = {c: read_failures(f"{W}/replay/{c}.failures.jsonl") for c in record_classes()}
    pca, rut_recs, post_unclassified, ic_resolved = post_call_audit(post_by_class, fails_by_class, marked)
    pca_rows = pca.pop("perMethod")
    not_captured = {}
    for c, post in post_by_class.items():
        for i, x in post.items():
            if x.get("postState") == "notCaptured" and (c, i) not in ic_resolved:
                not_captured.setdefault(c, {})[i] = x.get("reason", "")
    # D-015 (a), gate 0.2 (a): a record of a rust_unit_test method is not
    # captured, whatever else replay compares for it. The same holds for the records of a method that is
    # unclassified (with or without an explicit entry) or whose classification claim does not hold, and
    # for records with unclassified post state: none of them is captured, so all count toward the cap
    # (D-015: "records that are not captured count toward a 3% exclusion cap"); the methods themselves
    # still fail (a) through the unclassified count. A compared testFieldsAfter field (postState
    # "checked") does not hold the asserted state (the method is classified rust_unit_test exactly
    # because no snapshot holds it), so it no longer exempts the record from the 3% exclusion cap.
    for c, m in rut_recs.items():
        for i, why in m.items():
            not_captured.setdefault(c, {}).setdefault(i, why)
    # whole-processor no-op vacuity check (phase noop): mechanical, no free-text
    # escape. A record "passes under no-op" unless its no-op replay failed with an assertion-type
    # failure (assertion_failure). A class whose counted compiler_test_case records all pass under
    # the no-op is vacuous; in a vacuous class, a record stays counted in (a) only when replay
    # actually checks its post-call state (postState "checked") or replays at least one
    # postcondition; every other record of the class becomes notCaptured (excluded from (a),
    # counted in (e)).
    noop_rows, noop_missing, noop_demoted, vnc_errors = [], [], 0, []
    src_set = set(src_top_classes())
    vnc_all = sorted({p for c in ctc_classes() for p in vacuity_noop_classes(c) if isinstance(p, str) and p in src_set})
    vnc_roles = pass_roles(vnc_all) if vnc_all else {}
    for c in ctc_classes():
        done = f"{W}/noop/{c}.done.json"
        if not os.path.exists(done):
            noop_missing.append(c)
            continue
        dj = jload(done)
        rr = (dj.get("replay") or {}).get(c)
        nf = read_failures(f"{W}/noop/{c}.failures.jsonl")
        recl = list(iter_records(c))
        excl = set(marked.get(c, {})) | set(not_captured.get(c, {}))
        idx = [i for i, r in enumerate(recl) if r["kind"] == "compiler_test_case" and i not in excl]
        if rr is None or rr["records"] != counts[c]:
            noop_missing.append(f"{c} (noop replay did not complete, rc {dj['rc']})")
            continue
        caught = {i for i in idx if i in nf and assertion_failure(nf[i], recl[i])}
        nonassert = [i for i in idx if i in nf and i not in caught]
        passing = [i for i in idx if i not in caught]
        vac = bool(idx) and len(passing) == len(idx)
        # vacuityNoopClass: when the whole-processor no-op catches nothing,
        # the class is replayed again under the class-level NoopAgent for each named harness-run pass.
        # The class stays vacuous only if every counted record also passes under that mutation (no
        # assertion-type failure in a record that passes in (a)). A named class must be an in-scope
        # top-level src class with a pass role, must not be the recorded processor's own class, and the
        # agent must report rewritten entry points; otherwise (a) fails. No free text is read.
        vnc_rows = {}
        for p in vacuity_noop_classes(c):
            row = {"pass": p}
            vnc_rows[str(p)] = row
            if not isinstance(p, str) or p not in src_set or not in_scope_pass(p):
                row["error"] = "not an in-scope top-level src class"
            elif vnc_roles.get(p) != "pass":
                row["error"] = f"no pass role ({vnc_roles.get(p)})"
            elif any(outer((r.get("processor") or {}).get("class") or "") == p for r in recl):
                row["error"] = "is the recorded processor's own class (the whole-processor no-op covers it)"
            if "error" in row:
                vnc_errors.append(f"{c}: vacuityNoopClass {p!r}: {row['error']}")
                continue
            st = vnc_stem(c, p)
            if not os.path.exists(f"{st}.done.json"):
                noop_missing.append(f"{c}@{p}")
                row["error"] = "not run"
                continue
            dj2 = jload(f"{st}.done.json")
            rr2 = (dj2.get("replay") or {}).get(c)
            if rr2 is None or rr2["records"] != counts[c]:
                noop_missing.append(f"{c}@{p} (harness-pass no-op replay did not complete, rc {dj2['rc']})")
                row["error"] = "did not complete"
                continue
            rew = []
            if os.path.exists(f"{st}.agent.txt"):
                with open(f"{st}.agent.txt", errors="replace") as fa:
                    rew = [l[8:].strip() for l in fa if l.startswith("REWROTE ")]
            row["entryPointsRewritten"] = len(rew)
            if not rew:
                vnc_errors.append(f"{c}: vacuityNoopClass {p}: the agent rewrote no entry point")
                row["error"] = "no entry point rewritten"
                continue
            nf2 = read_failures(f"{st}.failures.jsonl")
            base = fails_by_class.get(c, {})
            c2 = {i for i in idx if i in nf2 and i not in base and assertion_failure(nf2[i], recl[i])}
            row["failAssertionType"] = len(c2)
            row["failNonAssertionNotCounted"] = len([i for i in idx if i in nf2 and i not in base and i not in c2])
            if vac and c2:
                row["rescued"] = True
        vac_by_processor = vac
        if vac and any(r.get("rescued") for r in vnc_rows.values()):
            vac = False
        kept, demoted = [], []
        if vac:
            post = post_by_class.get(c, {})
            for i in idx:
                x = post.get(i) or {}
                if x.get("postState") == "checked" or (x.get("postconditionsReplayed") or 0) > 0:
                    kept.append(i)
                else:
                    demoted.append(i)
                    not_captured.setdefault(c, {})[i] = ("vacuous class: every counted record passes with the whole "
                                                         "processor replaced by a no-op, and this record has no "
                                                         "replay-checked post-call state or postcondition")
        noop_demoted += len(demoted)
        noop_rows.append({"class": c, "ctcRecordsCounted": len(idx), "passUnderNoopProcessor": len(passing),
                          "failAssertionType": len(caught), "failNonAssertionNotCounted": len(nonassert),
                          "vacuous": vac, "vacuousUnderNoopProcessor": vac_by_processor,
                          "vacuityNoopClass": list(vnc_rows.values()),
                          "vacuousKeptCheckedPostState": len(kept), "vacuousDemotedNotCaptured": len(demoted)})
    # Postcondition count rule (FORMAT.md "Postconditions"). Every compiler_test_case
    # record that stays counted must replay exactly as many postconditions as the original call ran;
    # replay marks a mismatch notCaptured itself, and the gate re-checks it from the post lines.
    pc_mismatch = []
    for c in record_classes():
        post = post_by_class.get(c, {})
        for i, r in enumerate(iter_records(c)):
            if r["kind"] != "compiler_test_case" or i in not_captured.get(c, {}) or i in marked.get(c, {}):
                continue
            want = (r.get("expected") or {}).get("postconditions") or 0
            got = (post.get(i) or {}).get("postconditionsReplayed")
            if got != want:
                pc_mismatch.append({"class": c, "index": i, "recorded": want, "replayed": got})
    # Processor identity: replay dumps the processor the descriptor built
    # (ProcessorIdentity.java) and compares scalar/enum/String leaves with the recorded dump, object
    # by object (by class, after classMap), so passes held by wrappers are compared too. A counted
    # record with a differing leaf fails (a); records whose processor class differs are listed with
    # how many leaves could be compared; a missing proc file fails (a).
    proc_mismatch, proc_missing, proc_rows, proc_unverified = [], [], [], []
    for c in record_classes():
        pf = f"{W}/replay/{c}.proc.jsonl"
        has_ctc = any(r["kind"] == "compiler_test_case" for r in iter_records(c))
        if not has_ctc:
            continue
        if not os.path.exists(pf):
            proc_missing.append(c)
            continue
        st = {}
        cls_diff_zero = 0
        with open(pf) as fh:
            for line in fh:
                d = json.loads(line)
                i = d["index"]
                if i in not_captured.get(c, {}) or i in marked.get(c, {}):
                    continue
                st[d["status"]] = st.get(d["status"], 0) + 1
                if d["status"] == "mismatch":
                    proc_mismatch.append({"class": c, **d})
                if d["status"] == "classDiffers" and d.get("compared", 0) == 0:
                    cls_diff_zero += 1
        # A classDiffers record with nothing compared is unverified unless the
        # descriptor justifies the substitution in a non-blank `processorIdentityNote` (top level or
        # in a case); unjustified ones fail (a).
        note = _find_key(load_desc(c), "processorIdentityNote")
        if cls_diff_zero and not note:
            proc_unverified.append({"class": c, "records": cls_diff_zero})
        proc_rows.append({"class": c, "statuses": st, "classDiffersNoLeafCompared": cls_diff_zero,
                          "processorIdentityNote": (note or "")[:300]})
    proc_summary = {}
    for r in proc_rows:
        for k, v in r["statuses"].items():
            proc_summary[k] = proc_summary.get(k, 0) + v
    # D-017 item 8: processor identity is verified mechanically by the pass trace. ReplayMain ran with
    # the NoopAgent in TRACE mode and wrote <Class>.trace.jsonl: per record equal / mismatch (also a
    # replay failure) / unverified (no recorded passTrace). An unverified record counts as excluded
    # under the 3% cap; a missing or incomplete trace file fails (a). processorIdentityNote is not
    # evidence any more (the leaf comparison above stays as an additional check).
    trace_unverified, trace_missing, trace_mismatch, trace_status = {}, [], [], {}
    for c in record_classes():
        tf = f"{W}/replay/{c}.trace.jsonl"
        if not os.path.exists(tf):
            trace_missing.append(c)
            continue
        seen = 0
        with open(tf) as fh:
            for line in fh:
                d = json.loads(line)
                seen += 1
                trace_status[d["status"]] = trace_status.get(d["status"], 0) + 1
                if d["status"] == "unverified":
                    trace_unverified.setdefault(c, set()).add(d["index"])
                elif d["status"] == "mismatch":
                    trace_mismatch.append({"class": c, "index": d["index"], "method": d.get("method"),
                                           "want": d.get("want"), "got": d.get("got")})
        if seen != counts[c]:
            trace_missing.append(c)
    # ---------------- (a)
    a_rows, a_tot, a_pass, a_fail, a_excl, a_missing, a_fail_list = [], 0, 0, 0, 0, [], []
    a_not_captured = 0
    a_trace_unverified = 0
    for c in record_classes():
        done = f"{W}/replay/{c}.done.json"
        if not os.path.exists(done):
            a_missing.append(c)
            continue
        dj = jload(done)
        fails = read_failures(f"{W}/replay/{c}.failures.jsonl")
        n = counts[c]
        rr = (dj.get("replay") or {}).get(c)
        nc = set(not_captured.get(c, {})) - set(marked.get(c, {}))
        tu = set(trace_unverified.get(c, set())) - set(marked.get(c, {})) - nc
        excl = set(marked.get(c, {})) | nc | tu
        a_not_captured += len(nc)
        a_trace_unverified += len(tu)
        if rr is None or rr["records"] != n:
            a_fail_list.append({"class": c, "why": f"replay did not finish all {n} records (rc {dj['rc']}, {rr})"})
            a_fail += n
            a_tot += n
            continue
        f_nonexcl = [i for i in fails if i not in excl]
        a_tot += n - len(excl)
        a_excl += len(excl)
        a_fail += len(f_nonexcl)
        a_pass += n - len(excl) - len(f_nonexcl)
        for i in f_nonexcl[:5]:
            a_fail_list.append({"class": c, "index": i, "why": fails[i]["why"][:300]})
        a_rows.append({"class": c, "records": n, "excluded": len(excl), "excludedNotCaptured": len(nc),
                       "excludedTraceUnverified": len(tu),
                       "fail": len(f_nonexcl),
                       "excludedPassedAnyway": len([i for i in excl if i not in fails])})
    tot_records = sum(counts.values())
    a_cap_ok = a_excl <= EXCLUSION_CAP * tot_records
    a_ok = (not a_missing and a_fail == 0 and r6["ok"] and not post_unclassified and pca["unclassified"] == 0
            and pca["schemaErrorCount"] == 0 and a_cap_ok
            and not noop_missing and not vnc_errors and not pc_mismatch and not proc_mismatch and not proc_missing
            and not trace_missing and not trace_mismatch)
    jdump(f"{W}/postcall_audit_methods.json", pca_rows)
    jdump(f"{W}/post_state_unclassified.json", post_unclassified)
    rep["criteria"]["a"] = {"ok": a_ok, "records": sum(counts.values()), "replayed": a_tot, "pass": a_pass,
                            "excludedTotal": a_excl, "excludedFraction": a_excl / tot_records if tot_records else 0.0,
                            "exclusionCap": EXCLUSION_CAP, "exclusionCapOk": a_cap_ok,
                            "fail": a_fail, "excludedMarkedUnrepresentable": a_excl - a_not_captured - a_trace_unverified,
                            "excludedTraceUnverified": a_trace_unverified,
                            "passTrace": {"rule": "D-017 item 8: NoopAgent TRACE mode records the in-scope src top-level classes whose pass entry points (CompilerPass/HotSwapCompilerPass/CallGraphCompilerPass process, hotSwapScript, peephole optimizeSubtree/transpileSubtree, NodeTraversal callback methods) execute during the hooked call, in the recording run (record passTrace) and in replay; the sets must be equal (a difference fails the record); a record without a recorded passTrace counts as excluded under the 3% cap; processorIdentityNote is not accepted as evidence",
                                          "statusCounts": trace_status, "mismatch": len(trace_mismatch),
                                          "mismatchSample": trace_mismatch[:30], "missingOrIncompleteTraceFiles": trace_missing,
                                          "unverifiedRecords": sum(len(v) for v in trace_unverified.values()),
                                          "unverifiedByClass": {c: len(v) for c, v in sorted(trace_unverified.items())}},
                            "excludedPostCallNotCaptured": a_not_captured, "classesMissing": a_missing,
                            "postStateUnclassified": len(post_unclassified),
                            "postStateUnclassifiedByClass": collections_counter([x["class"] for x in post_unclassified]),
                            "postStateUnclassifiedSample": post_unclassified[:30],
                            "postCallAssertionAudit": pca, "postStateDemotedLibraryInternal": lib_demoted,
                            "noopProcessor": {"incomplete": noop_missing,
                                              "rule": "mechanical: vacuous classes keep only records with replay-checked post-call state or postconditions; the rest are notCaptured",
                                              "vacuousClasses": [r["class"] for r in noop_rows if r["vacuous"]],
                                              "vacuityNoopClassRule": "a class vacuous under the whole-processor no-op is replayed under the class-level NoopAgent for each descriptor vacuityNoopClass (in-scope src pass, not the processor's own class, entry points rewritten); it stays vacuous only if no counted record that passes in (a) fails with an assertion-type failure there",
                                              "rescuedByVacuityNoopClass": [r["class"] for r in noop_rows if r["vacuousUnderNoopProcessor"] and not r["vacuous"]],
                                              "vacuityNoopClassErrors": vnc_errors,
                                              "recordsDemotedNotCaptured": noop_demoted,
                                              "recordsCountedThatPassUnderNoop": sum(r["passUnderNoopProcessor"] for r in noop_rows),
                                              "perClass": noop_rows},
                            "postconditionCountMismatch": len(pc_mismatch), "postconditionCountMismatchSample": pc_mismatch[:30],
                            "processorIdentity": {"rule": "additional check (kept): counted compiler_test_case records: scalar, enum and String leaves of the replayed processor dump (depth 2) must equal the recorded ones, object by object by class after classMap; mismatch fails (a); classDiffers records are listed (classDiffersNoLeafCompared = nothing comparable); identity itself is verified by passTrace (D-017 item 8), not by processorIdentityNote",
                                                  "statusCounts": proc_summary, "mismatch": len(proc_mismatch),
                                                  "unverifiedClassDiffers": proc_unverified,
                                                  "unverifiedClassDiffersRecords": sum(x["records"] for x in proc_unverified),
                                                  "classDiffersNoLeafComparedTotal": sum(r["classDiffersNoLeafCompared"] for r in proc_rows),
                                                  "mismatchSample": proc_mismatch[:30], "missingProcFiles": proc_missing,
                                                  "perClass": proc_rows},
                            "failures": a_fail_list[:50], "rule6": r6, "perClass": a_rows}
    # ---------------- (b)
    b_rows, b_bad, s_out, s_ent = [], [], 0, 0
    nstat = 0
    for c in sorted(counts):
        sp = f"{STATS}/{c}.json"
        if not os.path.exists(sp):
            b_bad.append({"class": c, "why": "no stats file"})
            continue
        nstat += 1
        st = jload(sp)
        outm = sum(st.get("outermost", {}).values())
        ent = sum(st.get("entries", {}).values())
        s_out += outm
        s_ent += ent
        n = counts[c]
        dev = abs(n - outm) / outm if outm else (0.0 if n == 0 else 1.0)
        row = {"class": c, "records": n, "hookedOutermost": outm, "hookedAllDepths": ent, "deviation": round(dev, 6),
               "statsRecordsField": st.get("records")}
        if dev > 0.02:
            b_bad.append(row)
        if n or outm:
            b_rows.append(row)
    crashes = sorted(f for f in os.listdir(STATS) if f.endswith(".crash.json"))
    tot_rec = sum(counts.values())
    tot_dev = abs(tot_rec - s_out) / s_out if s_out else 1.0
    b_ok = not b_bad and tot_dev <= 0.02 and not crashes and nstat == len(counts)
    rep["criteria"]["b"] = {"ok": b_ok, "records": tot_rec, "hookedOutermost": s_out, "hookedAllDepths": s_ent,
                            "totalDeviation": tot_dev, "classesOutside2pct": b_bad, "statsDir": STATS,
                            "jvmCrashes": crashes, "perClass": b_rows}
    # ---------------- (c)
    c_res = {"ok": False}
    try:
        with open(f"{ROOT}/build/unit/all_test_classes.txt") as f:
            allc = [l.strip().rsplit(".", 1)[1] for l in f if l.strip()]
        o_missing = [n for n in allc if not os.path.exists(f"{W}/cov/orig/{n}.exec")]
        r_missing = [c for c in record_classes() if not os.path.exists(f"{W}/cov/replay/{c}.exec")]
        if o_missing or r_missing:
            c_res = {"ok": False, "incomplete": True, "origMissing": o_missing[:20], "origMissingCount": len(o_missing),
                     "replayMissing": r_missing[:20], "replayMissingCount": len(r_missing)}
        else:
            orig = coverage_xml([f"{W}/cov/orig/{n}.exec" for n in allc], "orig")
            repl = coverage_xml([f"{W}/cov/replay/{c}.exec" for c in record_classes()], "replay")
            # D-017 item 9: a harness-based test class is one whose original run makes at least one
            # hooked harness call, i.e. a record-bearing class (by (b), records == hooked calls).
            # Classes that inherit a harness (static class-hierarchy check of the pristine test/ tree)
            # but never call it are non-harness classes: they leave the denominator and must be
            # listed in RUST_UNIT_TESTS.md like every other in-scope non-harness class.
            hier_harness, hier_note = harness_hierarchy_classes()
            harness = set(record_classes())
            harness_zero_records = sorted(hier_harness - set(record_classes()))
            orig_h = coverage_xml([f"{W}/cov/orig/{n}.exec" for n in allc if n in harness], "orig_harness_classes")
            files = sorted(set(orig) | set(repl))
            tl = sum((orig.get(p) or repl.get(p))[0] + (orig.get(p) or repl.get(p))[1] for p in files)
            oc = sum(orig.get(p, (0, 0))[0] for p in files)
            rc_ = sum(repl.get(p, (0, 0))[0] for p in files)
            hc = sum(orig_h.get(p, (0, 0))[0] for p in files)
            loss = sorted(((orig.get(p, (0, 0))[0] - repl.get(p, (0, 0))[0], p) for p in files), reverse=True)[:30]
            # replay pass counts under the agent
            cov_fail = 0
            for c in record_classes():
                dj = jload(f"{W}/cov/replay/{c}.done.json")
                rr = (dj.get("replay") or {}).get(c)
                cov_fail += (rr["fail"] if rr else counts[c])
            orig_fail = []
            for n in allc:
                sp = f"{W}/cov/orig/stats/{n}.json"
                if os.path.exists(sp):
                    st = jload(sp)
                    for t, v in st.get("tests", {}).items():
                        if v.get("status") not in ("passed", "ignored", "skipped"):
                            orig_fail.append(t)
                else:
                    orig_fail.append(f"{n}: no stats (JVM rc {jload(f'{W}/cov/orig/{n}.done.json')['rc']})")
            # D-015 (c): the denominator is the original suite's harness-based test classes (the
            # record-bearing classes); the whole-suite ratio is reported alongside.
            ratio = (rc_ / hc) if hc else 0.0
            whole_ratio = (rc_ / oc) if oc else 0.0
            harness_fq = set()
            with open(f"{ROOT}/build/unit/all_test_classes.txt") as f_:
                all_fq = [l.strip() for l in f_ if l.strip()]
            nonh = [fq for fq in all_fq if fq.rsplit(".", 1)[1] not in harness and not fq.startswith(OUT_OF_SCOPE_PKGS)]
            listed = set()
            rut = f"{ROOT}/corpus/unit/RUST_UNIT_TESTS.md"
            if os.path.exists(rut):
                with open(rut, encoding="utf-8") as f_:
                    for l in f_:
                        mm = re.match(r"^- ([\w.$]+):", l)
                        if mm:
                            listed.add(mm.group(1))
            nonh_missing = [fq for fq in nonh if fq not in listed]
            rut_fresh = subprocess.run(["python3", f"{ROOT}/scripts/unit_rust_unit_tests.py", "--check"],
                                       capture_output=True, text=True).returncode == 0
            # Gate 0.2 (c) measures "in-scope src/". The verdict uses the files that
            # pass in_scope_src (the same scope test (d) ranks with: the docs/PORTING.md §2 directories and the
            # J2cl*/Polymer*/Chrome*/Debugger* files are dropped). The all-of-src/ figures are informational.
            oos = tuple(x.replace(".", "/") for x in OUT_OF_SCOPE_PKGS)
            ins = [p_ for p_ in files if in_scope_src(p_)]
            ins_o = sum(orig.get(p_, (0, 0))[0] for p_ in ins)
            ins_r = sum(repl.get(p_, (0, 0))[0] for p_ in ins)
            ins_h = sum(orig_h.get(p_, (0, 0))[0] for p_ in ins)
            ins_tl = sum((orig.get(p_) or repl.get(p_))[0] + (orig.get(p_) or repl.get(p_))[1] for p_ in ins)
            all_src_ratio = ratio
            ratio = (ins_r / ins_h) if ins_h else 0.0
            c_res_info_scope = {"excludedDirs": list(oos), "excludedNamePrefixes": list(OUT_OF_SCOPE_NAME_PREFIXES),
                                "srcFiles": len(ins), "srcLines": ins_tl, "origCoveredLines": ins_o,
                                "replayCoveredLines": ins_r, "wholeSuiteRatio": (ins_r / ins_o) if ins_o else 0.0,
                                "origHarnessClassesCoveredLines": ins_h,
                                "ratio": ratio, "neededReplayLinesFor98": int(-(-0.98 * ins_h // 1)),
                                "note": "the gate verdict (D-015 (c), gate 0.2 (c) 'in-scope src/')"}
            c_res = {"ok": ratio >= 0.98 and not nonh_missing and rut_fresh,
                     "scope": c_res_info_scope,
                     "info_allSrc": {"srcFiles": len(files), "srcLines": tl, "origCoveredLines": oc,
                                     "replayCoveredLines": rc_, "harnessCoveredLines": hc,
                                     "ratio": all_src_ratio, "wholeSuiteRatio": whole_ratio,
                                     "note": "informational only: all of src/, including the docs/PORTING.md §2 out-of-scope code"},
                     "denominator": "original suite restricted to the harness-based test classes (D-017 item 9): classes whose original run makes at least one hooked harness call (record-bearing classes); harness-hierarchy classes with zero records are non-harness classes and must be listed in RUST_UNIT_TESTS.md",
                     "harnessHierarchy": hier_note, "harnessClassesWithZeroRecords": harness_zero_records,
                     "harnessClasses": len(harness), "harnessCoveredLines": ins_h, "harnessPct": 100.0 * ins_h / ins_tl,
                     "wholeSuiteRatio": (ins_r / ins_o) if ins_o else 0.0,
                     "nonHarnessInScopeClasses": len(nonh), "nonHarnessNotListedInRustUnitTests": nonh_missing,
                     "rustUnitTestsMdFresh": rut_fresh,
                     "srcFiles": len(ins), "srcLines": ins_tl,
                     "origCoveredLines": ins_o, "origPct": 100.0 * ins_o / ins_tl, "replayCoveredLines": ins_r,
                     "replayPct": 100.0 * ins_r / ins_tl, "ratio": ratio, "threshold": 0.98,
                     "top30Loss": [{"file": p, "origCovered": orig_h.get(p, (0, 0))[0], "replayCovered": repl.get(p, (0, 0))[0],
                                    "loss": l, "lines": sum(orig.get(p) or repl.get(p))} for l, p in
                                   sorted(((orig_h.get(p_, (0, 0))[0] - repl.get(p_, (0, 0))[0], p_) for p_ in ins), reverse=True)[:30]],
                     "agent": prep["agent"], "cli": prep["cli"], "agentOptions": AGENT_OPTS,
                     "replayFailuresUnderAgent": cov_fail, "origSuiteNonPassingTests": orig_fail}
    except Exception as e:  # noqa
        c_res = {"ok": False, "error": repr(e)}
    rep["criteria"]["c"] = c_res
    # ---------------- (d)
    top, ranked, pm, units, facts = selected_passes()
    d_rows, d_ok, d_incomplete = [], True, []
    g_rows, info_rows = [], []
    for u in units:
        p, simple, members = u["key"], u["dir"], u["members"]
        recs_total, failed, failed_change, n_change, n_same, same_failed = 0, 0, 0, 0, 0, 0
        rewrote, loaded, agent_err = set(), set(), []
        failed_nonassert, nonassert_sample = 0, []
        unreached = True
        per_cls = []
        surv_cause, surv_cls = {}, {}
        for cls, uidx in sorted(u["records"].items()):
            recs = pm["records"][cls]
            excl = set(marked.get(cls, {}))
            idx = [i for i in uidx if i not in excl]
            if not idx:
                continue
            done = f"{W}/mut/{simple}/{cls}.done.json"
            if not os.path.exists(done):
                d_incomplete.append(f"{simple}/{cls}")
                continue
            fails = read_failures(f"{W}/mut/{simple}/{cls}.failures.jsonl")
            base = read_failures(f"{W}/replay/{cls}.failures.jsonl")
            dj = jload(done)
            rr = (dj.get("replay") or {}).get(cls)
            same = {i for i, r in enumerate(iter_records(cls)) if not change_expecting(cls, i, r, facts)}
            ap = f"{W}/mut/{simple}/{cls}.agent.txt"
            if os.path.exists(ap):
                with open(ap, errors="replace") as fa:
                    for l in fa:
                        if l.startswith("REWROTE "):
                            rewrote.add(l[8:].strip())
                        elif l.startswith("LOADED "):
                            loaded.add(l[7:].strip())
                        elif l.startswith("ERROR "):
                            agent_err.append(l.strip()[:200])
            # A unit whose JVM did not finish counts no record as failed and makes (d) incomplete.
            if rr is None or rr["records"] != counts[cls]:
                d_incomplete.append(f"{simple}/{cls} (replay did not complete, rc {dj['rc']})")
                rr = None
            recl = list(iter_records(cls))
            fany = {i for i in idx if (i in fails and i not in base)} if rr else set()
            fset = {i for i in fany if assertion_failure(fails[i], recl[i])}
            fnon = sorted(fany - fset)
            recs_total += len(idx)
            failed_nonassert += len(fnon)
            for i in fnon[:3]:
                nonassert_sample.append({"class": cls, "index": i, "why": fails[i]["why"][:200]})
            failed += len(fset)
            n_same += len([i for i in idx if i in same])
            n_change += len([i for i in idx if i not in same])
            failed_change += len([i for i in fset if i not in same])
            same_failed += len([i for i in fany if i in same])
            dist = len([i for i in idx if i not in same])
            if len(members) == 1:
                cons = pass_construction(cls, members[0], len(fset), dist)
            elif fset:
                cons = "reached (joint no-op of " + ", ".join(m.rsplit(".", 1)[1] for m in members) + ")"
            elif not dist:
                cons = "undetermined: every record of the unit in this class is no-op-passable"
            else:
                cons = "NOT reached: no record of the unit in this class fails under the joint no-op"
            # Informational only (never part of the verdict): why change-expecting records survive.
            if rr:
                for i in idx:
                    if i in same or i in fset:
                        continue
                    cause = d_survivor_cause(recl[i], recs[i], i in fany, cons, (post_by_class.get(cls) or {}).get(i))
                    surv_cause[cause] = surv_cause.get(cause, 0) + 1
                    sc = surv_cls.setdefault(cls, {})
                    sc[cause] = sc.get(cause, 0) + 1
            per_cls.append({"class": cls, "records": len(idx), "failedUnderMutation": len(fset),
                            "failedNonAssertion": len(fnon),
                            "harnessCompleted": rr is not None, "construction": cons})
        # D-015 (d): the pass mark is over change-expecting records only; records that expect no
        # change are reported (and expected to pass under the no-op).
        rate = failed_change / n_change if n_change else 0.0
        # Every pass-role class under P that the replays loaded must have at least one
        # REWROTE line (declared or synthesized for an inherited entry point), not just one class of P.
        rewrote_cls = {x.split("(", 1)[0].rsplit(".", 1)[0] for x in rewrote}
        loaded_roles = pass_roles(sorted(loaded)) if loaded else {}
        cand = sorted(c_ for c_ in loaded if loaded_roles.get(c_) == "pass" and c_ not in rewrote_cls)
        # A loaded pass-role class without a REWROTE line of its own is covered when
        # it has at least one executable entry point and every one of them is inherited from a class whose
        # REWROTE line names that exact method (TypedScopeCreator$NormalScopeBuilder inherits the final
        # shouldTraverse/visit of $AbstractScopeBuilder, which the agent rewrote). Every executable entry
        # point is still required to be neutralized.
        rewrote_methods = {x.split(" (", 1)[0] for x in rewrote}
        owners = entry_owners(cand) if cand else {}
        covered_inherited = sorted(c_ for c_ in cand if owners.get(c_) and all(
            f"{o}.{k}" in rewrote_methods for k, o in owners[c_]))
        loaded_unrewritten = sorted(c_ for c_ in cand if c_ not in covered_inherited)
        # Members of a jointly no-op'd group (or unit) with no REWROTE line in any class (reported).
        members_not_rewritten = sorted(m_ for m_ in members if not any(
            x == m_ or x.startswith(m_ + "$") for x in rewrote_cls))
        ok = rate >= 0.90 and bool(rewrote) and not agent_err and not loaded_unrewritten
        if u["kind"] == "group" and n_change == 0:
            # A group whose records all expect no change has nothing to catch (reported, not judged).
            ok = True
        if u["kind"] != "info":
            d_ok &= ok
        rows = d_rows if u["kind"] == "pass" else (g_rows if u["kind"] == "group" else info_rows)
        rows.append({"pass": p, "kind": u["kind"], "label": u["label"], "members": members,
                       "changeExpectingRecords": n_change, "changeExpectingFailed": failed_change,
                       "rate": rate, "ok": ok, "recordsAll": recs_total, "failedAll": failed,
                       "noChangeRecords": n_same, "noChangeRecordsFailingUnderMutation": same_failed,
                       "agentRewroteEntryPoints": sorted(rewrote), "agentLoadedClasses": len(loaded),
                       "agentLoadedPassClassesNotRewritten": loaded_unrewritten,
                       "agentLoadedPassClassesCoveredByInheritedRewrite": covered_inherited,
                       "membersWithoutRewroteLine": members_not_rewritten,
                       "agentErrors": agent_err[:5],
                       "failedNonAssertionNotCounted": failed_nonassert, "failedNonAssertionSample": nonassert_sample[:10],
                       "mutationReached": failed > 0, "perClass": per_cls,
                       "info_survivorsByCause": dict(sorted(surv_cause.items(), key=lambda kv: -kv[1])),
                       "info_survivorsByClass": surv_cls,
                       "recordsInClassesNotReached": sum(x["records"] for x in per_cls if x["construction"].startswith("NOT")),
                       "notReachedClasses": sorted({f"{x['class']}: {x['construction'][len('NOT reached: '):]}"
                                                    for x in per_cls if x["construction"].startswith("NOT")})})
    rep["criteria"]["d"] = {"ok": d_ok and not d_incomplete and len(d_rows) == 30, "incompleteUnits": d_incomplete,
                            "passes": d_rows, "passesReaching90": len([r for r in d_rows if r["ok"]]),
                            "groups": g_rows, "groupsTotal": len(g_rows),
                            "groupsWithChangeExpecting": len([r for r in g_rows if r["changeExpectingRecords"]]),
                            "groupsReaching90": len([r for r in g_rows if r["changeExpectingRecords"] and r["ok"]]),
                            "typeCheckAlone": info_rows[0] if info_rows else None,
                            "changeFacts": {"code": collections_counter([v[:60] for m in facts["code"].values() for v in m.values()]),
                                            "postcondition": collections_counter([v[:60] for m in facts["postcondition"].values() for v in m.values()])},
                            "unitsRule": d_units.__doc__.strip(),
                            "mutation": "class-level: oracle/replay/agent NoopAgent rewrites the pass class's entry points at load time (D-015 d)",
                            "ranking": ranked[:45],
                            "rankingRule": "in-scope passes (docs/PORTING.md §2 out-of-scope packages and J2cl*/Polymer*/Chrome*/Debugger* classes excluded, in_scope_src) by change-expecting records: " + change_expecting.__doc__.strip(),
                            "attribution": compute_pass_map.__doc__.strip()}
    # ---------------- (e)
    e_cls = {}
    union = 0
    listed_only = 0
    sec_counts = {}
    placeholder_unlisted = 0
    for c in record_classes():
        excl = marked.get(c, {})
        ncap = not_captured.get(c, {})
        for i, r in enumerate(iter_records(c)):
            u = r.get("unrepresentable") or []
            secs = placeholder_sections(r)
            for sec in secs:
                sec_counts[sec] = sec_counts.get(sec, 0) + 1
            if secs and not u:
                placeholder_unlisted += 1
            if u or i in excl:
                listed_only += 1
            if u or secs or i in excl or i in ncap:
                union += 1
                ent = e_cls.setdefault(c, {"records": 0, "recorderLevel": 0, "descriptorMarked": 0,
                                           "postCallNotCaptured": 0, "sections": {}, "reasons": {}})
                ent["records"] += 1
                for sec in secs:
                    ent["sections"][sec] = ent["sections"].get(sec, 0) + 1
                if u:
                    ent["recorderLevel"] += 1
                if i in ncap:
                    ent["postCallNotCaptured"] += 1
                    k = "post-call assertions not captured: " + ncap[i][:160]
                    ent["reasons"][k] = ent["reasons"].get(k, 0) + 1
                if i in excl:
                    ent["descriptorMarked"] += 1
                    k = "descriptor: " + excl[i][:160]
                    ent["reasons"][k] = ent["reasons"].get(k, 0) + 1
                for x in u:
                    path = re.sub(r"\{[kv]\d+\}", "{*}", re.sub(r"\[\d+\]", "[*]", str(x.get("path"))))
                    k = f"{path}: {x.get('reason')}"
                    ent["reasons"][k] = ent["reasons"].get(k, 0) + 1
    tot = sum(counts.values())
    # Do the recorder-level records nevertheless replay green? (informational)
    for c, ent in e_cls.items():
        fails = read_failures(f"{W}/replay/{c}.failures.jsonl")
        idxs = [i for i, r in enumerate(iter_records(c)) if (r.get("unrepresentable") or [])]
        ent["recorderLevelRecordsFailingReplay"] = len([i for i in idxs if i in fails])
        plan = None
        for fn in (f"{DESC}/{c}.json",):
            if os.path.exists(fn):
                d = jload(fn)
                plan = d.get("unrepresentableAudit") or d.get("note")
        ent["descriptorNote"] = (plan or "")[:400]
        ent["reasons"] = dict(sorted(ent["reasons"].items(), key=lambda kv: -kv[1])[:8])
    rep["criteria"]["e"] = {"ok": union <= 0.03 * tot, "records": tot, "withUnrepresentable": union,
                            "fraction": union / tot if tot else 0.0, "threshold": 0.03,
                            "definition": "records with a non-empty recorder-level `unrepresentable` list, or an "
                                          "{\"unrepresentable\":...} placeholder in any section (processor, options, "
                                          "testFields, testFieldsAfter, expected, harness), or marked unrepresentable in "
                                          "their descriptor, or whose post-call assertions are not captured "
                                          "(replay post state notCaptured, or postCallAssertions out_of_scope)",
                            "info_listedOrMarkedOnly": listed_only,
                            "recordsWithPlaceholderBySection": dict(sorted(sec_counts.items(), key=lambda kv: -kv[1])),
                            "consistency_placeholderRecordsWithEmptyList": placeholder_unlisted,
                            "perClass": dict(sorted(e_cls.items(), key=lambda kv: -kv[1]["records"]))}
    # Results must belong to the current inputs and gate code (prep wrote the stamp they were run with).
    now = compute_stamp()
    then = jload(f"{W}/stamp.json")
    rep["stamp"] = {"atPrep": then, "atReport": now, "match": now == then}
    rep["ok"] = all(rep["criteria"][k].get("ok") for k in "abcde") and now == then
    rep = repo_relative(rep)  # the committed report names no machine paths
    jdump(f"{ROOT}/gates/reports/gate_0_2.json", rep)
    write_md(rep)
    log("report written; ok =", rep["ok"], {k: rep["criteria"][k].get("ok") for k in "abcde"})
    return 0 if rep["ok"] else 1


def write_md(rep):
    C = rep["criteria"]
    L = ["# Gate 0.2 (unit corpus validity, docs/PORTING.md §4.2)", "",
         f"Generated {rep['generated']} by `gates/gate_0_2.sh` (`gates/lib/unit_gate02.py`). Work dir `build/gate02/`.", "",
         f"**Overall: {'PASS' if rep['ok'] else 'FAIL'}**", "",
         f"Result stamp (records, descriptors, hook stats, jars, JaCoCo jars, JVM flags, replay and helper sources, gate phase code): "
         f"`{rep['stamp']['atReport']['inputsSha256']}`; identical to the stamp the phases ran under: {rep['stamp']['match']}.", "",
         "| Criterion | Result | Measured |", "|---|---|---|"]
    a, b, c, d, e = (C[k] for k in "abcde")
    L.append(f"| (a) replay 100% | {'PASS' if a['ok'] else 'FAIL'} | {a['pass']}/{a['replayed']} pass, {a['fail']} fail, {a['excludedMarkedUnrepresentable']} excluded (descriptor-marked), "
             f"{a['excludedPostCallNotCaptured']} excluded (post-call not captured), {a['excludedTraceUnverified']} excluded (no verifiable passTrace); exclusions {a['excludedTotal']} = {a['excludedFraction']:.2%} (cap 3%: {'ok' if a['exclusionCapOk'] else 'EXCEEDED'}); post state unclassified {a['postStateUnclassified']}; "
             f"post-call-assertion methods unclassified {a['postCallAssertionAudit']['unclassified']}/{a['postCallAssertionAudit']['methods']}; "
             f"records demoted by the no-op vacuity rule {a['noopProcessor']['recordsDemotedNotCaptured']}; postcondition-count mismatches {a['postconditionCountMismatch']}; processor identity (passTrace, D-017 item 8): statuses {a['passTrace']['statusCounts']}, mismatch {a['passTrace']['mismatch']}, unverified (excluded) {a['excludedTraceUnverified']}, missing/incomplete trace files {len(a['passTrace']['missingOrIncompleteTraceFiles'])}; processor leaf check (additional): mismatch {a['processorIdentity']['mismatch']}, statuses {a['processorIdentity']['statusCounts']}; rule 6 {'clean' if a['rule6']['ok'] else 'VIOLATED'} |")
    L.append(f"| (b) records within 2% of hooked calls | {'PASS' if b['ok'] else 'FAIL'} | {b['records']} records vs {b['hookedOutermost']} hooked (outermost) calls, deviation {b['totalDeviation']:.4%}; {len(b['classesOutside2pct'])} classes outside 2% |")
    if "ratio" in c:
        L.append(f"| (c) coverage ratio >= 98% | {'PASS' if c['ok'] else 'FAIL'} | in-scope src/ ({c['srcFiles']} files): harness-class original {c['harnessPct']:.2f}%, replay {c['replayPct']:.2f}%, ratio {c['ratio']:.4f} (whole suite {c['origPct']:.2f}%, ratio {c['wholeSuiteRatio']:.4f}; all of src/, informational: ratio {c['info_allSrc']['ratio']:.4f}); non-harness in-scope classes not in RUST_UNIT_TESTS.md: {len(c['nonHarnessNotListedInRustUnitTests'])} of {c['nonHarnessInScopeClasses']}; RUST_UNIT_TESTS.md fresh: {c['rustUnitTestsMdFresh']} |")
    else:
        L.append(f"| (c) coverage ratio >= 98% | FAIL | not measured: {json.dumps(c)[:200]} |")
    nd = sum(1 for r in d["passes"] if r["ok"])
    ng = d.get("groupsReaching90", 0)
    L.append(f"| (d) class-level no-op mutation >= 90% of change-expecting records per pass and per group | {'PASS' if d['ok'] else 'FAIL'} | {nd}/{len(d['passes'])} passes, {ng}/{d.get('groupsWithChangeExpecting', 0)} groups with change-expecting records reach 90% |")
    L.append(f"| (e) unrepresentable <= 3% | {'PASS' if e['ok'] else 'FAIL'} | {e['withUnrepresentable']}/{e['records']} = {e['fraction']:.2%} |")
    L += ["", "## (a) Replay", "",
          f"Every record-bearing class ({len(a['perClass'])} classes) was replayed through `ReplayMain` with the same JVM flags as "
          "`scripts/unit_replay.sh` (no coverage agent), cwd = the recording workspace. Records marked unrepresentable in a "
          "descriptor are excluded from the denominator and counted in (e).", ""]
    if a["failures"]:
        L += ["Failures (first 50):", ""] + [f"- {x}" for x in a["failures"]] + [""]
    if a["classesMissing"]:
        L += [f"Classes not replayed: {a['classesMissing']}", ""]
    r6 = a["rule6"]
    L += ["**Rule 6 proof.** Replay classpath = `" + "` + `".join(r6["classpath"]) + "`. "
          f"All {r6['classEntries']} class entries are listed in `{r6['entriesListing']}`. Each entry's name was matched "
          "against `Test(\\$.*)?\\.class$` and each class file's `SourceFile` attribute was parsed and matched against `Test.java$`. "
          f"Hits by name: {len(r6['nameEndsInTest'])}; hits by SourceFile: {len(r6['sourceFileEndsInTestJava'])}; "
          f"class-file parse errors: {len(r6['parseErrors'])}. Allowed (the replay harness's own classes, sources in `oracle/replay/src`): "
          f"{len(r6['allowedReplayHarnessClasses'])}. Support jar sha256 `{r6['supportJarSha256']}`.", ""]
    for x in (r6["nameEndsInTest"] + r6["sourceFileEndsInTestJava"])[:30]:
        L.append(f"- VIOLATION {x}")
    u = r6["unzipGrepCrossCheck"]
    L += [f"Independent cross-check: `unzip -Z1` lists {u['jarEntries']} entries of the support jar, and the replay class "
          f"directory holds {u['replayClassFiles']} class files (all listed in `{u['listing']}`). grep `{u['grep']}` over them: "
          f"{len(u['jarHits'])} hits in the jar, {len(u['jarHitsInClosureNamespaces'])} of them in `com/google/javascript` or "
          f"`com/google/debugging` (must be 0); {len(u['replayClassesHits'])} hits in replay-classes, all of them the replay "
          f"harness or helper-compiled classes ({len(u['replayClassesUnexplained'])} unexplained, must be 0). The original suite jar "
          "`unit_all_tests.jar` is not on the replay classpath.", ""]
    for x in u["jarHitsInClosureNamespaces"] + u["replayClassesUnexplained"]:
        L.append(f"- VIOLATION (cross-check) {x}")
    L += ["Not counted as violations, listed for audit:", ""]
    L += [f"- helper class with a Test-like binary name, compiled from a verbatim helper (not from a `*Test.java`): `{x}`"
          for x in r6.get("helperClassesWithTestLikeBinaryNames", [])]
    L += [f"- third-party library class (not in Closure's namespaces, no source under the reference `test/`): `{x}`"
          for x in r6.get("thirdPartyTestNamedNotOriginal", [])]
    L.append("")
    L.append(f"The recording workspace `src/` is byte-identical to the pristine `src/` (diff -rq): "
             f"{rep['inputs']['recordingWorkspaceSrcIdenticalToPristine']}.")
    L.append("")
    L += ["## (a) Post-call state, post-call assertions and no-op vacuity", "",
          f"Post-call state (FORMAT.md \"Post-call state\"): unclassified records {a['postStateUnclassified']} by class "
          f"{json.dumps(a['postStateUnclassifiedByClass'])}; excluded as not captured {a['excludedPostCallNotCaptured']}. "
          f"Of these, records that replay compares but whose compared value holds library-internal object dumps "
          f"(protobuf messages, com.google.protobuf.*, com.google.common.*; FORMAT.md rule 1a), demoted from "
          f"checked to unclassified: {json.dumps(a.get('postStateDemotedLibraryInternal'))}.", "",
          f"Static post-call-assertion audit (gates/lib/unit_postassert_scan.py): {a['postCallAssertionAudit']['methods']} methods, "
          f"{a['postCallAssertionAudit']['captured']} captured, {a['postCallAssertionAudit']['rustUnitTest']} Rust unit test, "
          f"{a['postCallAssertionAudit']['claimInvalid']} with a captured claim the records do not support, "
          f"{a['postCallAssertionAudit']['unclassified']} unclassified (including invalid claims); postcall file schema errors "
          f"{a['postCallAssertionAudit']['schemaErrorCount']}; descriptor `postCallAssertions` entries ignored (must move to "
          f"corpus/unit/postcall/): {a['postCallAssertionAudit']['legacyDescriptorCount']}. Classification source: "
          "corpus/unit/postcall/<Class>.json (FORMAT.md). Per class: `criteria.a.postCallAssertionAudit.perClass` in gate_0_2.json; "
          "per method: build/gate02/postcall_audit_methods.json.", "",
          f"Exclusions (D-015 (a), cap 3% of all records): {a['excludedTotal']} = {a['excludedFraction']:.2%}.", "",
          f"Whole-processor no-op (`--noop-processor`): {a['noopProcessor']['recordsCountedThatPassUnderNoop']} counted compiler_test_case records "
          f"pass (no assertion-type failure). Vacuous classes: {', '.join(a['noopProcessor']['vacuousClasses']) or 'none'}; "
          f"{a['noopProcessor']['recordsDemotedNotCaptured']} of their records have no replay-checked post-call state or postcondition and "
          f"are notCaptured (excluded from (a), counted in (e)). Postcondition-count mismatches: {a['postconditionCountMismatch']}.", "",
          f"Harness-pass no-op (descriptor `vacuityNoopClass`, class-level NoopAgent): classes rescued from vacuity "
          f"{json.dumps({r['class']: [(v['pass'], v.get('failAssertionType')) for v in r['vacuityNoopClass']] for r in a['noopProcessor']['perClass'] if r['vacuousUnderNoopProcessor'] and not r['vacuous']})}; "
          f"classes still vacuous although they name one {json.dumps({r['class']: [(v['pass'], v.get('failAssertionType'), v.get('error')) for v in r['vacuityNoopClass']] for r in a['noopProcessor']['perClass'] if r['vacuous'] and r['vacuityNoopClass']})}; "
          f"errors {json.dumps(a['noopProcessor']['vacuityNoopClassErrors'])}.", "",
          f"Integration api=compile records resolved (not notCaptured): {json.dumps(a['postCallAssertionAudit']['integrationCompileResolved'])} "
          f"by class {json.dumps(a['postCallAssertionAudit']['integrationCompileResolvedByClass'])} (noPostCallAssertion = the static scan "
          f"flags no assertion after the call; capturedClaimVerified = the method's captured claim via observed.* holds).", ""]
    L += ["## (b) Record count vs hooked calls", "",
          f"Records are counted independently (lines in `corpus/unit/records/*.jsonl.gz`) and compared per class with the "
          f"hook's outermost-call count in `{b['statsDir']}`. Total {b['records']} records vs {b['hookedOutermost']} outermost hooked "
          f"calls (deviation {b['totalDeviation']:.4%}); hooked entries at all depths, including nested calls that by design are not recorded: "
          f"{b['hookedAllDepths']}. JVM crashes during recording: {len(b['jvmCrashes'])}.", ""]
    for x in b["classesOutside2pct"][:40]:
        L.append(f"- outside 2%: {x}")
    L += ["", "Caveat: the recorder writes exactly one record per outermost hooked call, so this comparison mainly guards "
          "against lost or truncated record files. Calls that bypass the hooked harness APIs are neither counted nor recorded "
          "(known cases: about 8,000 `printHelper` comparisons in PeepholeFoldConstantsTest; 70 TypeCheckTest tests that use "
          "`testClosureTypes*`, `typeCheck(Node)`, `testNameNode` or `TypedScopeCreator` directly). Their effect shows up in (c), "
          "not here. The hook counts only when recording is on, so the counts come from the recording run itself."]
    L += ["", "## (c) Coverage", ""]
    if "ratio" in c:
        L += [f"JaCoCo {c['agent']['version']} from Maven Central: agent `{c['agent']['url']}` sha256 `{c['agent']['sha256']}`; "
              f"CLI `{c['cli']['url']}` sha256 `{c['cli']['sha256']}`. Agent options (identical in both runs): `{c['agentOptions']}`. "
              "Both runs are plain `java -javaagent:...` outside Bazel, one JVM per test class, same JVM flags, cwd = recording workspace, "
              "classes from `unit_support_deploy.jar`. Original suite = all 432 `*Test` classes via `UnitRecordingMain` with the "
              "recording hook off. Lines are counted for source files that exist under the pristine `src/`.", "",
          f"Scope (gate 0.2 (c) 'in-scope src/'): the verdict counts only source files that are in scope under §2, "
          f"the same test (d) ranks passes with: the directories {', '.join(c['scope']['excludedDirs'])} and the files whose simple name starts with "
          f"{', '.join(c['scope']['excludedNamePrefixes'])} are dropped. {c['srcFiles']} in-scope source files, {c['srcLines']} lines.", "",
          f"| in-scope src/ | covered lines | of {c['srcLines']} | % |", "|---|---:|---:|---:|",
          f"| original suite | {c['origCoveredLines']} | {c['srcLines']} | {c['origPct']:.2f} |",
          f"| original suite, harness-based classes only ({c['harnessClasses']}: classes with at least one hooked call, D-017 item 9) | {c['harnessCoveredLines']} | {c['srcLines']} | {c['harnessPct']:.2f} |",
          f"| corpus replay | {c['replayCoveredLines']} | {c['srcLines']} | {c['replayPct']:.2f} |", "",
          f"Classes that inherit a harness (static class-hierarchy check of the pristine test/ tree, {c['harnessHierarchy']}) but produced zero records: non-harness under D-017 item 9, not in the denominator, listed in RUST_UNIT_TESTS.md: {c['harnessClassesWithZeroRecords']}.", "",
          f"Gate measure (D-015, in-scope src/): replay / harness-based classes = **{c['ratio']:.4f}** (threshold 0.98; "
          f"{c['scope']['neededReplayLinesFor98']} replay-covered lines needed, {c['replayCoveredLines']} covered). "
          f"Whole-suite ratio on in-scope src/ (reported, not the gate): {c['wholeSuiteRatio']:.4f}. "
          f"In-scope non-harness test classes: {c['nonHarnessInScopeClasses']}, not listed in corpus/unit/RUST_UNIT_TESTS.md: {c['nonHarnessNotListedInRustUnitTests'][:20]}.", "",
          f"Informational only (not the gate): over all of src/ ({c['info_allSrc']['srcFiles']} files, out-of-scope code included) the "
          f"original suite covers {c['info_allSrc']['origCoveredLines']} lines, its harness-based classes {c['info_allSrc']['harnessCoveredLines']}, "
          f"the replay {c['info_allSrc']['replayCoveredLines']}: ratio {c['info_allSrc']['ratio']:.4f}, whole-suite ratio {c['info_allSrc']['wholeSuiteRatio']:.4f}. "
          f"Replay failures under the agent: {c['replayFailuresUnderAgent']}. "
          f"Original-suite tests not passing in the coverage run: {len(c['origSuiteNonPassingTests'])} {c['origSuiteNonPassingTests'][:10]}.", "",
          "Top 30 in-scope src files by lost covered lines (original harness-based classes minus replay):", "",
          "| file | lines | original | replay | loss |", "|---|---:|---:|---:|---:|"]
        L += [f"| {x['file']} | {x['lines']} | {x['origCovered']} | {x['replayCovered']} | {x['loss']} |" for x in c["top30Loss"]]
    else:
        L.append(f"Not measured: `{json.dumps(c)[:600]}`")
    L += ["", "## (d) No-op mutation", "",
          "Attribution of records to passes: " + d["attribution"].replace("\n", " "), "",
          "Ranking (D-015): the 30 in-scope passes with the most change-expecting records. " + d["rankingRule"].replace("\n", " "), "",
          "Mutation (D-015): " + d["mutation"] + ". For each pass, every class holding records of the pass was replayed under the agent "
          "(`-javaagent:noop-agent.jar=<FQCN>`). A record counts as failed when it fails with an assertion-type failure under mutation "
          "and passed in (a); crashes of test-side code never count. The rate is over change-expecting records; records that expect "
          "no change are reported and are expected to pass. A pass whose entry points the agent never rewrote fails, and so does a pass "
          "with a loaded pass-role class under P that has no REWROTE line (declared or synthesized for an inherited entry point), "
          "unless every executable entry point of that class is inherited from a method with a REWROTE line ("
          "such classes are listed in brackets as covered).", "",
          "| pass | change-expecting | failed | rate | no-change records | no-change failing | entry points rewritten | loaded pass classes not rewritten | >= 90% |",
          "|---|---:|---:|---:|---:|---:|---|---|---|"]
    for r in d["passes"]:
        L.append(f"| {r.get('label') or r['pass'].replace('com.google.javascript.jscomp.', '')} | {r['changeExpectingRecords']} | {r['changeExpectingFailed']} | "
                 f"{r['rate']:.1%} | {r['noChangeRecords']} | {r['noChangeRecordsFailingUnderMutation']} | "
                 f"{len(r['agentRewroteEntryPoints'])} | {', '.join(x.rsplit('.', 1)[1] for x in r.get('agentLoadedPassClassesNotRewritten', [])) or 0}"
                 f"{(' [covered: ' + ', '.join(x.rsplit('.', 1)[1] for x in r['agentLoadedPassClassesCoveredByInheritedRewrite']) + ']') if r.get('agentLoadedPassClassesCoveredByInheritedRewrite') else ''} | {'yes' if r['ok'] else 'NO'} |")
    L += ["", "### (d) Groups (D-017 item 11)", "", "Measurement units: " + d.get("unitsRule", "").replace("\n", " "), "",
          f"Change-expecting facts (D-017 item 10): code comparison statuses {json.dumps(d.get('changeFacts', {}).get('code'))}; "
          f"postcondition comparison statuses {json.dumps(d.get('changeFacts', {}).get('postcondition'))}.", "",
          "Each group is no-op'd jointly (`-javaagent:noop-agent.jar=A;B;C`). A group whose records all expect no change is reported and not judged.", "",
          "| group | change-expecting | failed | rate | no-change records | no-change failing | classes | members without a REWROTE line (reported) | loaded pass classes not rewritten | >= 90% |",
          "|---|---:|---:|---:|---:|---:|---|---|---|---|"]
    for r in d.get("groups", []):
        L.append(f"| {r['label']} | {r['changeExpectingRecords']} | {r['changeExpectingFailed']} | "
                 f"{format(r['rate'], '.1%') if r['changeExpectingRecords'] else 'n/a'} | {r['noChangeRecords']} | "
                 f"{r['noChangeRecordsFailingUnderMutation']} | {', '.join(x['class'] for x in r['perClass'])[:200]} | "
                 f"{', '.join(x.rsplit('.', 1)[1] for x in r.get('membersWithoutRewroteLine', [])) or 0} | "
                 f"{', '.join(x.rsplit('.', 1)[1] for x in r.get('agentLoadedPassClassesNotRewritten', [])) or 0} | "
                 f"{('yes' if r['ok'] else 'NO') if r['changeExpectingRecords'] else 'not judged'} |")
    ta = d.get("typeCheckAlone")
    if ta:
        L += ["", f"TypeCheck alone (information, not part of the verdict): {ta['changeExpectingFailed']}/{ta['changeExpectingRecords']} "
              f"change-expecting records fail ({ta['rate']:.1%}); {ta['noChangeRecords']} no-change records, "
              f"{ta['noChangeRecordsFailingUnderMutation']} of them failing."]
    if d["incompleteUnits"]:
        L += ["", f"Incomplete mutation units: {d['incompleteUnits'][:30]}"]
    causes = list(D_SURVIVOR_CAUSES)
    L += ["", "### (d) Surviving change-expecting records by cause (informational, not part of the verdict)", "",
          "A surviving record is change-expecting, passed in (a), and did not fail with an assertion-type failure under "
          "the class-level no-op of the pass. Each survivor gets the first matching cause:", ""]
    L += [f"- `{k}`: {v}." for k, v in D_SURVIVOR_CAUSES.items()]
    L += ["", "| pass | survivors | " + " | ".join(causes) + " | classes holding most survivors |",
          "|---|---:|" + "---:|" * len(causes) + "---|"]
    for r in d["passes"]:
        sc = r.get("info_survivorsByCause") or {}
        if not sc:
            continue
        top_cls = sorted(((sum(v.values()), c) for c, v in (r.get("info_survivorsByClass") or {}).items()), reverse=True)[:3]
        L.append(f"| {r['pass'].replace('com.google.javascript.jscomp.', '')} | {sum(sc.values())} | "
                 + " | ".join(str(sc.get(k, 0)) for k in causes) + " | "
                 + ", ".join(f"{c} ({n})" for n, c in top_cls) + " |")
    L += ["", "## (e) Unrepresentable records", "", f"Definition: {e['definition']}. {e['withUnrepresentable']} of {e['records']} records = {e['fraction']:.2%} (limit 3%).", "",
          f"Records with a placeholder, by section: {json.dumps(e['recordsWithPlaceholderBySection'])}. Listed-or-marked only (the pre-round-1 definition): "
          f"{e['info_listedOrMarkedOnly']}. Consistency: records with a placeholder but an empty `unrepresentable` list: {e['consistency_placeholderRecordsWithEmptyList']} (must be 0).", "",
          "| class | records | recorder-level | descriptor-marked | post-call not captured | sections | recorder-level failing replay | main reasons |", "|---|---:|---:|---:|---:|---|---:|---|"]
    for cls, ent in e["perClass"].items():
        rs = "; ".join(f"{k} ({v})" for k, v in list(ent["reasons"].items())[:3]).replace("|", "\\|")
        secs = ", ".join(f"{k} {v}" for k, v in sorted(ent.get("sections", {}).items()))
        L.append(f"| {cls} | {ent['records']} | {ent['recorderLevel']} | {ent['descriptorMarked']} | {ent.get('postCallNotCaptured', 0)} | {secs} | {ent['recorderLevelRecordsFailingReplay']} | {rs[:400]} |")
    L += ["", "Per-class plans: each descriptor's `unrepresentableAudit`/`note` is copied into `gate_0_2.json` (`criteria.e.perClass.*.descriptorNote`)."]
    with open(f"{ROOT}/gates/reports/gate_0_2.md", "w") as f:
        f.write("\n".join(L) + "\n")


if __name__ == "__main__":
    cmd = sys.argv[1]
    fn = {"prep": cmd_prep, "replay": cmd_replay, "cov-orig": cmd_cov_orig, "cov-replay": cmd_cov_replay,
          "mutate": cmd_mutate, "noop": cmd_noop, "report": cmd_report}[cmd]
    sys.exit(fn() or 0)
