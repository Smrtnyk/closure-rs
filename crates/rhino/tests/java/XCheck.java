import com.google.javascript.rhino.dtoa.DToA;
import java.util.SplittableRandom;
public class XCheck {
  static String enc(String s) {
    StringBuilder b = new StringBuilder("=");
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (c == '\\') b.append("\\\\");
      else if (c >= 0x20 && c <= 0x7e) b.append(c);
      else b.append(String.format("\\u%04x", (int) c));
    }
    return b.toString();
  }
  public static void main(String[] a) {
    SplittableRandom r = new SplittableRandom(0x5eed2026L);
    StringBuilder out = new StringBuilder();
    int n = Integer.parseInt(a[0]);
    for (int i = 0; i < n; i++) {
      double d;
      switch (i % 3) {
        case 0: d = Double.longBitsToDouble(r.nextLong()); break;
        case 1: d = Double.parseDouble((r.nextLong() % 100000000000L) + "e" + (r.nextInt(640) - 330)); break;
        default: d = Double.parseDouble(r.nextInt(1000000) + "." + r.nextInt(1000000)); break;
      }
      long bits = Double.doubleToRawLongBits(d);
      // Subnormals (some trigger the pinned d2b bug and run for hours in Java) are covered by the
      // main corpus with its k-estimate infeasibility rule; this cross-check samples normal doubles.
      if (d != 0 && Math.abs(d) < Double.MIN_NORMAL) continue;
      out.setLength(0);
      String dtoa;
      try {
        dtoa = enc(DToA.numberToString(d));
      } catch (RuntimeException e) {
        dtoa = "!" + e.getClass().getName() + ": " + e.getMessage();
      }
      out.append(String.format("%016x", bits)).append('\t').append(dtoa).append('\t').append(Double.toString(d));
      System.out.println(out);
    }
  }
}
