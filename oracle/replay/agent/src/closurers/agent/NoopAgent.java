package closurers.agent;

import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.PrintStream;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.nio.charset.StandardCharsets;
import java.security.ProtectionDomain;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import jdk.internal.org.objectweb.asm.ClassReader;
import jdk.internal.org.objectweb.asm.ClassVisitor;
import jdk.internal.org.objectweb.asm.ClassWriter;
import jdk.internal.org.objectweb.asm.MethodVisitor;
import jdk.internal.org.objectweb.asm.Opcodes;

/**
 * Class-level no-op mutation for gate 0.2(d) (D-015, HARNESS.md
 * "Class-level no-op mutation"). Usage:
 *
 * <pre>-javaagent:noop-agent.jar=&lt;pass FQCN&gt;[;&lt;pass FQCN&gt;...][,report=&lt;file&gt;]</pre>
 * <pre>-javaagent:noop-agent.jar=trace=&lt;scope file&gt;[,report=&lt;file&gt;]</pre>
 *
 * <p>Group no-op (D-017 item 11): several FQCNs separated by {@code ;} are no-op'd jointly in the
 * same JVM (each one and its nested classes, exactly as for a single target).
 *
 * <p>TRACE mode (D-017 item 8, HARNESS.md "Pass trace"): no class is no-op'd. Every class whose
 * top-level class FQCN is listed in the scope file (one FQCN per line: the in-scope src top-level
 * classes) gets, at the start of each declared concrete entry point of the same entry-point set as
 * the no-op rewrite, a call {@code closurers.agent.Trace.hit(id)}, id = that top-level class.
 * {@link Trace#begin()} / {@link Trace#end()} bracket one hooked call; end() returns the sorted set
 * of top-level classes whose entry points executed in between (the record's {@code passTrace}).
 *
 * When the JVM loads the pass class P, or a class nested in P (binary name P$...), the bodies of its
 * declared pass entry points are replaced before the class is defined, so the pass is a no-op
 * wherever it is constructed (harness, descriptor, helper, wrapper). src/ is not modified; the
 * rewrite happens only in this JVM's memory. Entry points (declared, non-abstract methods only):
 *
 * <ul>
 *   <li>CompilerPass: {@code process(Node,Node)V}, {@code hotSwapScript(Node,Node)V} -> return
 *   <li>OptimizeCalls.CallGraphCompilerPass: {@code process(Node,Node,ReferenceMap)V} -> return
 *   <li>AbstractPeepholeOptimization / AbstractPeepholeTranspilation: {@code optimizeSubtree(Node)},
 *       {@code transpileSubtree(Node)} -> return the argument unchanged
 *   <li>NodeTraversal callback roles (a pass may be handed to NodeTraversal directly, e.g. {@code
 *       NodeTraversal.traverse(compiler, root, new P(compiler))}): {@code
 *       shouldTraverse(NodeTraversal,Node,Node)Z} -> true; {@code visit(NodeTraversal,Node,Node)V},
 *       {@code enterScope/exitScope/enterScopeWithCfg/exitScopeWithCfg(NodeTraversal)V}, {@code
 *       enterChangedScopeRoot(AbstractCompiler,Node)V} -> return
 * </ul>
 *
 * Inherited entry points: when P or P$... inherits a concrete entry point from a
 * superclass outside P (for example {@code ReplaceMessages$FullReplacementPass extends
 * JsMessageVisitor}, whose process/visit are declared in JsMessageVisitor), the agent synthesizes an
 * overriding no-op method in the loaded class and reports it as {@code REWROTE <class>.<name><desc>
 * (synthesized, inherited from <superclass>)}. An inherited entry point declared final cannot be
 * overridden and is reported as {@code FINAL-INHERITED <class>.<name><desc> <superclass>}.
 *
 * <p>Every rewritten method is written to the report file as {@code REWROTE <class>.<name><desc>}, and
 * every load of a matching class as {@code LOADED <class>}, so the gate can prove the mutation
 * took effect.
 */
public final class NoopAgent {
  private static final String NODE = "Lcom/google/javascript/rhino/Node;";
  private static final String NT = "Lcom/google/javascript/jscomp/NodeTraversal;";
  private static final String PROCESS2 = "(" + NODE + NODE + ")V";
  private static final String PROCESS3 =
      "(" + NODE + NODE + "Lcom/google/javascript/jscomp/OptimizeCalls$ReferenceMap;)V";
  private static final String SUBTREE = "(" + NODE + ")" + NODE;
  private static final String CB3V = "(" + NT + NODE + NODE + ")V";
  private static final String CB3Z = "(" + NT + NODE + NODE + ")Z";
  private static final String SCOPE = "(" + NT + ")V";
  private static final String CHANGED_ROOT =
      "(Lcom/google/javascript/jscomp/AbstractCompiler;" + NODE + ")V";

  /** No-op targets, internal names, e.g. com/google/javascript/jscomp/RenameVars (group no-op). */
  private static final List<String> targets = new ArrayList<>();
  /** TRACE mode: in-scope src top-level classes (internal names); null when not tracing. */
  private static Set<String> traceScope;
  private static PrintStream report;

  private NoopAgent() {}

  public static void premain(String args, Instrumentation inst) throws Exception {
    if (args == null || args.isBlank()) {
      throw new IllegalArgumentException("NoopAgent needs the pass FQCN as its argument");
    }
    String reportPath = null;
    String[] parts = args.split(",");
    String tracePath = null;
    if (parts[0].trim().startsWith("trace=")) {
      tracePath = parts[0].trim().substring("trace=".length());
    } else {
      for (String t : parts[0].split(";")) {
        if (!t.isBlank()) {
          targets.add(t.trim().replace('.', '/'));
        }
      }
    }
    for (int i = 1; i < parts.length; i++) {
      if (parts[i].startsWith("report=")) {
        reportPath = parts[i].substring("report=".length());
      }
    }
    report =
        reportPath == null
            ? System.err
            : new PrintStream(new FileOutputStream(reportPath, true), true, StandardCharsets.UTF_8);
    if (tracePath != null) {
      traceScope = new HashSet<>();
      for (String l : java.nio.file.Files.readAllLines(java.nio.file.Path.of(tracePath))) {
        if (!l.isBlank()) {
          traceScope.add(l.trim().replace('.', '/'));
        }
      }
      if (traceScope.isEmpty()) {
        throw new IllegalArgumentException("NoopAgent trace scope file is empty: " + tracePath);
      }
      Trace.enable();
      if (reportPath != null) {
        report.println("TRACE " + traceScope.size() + " in-scope top-level classes");
      }
    } else {
      if (targets.isEmpty()) {
        throw new IllegalArgumentException("NoopAgent needs the pass FQCN(s) as its argument");
      }
      for (String t : targets) {
        report.println("TARGET " + t.replace('/', '.'));
      }
    }
    inst.addTransformer(new Transformer(), false);
  }

  static boolean matches(String internalName) {
    if (internalName == null) {
      return false;
    }
    for (String target : targets) {
      if (internalName.equals(target) || internalName.startsWith(target + "$")) {
        return true;
      }
    }
    return false;
  }

  static String topLevel(String internalName) {
    int d = internalName.indexOf('$');
    return d < 0 ? internalName : internalName.substring(0, d);
  }

  static boolean traced(String internalName) {
    return traceScope != null && internalName != null && traceScope.contains(topLevel(internalName));
  }

  static boolean isPassEntry(String name, String desc) {
    return (name.equals("process") && (desc.equals(PROCESS2) || desc.equals(PROCESS3)))
        || (name.equals("hotSwapScript") && desc.equals(PROCESS2))
        || ((name.equals("optimizeSubtree") || name.equals("transpileSubtree"))
            && desc.equals(SUBTREE));
  }

  static boolean isCallbackEntry(String name, String desc) {
    return ((name.equals("visit") && desc.equals(CB3V))
        || (name.equals("shouldTraverse") && desc.equals(CB3Z))
        || ((name.equals("enterScope")
                || name.equals("exitScope")
                || name.equals("enterScopeWithCfg")
                || name.equals("exitScopeWithCfg"))
            && desc.equals(SCOPE))
        || (name.equals("enterChangedScopeRoot") && desc.equals(CHANGED_ROOT)));
  }

  static final class Transformer implements ClassFileTransformer {
    @Override
    public byte[] transform(
        ClassLoader loader,
        String className,
        Class<?> redefined,
        ProtectionDomain pd,
        byte[] bytes) {
      if (traceScope != null) {
        if (!traced(className)) {
          return null;
        }
        try {
          return instrumentTrace(className, bytes);
        } catch (Throwable t) {
          report.println("ERROR " + className.replace('/', '.') + " " + t);
          t.printStackTrace(report);
          throw new IllegalStateException("trace instrumentation failed for " + className, t);
        }
      }
      if (!matches(className)) {
        return null;
      }
      try {
        return rewrite(loader, className, bytes);
      } catch (Throwable t) {
        report.println("ERROR " + className.replace('/', '.') + " " + t);
        t.printStackTrace(report);
        return null;
      }
    }
  }

  /** Nearest declaration of each entry point (name+desc) in the superclass chain of cr: value is
   * {declaring class, access}. Declarations in classes matching P end the search for that method
   * (they are rewritten when they load). */
  static Map<String, Object[]> inheritedEntries(ClassLoader loader, ClassReader cr) {
    Map<String, Object[]> out = new LinkedHashMap<>();
    Set<String> seen = new HashSet<>();
    String sup = cr.getSuperName();
    while (sup != null && !sup.equals("java/lang/Object")) {
      byte[] b = readClass(loader, sup);
      if (b == null) {
        report.println("UNRESOLVED-SUPER " + sup.replace('/', '.'));
        break;
      }
      ClassReader sr = new ClassReader(b);
      final String owner = sup;
      final boolean inP = matches(owner);
      sr.accept(
          new ClassVisitor(Opcodes.ASM9) {
            @Override
            public MethodVisitor visitMethod(
                int access, String name, String desc, String sig, String[] exc) {
              if ((access & (Opcodes.ACC_STATIC | Opcodes.ACC_PRIVATE)) != 0
                  || (access & (Opcodes.ACC_BRIDGE | Opcodes.ACC_SYNTHETIC)) != 0
                  || !(isPassEntry(name, desc) || isCallbackEntry(name, desc))) {
                return null;
              }
              String key = name + desc;
              if (seen.add(key) && !inP && (access & Opcodes.ACC_ABSTRACT) == 0) {
                out.put(key, new Object[] {owner, access});
              }
              return null;
            }
          },
          ClassReader.SKIP_CODE);
      sup = sr.getSuperName();
    }
    return out;
  }

  static byte[] readClass(ClassLoader loader, String internalName) {
    ClassLoader l = loader != null ? loader : ClassLoader.getSystemClassLoader();
    try (InputStream in = l.getResourceAsStream(internalName + ".class")) {
      return in == null ? null : in.readAllBytes();
    } catch (Exception e) {
      return null;
    }
  }

  static void emitNoop(MethodVisitor mv, int access, String desc) {
    mv.visitCode();
    if (desc.endsWith(")V")) {
      mv.visitInsn(Opcodes.RETURN);
    } else if (desc.endsWith(")Z")) {
      mv.visitInsn(Opcodes.ICONST_1);
      mv.visitInsn(Opcodes.IRETURN);
    } else {
      // optimizeSubtree / transpileSubtree: return the subtree unchanged.
      int slot = (access & Opcodes.ACC_STATIC) != 0 ? 0 : 1;
      mv.visitVarInsn(Opcodes.ALOAD, slot);
      mv.visitInsn(Opcodes.ARETURN);
    }
    mv.visitMaxs(0, 0);
    mv.visitEnd();
  }

  /** TRACE mode: prepend Trace.hit(id of the top-level class) to every declared concrete entry
   * point (same entry-point set as the no-op rewrite). Returns null when the class has none. */
  static byte[] instrumentTrace(String className, byte[] bytes) {
    ClassReader cr = new ClassReader(bytes);
    final int id = Trace.id(topLevel(className).replace('/', '.'));
    final boolean[] changed = {false};
    ClassWriter cw = new ClassWriter(cr, ClassWriter.COMPUTE_MAXS);
    cr.accept(
        new ClassVisitor(Opcodes.ASM9, cw) {
          @Override
          public MethodVisitor visitMethod(
              int access, String name, String desc, String sig, String[] exc) {
            MethodVisitor mv = super.visitMethod(access, name, desc, sig, exc);
            boolean concrete = (access & (Opcodes.ACC_ABSTRACT | Opcodes.ACC_NATIVE)) == 0;
            boolean bridge = (access & (Opcodes.ACC_BRIDGE | Opcodes.ACC_SYNTHETIC)) != 0;
            boolean isStatic = (access & Opcodes.ACC_STATIC) != 0;
            if (!concrete
                || bridge
                || isStatic
                || !(isPassEntry(name, desc) || isCallbackEntry(name, desc))) {
              return mv;
            }
            changed[0] = true;
            return new MethodVisitor(Opcodes.ASM9, mv) {
              @Override
              public void visitCode() {
                super.visitCode();
                super.visitLdcInsn(id);
                super.visitMethodInsn(
                    Opcodes.INVOKESTATIC, "closurers/agent/Trace", "hit", "(I)V", false);
              }
            };
          }
        },
        0);
    return changed[0] ? cw.toByteArray() : null;
  }

  static byte[] rewrite(ClassLoader loader, String className, byte[] bytes) {
    ClassReader cr = new ClassReader(bytes);
    boolean isInterface = (cr.getAccess() & Opcodes.ACC_INTERFACE) != 0;
    Map<String, Object[]> inherited =
        isInterface ? new LinkedHashMap<>() : inheritedEntries(loader, cr);
    Set<String> declared = new HashSet<>();
    List<String> synthesized = new ArrayList<>();
    List<String> finalInherited = new ArrayList<>();
    List<String> rewrote = new ArrayList<>();
    ClassWriter cw = new ClassWriter(cr, ClassWriter.COMPUTE_MAXS);
    cr.accept(
        new ClassVisitor(Opcodes.ASM9, cw) {
          @Override
          public MethodVisitor visitMethod(
              int access, String name, String desc, String sig, String[] exc) {
            declared.add(name + desc);
            boolean concrete = (access & (Opcodes.ACC_ABSTRACT | Opcodes.ACC_NATIVE)) == 0;
            boolean bridge = (access & (Opcodes.ACC_BRIDGE | Opcodes.ACC_SYNTHETIC)) != 0;
            if (!concrete
                || bridge
                || !(isPassEntry(name, desc) || isCallbackEntry(name, desc))) {
              return super.visitMethod(access, name, desc, sig, exc);
            }
            MethodVisitor mv = super.visitMethod(access, name, desc, sig, exc);
            emitNoop(mv, access, desc);
            rewrote.add(name + desc);
            return null; // drop the original body
          }

          @Override
          public void visitEnd() {
            for (Map.Entry<String, Object[]> e : inherited.entrySet()) {
              String key = e.getKey();
              if (declared.contains(key)) {
                continue;
              }
              String owner = ((String) e.getValue()[0]).replace('/', '.');
              int acc = (Integer) e.getValue()[1];
              int p = key.indexOf('(');
              String name = key.substring(0, p);
              String desc = key.substring(p);
              if ((acc & Opcodes.ACC_FINAL) != 0) {
                finalInherited.add(key + " " + owner);
                continue;
              }
              int access = acc & (Opcodes.ACC_PUBLIC | Opcodes.ACC_PROTECTED);
              MethodVisitor mv = super.visitMethod(access, name, desc, null, null);
              emitNoop(mv, access, desc);
              synthesized.add(key + " (synthesized, inherited from " + owner + ")");
            }
            super.visitEnd();
          }
        },
        0);
    String dotted = className.replace('/', '.');
    report.println("LOADED " + dotted);
    for (String m : rewrote) {
      report.println("REWROTE " + dotted + "." + m);
    }
    for (String m : synthesized) {
      report.println("REWROTE " + dotted + "." + m);
    }
    for (String m : finalInherited) {
      report.println("FINAL-INHERITED " + dotted + "." + m);
    }
    return rewrote.isEmpty() && synthesized.isEmpty() ? null : cw.toByteArray();
  }
}
