/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder for the anonymous
 * CompilerPass that InlinePropertiesTest.getProcessor (lines 59-82) returns when the test field
 * runSmartNameRemoval is true. Copied VERBATIM from
 * test/com/google/javascript/jscomp/InlinePropertiesTest.java (closure-compiler commit bb8c8e7):
 *   Anon1 = InlinePropertiesTest$1: anonymous class body, lines 74-78.
 * Generated (not copied): the static nested class declaration, and the two final fields plus the
 * constructor that stand in for the captured effectively-final locals `pass` (line 61,
 * `new InlineProperties(compiler)`) and `removalPass` (lines 63-71, the RemoveUnusedCode.Builder
 * chain). The descriptor builds both with DSL expressions, in the test's order (pass first), and
 * passes them as constructor args (pass, removalPass).
 * DSL name: InlinePropertiesTest_Helpers.Anon1
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.Node;

final class InlinePropertiesTest_Helpers {
  /** InlinePropertiesTest$1 (getProcessor, runSmartNameRemoval): anonymous CompilerPass, body lines 74-78. */
  static final class Anon1 implements CompilerPass {
    private final CompilerPass pass;
    private final CompilerPass removalPass;

    Anon1(CompilerPass pass, CompilerPass removalPass) {
      this.pass = pass;
      this.removalPass = removalPass;
    }

        @Override
        public void process(Node externs, Node root) {
          removalPass.process(externs, root);
          pass.process(externs, root);
        }
  }
}
