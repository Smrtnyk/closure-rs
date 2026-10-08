import com.google.javascript.rhino.dtoa.DToA;
public class CharProbe {
  public static void main(String[] args) {
    for(long high=0;high<1000;high++) {
      long bits=(high<<32)|0x80000000L;
      String s=DToA.numberToString(Double.longBitsToDouble(bits));
      boolean surrogate=false;
      for(int i=0;i<s.length();i++)if(Character.isSurrogate(s.charAt(i)))surrogate=true;
      if(high<2 || surrogate) {
        System.out.println(DToAHarness.hex(bits)+"\t"+DToAHarness.encode(s));
      }
      if(surrogate)break;
    }
  }
}
