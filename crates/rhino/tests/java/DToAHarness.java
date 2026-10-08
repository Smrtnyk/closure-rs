import com.google.javascript.rhino.dtoa.DToA;
import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.SplittableRandom;
import java.util.zip.GZIPOutputStream;

/** Deterministic golden generation using only the pinned Closure jar and project JDK 21. */
public final class DToAHarness {
  /** corpus-cache/dtoa of the main checkout: $CLOSURE_RS_ROOT, else the working directory. */
  static final Path ROOT =
      Path.of(System.getenv().getOrDefault("CLOSURE_RS_ROOT", ".")).resolve("corpus-cache/dtoa");
  static final long SIGN = Long.MIN_VALUE;
  static final long FRACTION = (1L << 52) - 1;
  static final SplittableRandom RNG = new SplittableRandom(0x44546f415f323031L);
  static Method dtostr;
  static Method dtoa;
  static Method d2b;
  static String categoryName;
  static long categoryDoubles, categoryInfeasible, infeasible;
  static BufferedWriter output;
  static BufferedWriter sample;
  static long doubles, rows;
  static String kind;
  static int categoryRows;
  static final int[] PREC = {0, 1, 2, 6, 14, 15, 16, 17, 20, 50, 100};

  static BufferedWriter writer(String file) throws IOException {
    return new BufferedWriter(new OutputStreamWriter(new GZIPOutputStream(
        Files.newOutputStream(ROOT.resolve(file)), 1 << 20), StandardCharsets.UTF_8), 1 << 20);
  }
  static String hex(long b) {
    String s = Long.toHexString(b);
    return "0".repeat(16 - s.length()) + s;
  }
  static void category(String name) {
    finishCategory();
    categoryName = name;
    categoryDoubles = categoryInfeasible = 0;
    categoryRows = 0;
    System.err.println(kind + " category " + name + " at " + doubles + " doubles");
  }
  static void finishCategory() {
    if (categoryName != null) {
      System.err.println(kind + " category " + categoryName + ": doubles=" + categoryDoubles
          + " infeasible=" + categoryInfeasible);
    }
  }
  static String encode(String s) {
    StringBuilder encoded = new StringBuilder("=");
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (c == '\\') encoded.append("\\\\");
      else if (c < 0x20 || c > 0x7e) {
        encoded.append("\\u");
        String hex = Integer.toHexString(c);
        encoded.append("0".repeat(4 - hex.length())).append(hex);
      } else encoded.append(c);
    }
    return encoded.toString();
  }
  static String exception(Throwable ex) {
    return "!" + ex.getClass().getName() + ": " + ex.getMessage();
  }
  static final int Exp_shift1 = 20, Exp_mask = 0x7ff00000, Frac_mask1 = 0xfffff;
  static final int Exp_11 = 0x3ff00000, Exp_msk1 = 0x100000, Bias = 1023, P = 53;
  static int word0(double d) { return (int)(Double.doubleToLongBits(d) >>> 32); }
  static int word1(double d) { return (int)Double.doubleToLongBits(d); }
  static double setWord0(double d, int i) {
    return Double.longBitsToDouble(((long)i << 32) | (Double.doubleToLongBits(d) & 0xffffffffL));
  }
  static int javaKEstimate(double d) throws Exception {
    d = Math.abs(d);
    int[] be = new int[1], bbits = new int[1];
    java.math.BigInteger b;
    int i, k;
    long x;
    double d2, ds;
    boolean denorm;
    b = (java.math.BigInteger)d2b.invoke(null, d, be, bbits);
    if ((i = (word0(d) >>> Exp_shift1 & (Exp_mask >> Exp_shift1))) != 0) {
      d2 = setWord0(d, (word0(d) & Frac_mask1) | Exp_11);
      /* log(x)   ~=~ log(1.5) + (x-1.5)/1.5
       * log10(x)  =  log(x) / log(10)
       *      ~=~ log(1.5)/log(10) + (x-1.5)/(1.5*log(10))
       * log10(d) = (i-Bias)*log(2)/log(10) + log10(d2)
       *
       * This suggests computing an approximation k to log10(d) by
       *
       * k = (i - Bias)*0.301029995663981
       *  + ( (d2-1.5)*0.289529654602168 + 0.176091259055681 );
       *
       * We want k to be too large rather than too small.
       * The error in the first-order Taylor series approximation
       * is in our favor, so we just round up the constant enough
       * to compensate for any error in the multiplication of
       * (i - Bias) by 0.301029995663981; since |i - Bias| <= 1077,
       * and 1077 * 0.30103 * 2^-52 ~=~ 7.2e-14,
       * adding 1e-13 to the constant term more than suffices.
       * Hence we adjust the constant term to 0.1760912590558.
       * (We could get a more accurate k by invoking log10,
       *  but this is probably not worthwhile.)
       */
      i -= Bias;
      denorm = false;
    } else {
      /* d is denormalized */
      i = bbits[0] + be[0] + (Bias + (P - 1) - 1);
      x =
          (i > 32)
              ? ((long) word0(d)) << (64 - i) | word1(d) >>> (i - 32)
              : ((long) word1(d)) << (32 - i);
      //            d2 = x;
      //            word0(d2) -= 31*Exp_msk1; /* adjust exponent */
      d2 = setWord0(x, word0(x) - 31 * Exp_msk1);
      i -= (Bias + (P - 1) - 1) + 1;
      denorm = true;
    }
    /* At this point d = f*2^i, where 1 <= f < 2.  d2 is an approximation of f. */
    ds = (d2 - 1.5) * 0.289529654602168 + 0.1760912590558 + i * 0.301029995663981;
    k = (int) ds;
    if (ds < 0.0 && ds != k) k--; /* want k = floor(ds) */
    return k;
  }
  static void line(String s) throws IOException {
    output.write(s); output.newLine();
    // Keep category heads plus a deterministic stride through every category.
    if (categoryRows < 20 || categoryRows % 601 == 0) {
      sample.write(s); sample.newLine();
    }
    categoryRows++; rows++;
  }
  static void emit(long bits) throws Exception {
    double d = Double.longBitsToDouble(bits);
    String b = hex(bits);
    String exclusion = null;
    if (!kind.equals("jdk") && Double.isFinite(d) && d != 0.0) {
      int k = javaKEstimate(d);
      long magnitude = Math.abs((long)k);
      if (20000 < magnitude && magnitude < 715827894) {
        exclusion = "?infeasible k=" + k;
        infeasible++; categoryInfeasible++;
      }
    }
    categoryDoubles++;

    if (kind.equals("jdk")) {
      line(b + "\t" + Double.toString(d));
    } else if (kind.equals("standard")) {
      String s = exclusion;
      if (s == null) {
        try { s = encode(DToA.numberToString(d)); }
        catch (RuntimeException ex) { s = exception(ex); }
      }
      line(b + "\t" + s + "\t" + Double.toString(d));
    } else if (kind.equals("modes")) {
      for (int mode = 1; mode <= 4; mode++) {
        int precision = mode == 1 ? 0 : PREC[(int)((doubles + 3 * mode) % PREC.length)];
        if (mode == 4 && precision == 0) precision = 1;
        // Exponential takes significant digits; JS toExponential(100) requests 101.
        if (mode == 3) precision++;
        StringBuilder buffer = new StringBuilder();
        String s;
        try {
          if (exclusion != null) s = exclusion;
          else { dtostr.invoke(null, buffer, mode, precision, d); s = encode(buffer.toString()); }
        } catch (InvocationTargetException ex) {
          s = exception(ex.getCause());
        }
        line(b + "\t" + mode + "\t" + precision + "\t" + s);
      }
    } else {
      for (int mode = -1; mode <= 10; mode++) {
        for (boolean bias : new boolean[]{false, true}) {
          int precision = PREC[(int)((doubles + mode + 12) % PREC.length)];
          if ((doubles & 7) == 0) precision = -5;
          StringBuilder buffer = new StringBuilder();
          boolean[] sign = new boolean[1];
          String s;
          try {
            if (exclusion != null) s = exclusion;
            else {
              int decpt = (int)dtoa.invoke(null, d, mode, bias, precision, sign, buffer);
              s = decpt + "\t" + sign[0] + "\t" + encode(buffer.toString());
            }
          } catch (InvocationTargetException ex) {
            s = exception(ex.getCause());
          }
          line(b + "\t" + mode + "\t" + bias + "\t" + precision + "\t" + s);
        }
      }
    }
    doubles++;
  }
  static void pair(long bits) throws Exception { emit(bits); emit(bits ^ SIGN); }
  static void neighbours(double d) throws Exception {
    pair(Double.doubleToRawLongBits(d));
    pair(Double.doubleToRawLongBits(Math.nextDown(d)));
    pair(Double.doubleToRawLongBits(Math.nextUp(d)));
  }
  static void generate(boolean full) throws Exception {
    category("special");
    for (long b : new long[]{0, 1, 2, 3, 0x000fffffffffffffL,
        0x0010000000000000L, 0x0010000000000001L, 0x7fefffffffffffffL,
        0x7ff0000000000000L, 0x7ff8000000000000L, 0x7ff0000000000001L,
        0x7fffffffffffffffL, 0x7ff123456789abcdL}) pair(b);
    category("powers_two");
    for (int e = -1074; e <= 1023; e++) neighbours(Math.scalb(1.0, e));
    category("powers_ten");
    for (int e = -325; e <= 309; e++) neighbours(Double.parseDouble("1e" + e));
    category("sparse_subnormal");
    for (int i = 0; i < 52; i++) {
      pair(1L << i);
      for (int j = i + 1; j < 52; j++) {
        pair((1L << i) | (1L << j));
        for (int k = j + 1; k < 52; k++) pair((1L << i) | (1L << j) | (1L << k));
      }
    }
    category("random_subnormal");
    for (int i = 0; i < (full ? 100_000 : 10_000); i++) pair(RNG.nextLong() & FRACTION);
    category("integers");
    for (int i = 0; i <= 200_000; i += full ? 1 : 10) pair(Double.doubleToRawLongBits((double)i));
    category("thresholds");
    for (double v : new double[]{0, 0.001, 1e7, 1e21, 1e-6, 1e-7, 0x1p53, 0x1p63,
        123456789012345680000.0, 2e23, 9007199254740993.0}) {
      neighbours(v);
      for (int j = -128; j <= 128; j++) {
        neighbours(v + j);
        neighbours(v + j * Math.ulp(v));
      }
    }
    category("decimal_literals");
    for (int i = 0; i < (full ? 200_000 : 25_000); i++) {
      int digits = 1 + RNG.nextInt(17);
      StringBuilder literal = new StringBuilder();
      literal.append((char)('1' + RNG.nextInt(9)));
      if (digits > 1) literal.append('.');
      for (int j = 1; j < digits; j++) literal.append((char)('0' + RNG.nextInt(10)));
      literal.append('e').append(RNG.nextInt(-330, 311));
      pair(Double.doubleToRawLongBits(Double.parseDouble(literal.toString())));
    }
    category("random_bits");
    for (int i = 0; i < (full ? 3_000_000 : 100_000); i++) pair(RNG.nextLong());
  }
  public static void main(String[] args) throws Exception {
    kind = args[0];
    dtostr = DToA.class.getDeclaredMethod("JS_dtostr", StringBuilder.class, int.class, int.class, double.class);
    dtostr.setAccessible(true);
    dtoa = DToA.class.getDeclaredMethod("JS_dtoa", double.class, int.class, boolean.class, int.class, boolean[].class, StringBuilder.class);
    dtoa.setAccessible(true);
    d2b = DToA.class.getDeclaredMethod("d2b", double.class, int[].class, int[].class);
    d2b.setAccessible(true);
    output = writer(kind + ".tsv.gz");
    sample = writer(kind + "-sample.tsv.gz");
    if (kind.equals("raw")) {
      category("special_and_all_internal_modes");
      for (long b : new long[]{0, 1, 2, 3, 0x000fffffffffffffL, 0x0010000000000000L,
          0x0010000000000001L, 0x7fefffffffffffffL, 0x7ff0000000000000L,
          0x7ff8000000000000L}) pair(b);
      for (double v : new double[]{0.1, 1.0, 2.5, 1e23, 1e-7, 1e21, 1e7, 0x1p53}) neighbours(v);
      for (int i = 0; i < 10_000; i++) pair(RNG.nextLong());
    } else if (kind.equals("picked")) {
      kind = "standard";
      for (double v : new double[]{0.0, -0.0, Double.NaN, Double.POSITIVE_INFINITY,
          Double.NEGATIVE_INFINITY, 1e21, 1e-7, 123456789012345680000.0, 5e-324,
          Double.MIN_VALUE, 1e7, 0.001, 1e-4, 2e23, 9007199254740993.0,
          Double.MIN_NORMAL, Double.MAX_VALUE, 1e23, 0.1, 2.5}) emit(Double.doubleToRawLongBits(v));
    } else { generate(kind.equals("standard") || kind.equals("jdk")); }
    finishCategory();
    output.close(); sample.close();
    System.err.println(kind + ": doubles=" + doubles + " rows=" + rows + " infeasible=" + infeasible);
  }
}
