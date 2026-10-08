package closurers.agent;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Pass trace of NoopAgent TRACE mode (D-017 item 8, HARNESS.md "Pass trace"). Instrumented entry
 * points call {@link #hit(int)}; the recorder (recording run) and ReplayMain (replay) call {@link
 * #begin()} before and {@link #end()} after one hooked call, by reflection, so neither depends on
 * the agent being present. The trace is global (not per thread): the compiler may run passes on its
 * own thread, and one test class runs its hooked calls sequentially.
 */
public final class Trace {
  private static final int MAX = 1 << 16;
  private static final Map<String, Integer> IDS = new ConcurrentHashMap<>();
  private static final String[] NAMES = new String[MAX];
  private static final boolean[] HIT = new boolean[MAX];
  private static volatile boolean enabled = false;
  private static volatile boolean overflow = false;

  private Trace() {}

  static void enable() {
    enabled = true;
  }

  /** True when the JVM runs with the agent in TRACE mode. */
  public static boolean active() {
    return enabled;
  }

  static synchronized int id(String topLevelFqcn) {
    Integer i = IDS.get(topLevelFqcn);
    if (i != null) {
      return i;
    }
    int n = IDS.size();
    if (n >= MAX) {
      overflow = true;
      return MAX - 1;
    }
    NAMES[n] = topLevelFqcn;
    IDS.put(topLevelFqcn, n);
    return n;
  }

  public static void hit(int id) {
    HIT[id] = true;
  }

  public static synchronized void begin() {
    Arrays.fill(HIT, false);
  }

  /** Sorted FQCNs of the top-level in-scope src classes whose entry points ran since begin(). */
  public static synchronized List<String> end() {
    if (overflow) {
      throw new IllegalStateException("Trace: more than " + MAX + " traced classes");
    }
    List<String> out = new ArrayList<>();
    int n = IDS.size();
    for (int i = 0; i < n; i++) {
      if (HIT[i]) {
        out.add(NAMES[i]);
      }
    }
    out.sort(null);
    return out;
  }
}
