package com.google.javascript.jscomp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Processor identity sub-check. Compares the recorded `processor` dump with the
 * dump of the processor the descriptor built (same encoder, depth 2). Leaves compared: JSON
 * primitives, null, `enum`, `long`, `double` and `char` values (scalar, enum and String leaves),
 * and list/arrayOf/set/map containers whose items are all such scalars (lists in
 * order, sets as sorted multisets, maps as sorted entry sets). Other containers, `ref`, lambdas and
 * unrepresentable values are not leaves. Every `object` dump in
 * either tree (the processor itself included) is indexed by its class name (the recorded names
 * mapped through the descriptor's classMap); objects of the same class found in both trees are
 * compared leaf by leaf on their common field paths, matched as a multiset (see compare). Protobuf
 * memo caches (memoizedHashCode, memoizedSize, memoizedIsInitialized) are not leaves. So a pass held
 * by a wrapper (PhaseOptimizer, a helper Processor, a Proxy) is compared wherever it sits.
 * status: "equal" (same top class, every compared leaf equal), "mismatch" (some compared leaf
 * differs), "classDiffers" (top classes differ and no compared leaf differs; `compared` says how
 * many leaves could be compared), "noReplayedProcessor" (the processor never ran in replay while
 * the record has one), "bothNull", "dumpError".
 */
final class ProcessorIdentity {
  private ProcessorIdentity() {}

  static JsonObject compare(JsonObject rec, JsonObject replayed, Map<String, String> classMap) {
    JsonObject out = new JsonObject();
    JsonElement re = rec.get("processor");
    JsonObject recorded = re == null || re.isJsonNull() ? null : re.getAsJsonObject();
    if (recorded == null && replayed == null) {
      out.addProperty("status", "bothNull");
      return out;
    }
    if (recorded == null) {
      // The original processor never ran (e.g. parse error); replay ran one: nothing to compare.
      out.addProperty("status", "bothNull");
      out.addProperty("note", "recorded processor null");
      return out;
    }
    String rc = map(str(recorded.get("class")), classMap);
    out.addProperty("recordedClass", str(recorded.get("class")));
    if (replayed == null) {
      out.addProperty("status", "noReplayedProcessor");
      return out;
    }
    if (replayed.has("error")) {
      out.addProperty("status", "dumpError");
      out.addProperty("error", replayed.get("error").getAsString());
      return out;
    }
    String pc = str(replayed.get("class"));
    out.addProperty("replayedClass", pc);
    Map<String, List<Map<String, String>>> a = new LinkedHashMap<>();
    Map<String, List<Map<String, String>>> b = new LinkedHashMap<>();
    index(rc, recorded.get("fields"), a, classMap);
    index(pc, replayed.get("fields"), b, classMap);
    int compared = 0;
    JsonArray mism = new JsonArray();
    int nm = 0;
    for (Map.Entry<String, List<Map<String, String>>> e : a.entrySet()) {
      List<Map<String, String>> other = b.get(e.getKey());
      if (other == null) {
        continue;
      }
      // Objects of one class are matched as a multiset: each recorded object takes the first unused
      // replayed object whose common leaves are all equal, else the first unused one (whose
      // differing leaves then count as mismatches). Rule lists built in another order still match.
      boolean[] used = new boolean[other.size()];
      for (Map<String, String> x : e.getValue()) {
        int pick = -1;
        for (int j = 0; j < other.size() && pick < 0; j++) {
          if (!used[j] && differing(x, other.get(j)) == 0) {
            pick = j;
          }
        }
        for (int j = 0; j < other.size() && pick < 0; j++) {
          if (!used[j]) {
            pick = j;
          }
        }
        if (pick < 0) {
          break;
        }
        used[pick] = true;
        Map<String, String> y = other.get(pick);
        for (Map.Entry<String, String> l : x.entrySet()) {
          if (!y.containsKey(l.getKey())) {
            continue;
          }
          compared++;
          if (!l.getValue().equals(y.get(l.getKey()))) {
            nm++;
            if (mism.size() < 5) {
              JsonObject m = new JsonObject();
              m.addProperty("object", e.getKey());
              m.addProperty("path", l.getKey());
              m.addProperty("recorded", l.getValue());
              m.addProperty("replayed", y.get(l.getKey()));
              mism.add(m);
            }
          }
        }
      }
    }
    out.addProperty("compared", compared);
    out.addProperty("mismatches", nm);
    if (nm > 0) {
      out.add("examples", mism);
      out.addProperty("status", "mismatch");
    } else if (!rc.equals(pc)) {
      out.addProperty("status", "classDiffers");
    } else {
      out.addProperty("status", "equal");
    }
    return out;
  }

  private static int differing(Map<String, String> x, Map<String, String> y) {
    int n = 0;
    for (Map.Entry<String, String> l : x.entrySet()) {
      if (y.containsKey(l.getKey()) && !l.getValue().equals(y.get(l.getKey()))) {
        n++;
      }
    }
    return n;
  }

  /** protobuf memo caches; memoizedHashCode depends on descriptor identity hashes (run-dependent). */
  private static boolean isCache(String path) {
    String last = path.substring(path.lastIndexOf('.') + 1);
    return last.equals("memoizedHashCode") || last.equals("memoizedSize") || last.equals("memoizedIsInitialized");
  }

  private static String str(JsonElement e) {
    return e == null || e.isJsonNull() ? null : e.getAsString();
  }

  private static String map(String c, Map<String, String> classMap) {
    return c == null ? "null" : classMap.getOrDefault(c, c);
  }

  /** Indexes the object with class c and fields f, and every nested object dump. */
  private static void index(String c, JsonElement f, Map<String, List<Map<String, String>>> out,
      Map<String, String> classMap) {
    Map<String, String> leaves = new LinkedHashMap<>();
    List<JsonObject> nested = new ArrayList<>();
    if (f != null && f.isJsonObject()) {
      for (Map.Entry<String, JsonElement> e : f.getAsJsonObject().entrySet()) {
        leaves(e.getValue(), e.getKey(), leaves, nested);
      }
    }
    out.computeIfAbsent(c, k -> new ArrayList<>()).add(leaves);
    for (JsonObject o : nested) {
      index(map(str(o.get("object")), classMap), o.get("fields"), out, classMap);
    }
  }

  private static void leaves(JsonElement v, String path, Map<String, String> out, List<JsonObject> nested) {
    if (isCache(path)) {
      return;
    }
    if (v == null || v.isJsonNull()) {
      out.put(path, "null");
      return;
    }
    if (v.isJsonPrimitive()) {
      out.put(path, v.toString());
      return;
    }
    if (v.isJsonArray()) {
      int i = 0;
      for (JsonElement x : v.getAsJsonArray()) {
        collectNested(x, nested);
        i++;
      }
      return;
    }
    JsonObject o = v.getAsJsonObject();
    if (o.has("enum") && o.has("name")) {
      out.put(path, "enum:" + o.get("enum").getAsString() + "." + o.get("name").getAsString());
      return;
    }
    if (o.has("long") || o.has("double") || o.has("char")) {
      out.put(path, o.toString());
      return;
    }
    String sc = scalarContainer(o);
    if (sc != null) {
      // A container whose items are all scalar leaves is itself a leaf.
      out.put(path, sc);
      return;
    }
    if (o.has("object") && o.has("fields")) {
      // A nested object: its own leaves under this path, and indexed by class as well.
      nested.add(o);
      JsonObject fs = o.getAsJsonObject("fields");
      for (Map.Entry<String, JsonElement> e : fs.entrySet()) {
        leaves(e.getValue(), path + "." + e.getKey(), out, new ArrayList<>());
      }
      return;
    }
    collectNested(o, nested);
  }

  /** The scalar leaf string of v, or null when v is not a scalar (primitive, null, enum, long/double/char). */
  private static String scalar(JsonElement v) {
    if (v == null || v.isJsonNull()) {
      return "null";
    }
    if (v.isJsonPrimitive()) {
      return v.toString();
    }
    if (!v.isJsonObject()) {
      return null;
    }
    JsonObject o = v.getAsJsonObject();
    if (o.has("enum") && o.has("name") && o.size() == 2) {
      return "enum:" + o.get("enum").getAsString() + "." + o.get("name").getAsString();
    }
    if (o.size() == 1 && (o.has("long") || o.has("double") || o.has("char"))) {
      return o.toString();
    }
    return null;
  }

  /**
   * A list/arrayOf (items in order), set (sorted multiset) or map (sorted entry
   * set) whose items, keys and values are all scalars is compared as one leaf; `impl` is ignored
   * (FORMAT.md "Processor identity"). Returns null for any other value.
   */
  private static String scalarContainer(JsonObject o) {
    JsonArray items = null;
    String kind = null;
    if (o.has("list") && o.get("list").isJsonArray()) {
      kind = "list";
      items = o.getAsJsonArray("list");
    } else if (o.has("arrayOf") && o.has("items") && o.get("items").isJsonArray()) {
      kind = "list";
      items = o.getAsJsonArray("items");
    } else if (o.has("set") && o.get("set").isJsonArray()) {
      kind = "set";
      items = o.getAsJsonArray("set");
    } else if (o.has("map") && o.get("map").isJsonArray()) {
      kind = "map";
      items = o.getAsJsonArray("map");
    }
    if (items == null) {
      return null;
    }
    List<String> parts = new ArrayList<>();
    for (JsonElement x : items) {
      if (kind.equals("map")) {
        if (!x.isJsonArray() || x.getAsJsonArray().size() != 2) {
          return null;
        }
        String k = scalar(x.getAsJsonArray().get(0));
        String val = scalar(x.getAsJsonArray().get(1));
        if (k == null || val == null) {
          return null;
        }
        parts.add(k + "=" + val);
      } else {
        String s = scalar(x);
        if (s == null) {
          return null;
        }
        parts.add(s);
      }
    }
    if (!kind.equals("list")) {
      java.util.Collections.sort(parts);
    }
    return kind + ":" + new com.google.gson.Gson().toJson(parts);
  }

  /** Finds object dumps inside containers (list/set/map/arrayOf/optional), not their leaves. */
  private static void collectNested(JsonElement v, List<JsonObject> nested) {
    if (v == null || v.isJsonNull() || v.isJsonPrimitive()) {
      return;
    }
    if (v.isJsonArray()) {
      for (JsonElement x : v.getAsJsonArray()) {
        collectNested(x, nested);
      }
      return;
    }
    JsonObject o = v.getAsJsonObject();
    if (o.has("object") && o.has("fields")) {
      nested.add(o);
      return;
    }
    if (o.has("unrepresentable") || o.has("ref")) {
      return;
    }
    for (Map.Entry<String, JsonElement> e : o.entrySet()) {
      collectNested(e.getValue(), nested);
    }
  }
}
