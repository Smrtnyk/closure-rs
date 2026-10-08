/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Copied from
 * test/com/google/javascript/jscomp/ValidityCheckTest.java (closure-compiler commit bb8c8e7):
 *   field otherPass:                          line 37, VERBATIM.
 *   getProcessor(Compiler):                   lines 47-55, VERBATIM (minus @Override, line 46);
 *                                             its anonymous class is ValidityCheckTest$1.
 *   class bodies of the anonymous CompilerPass instances the tests assign to otherPass, VERBATIM:
 *     OtherPass2 = ValidityCheckTest$2, testUnnormalizeNodeTypes,      lines 61-69;
 *     OtherPass3 = ValidityCheckTest$3, testUnnormalized,              lines 84-87;
 *     OtherPass4 = ValidityCheckTest$4, testConstantAnnotationMismatch, lines 102-112.
 * Generated (not from the test):
 *   - the holder class and the OtherPassN class declaration lines;
 *   - lastCompiler/getLastCompiler(): stands in for CompilerTestCase#getLastCompiler() of the test
 *     instance. During CompilerTestCase#testInternal the harness's lastCompiler is exactly the
 *     Compiler passed to getProcessor (also after a multistage serialize/deserialize step, which
 *     reassigns both), so GetProcessor records that compiler here before calling getProcessor
 *     (AggressiveInlineAliasesTest_Helpers precedent);
 *   - GetProcessor: the DSL entry point. It plays the test method's role of assigning otherPass
 *     (the copy of the anonymous class named by the record's testFields.otherPass.object; an
 *     unknown name throws), then calls the verbatim getProcessor and delegates process() to it.
 * DSL name: ValidityCheckTest_Helpers.GetProcessor
 */
package com.google.javascript.jscomp;

import com.google.javascript.jscomp.AbstractCompiler.LifeCycleStage;
import com.google.javascript.rhino.Node;
import com.google.javascript.rhino.Token;
import org.jspecify.annotations.Nullable;

final class ValidityCheckTest_Helpers {

  private @Nullable CompilerPass otherPass = null;

  // Generated: see header.
  private Compiler lastCompiler;

  // Generated: see header.
  private Compiler getLastCompiler() {
    return lastCompiler;
  }

  // Generated: DSL entry point, see header.
  private class GetProcessor implements CompilerPass {
    private final CompilerPass delegate;

    private GetProcessor(Compiler compiler, String otherPassClass) {
      lastCompiler = compiler;
      switch (otherPassClass) {
        case "com.google.javascript.jscomp.ValidityCheckTest$2" -> otherPass = new OtherPass2();
        case "com.google.javascript.jscomp.ValidityCheckTest$3" -> otherPass = new OtherPass3();
        case "com.google.javascript.jscomp.ValidityCheckTest$4" -> otherPass = new OtherPass4();
        default -> throw new IllegalStateException("unknown otherPass class " + otherPassClass);
      }
      delegate = getProcessor(compiler);
    }

    @Override
    public void process(Node externs, Node root) {
      delegate.process(externs, root);
    }
  }

  protected CompilerPass getProcessor(final Compiler compiler) {
    return new CompilerPass() {
      @Override
      public void process(Node externs, Node root) {
        otherPass.process(externs, root);
        (new ValidityCheck(compiler)).process(externs, root);
      }
    };
  }

  // ValidityCheckTest$2 (testUnnormalizeNodeTypes): body lines 61-69.
  private class OtherPass2 implements CompilerPass {
          @Override
          public void process(Node externs, Node root) {
            AbstractCompiler compiler = getLastCompiler();
            Node script = root.getFirstChild();
            final Node ifNode = new Node(Token.IF, new Node(Token.TRUE), new Node(Token.EMPTY));
            ifNode.srcrefTree(script);
            root.getFirstChild().addChildToBack(ifNode);
            compiler.reportChangeToEnclosingScope(script);
          }
  }

  // ValidityCheckTest$3 (testUnnormalized): body lines 84-87.
  private class OtherPass3 implements CompilerPass {
          @Override
          public void process(Node externs, Node root) {
            getLastCompiler().setLifeCycleStage(LifeCycleStage.NORMALIZED);
          }
  }

  // ValidityCheckTest$4 (testConstantAnnotationMismatch): body lines 102-112.
  private class OtherPass4 implements CompilerPass {
          @Override
          public void process(Node externs, Node root) {
            AbstractCompiler compiler = getLastCompiler();
            Node script = root.getFirstChild();
            Node name = Node.newString(Token.NAME, "x");
            name.putBooleanProp(Node.IS_CONSTANT_NAME, true);
            final Node exprResult = new Node(Token.EXPR_RESULT, name).srcrefTree(script);
            script.addChildToBack(exprResult);
            compiler.reportChangeToEnclosingScope(script);
            compiler.setLifeCycleStage(LifeCycleStage.NORMALIZED);
          }
  }
}
