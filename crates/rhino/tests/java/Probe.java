import com.google.javascript.rhino.dtoa.DToA;
import java.lang.reflect.*;
public class Probe {
  public static void main(String[] args) throws Exception {
    double d = Double.longBitsToDouble(Long.parseUnsignedLong(args[0],16));
    System.out.println("jdk="+Double.toString(d));
    Method d2b = DToA.class.getDeclaredMethod("d2b", double.class, int[].class, int[].class);
    d2b.setAccessible(true);
    int[] e={0},bits={0};
    System.out.println("b="+d2b.invoke(null,d,e,bits)+" e="+e[0]+" bits="+bits[0]);
    long before=System.nanoTime();
    try {
      String s=DToA.numberToString(d);
      System.out.println("length="+s.length()+" nanos="+(System.nanoTime()-before));
      System.out.println("start="+DToAHarness.encode(s.substring(0,Math.min(s.length(),100))));
      System.out.println("end="+DToAHarness.encode(s.substring(Math.max(0,s.length()-100))));
      if (args.length > 1 && args[1].equals("record")) {
        System.out.println(DToAHarness.hex(Double.doubleToRawLongBits(d))+"\t"
            +DToAHarness.encode(s)+"\t"+Double.toString(d));
      }
    } catch (RuntimeException ex) {
      System.out.println("output="+DToAHarness.exception(ex));
    }
  }
}
