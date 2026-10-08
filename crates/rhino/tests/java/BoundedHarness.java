import com.google.javascript.rhino.dtoa.DToA;
import java.lang.reflect.*;
import java.util.SplittableRandom;

/** Legacy supplemental normal-value generator; full-mix acceptance uses DToAHarness. */
public class BoundedHarness {
  static Method dtostr;
  static SplittableRandom rng = new SplittableRandom(0x44546f415f323032L);
  static long count;
  static int[] precision = {0,1,2,6,14,15,16,17,20,50,100};
  static void emit(long bits) throws Exception {
    double d=Double.longBitsToDouble(bits);
    String hex = DToAHarness.hex(bits);
    String exclusion = null;
    if (Double.isFinite(d) && d != 0.0) {
      int k = DToAHarness.javaKEstimate(d);
      long magnitude = Math.abs((long)k);
      if (20000 < magnitude && magnitude < 715827894) {
        exclusion = "?infeasible k=" + k;
        DToAHarness.infeasible++; DToAHarness.categoryInfeasible++;
      }
    }
    DToAHarness.categoryDoubles++;
    if (DToAHarness.kind.equals("bounded-standard")) {
      String s = exclusion;
      if (s == null) {
        try { s = DToAHarness.encode(DToA.numberToString(d)); }
        catch (RuntimeException ex) { s = DToAHarness.exception(ex); }
      }
      DToAHarness.line(hex + "\t" + s + "\t" + Double.toString(d));
    } else {
      for (int mode=1;mode<=4;mode++) {
        int p=mode==1 ? 0 : precision[(int)((count+3*mode)%precision.length)];
        if(mode==4 && p==0) p=1;
        if(mode==3) p++;
        StringBuilder buffer=new StringBuilder();
        String s;
        try {
          if (exclusion != null) s = exclusion;
          else {dtostr.invoke(null,buffer,mode,p,d);s=DToAHarness.encode(buffer.toString());}
        }
        catch(InvocationTargetException e){s=DToAHarness.exception(e.getCause());}
        DToAHarness.line(hex+"\t"+mode+"\t"+p+"\t"+s);
      }
    }
    count++;
    DToAHarness.doubles=count;
  }
  static void pair(long b) throws Exception {emit(b);emit(b^Long.MIN_VALUE);}
  static void neighbours(double d) throws Exception {
    pair(Double.doubleToRawLongBits(d));
    pair(Double.doubleToRawLongBits(Math.nextDown(d)));
    pair(Double.doubleToRawLongBits(Math.nextUp(d)));
  }
  public static void main(String[] args) throws Exception {
    DToAHarness.kind=args[0];
    boolean standard=args[0].equals("bounded-standard");
    DToAHarness.output=DToAHarness.writer(args[0]+".tsv.gz");
    DToAHarness.sample=DToAHarness.writer(args[0]+"-sample.tsv.gz");
    dtostr=DToA.class.getDeclaredMethod("JS_dtostr",StringBuilder.class,int.class,int.class,double.class);
    dtostr.setAccessible(true);
    DToAHarness.d2b=DToA.class.getDeclaredMethod("d2b",double.class,int[].class,int[].class);
    DToAHarness.d2b.setAccessible(true);
    DToAHarness.category("special");
    for(long b:new long[]{0,1,2,3,0x000fffffffffffffL,0x0010000000000000L,
        0x0010000000000001L,0x7fefffffffffffffL,0x7ff0000000000000L,
        0x7ff8000000000000L,0x7ff0000000000001L,0x7fffffffffffffffL,
        0x7ff123456789abcdL})pair(b);
    DToAHarness.category("normal_powers_two");
    for(int e=-1022;e<=1023;e++)neighbours(Math.scalb(1.0,e));
    DToAHarness.category("normal_powers_ten");
    for(int e=-308;e<=309;e++)neighbours(Double.parseDouble("1e"+e));
    DToAHarness.category("integers");
    for(int i=0;i<=200_000;i+=standard?1:10)pair(Double.doubleToRawLongBits((double)i));
    DToAHarness.category("thresholds");
    for(double v:new double[]{0,0.001,1e7,1e21,1e-6,1e-7,0x1p53,0x1p63,
        123456789012345680000.0,2e23,9007199254740993.0}) {
      neighbours(v);
      for(int j=-128;j<=128;j++){neighbours(v+j);neighbours(v+j*Math.ulp(v));}
    }
    DToAHarness.category("normal_decimal_literals");
    for(int i=0;i<(standard?200_000:25_000);i++) {
      int digits=1+rng.nextInt(17);
      StringBuilder literal=new StringBuilder();literal.append((char)('1'+rng.nextInt(9)));
      if(digits>1)literal.append('.');
      for(int j=1;j<digits;j++)literal.append((char)('0'+rng.nextInt(10)));
      literal.append('e').append(rng.nextInt(-300,311));
      pair(Double.doubleToRawLongBits(Double.parseDouble(literal.toString())));
    }
    DToAHarness.category("normal_random_bits");
    for(int i=0;i<(standard?3_000_000:125_000);i++) {
      long fraction=rng.nextLong()&0xfffffffffffffL;
      long exponent=(long)rng.nextInt(1,2047)<<52;
      pair(exponent|fraction);
    }
    DToAHarness.finishCategory();
    DToAHarness.output.close();DToAHarness.sample.close();
    System.err.println(args[0]+": doubles="+count+" rows="+DToAHarness.rows);
  }
}
