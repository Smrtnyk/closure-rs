import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Prints the iteration order of {@code new ConcurrentHashMap<String, String>()} after a sequence
 * of {@code putAll} (from an insertion-ordered map) and {@code put} calls, one case per line:
 * name, operations ({@code A:k1,k2} = putAll, {@code P:k} = put, separated by {@code ;}), and
 * {@code keySet()} order. Writes tests/data/java_concurrent_hash_map_order.tsv (see
 * ../java_concurrent_hash_map_test.rs).
 */
public class ConcurrentHashMapOrder {
  private static final StringBuilder out = new StringBuilder();

  /** {@code ConcurrentHashMap#spread}. */
  static int spread(int h) {
    return (h ^ (h >>> 16)) & 0x7fffffff;
  }

  /** Distinct path-like keys, in a scrambled but deterministic order. */
  static List<String> paths(int count, int seed) {
    List<String> keys = new ArrayList<>();
    long state = seed;
    for (int i = 0; i < count; i++) {
      state = (state * 6364136223846793005L + 1442695040888963407L);
      int r = (int) (state >>> 33);
      keys.add("src/dir" + (r % 7) + "/sub" + (r % 3) + "/file" + i + ".js");
    }
    return keys;
  }

  /** {@code count} distinct keys with the same String#hashCode ("Aa" and "BB" collide). */
  static List<String> collisions(String prefix, int count) {
    List<String> keys = new ArrayList<>();
    int bits = 0;
    while ((1 << bits) < count) {
      bits++;
    }
    for (int i = 0; i < count; i++) {
      StringBuilder key = new StringBuilder(prefix);
      for (int b = bits - 1; b >= 0; b--) {
        key.append(((i >> b) & 1) == 0 ? "Aa" : "BB");
      }
      keys.add(key.append(".js").toString());
    }
    return keys;
  }

  /** Distinct path keys whose spread hash falls into bucket {@code bucket} of a 16-slot table. */
  static List<String> sameBucket(int count, int bucket) {
    List<String> keys = new ArrayList<>();
    for (int i = 0; keys.size() < count; i++) {
      String key = "lib/m" + i + ".js";
      if ((spread(key.hashCode()) & 15) == bucket) {
        keys.add(key);
      }
    }
    return keys;
  }

  /** {@code count} distinct keys {@code prefix + i + ".js"} whose spread hash masked by {@code mask} is {@code bucket}. */
  static List<String> masked(String prefix, int count, int mask, int bucket) {
    List<String> keys = new ArrayList<>();
    for (int i = 0; keys.size() < count; i++) {
      String key = prefix + i + ".js";
      if ((spread(key.hashCode()) & mask) == bucket) {
        keys.add(key);
      }
    }
    return keys;
  }

  /** {@code count} distinct keys outside bucket {@code bucket} of a 64-slot table. */
  static List<String> fillers(String prefix, int count, int bucket) {
    List<String> keys = new ArrayList<>();
    for (int i = 0; keys.size() < count; i++) {
      String key = prefix + i + ".js";
      if ((spread(key.hashCode()) & 63) != bucket) {
        keys.add(key);
      }
    }
    return keys;
  }

  static final class Case {
    final String name;
    final List<String> ops = new ArrayList<>();
    final ConcurrentHashMap<String, String> map = new ConcurrentHashMap<>();

    Case(String name) {
      this.name = name;
    }

    Case putAll(List<String> keys) {
      Map<String, String> source = new LinkedHashMap<>();
      for (String key : keys) {
        source.put(key, key);
      }
      map.putAll(source);
      ops.add("A:" + String.join(",", keys));
      return this;
    }

    Case put(List<String> keys) {
      for (String key : keys) {
        map.put(key, key);
        ops.add("P:" + key);
      }
      return this;
    }

    void print() {
      out.append(name)
          .append('\t')
          .append(String.join(";", ops))
          .append('\t')
          .append(String.join(",", map.keySet()))
          .append('\n');
    }
  }

  public static void main(String[] args) {
    int[] sizes = {1, 2, 3, 6, 11, 12, 13, 24, 25, 48, 49, 96, 97, 200, 384, 385};
    for (int size : sizes) {
      new Case("put-" + size).put(paths(size, size)).print();
    }
    for (int size : sizes) {
      new Case("putAll-" + size).putAll(paths(size, size + 1)).print();
    }
    List<String> mixed = paths(120, 7);
    new Case("putAll-13-put-107")
        .putAll(mixed.subList(0, 13))
        .put(mixed.subList(13, 120))
        .print();
    new Case("put-40-putAll-80")
        .put(mixed.subList(0, 40))
        .putAll(mixed.subList(40, 120))
        .print();
    new Case("putAll-30-reput")
        .putAll(mixed.subList(0, 30))
        .put(mixed.subList(10, 20))
        .put(mixed.subList(30, 35))
        .print();
    for (int size : new int[] {2, 8, 9, 10, 16, 40, 64}) {
      new Case("collide-put-" + size).put(collisions("src/", size)).print();
      new Case("collide-putAll-" + size).putAll(collisions("src/", size)).print();
    }
    List<String> collide = collisions("x/", 32);
    new Case("collide-tree-reput")
        .put(collide.subList(0, 20))
        .put(collide.subList(3, 12))
        .put(collide.subList(20, 32))
        .print();
    new Case("collide-list-reput-ninth")
        .put(collide.subList(0, 8))
        .put(List.of(collide.get(8)))
        .put(List.of(collide.get(8)))
        .print();
    new Case("collide-mixed")
        .putAll(paths(50, 3))
        .put(collisions("y/", 24))
        .put(paths(60, 4))
        .print();
    for (int size : new int[] {8, 9, 12, 20, 40}) {
      new Case("bucket-put-" + size).put(sameBucket(size, 3)).print();
      new Case("bucket-putAll-" + size).putAll(sameBucket(size, 5)).print();
    }
    List<String> bucket = sameBucket(30, 0);
    new Case("bucket-reput")
        .put(bucket.subList(0, 8))
        .put(bucket.subList(7, 8))
        .put(bucket.subList(8, 30))
        .print();
    // A tree bin of a 64-slot table that a resize splits: halves of at most UNTREEIFY_THRESHOLD
    // nodes become lists (new keys append), larger ones stay trees (new keys go first).
    for (int lo : new int[] {5, 8, 2}) {
      int hi = 10 - lo;
      List<String> filler = fillers("f/", 60, 7);
      new Case("tree-split-" + lo + "-" + hi)
          .put(filler.subList(0, 30))
          .put(masked("lo/", lo, 127, 7))
          .put(masked("hi/", hi, 127, 71))
          .put(filler.subList(30, 60))
          .put(masked("lo2/", 3, 127, 7))
          .put(masked("hi2/", 3, 127, 71))
          .print();
    }
    System.out.print(out);
  }
}
