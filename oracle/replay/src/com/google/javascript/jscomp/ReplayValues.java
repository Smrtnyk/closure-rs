/*
 * closure-rs unit-corpus replay (docs/PORTING.md §4.2): decoder for the record value encoding
 * documented in corpus/unit/FORMAT.md ("Value encoding"), plus reflective field restore.
 */
package com.google.javascript.jscomp;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSet;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.regex.Pattern;
import com.google.javascript.rhino.StaticSourceFile.SourceKind;

/** Decodes FORMAT.md-encoded values into Java objects. */
public final class ReplayValues {
  private ReplayValues() {}

  /** Thrown when a value cannot be rebuilt (e.g. it was recorded as unrepresentable). */
  public static final class Undecodable extends RuntimeException {
    public Undecodable(String msg) {
      super(msg);
    }
  }

  /** Java packages of the protobuf messages generated from the reference src/ .proto files. */
  private static final String[] PROTO_JAVA_PACKAGES = {
    "com.google.javascript.jscomp", "com.google.javascript.jscomp.serialization",
    "com.google.debugging.sourcemap.proto", "com.google.javascript.jscomp.instrumentation.reporter.proto",
  };

  /**
   * FORMAT.md "Neutral encodings" (D-017 item 2): rebuilds a protobuf message from {"proto": full
   * name, "fields": {...}}. The message class is the target class when it is a generated message,
   * else the generated class of that full name in one of PROTO_JAVA_PACKAGES (checked against its
   * descriptor's full name).
   */
  private static Object proto(JsonObject o, Class<?> target) {
    String full = o.get("proto").getAsString();
    Class<?> mc = null;
    if (target != null && com.google.protobuf.Message.class.isAssignableFrom(target) && target != com.google.protobuf.Message.class) {
      mc = target;
    } else {
      String rest = full.contains(".") ? full.substring(full.indexOf('.') + 1) : full;
      for (String pkg : PROTO_JAVA_PACKAGES) {
        try {
          Class<?> k = Class.forName(pkg + "." + rest.replace('.', '$'));
          com.google.protobuf.Descriptors.Descriptor d =
              (com.google.protobuf.Descriptors.Descriptor) k.getMethod("getDescriptor").invoke(null);
          if (d.getFullName().equals(full)) {
            mc = k;
            break;
          }
        } catch (ReflectiveOperationException | ClassCastException ex) {
          // try the next package
        }
      }
    }
    if (mc == null) {
      throw new Undecodable("no generated protobuf class for " + full);
    }
    try {
      com.google.protobuf.Message.Builder b = (com.google.protobuf.Message.Builder) mc.getMethod("newBuilder").invoke(null);
      return protoFill(b, o).build();
    } catch (ReflectiveOperationException ex) {
      throw new Undecodable("cannot build protobuf " + full + ": " + ex);
    }
  }

  private static com.google.protobuf.Message.Builder protoFill(com.google.protobuf.Message.Builder b, JsonObject o) {
    com.google.protobuf.Descriptors.Descriptor d = b.getDescriptorForType();
    if (!d.getFullName().equals(o.get("proto").getAsString())) {
      throw new Undecodable("protobuf type " + o.get("proto").getAsString() + " where " + d.getFullName() + " is expected");
    }
    for (Map.Entry<String, JsonElement> e : o.getAsJsonObject("fields").entrySet()) {
      com.google.protobuf.Descriptors.FieldDescriptor fd = d.findFieldByName(e.getKey());
      if (fd == null) {
        throw new Undecodable("no field " + e.getKey() + " in " + d.getFullName());
      }
      if (fd.isRepeated()) {
        for (JsonElement x : e.getValue().getAsJsonArray()) {
          b.addRepeatedField(fd, protoScalar(b, fd, x));
        }
      } else {
        b.setField(fd, protoScalar(b, fd, e.getValue()));
      }
    }
    return b;
  }

  private static Object protoScalar(com.google.protobuf.Message.Builder owner, com.google.protobuf.Descriptors.FieldDescriptor fd, JsonElement x) {
    switch (fd.getJavaType()) {
      case MESSAGE:
        return protoFill(owner.newBuilderForField(fd), x.getAsJsonObject()).build();
      case ENUM: {
        com.google.protobuf.Descriptors.EnumValueDescriptor v = fd.getEnumType().findValueByName(x.getAsString());
        if (v == null) {
          throw new Undecodable("no enum value " + x + " in " + fd.getEnumType().getFullName());
        }
        return v;
      }
      case BYTE_STRING: {
        com.google.protobuf.Descriptors.Descriptor d = owner.getDescriptorForType();
        if (x.isJsonPrimitive()) {
          // StringPoolProto.strings: UTF-8 strings
          return com.google.protobuf.ByteString.copyFromUtf8(x.getAsString());
        }
        JsonObject xo = x.getAsJsonObject();
        if (xo.has("astNode")) {
          return protoFill(com.google.javascript.jscomp.serialization.AstNode.newBuilder(), xo.getAsJsonObject("astNode"))
              .build().toByteString();
        }
        return com.google.protobuf.ByteString.copyFrom(java.util.Base64.getDecoder().decode(xo.get("bytes").getAsString()));
      }
      case LONG:
        return Long.parseLong(x.getAsJsonObject().get("long").getAsString());
      case FLOAT:
        return (float) Double.parseDouble(x.getAsJsonObject().get("double").getAsString());
      case DOUBLE:
        return Double.parseDouble(x.getAsJsonObject().get("double").getAsString());
      case BOOLEAN:
        return x.getAsBoolean();
      case INT:
        return x.getAsInt();
      default:
        return x.getAsString();
    }
  }

  public static Object decode(JsonElement e, Class<?> target) {
    if (e == null || e.isJsonNull()) {
      return null;
    }
    if (e.isJsonPrimitive()) {
      JsonPrimitive p = e.getAsJsonPrimitive();
      if (p.isBoolean()) {
        return p.getAsBoolean();
      }
      if (p.isString()) {
        return p.getAsString();
      }
      int i = p.getAsInt();
      if (target == short.class || target == Short.class) {
        return (short) i;
      }
      if (target == byte.class || target == Byte.class) {
        return (byte) i;
      }
      if (target == long.class || target == Long.class) {
        return (long) i;
      }
      if (target == double.class || target == Double.class) {
        return (double) i;
      }
      return i;
    }
    if (e.isJsonArray()) {
      List<Object> out = new ArrayList<>();
      for (JsonElement x : e.getAsJsonArray()) {
        out.add(decode(x, Object.class));
      }
      return out;
    }
    JsonObject o = e.getAsJsonObject();
    if (o.has("long")) {
      return Long.parseLong(o.get("long").getAsString());
    }
    if (o.has("double")) {
      double d = Double.parseDouble(o.get("double").getAsString());
      if (target == float.class || target == Float.class) {
        return (float) d;
      }
      return d;
    }
    if (o.has("char")) {
      return o.get("char").getAsString().charAt(0);
    }
    if (o.has("enum")) {
      return enumValue(o.get("enum").getAsString(), o.get("name").getAsString());
    }
    if (o.has("diagnosticType")) {
      JsonObject t = o.getAsJsonObject("diagnosticType");
      return DiagnosticType.make(
          t.get("key").getAsString(),
          CheckLevel.valueOf(t.get("level").getAsString()),
          "{0}");
    }
    if (o.has("diagnosticGroup")) {
      return diagnosticGroup(o.getAsJsonObject("diagnosticGroup"));
    }
    if (o.has("classRef")) {
      return classForName(o.get("classRef").getAsString());
    }
    if (o.has("regex")) {
      return Pattern.compile(o.get("regex").getAsString(), o.get("flags").getAsInt());
    }
    if (o.has("sourceFile")) {
      return sourceFile(o.getAsJsonObject("sourceFile"));
    }
    if (o.has("optional")) {
      JsonElement v = o.get("optional");
      return v.isJsonNull() ? Optional.empty() : Optional.of(decode(v, Object.class));
    }
    if (o.has("list") || o.has("set")) {
      boolean isSet = o.has("set");
      List<Object> items = new ArrayList<>();
      for (JsonElement x : o.getAsJsonArray(isSet ? "set" : "list")) {
        items.add(decode(x, Object.class));
      }
      String impl = o.has("impl") ? o.get("impl").getAsString() : "";
      return collection(items, isSet, impl);
    }
    if (o.has("map")) {
      Map<Object, Object> m = new LinkedHashMap<>();
      for (JsonElement kv : o.getAsJsonArray("map")) {
        JsonArray a = kv.getAsJsonArray();
        m.put(decode(a.get(0), Object.class), decode(a.get(1), Object.class));
      }
      String impl = o.has("impl") ? o.get("impl").getAsString() : "";
      if (impl.startsWith("com.google.common.collect.")) {
        return ImmutableMap.copyOf(m);
      }
      if (impl.equals("java.util.HashMap")) {
        return new java.util.HashMap<>(m);
      }
      if (impl.equals("java.util.TreeMap")) {
        return new java.util.TreeMap<>(m);
      }
      return m;
    }
    if (o.has("arrayOf")) {
      Class<?> comp = classForName(o.get("arrayOf").getAsString());
      JsonArray items = o.getAsJsonArray("items");
      Object arr = Array.newInstance(comp, items.size());
      for (int i = 0; i < items.size(); i++) {
        Array.set(arr, i, decode(items.get(i), comp));
      }
      return arr;
    }
    if (o.has("composeWarningsGuard")) {
      JsonArray guards = o.getAsJsonObject("composeWarningsGuard").getAsJsonArray("guards");
      // Effective order = iteration order; later-added guards of equal priority come first,
      // so add in reverse.
      List<WarningsGuard> gs = new ArrayList<>();
      for (JsonElement g : guards) {
        gs.add((WarningsGuard) decode(g, WarningsGuard.class));
      }
      ComposeWarningsGuard cg = new ComposeWarningsGuard();
      for (int i = gs.size() - 1; i >= 0; i--) {
        cg.addGuard(gs.get(i));
      }
      return cg;
    }
    if (o.has("proto")) {
      return proto(o, target);
    }
    if (o.has("object")) {
      return object(o.get("object").getAsString(), o.getAsJsonObject("fields"));
    }
    if (o.has("ref")) {
      throw new Undecodable("reference value " + o.get("ref").getAsString());
    }
    if (o.has("unrepresentable")) {
      throw new Undecodable("unrepresentable value " + o.get("unrepresentable").getAsString());
    }
    throw new Undecodable("unknown value encoding " + o);
  }

  static SourceFile sourceFile(JsonObject f) {
    String kind = f.has("kind") ? f.get("kind").getAsString() : "STRONG";
    return SourceFile.fromCode(
        f.get("name").getAsString(), f.get("code").getAsString(), SourceKind.valueOf(kind));
  }

  static List<SourceFile> sourceFiles(JsonElement e) {
    if (e == null || e.isJsonNull()) {
      return null;
    }
    List<SourceFile> out = new ArrayList<>();
    for (JsonElement x : e.getAsJsonArray()) {
      out.add(sourceFile(x.getAsJsonObject()));
    }
    return out;
  }

  static List<JSChunk> chunks(JsonElement e) {
    Map<String, JSChunk> byName = new LinkedHashMap<>();
    List<JSChunk> out = new ArrayList<>();
    for (JsonElement x : e.getAsJsonArray()) {
      JsonObject c = x.getAsJsonObject();
      JSChunk chunk = new JSChunk(c.get("name").getAsString());
      for (JsonElement d : c.getAsJsonArray("deps")) {
        chunk.addDependency(byName.get(d.getAsString()));
      }
      for (JsonElement in : c.getAsJsonArray("inputs")) {
        chunk.add(sourceFile(in.getAsJsonObject()));
      }
      byName.put(chunk.getName(), chunk);
      out.add(chunk);
    }
    return out;
  }

  private static Object collection(List<Object> items, boolean isSet, String impl) {
    if (impl.startsWith("com.google.common.collect.")) {
      return isSet ? ImmutableSet.copyOf(items) : ImmutableList.copyOf(items);
    }
    if (impl.equals("java.util.HashSet")) {
      return new java.util.HashSet<>(items);
    }
    if (impl.equals("java.util.TreeSet")) {
      return new java.util.TreeSet<>(items);
    }
    if (impl.equals("java.util.LinkedList")) {
      return new java.util.LinkedList<>(items);
    }
    if (impl.startsWith("java.util.Collections$Unmodifiable") || impl.startsWith("java.util.ImmutableCollections")) {
      return isSet
          ? java.util.Collections.unmodifiableSet(new LinkedHashSet<>(items))
          : java.util.Collections.unmodifiableList(items);
    }
    if (isSet) {
      return new LinkedHashSet<>(items);
    }
    return new ArrayList<>(items);
  }

  static DiagnosticGroup diagnosticGroup(JsonObject g) {
    JsonElement name = g.get("group");
    if (name != null && !name.isJsonNull()) {
      String n = name.getAsString();
      if (n.startsWith("DiagnosticGroups.")) {
        try {
          Field f = DiagnosticGroups.class.getDeclaredField(n.substring("DiagnosticGroups.".length()));
          f.setAccessible(true);
          return (DiagnosticGroup) f.get(null);
        } catch (ReflectiveOperationException ex) {
          throw new Undecodable("no DiagnosticGroups field " + n);
        }
      }
      DiagnosticGroup r = DiagnosticGroups.getRegisteredGroups().get(n);
      if (r != null) {
        return r;
      }
    }
    List<DiagnosticType> types = new ArrayList<>();
    for (JsonElement t : g.getAsJsonArray("types")) {
      types.add(DiagnosticType.error(t.getAsString(), "{0}"));
    }
    return new DiagnosticGroup(types.toArray(new DiagnosticType[0]));
  }

  @SuppressWarnings({"unchecked", "rawtypes"})
  static Object enumValue(String cls, String name) {
    return Enum.valueOf((Class) classForName(cls), name);
  }

  /**
   * Descriptor "classMap" of the class being replayed (DSL.md): recorded FQCN of a class declared
   * in a *Test class (kept off the classpath by rule 6) -> verbatim helper class FQCN.
   */
  public static volatile Map<String, String> classMap = Map.of();

  static Class<?> classForName(String n) {
    String mapped = classMap.get(n);
    if (mapped != null) {
      n = mapped;
    }
    switch (n) {
      case "int":
        return int.class;
      case "long":
        return long.class;
      case "boolean":
        return boolean.class;
      case "double":
        return double.class;
      case "char":
        return char.class;
      default:
        break;
    }
    try {
      return Class.forName(n, false, ReplayValues.class.getClassLoader());
    } catch (ClassNotFoundException ex) {
      throw new Undecodable("class not found " + n);
    }
  }

  /** Generic object: special-cased guava Optional; otherwise no-arg construct + field restore. */
  static Object object(String cls, JsonObject fields) {
    if (cls.equals("com.google.common.base.Present")) {
      return com.google.common.base.Optional.of(decode(fields.get("reference"), Object.class));
    }
    if (cls.equals("com.google.common.base.Absent")) {
      return com.google.common.base.Optional.absent();
    }
    Class<?> c = classForName(cls);
    if (c.isRecord()) {
      // Records have truly final fields: rebuild through the canonical constructor.
      java.lang.reflect.RecordComponent[] rc = c.getRecordComponents();
      Class<?>[] types = new Class<?>[rc.length];
      Object[] vals = new Object[rc.length];
      for (int i = 0; i < rc.length; i++) {
        types[i] = rc[i].getType();
        vals[i] = adapt(decode(fields.get(rc[i].getName()), types[i]), types[i]);
      }
      try {
        Constructor<?> k = c.getDeclaredConstructor(types);
        k.setAccessible(true);
        return k.newInstance(vals);
      } catch (ReflectiveOperationException ex) {
        throw new Undecodable("cannot build record " + cls + ": " + ex);
      }
    }
    Object inst = instantiate(c);
    setFields(inst, c, Object.class, fields);
    return inst;
  }

  static Object instantiate(Class<?> c) {
    try {
      Constructor<?> k = c.getDeclaredConstructor();
      k.setAccessible(true);
      return k.newInstance();
    } catch (ReflectiveOperationException ex) {
      try {
        Field uf = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        uf.setAccessible(true);
        Object unsafe = uf.get(null);
        return unsafe.getClass().getMethod("allocateInstance", Class.class).invoke(unsafe, c);
      } catch (ReflectiveOperationException ex2) {
        throw new Undecodable("cannot instantiate " + c.getName() + ": " + ex2);
      }
    }
  }

  /** Finds a field by its FORMAT.md name (own field, or SimpleName.field for inherited). */
  static Field findField(Class<?> c, Class<?> stop, String name) {
    for (Class<?> k = c; k != null && k != stop && k != Object.class; k = k.getSuperclass()) {
      for (Field f : k.getDeclaredFields()) {
        if (Modifier.isStatic(f.getModifiers())) {
          continue;
        }
        String n = (k == c ? "" : k.getSimpleName() + ".") + f.getName();
        if (n.equals(name) || (k.getName() + "." + f.getName()).equals(name)) {
          return f;
        }
      }
    }
    return null;
  }

  /** Restores every field in `fields` (FORMAT.md field-dump object) onto inst. */
  static void setFields(Object inst, Class<?> c, Class<?> stop, JsonObject fields) {
    for (Map.Entry<String, JsonElement> e : fields.entrySet()) {
      setField(inst, c, stop, e.getKey(), e.getValue());
    }
  }

  static void setField(Object inst, Class<?> c, Class<?> stop, String name, JsonElement value) {
    Field f = findField(c, stop, name);
    if (f == null) {
      throw new Undecodable("no field " + name + " in " + c.getName());
    }
    Object v = decode(value, f.getType());
    v = adapt(v, f.getType());
    try {
      f.setAccessible(true);
      f.set(inst, v);
    } catch (ReflectiveOperationException | RuntimeException ex) {
      throw new Undecodable("cannot set " + c.getName() + "." + name + ": " + ex);
    }
  }

  /** Adapts generic decoded collections to the declared field type where needed. */
  @SuppressWarnings({"unchecked", "rawtypes"})
  static Object adapt(Object v, Class<?> t) {
    if (v == null) {
      return null;
    }
    if (t == ImmutableList.class && !(v instanceof ImmutableList)) {
      return ImmutableList.copyOf((Collection) v);
    }
    if (t == ImmutableSet.class && !(v instanceof ImmutableSet)) {
      return ImmutableSet.copyOf((Collection) v);
    }
    if (t == ImmutableMap.class && !(v instanceof ImmutableMap)) {
      return ImmutableMap.copyOf((Map) v);
    }
    if (t == com.google.common.collect.ImmutableBiMap.class && v instanceof Map<?, ?> m) {
      return com.google.common.collect.ImmutableBiMap.copyOf(m);
    }
    if (!t.isInstance(v) && !t.isInterface() && !Modifier.isAbstract(t.getModifiers())) {
      if (v instanceof Map<?, ?> m && Map.class.isAssignableFrom(t)) {
        Map<Object, Object> n = (Map<Object, Object>) instantiate(t);
        n.putAll(m);
        return n;
      }
      if (v instanceof Collection<?> col && Collection.class.isAssignableFrom(t)) {
        Collection<Object> n = (Collection<Object>) instantiate(t);
        n.addAll(col);
        return n;
      }
    }
    if (t == com.google.common.base.Optional.class && v instanceof Optional<?> jo) {
      return com.google.common.base.Optional.fromJavaUtil(jo);
    }
    if (t == Set.class && v instanceof List<?> l) {
      return new LinkedHashSet<>(l);
    }
    return v;
  }
}
