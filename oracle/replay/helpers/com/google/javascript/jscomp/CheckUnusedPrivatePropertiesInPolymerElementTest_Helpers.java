/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). The holder class and the nested
 * class declaration line are generated; the nested class body is getProcessor(Compiler), copied
 * VERBATIM from
 *   test/com/google/javascript/jscomp/CheckUnusedPrivatePropertiesInPolymerElementTest.java
 *   lines 54-65
 * (closure-compiler commit bb8c8e7), i.e. lines 53-65 minus the @Override on line 53 (the host
 * has no superclass declaring getProcessor). The returned anonymous CompilerPass is the test's
 * CheckUnusedPrivatePropertiesInPolymerElementTest$1; it constructs PolymerPass, TypeCheck and
 * CheckUnusedPrivateProperties inside process(), as the original does. The descriptor calls
 * getProcessor on a fresh host, so the processor is exactly the pass the test builds.
 * DSL name: CheckUnusedPrivatePropertiesInPolymerElementTest_Helpers.GetProcessorHost
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.lint.CheckUnusedPrivateProperties;
import com.google.javascript.rhino.Node;

final class CheckUnusedPrivatePropertiesInPolymerElementTest_Helpers {
  private static final class GetProcessorHost {
  protected CompilerPass getProcessor(final Compiler compiler) {
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        new PolymerPass(compiler).process(externs, root);
        new TypeCheck(
                compiler, compiler.getReverseAbstractInterpreter(), compiler.getTypeRegistry())
            .processForTesting(externs, root);
        new CheckUnusedPrivateProperties(compiler).process(externs, root);
      }
    };
  }
  }
}
