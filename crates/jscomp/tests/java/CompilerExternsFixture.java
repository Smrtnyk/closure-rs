import com.google.javascript.jscomp.testing.TestExternsBuilder;
public final class CompilerExternsFixture {
  public static void main(String[] args) {
    TestExternsBuilder builder = new TestExternsBuilder();
    if (args.length > 0 && args[0].equals("alert")) {
      builder.addAlert();
    } else {
      builder.addConsole();
    }
    System.out.print(builder.build());
  }
}
