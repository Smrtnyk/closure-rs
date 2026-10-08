package com.google.javascript.rhino;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

/**
 * Package-private access to Node properties for the oracle's parse_dump. Enumerates every
 * Node.Prop constant (in ordinal order) and returns the ones present on a node, with their raw
 * int or object value.
 */
public final class OracleNodeAccess {
  private OracleNodeAccess() {}

  /** One present property. kind is "int" or "object". */
  public static final class PropValue {
    public final String name;
    public final int ordinal;
    public final String kind;
    public final int intValue;
    public final Object objectValue;

    PropValue(String name, int ordinal, String kind, int intValue, Object objectValue) {
      this.name = name;
      this.ordinal = ordinal;
      this.kind = kind;
      this.intValue = intValue;
      this.objectValue = objectValue;
    }
  }

  private static Method intGetter;
  private static Method objGetter;

  /** All Node.Prop names in ordinal order (the exhaustive enumeration). */
  public static List<String> allPropNames() {
    List<String> out = new ArrayList<>();
    for (Node.Prop p : Node.Prop.values()) {
      out.add(p.name());
    }
    return out;
  }

  public static List<PropValue> presentProps(Node n) throws ReflectiveOperationException {
    List<PropValue> out = new ArrayList<>();
    for (Node.Prop p : Node.Prop.values()) {
      Object item = n.lookupProperty(p);
      if (item == null) {
        continue;
      }
      String cls = item.getClass().getSimpleName();
      if (cls.equals("IntPropListItem")) {
        if (intGetter == null) {
          intGetter = findMethod(item.getClass(), "getIntValue");
        }
        out.add(new PropValue(p.name(), p.ordinal(), "int", (Integer) intGetter.invoke(item), null));
      } else {
        if (objGetter == null) {
          objGetter = findMethod(item.getClass(), "getObjectValue");
        }
        out.add(new PropValue(p.name(), p.ordinal(), "object", 0, objGetter.invoke(item)));
      }
    }
    return out;
  }

  private static Method findMethod(Class<?> c, String name) throws NoSuchMethodException {
    for (Class<?> k = c; k != null; k = k.getSuperclass()) {
      for (Method m : k.getDeclaredMethods()) {
        if (m.getName().equals(name) && m.getParameterCount() == 0) {
          m.setAccessible(true);
          return m;
        }
      }
    }
    throw new NoSuchMethodException(name);
  }
}
