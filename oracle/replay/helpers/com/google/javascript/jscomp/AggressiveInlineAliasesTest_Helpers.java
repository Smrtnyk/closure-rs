/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Copied from
 * test/com/google/javascript/jscomp/AggressiveInlineAliasesTest.java (closure-compiler commit
 * bb8c8e7):
 *   validateGlobalNamespace(GlobalNamespace):   lines 3341-3398 (javadoc and method), VERBATIM.
 * Generated (not from the test):
 *   - the holder class, which stands in for the test instance that `this::validateGlobalNamespace`
 *     (getProcessor, line 60) binds;
 *   - lastCompiler/getLastCompiler(): stand in for CompilerTestCase#lastCompiler and
 *     #getLastCompiler() of the test instance. During CompilerTestCase#testInternal the harness's
 *     lastCompiler is exactly the Compiler passed to getProcessor (CompilerTestCase.java lines 947
 *     and 1207, including after the multistage serialize/deserialize step, which reassigns both),
 *     so the descriptor sets lastCompiler to that compiler with "withFields".
 * getProcessor itself (lines 47-63) is written in the DSL (corpus/unit/descriptors/
 * AggressiveInlineAliasesTest.json); the callback is a DSL lambda calling validateGlobalNamespace.
 * DSL names: AggressiveInlineAliasesTest_Helpers (holder, no-argument constructor)
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertWithMessage;

import com.google.javascript.jscomp.GlobalNamespace.Name;

final class AggressiveInlineAliasesTest_Helpers {
  // Generated: see header.
  private Compiler lastCompiler;

  // Generated: see header.
  private Compiler getLastCompiler() {
    return lastCompiler;
  }

  /**
   * To ensure that as we modify the AST, the GlobalNamespace stays up-to-date, we do a consistency
   * check after every unit test.
   *
   * <p>This check compares the names in the global namespace in the pass with a freshly-created
   * global namespace.
   */
  public void validateGlobalNamespace(GlobalNamespace passGlobalNamespace) {
    GlobalNamespace expectedGlobalNamespace =
        new GlobalNamespace(getLastCompiler(), getLastCompiler().getJsRoot());

    // GlobalNamespace (understandably) does not override equals. It would be silly to put it in
    // a datastructure. Neither does GlobalNamespace.Name (which probably could?)
    // So to compare equality: we verify that
    //  1. the two namespaces have the same qualified names, bar extern names
    //  2. each name has the same number of references in both namespaces
    //  3. each name has the same computed `Inlinability`
    for (Name expectedName : expectedGlobalNamespace.getNameForest()) {
      if (expectedName.inExterns()) {
        continue;
      }
      String fullName = expectedName.getFullName();
      Name actualName = passGlobalNamespace.getSlot(expectedName.getFullName());
      assertWithMessage(fullName).that(actualName).isNotNull();

      assertWithMessage(fullName)
          .that(actualName.getAliasingGets())
          .isEqualTo(expectedName.getAliasingGets());
      assertWithMessage(fullName)
          .that(actualName.getSubclassingGets())
          .isEqualTo(expectedName.getSubclassingGets());
      assertWithMessage(fullName)
          .that(actualName.getLocalSets())
          .isEqualTo(expectedName.getLocalSets());
      assertWithMessage(fullName)
          .that(actualName.getGlobalSets())
          .isEqualTo(expectedName.getGlobalSets());
      assertWithMessage(fullName)
          .that(actualName.getDeleteProps())
          .isEqualTo(expectedName.getDeleteProps());
      assertWithMessage(fullName)
          .that(actualName.getCallGets())
          .isEqualTo(expectedName.getCallGets());
      assertWithMessage("%s: canCollapseOrInline()", fullName)
          .that(actualName.canCollapseOrInline())
          .isEqualTo(expectedName.canCollapseOrInline());
      assertWithMessage("%s: canCollapseOrInlineChildNames()", fullName)
          .that(actualName.canCollapseOrInlineChildNames())
          .isEqualTo(expectedName.canCollapseOrInlineChildNames());
    }
    // Verify that no names in the actual name forest are not present in the expected name forest
    for (Name actualName : passGlobalNamespace.getNameForest()) {
      String actualFullName = actualName.getFullName();
      assertWithMessage(actualFullName)
          .that(expectedGlobalNamespace.getSlot(actualFullName))
          .isNotNull();
    }
  }
}
