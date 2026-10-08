/*
 * closure-rs unit-corpus helper (corpus/unit/DSL.md "helper"). Generated holder class whose
 * fields mirror PureFunctionIdentifierTest's fields of the same names and initial values
 * (lines 39-42; regExpHaveSideEffects and enableArtificialPurityDebugError are restored from the
 * record's testFields by the descriptor). Generated lines are the holder/class declarations,
 * constructors and fields that stand for lambda captures; every method body below is copied
 * VERBATIM (closure-compiler commit bb8c8e7) from:
 *   test/com/google/javascript/jscomp/PureFunctionIdentifierTest.java
 *     NoSideEffectCallEnumerator: lines 180-216, except that the Driver construction on the
 *       `new PureFunctionIdentifier.Driver(compiler).process(externs, root)` line is hoisted into a
 *       constructor argument (built by the descriptor with DSL `new`; Driver's constructor only
 *       stores the compiler, so construction time is unobservable)
 *     AssertPureCallsMarkedPostcondition.verify: body of the postcondition lambda in
 *       assertPureCallsMarked, lines 3813-3818 (captures: this, expected, post)
 *     Post_<testMethod>.verify: body of the `compiler -> {...}` Postcondition lambda passed as
 *       `post` by that test method (captures: this only):
 *       testFunctionProperties1: lines 3408-3412
 *       testTaggedTemplatelit_propagatesCalleeSideEffects: lines 3436-3439
 *       testCallCache: lines 3633-3636
 *       testCallCache_withKeyFn: lines 3652-3655
 *       testCallCache_anonymousFn: lines 3666-3669
 *       testCallCache_anonymousFn_hasSideEffects: lines 3683-3686
 *       testCallCache_hasSideEffects: lines 3701-3704
 *       testCallCache_withKeyFn_hasSideEffects: lines 3720-3723
 *       testCallCache_propagatesSideEffects: lines 3739-3747
 *   test/com/google/javascript/jscomp/CompilerTestCase.java
 *     findQualifiedNameNode / findQualifiedNameNodes: lines 1748-1764 (harness instance methods
 *     the Post_* lambdas call on the captured test instance)
 * DSL names: PureFunctionIdentifierTest_Helpers (holder),
 *   PureFunctionIdentifierTest_Helpers.NoSideEffectCallEnumerator,
 *   PureFunctionIdentifierTest_Helpers.AssertPureCallsMarkedPostcondition,
 *   PureFunctionIdentifierTest_Helpers.Post_<testMethod>
 */
package com.google.javascript.jscomp;

import static com.google.common.truth.Truth.assertThat;

import com.google.common.collect.Iterables;
import com.google.javascript.jscomp.CompilerTestCase.Postcondition;
import com.google.javascript.jscomp.NodeTraversal.AbstractPostOrderCallback;
import com.google.javascript.jscomp.testing.JSCompCorrespondences;
import com.google.javascript.rhino.Node;
import java.util.ArrayList;
import java.util.List;
import org.jspecify.annotations.Nullable;

final class PureFunctionIdentifierTest_Helpers {
  List<Node> noSideEffectCalls;

  boolean regExpHaveSideEffects = true;
  boolean enableArtificialPurityDebugError = false;

  /**
   * Run PureFunctionIdentifier, then gather a list of calls that are marked as having no side
   * effects.
   */
  private class NoSideEffectCallEnumerator extends AbstractPostOrderCallback
      implements CompilerPass {
    private final Compiler compiler;
    // Generated: the PureFunctionIdentifier.Driver the verbatim body runs is
    // built by the descriptor (DSL `new` of PureFunctionIdentifier$Driver) and passed in, so that
    // --mutate-noop PureFunctionIdentifier replaces only the pass under test, not this enumerator.
    private final CompilerPass driver;

    NoSideEffectCallEnumerator(Compiler compiler, CompilerPass driver) {
      this.compiler = compiler;
      this.driver = driver;
    }

    @Override
    public void process(Node externs, Node root) {
      noSideEffectCalls = new ArrayList<>();
      // TODO(nickreid): Move these into 'getOptions' and 'getCompiler' overrides.
      compiler.setHasRegExpGlobalReferences(regExpHaveSideEffects);
      compiler.getOptions().setUseTypesForLocalOptimization(true);

      driver.process(externs, root); // verbatim line 199: new PureFunctionIdentifier.Driver(compiler).process(externs, root);
      NodeTraversal.traverse(compiler, externs, this);
      NodeTraversal.traverse(compiler, root, this);
    }

    @Override
    public void visit(NodeTraversal t, Node n, Node parent) {
      if (n.isNew()) {
        if (!compiler.getAstAnalyzer().constructorCallHasSideEffects(n)) {
          noSideEffectCalls.add(n.getFirstChild());
        }
      } else if (NodeUtil.isInvocation(n)) {
        if (!compiler.getAstAnalyzer().functionCallHasSideEffects(n)) {
          noSideEffectCalls.add(n.getFirstChild());
        }
      }
    }
  }

  // Generated: the postcondition lambda of assertPureCallsMarked (lines 3807-3820); the
  // constructor parameters are its captured `expected` and `post`.
  final class AssertPureCallsMarkedPostcondition implements Postcondition {
    private final List<String> expected;
    private final @Nullable Postcondition post;

    AssertPureCallsMarkedPostcondition(List<String> expected, @Nullable Postcondition post) {
      this.expected = expected;
      this.post = post;
    }

    @Override
    public void verify(Compiler compiler) {
              assertThat(noSideEffectCalls)
                  .comparingElementsUsing(JSCompCorrespondences.EQUALITY_WHEN_PARSED_AS_EXPRESSION)
                  .containsExactlyElementsIn(expected);
              if (post != null) {
                post.verify(compiler);
              }
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testFunctionProperties1 (lines 3407-3413).
  final class Post_testFunctionProperties1 implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot();
          Node call = findQualifiedNameNode("g.call", lastRoot).getParent();
          assertThat(call.getSideEffectFlags())
              .isEqualTo(
                  new Node.SideEffectFlags().clearAllFlags().setMutatesArguments().valueOf());
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testTaggedTemplatelit_propagatesCalleeSideEffects (lines 3435-3440).
  final class Post_testTaggedTemplatelit_propagatesCalleeSideEffects implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot();
          Node tagDef = findQualifiedNameNode("tag", lastRoot).getParent();
          Node fooDef = findQualifiedNameNode("foo", lastRoot).getParent();
          assertThat(fooDef.getSideEffectFlags()).isEqualTo(tagDef.getSideEffectFlags());
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache (lines 3632-3637).
  final class Post_testCallCache implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isTrue();
          assertThat(call.mayMutateGlobalStateOrThrow()).isFalse();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_withKeyFn (lines 3651-3656).
  final class Post_testCallCache_withKeyFn implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isTrue();
          assertThat(call.mayMutateGlobalStateOrThrow()).isFalse();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_anonymousFn (lines 3665-3670).
  final class Post_testCallCache_anonymousFn implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isTrue();
          assertThat(call.mayMutateGlobalStateOrThrow()).isFalse();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_anonymousFn_hasSideEffects (lines 3682-3687).
  final class Post_testCallCache_anonymousFn_hasSideEffects implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isFalse();
          assertThat(call.mayMutateGlobalStateOrThrow()).isTrue();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_hasSideEffects (lines 3700-3705).
  final class Post_testCallCache_hasSideEffects implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isFalse();
          assertThat(call.mayMutateGlobalStateOrThrow()).isTrue();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_withKeyFn_hasSideEffects (lines 3719-3724).
  final class Post_testCallCache_withKeyFn_hasSideEffects implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node call = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(call.isNoSideEffectsCall()).isFalse();
          assertThat(call.mayMutateGlobalStateOrThrow()).isTrue();
    }
  }

  // Generated: the Postcondition lambda passed as `post` in testCallCache_propagatesSideEffects (lines 3738-3748).
  final class Post_testCallCache_propagatesSideEffects implements Postcondition {
    @Override
    public void verify(Compiler compiler) {
          Node lastRoot = compiler.getRoot().getLastChild();
          Node cacheCall = findQualifiedNameNode("goog.reflect.cache", lastRoot).getParent();
          assertThat(cacheCall.isNoSideEffectsCall()).isTrue();
          assertThat(cacheCall.mayMutateGlobalStateOrThrow()).isFalse();

          Node helperCall =
              Iterables.getLast(findQualifiedNameNodes("helper", lastRoot)).getParent();
          assertThat(helperCall.isNoSideEffectsCall()).isTrue();
          assertThat(helperCall.mayMutateGlobalStateOrThrow()).isFalse();
    }
  }

  /** Finds the first matching qualified name node in post-traversal order. */
  protected final Node findQualifiedNameNode(final String name, Node root) {
    return findQualifiedNameNodes(name, root).get(0);
  }

  /** Finds all the matching qualified name nodes in post-traversal order. */
  protected final List<Node> findQualifiedNameNodes(final String name, Node root) {
    final List<Node> matches = new ArrayList<>();
    NodeUtil.visitPostOrder(
        root,
        n -> {
          if (n.matchesQualifiedName(name)) {
            matches.add(n);
          }
        });
    return matches;
  }
}
