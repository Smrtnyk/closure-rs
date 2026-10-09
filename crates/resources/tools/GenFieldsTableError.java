// Prints how RuntimeJsLibManager.FieldsTable fails on the transpilation_libs.txt found first on the
// classpath. Run: java -cp <dir with com/google/javascript/jscomp/js/transpilation_libs.txt>:closure-compiler.jar:. GenFieldsTableError
import java.lang.reflect.*;

public class GenFieldsTableError {
  public static void main(String[] args) throws Exception {
    // Loaded without initializing, so the static initializer runs (and fails) in the try below.
    Class<?> ft = Class.forName("com.google.javascript.jscomp.js.RuntimeJsLibManager$FieldsTable",
        false, GenFieldsTableError.class.getClassLoader());
    try {
      Field inst = ft.getDeclaredField("INSTANCE");
      inst.setAccessible(true);
      System.out.println("ok\t" + inst.get(null));
    } catch (ExceptionInInitializerError e) {
      Throwable c = e.getCause();
      System.out.println(c.getClass().getName() + "\t" + c.getMessage());
    }
  }
}
