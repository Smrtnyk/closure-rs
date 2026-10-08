/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The test field
 * `propertyNameFilter` (Predicate<Node>) of
 *   test/com/google/javascript/jscomp/RenamePropertiesTest.java (closure-compiler commit bb8c8e7)
 * holds one of five lambdas when getProcessor runs. The holder class and the nested classes'
 * declarations and constructors are generated; each test() body is the lambda body copied
 * VERBATIM (lambda parameter names kept):
 *   DefaultFilter:                     line 46     (field initialiser `(Node name) -> true`)
 *   GetPrefixFilter:                   lines 950-954 (testPrototypePropertiesUsingFilterFunction)
 *   FilterOnFilename:                  lines 972-976 (testPrototypePropertiesUsingFilterOnFilename)
 *   RenameFunctionsFilterOnFilename:   lines 1014-1018 (testPrototypeAndRenameFunctionsFilterOnFilename)
 *   RenameFunctionsFilterOnFilename2:  lines 1054-1058 (testPrototypeAndRenameFunctionsFilterOnFilename2)
 * DSL names: RenamePropertiesTest_Helpers.<Name>
 */
package com.google.javascript.jscomp;

import static com.google.common.base.Strings.nullToEmpty;

import com.google.javascript.rhino.Node;
import java.util.function.Predicate;

final class RenamePropertiesTest_Helpers {
  private static class DefaultFilter implements Predicate<Node> {
    private DefaultFilter() {}

    @Override
    public boolean test(Node name) {
      return true;
    }
  }

  private static class GetPrefixFilter implements Predicate<Node> {
    private GetPrefixFilter() {}

    @Override
    public boolean test(Node node) {
      String name = node.getString();
      name = nullToEmpty(name);
      return name.startsWith("get");
    }
  }

  private static class FilterOnFilename implements Predicate<Node> {
    private FilterOnFilename() {}

    @Override
    public boolean test(Node node) {
      String name = node.getSourceFileName();
      name = nullToEmpty(name);
      return name.equals("foo.js");
    }
  }

  private static class RenameFunctionsFilterOnFilename implements Predicate<Node> {
    private RenameFunctionsFilterOnFilename() {}

    @Override
    public boolean test(Node node) {
      String name = node.getSourceFileName();
      name = nullToEmpty(name);
      return name.equals("foo.js");
    }
  }

  private static class RenameFunctionsFilterOnFilename2 implements Predicate<Node> {
    private RenameFunctionsFilterOnFilename2() {}

    @Override
    public boolean test(Node node) {
      String name = node.getSourceFileName();
      name = nullToEmpty(name);
      return name.equals("foo.js");
    }
  }
}
