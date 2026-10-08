import java.lang.reflect.*;
import java.math.BigInteger;

/** Exercise the pinned JDK's real range checks without computing enormous powers. */
public final class BigIntegerRangeHarness {
  static final int MAX_MAG_LENGTH = Integer.MAX_VALUE / Integer.SIZE + 1;
  static Constructor<BigInteger> constructor;
  static BigInteger magnitude(int length, int first) throws Exception {
    int[] words = new int[length];
    words[0] = first;
    return constructor.newInstance(words, 1);
  }
  interface Operation { Object run() throws Exception; }
  static void check(String name, Operation operation) {
    try { System.out.println(name + "\t" + DToAHarness.encode(operation.run().toString())); }
    catch (Exception ex) {
      Throwable cause = ex instanceof InvocationTargetException ? ex.getCause() : ex;
      System.out.println(name + "\t" + DToAHarness.exception(cause));
    }
  }
  static void checkMaximum() throws Exception {
    BigInteger maximum = magnitude(MAX_MAG_LENGTH, 0x40000000);
    check("checkRange 2147483647 bits", maximum::bitLength);
    check("shiftLeft(1) at limit", () -> maximum.shiftLeft(1));
    System.gc();
    check("multiply(5) at limit", () -> maximum.multiply(BigInteger.valueOf(5)));
  }
  public static void main(String[] args) throws Exception {
    constructor = BigInteger.class.getDeclaredConstructor(int[].class, int.class);
    constructor.setAccessible(true);
    check("checkRange 2147483648 bits", () -> magnitude(MAX_MAG_LENGTH, 0x80000000));
    System.gc();
    checkMaximum();
    System.gc();
    BigInteger x = magnitude(MAX_MAG_LENGTH / 2, 0x80000000);
    BigInteger y = magnitude(MAX_MAG_LENGTH / 2 + 1, 1);
    check("Toom-Cook 1073741824 + 1073741825 bits", () -> x.multiply(y));
    Method square = BigInteger.class.getDeclaredMethod("square");
    square.setAccessible(true);
    check("square 1073741825 bits", () -> square.invoke(y));
    check("pow5 715827894", () -> BigInteger.valueOf(5).pow(715827894));
    check("pow5 -1", () -> BigInteger.valueOf(5).pow(-1));
  }
}
