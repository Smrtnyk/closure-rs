/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper") for RemoveUnnecessarySyntheticExternsTest.
 * Source: test/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExternsTest.java at closure-compiler commit bb8c8e7.
 * This holder stands in for one test instance (one record; the descriptor creates it once per
 * record with {"once":...}).
 *
 * Copied VERBATIM (indentation unchanged, nothing else edited):
 *   imports IR, Node, LinkedHashSet, Nullable:     lines 19-22
 *   field syntheticExternsToAdd (with comment):    lines 31-32
 *   setUp statement initialising the field:        line 39 (super.setUp() and
 *                                                  enableMultistageCompilation() on lines 37-38
 *                                                  only touch harness fields, restored from
 *                                                  record.harness.fields)
 *   createUnfulfilledDeclaration:                  lines 174-180
 *   test-method prologues (the syntheticExternsToAdd.add(...) statements that run before
 *   testExternChanges(...)):
 *     doesntChangeSyntheticExternThatIsNotDeclaredInCode: lines 55-56
 *     doesntRemoveSyntheticExternIfShadowedInCode: lines 63-63
 *     removesSyntheticExternDeclaredInCode: lines 73-74
 *     removesSyntheticExternDeclaredInGoogProvide: lines 86-87
 *     doesNotRemovesSyntheticExternForLegacyGoogModule: lines 96-97
 *     removesSyntheticExternDeclaredInLegacyGoogModuleNamespace: lines 105-106
 *     removesSyntheticExternDeclaredInOtherExterns: lines 116-117
 *     removesDistinctSyntheticExternsDeclaredInCode: lines 125-126
 *     removesDuplicateUnfulfilledSyntheticDeclarations: lines 133-136
 *     removesSyntheticExternDeclaredInCode_multipleCodeDeclarations: lines 144-145
 *     doesntChangeSyntheticExtern_declaredInCodeNotMarkedUnfulfilled: lines 152-155
 *     removesOnlyUnfulfilledSyntheticExterns_ifMixOfFulfilledAndUnfulfilled: lines 162-168
 * Generated (not from the test): the holder class itself, the setUp/prologue method signatures
 * (prologues named after the test methods) and their trailing "return this;", which lets the DSL
 * chain {"call": setUp} -> {"call": prologue}.
 * getProcessor (lines 41-51) is NOT in this holder: the descriptor writes it in the DSL as a
 * CompilerPass lambda that reads this holder's syntheticExternsToAdd (getField), so the
 * RemoveUnnecessarySyntheticExterns construction is visible to --mutate-noop.
 */
package com.google.javascript.jscomp;

import com.google.javascript.rhino.IR;
import com.google.javascript.rhino.Node;
import java.util.LinkedHashSet;
import org.jspecify.annotations.Nullable;

final class RemoveUnnecessarySyntheticExternsTest_Helpers {
  // use this set to simulate an earlier compiler pass declaring a synthetic extern
  private @Nullable LinkedHashSet<Node> syntheticExternsToAdd = null;

  RemoveUnnecessarySyntheticExternsTest_Helpers setUp() {
    this.syntheticExternsToAdd = new LinkedHashSet<>();
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers doesntChangeSyntheticExternThatIsNotDeclaredInCode() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers doesntRemoveSyntheticExternIfShadowedInCode() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesSyntheticExternDeclaredInCode() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesSyntheticExternDeclaredInGoogProvide() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers doesNotRemovesSyntheticExternForLegacyGoogModule() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesSyntheticExternDeclaredInLegacyGoogModuleNamespace() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesSyntheticExternDeclaredInOtherExterns() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesDistinctSyntheticExternsDeclaredInCode() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesDuplicateUnfulfilledSyntheticDeclarations() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesSyntheticExternDeclaredInCode_multipleCodeDeclarations() {
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("y"));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers doesntChangeSyntheticExtern_declaredInCodeNotMarkedUnfulfilled() {
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to prevent renaming, not just enforce that all referenced
    // names are declared.
    this.syntheticExternsToAdd.add(IR.var(IR.name("x")));
    return this;
  }

  RemoveUnnecessarySyntheticExternsTest_Helpers removesOnlyUnfulfilledSyntheticExterns_ifMixOfFulfilledAndUnfulfilled() {
    // add an extern that is not marked as an 'unfulfilled declaration'
    // we assume this extern was added to actually prevent renaming, and so must not be removed
    // even if a duplicate of a non-synthetic extern.
    this.syntheticExternsToAdd.add(IR.var(IR.name("x")));
    // add a duplicate declaration of 'x', where the second and third only are unfulfilled.
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    this.syntheticExternsToAdd.add(createUnfulfilledDeclaration("x"));
    return this;
  }

  private Node createUnfulfilledDeclaration(String name) {
    // only VAR nodes are allowed to be marked "synthesized unfulfilled" so no need to test other
    // kinds of declarations.
    Node declaration = IR.var(IR.name(name));
    declaration.setIsSynthesizedUnfulfilledNameDeclaration(true);
    return declaration;
  }
}
