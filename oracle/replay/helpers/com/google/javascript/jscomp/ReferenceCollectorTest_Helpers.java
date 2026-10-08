/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder for the
 * ReferenceCollector.Behavior callbacks that ReferenceCollectorTest passes to getProcessor through
 * its `behavior` field (getProcessor: lines 44-48; testBehavior: lines 57-60). Copied VERBATIM from
 * test/com/google/javascript/jscomp/ReferenceCollectorTest.java (closure-compiler commit bb8c8e7):
 *   IS_DECLARATION: lines 62-63
 *   Anon1 = ReferenceCollectorTest$1 (testVarInBlock): anonymous class body, lines 256-263
 *   Anon2 = ReferenceCollectorTest$2 (testVarInLoopNotAssignedOnlyOnceInLifetime): anonymous class body, lines 271-277
 *   Anon3 = ReferenceCollectorTest$3 (testVarInFunctionNotAssignedOnlyOnceInLifetime): anonymous class body, lines 305-311
 *   Anon4 = ReferenceCollectorTest$4 (testParameterAssignedOnlyOnceInLifetime): anonymous class body, lines 322-328
 *   Anon5 = ReferenceCollectorTest$5 (testModifiedParameterNotAssignedOnlyOnceInLifetime): anonymous class body, lines 337-343
 *   Anon6 = ReferenceCollectorTest$6 (testVarAssignedOnceInLifetime1): anonymous class body, lines 351-357
 *   Anon7 = ReferenceCollectorTest$7 (testVarAssignedOnceInLifetime2): anonymous class body, lines 368-374
 *   Anon8 = ReferenceCollectorTest$8 (testVarAssignedOnceInLifetime3): anonymous class body, lines 382-391
 *   Anon9 = ReferenceCollectorTest$9 (testLetAssignedOnceInLifetime1): anonymous class body, lines 427-436
 *   Anon10 = ReferenceCollectorTest$10 (testLetAssignedOnceInLifetime2): anonymous class body, lines 452-461
 *   Anon11 = ReferenceCollectorTest$11 (testBasicBlocks): anonymous class body, lines 476-485
 *   Anon12 = ReferenceCollectorTest$12 (testBasicBlocksInConditionals): anonymous class body, lines 499-511
 *   Anon13 = ReferenceCollectorTest$13 (nullishCoalesce): anonymous class body, lines 520-530
 *   Anon14 = ReferenceCollectorTest$14 (optChain): anonymous class body, lines 539-549
 *   Anon15 = ReferenceCollectorTest$15 (testThis): anonymous class body, lines 568-575
 *   Lambda_testIterableRest_declaration = the Behavior lambda in testIterableRest_declaration: lambda body, lines 70-74
 *   Lambda_testObjectRest_declaration = the Behavior lambda in testObjectRest_declaration: lambda body, lines 83-87
 *   Lambda_testClass = the Behavior lambda in testClass: lambda body, lines 98-104
 *   Lambda_testClass_withInstantiation = the Behavior lambda in testClass_withInstantiation: lambda body, lines 116-126
 *   Lambda_testClassExpression_assignedToConst_andInstantiated = the Behavior lambda in testClassExpression_assignedToConst_andInstantiated: lambda body, lines 138-143
 *   Lambda_testNamedClassExpression_assignedToConst_andInstantiated = the Behavior lambda in testNamedClassExpression_assignedToConst_andInstantiated: lambda body, lines 155-160
 *   Lambda_testImport1 = the Behavior lambda in testImport1: lambda body, lines 169-175
 *   Lambda_testImport2 = the Behavior lambda in testImport2: lambda body, lines 184-190
 *   Lambda_testImport2_alternate = the Behavior lambda in testImport2_alternate: lambda body, lines 199-205
 *   Lambda_testImport3 = the Behavior lambda in testImport3: lambda body, lines 214-221
 *   Lambda_testImport4 = the Behavior lambda in testImport4: lambda body, lines 230-239
 *   Lambda_testLetInLoopAssignedOnceInLifetime = the Behavior lambda in testLetInLoopAssignedOnceInLifetime: lambda body, lines 286-292
 * Each anonymous `new Behavior() { ... }` becomes a generated static nested class whose body is
 * the anonymous class body; each `(t, rm) -> { ... }` lambda becomes a generated static nested
 * class whose afterExitScope(NodeTraversal t, ReferenceMap rm) body is the lambda body. The
 * callbacks only assert (Truth / NodeSubject); a failed assertion surfaces as the replayed
 * outcome, exactly as in the test. (ReferenceCollectorTest$16, in
 * testProcessScopeThatsNotABasicBlock, is not a testBehavior call and has no record.)
 * DSL names: ReferenceCollectorTest_Helpers.Anon<N>, ReferenceCollectorTest_Helpers.Lambda_<testMethod>
 */
package com.google.javascript.jscomp;

import static com.google.common.base.Preconditions.checkNotNull;
import static com.google.common.truth.Truth.assertThat;
import static com.google.javascript.rhino.testing.NodeSubject.assertNode;

import com.google.common.truth.Correspondence;
import com.google.javascript.jscomp.ReferenceCollector.Behavior;
import com.google.javascript.rhino.Token;

final class ReferenceCollectorTest_Helpers {
  private static final Correspondence<Reference, Boolean> IS_DECLARATION =
      Correspondence.transforming(Reference::isDeclaration, "isDeclaration() is");

  /** ReferenceCollectorTest$1 (testVarInBlock): anonymous Behavior, body lines 256-263. */
  static final class Anon1 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isBlockScope() && t.getScope().getParent().isFunctionBlockScope()) {
              ReferenceCollection y = rm.getReferences(t.getScope().getVar("y"));
              assertThat(y.isAssignedOnceInLifetime()).isTrue();
              assertThat(y.isWellDefined()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$2 (testVarInLoopNotAssignedOnlyOnceInLifetime): anonymous Behavior, body lines 271-277. */
  static final class Anon2 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isBlockScope()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isFalse();
            }
          }
  }

  /** ReferenceCollectorTest$3 (testVarInFunctionNotAssignedOnlyOnceInLifetime): anonymous Behavior, body lines 305-311. */
  static final class Anon3 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isGlobal()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isFalse();
            }
          }
  }

  /** ReferenceCollectorTest$4 (testParameterAssignedOnlyOnceInLifetime): anonymous Behavior, body lines 322-328. */
  static final class Anon4 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isFunctionScope()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$5 (testModifiedParameterNotAssignedOnlyOnceInLifetime): anonymous Behavior, body lines 337-343. */
  static final class Anon5 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isFunctionScope()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isFalse();
            }
          }
  }

  /** ReferenceCollectorTest$6 (testVarAssignedOnceInLifetime1): anonymous Behavior, body lines 351-357. */
  static final class Anon6 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isFunctionBlockScope()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$7 (testVarAssignedOnceInLifetime2): anonymous Behavior, body lines 368-374. */
  static final class Anon7 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isBlockScope() && !t.getScope().isFunctionBlockScope()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.isAssignedOnceInLifetime()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$8 (testVarAssignedOnceInLifetime3): anonymous Behavior, body lines 382-391. */
  static final class Anon8 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isCatchScope()) {
              ReferenceCollection e = rm.getReferences(t.getScope().getVar("e"));
              assertThat(e.isAssignedOnceInLifetime()).isTrue();
              ReferenceCollection y = rm.getReferences(t.getScope().getVar("y"));
              assertThat(y.isAssignedOnceInLifetime()).isTrue();
              assertThat(y.isWellDefined()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$9 (testLetAssignedOnceInLifetime1): anonymous Behavior, body lines 427-436. */
  static final class Anon9 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isCatchScope()) {
              ReferenceCollection e = rm.getReferences(t.getScope().getVar("e"));
              assertThat(e.isAssignedOnceInLifetime()).isTrue();
              ReferenceCollection y = rm.getReferences(t.getScope().getVar("y"));
              assertThat(y.isAssignedOnceInLifetime()).isTrue();
              assertThat(y.isWellDefined()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$10 (testLetAssignedOnceInLifetime2): anonymous Behavior, body lines 452-461. */
  static final class Anon10 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isCatchScope()) {
              ReferenceCollection e = rm.getReferences(t.getScope().getVar("e"));
              assertThat(e.isAssignedOnceInLifetime()).isTrue();
              ReferenceCollection y = rm.getReferences(t.getScope().getVar("y"));
              assertThat(y.isAssignedOnceInLifetime()).isTrue();
              assertThat(y.isWellDefined()).isTrue();
            }
          }
  }

  /** ReferenceCollectorTest$11 (testBasicBlocks): anonymous Behavior, body lines 476-485. */
  static final class Anon11 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isGlobal()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.references).hasSize(3);
              assertNode(x.references.get(0).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(1).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(2).getBasicBlock().getRoot()).hasType(Token.CASE);
            }
          }
  }

  /** ReferenceCollectorTest$12 (testBasicBlocksInConditionals): anonymous Behavior, body lines 499-511. */
  static final class Anon12 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isGlobal()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.references).hasSize(4);
              // first child of || is not a boundary, but the second child is.
              assertNode(x.references.get(0).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(1).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(2).getBasicBlock().getRoot()).hasType(Token.NAME);
              // second child of `y = (x = 1)` is a boundary
              assertNode(x.references.get(3).getBasicBlock().getRoot()).hasType(Token.ASSIGN);
            }
          }
  }

  /** ReferenceCollectorTest$13 (nullishCoalesce): anonymous Behavior, body lines 520-530. */
  static final class Anon13 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isGlobal()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.references).hasSize(3);
              assertNode(x.references.get(0).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(1).getBasicBlock().getRoot()).hasType(Token.ROOT);
              // first child of ?? is not a boundary, but the second child is.
              assertNode(x.references.get(2).getBasicBlock().getRoot()).hasType(Token.ASSIGN);
            }
          }
  }

  /** ReferenceCollectorTest$14 (optChain): anonymous Behavior, body lines 539-549. */
  static final class Anon14 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isGlobal()) {
              ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
              assertThat(x.references).hasSize(3);
              assertNode(x.references.get(0).getBasicBlock().getRoot()).hasType(Token.ROOT);
              assertNode(x.references.get(1).getBasicBlock().getRoot()).hasType(Token.ROOT);
              // first child of `?.` is not a boundary, but the second child is.
              assertNode(x.references.get(2).getBasicBlock().getRoot()).hasType(Token.ASSIGN);
            }
          }
  }

  /** ReferenceCollectorTest$15 (testThis): anonymous Behavior, body lines 568-575. */
  static final class Anon15 implements Behavior {
          @Override
          public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
            if (t.getScope().isFunctionBlockScope()
                && t.getScopeRoot().getParent().getFirstChild().matchesName("m")) {
              ReferenceCollection self = rm.getReferences(t.getScope().getVar("self"));
              assertThat(self.isEscaped()).isFalse();
            }
          }
  }

  /** Lambda Behavior of testIterableRest_declaration (`(NodeTraversal t, ReferenceMap rm) -> {` at line 69): body lines 70-74. */
  static final class Lambda_testIterableRest_declaration implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          ReferenceCollection arrReferenceCollection = rm.getReferences(t.getScope().getVar("arr"));
          assertThat(arrReferenceCollection)
              .comparingElementsUsing(IS_DECLARATION)
              .containsExactly(true, false)
              .inOrder();
    }
  }

  /** Lambda Behavior of testObjectRest_declaration (`(NodeTraversal t, ReferenceMap rm) -> {` at line 82): body lines 83-87. */
  static final class Lambda_testObjectRest_declaration implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          ReferenceCollection objReferenceCollection = rm.getReferences(t.getScope().getVar("obj"));
          assertThat(objReferenceCollection)
              .comparingElementsUsing(IS_DECLARATION)
              .containsExactly(true, false)
              .inOrder();
    }
  }

  /** Lambda Behavior of testClass (`(NodeTraversal t, ReferenceMap rm) -> {` at line 97): body lines 98-104. */
  static final class Lambda_testClass implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isGlobal()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("Foo"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testClass_withInstantiation (`(NodeTraversal t, ReferenceMap rm) -> {` at line 115): body lines 116-126. */
  static final class Lambda_testClass_withInstantiation implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isGlobal()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("Foo"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            // TODO(lharker): we should consider this well-defined.
            assertThat(x.isWellDefined()).isFalse();
            assertThat(x)
                .comparingElementsUsing(IS_DECLARATION)
                .containsExactly(true, false)
                .inOrder();
          }
    }
  }

  /** Lambda Behavior of testClassExpression_assignedToConst_andInstantiated (`(NodeTraversal t, ReferenceMap rm) -> {` at line 137): body lines 138-143. */
  static final class Lambda_testClassExpression_assignedToConst_andInstantiated implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isGlobal()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("Foo"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
          }
    }
  }

  /** Lambda Behavior of testNamedClassExpression_assignedToConst_andInstantiated (`(NodeTraversal t, ReferenceMap rm) -> {` at line 154): body lines 155-160. */
  static final class Lambda_testNamedClassExpression_assignedToConst_andInstantiated implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isGlobal()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("Foo"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
          }
    }
  }

  /** Lambda Behavior of testImport1 (`(NodeTraversal t, ReferenceMap rm) -> {` at line 168): body lines 169-175. */
  static final class Lambda_testImport1 implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isModuleScope()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testImport2 (`(NodeTraversal t, ReferenceMap rm) -> {` at line 183): body lines 184-190. */
  static final class Lambda_testImport2 implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isModuleScope()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testImport2_alternate (`(NodeTraversal t, ReferenceMap rm) -> {` at line 198): body lines 199-205. */
  static final class Lambda_testImport2_alternate implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isModuleScope()) {
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testImport3 (`(NodeTraversal t, ReferenceMap rm) -> {` at line 213): body lines 214-221. */
  static final class Lambda_testImport3 implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isModuleScope()) {
            assertThat(t.getScope().getVar("y")).isNull();
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testImport4 (`(NodeTraversal t, ReferenceMap rm) -> {` at line 229): body lines 230-239. */
  static final class Lambda_testImport4 implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isModuleScope()) {
            Var var = t.getScope().getVar("x");
            checkNotNull(var);
            ReferenceCollection x = rm.getReferences(t.getScope().getVar("x"));
            checkNotNull(x);

            assertThat(x.isAssignedOnceInLifetime()).isTrue();
            assertThat(x.isWellDefined()).isTrue();
            assertThat(x).comparingElementsUsing(IS_DECLARATION).containsExactly(true).inOrder();
          }
    }
  }

  /** Lambda Behavior of testLetInLoopAssignedOnceInLifetime (`(t, rm) -> {` at line 285): body lines 286-292. */
  static final class Lambda_testLetInLoopAssignedOnceInLifetime implements Behavior {
    @Override
    public void afterExitScope(NodeTraversal t, ReferenceMap rm) {
          if (t.getScope().isBlockScope()) {
            Var xVar = t.getScope().getVar("x");
            assertThat(xVar).isNotNull();
            assertThat(xVar.isLet()).isTrue();
            ReferenceCollection x = rm.getReferences(xVar);
            assertThat(x.isAssignedOnceInLifetime()).isTrue();
          }
    }
  }
}
