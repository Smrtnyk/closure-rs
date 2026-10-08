import com.google.javascript.jscomp.ConformanceConfig;
import com.google.protobuf.TextFormat;
import java.nio.file.*;
import java.util.*;

public class TfGolden {
  public static void main(String[] args) throws Exception {
    // input: file with cases separated by lines "=====\n"
    String all = new String(Files.readAllBytes(Paths.get(args[0])), "UTF-8");
    String[] cases = all.split("\n=====\n", -1);
    StringBuilder out = new StringBuilder();
    for (String c : cases) {
      ConformanceConfig.Builder b = ConformanceConfig.newBuilder();
      String res;
      try {
        TextFormat.merge(c, b);
        res = "OK\n" + TextFormat.printer().printToString(b.build());
      } catch (Exception e) {
        res = "ERR " + e.getClass().getName() + "\n" + e.getMessage();
      }
      out.append(res).append("\n=====\n");
    }
    System.out.print(out);
  }
}
