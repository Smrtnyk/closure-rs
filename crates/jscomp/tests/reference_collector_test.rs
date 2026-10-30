/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 8ac03b6:
//   src/com/google/javascript/jscomp/ReferenceCollector.java,
//   test/com/google/javascript/jscomp/ReferenceCollectorTest.java.

//! Port of the `ReferenceCollectorTest.java` tests that have no corpus record. The other
//! `testBehavior` tests replay from corpus/unit/records (ReferenceCollectorTest, 30 records).
use closure_jscomp::node_traversal::NodeTraversal;
use closure_jscomp::reference::Reference;
use closure_jscomp::reference_collector::{Behavior, ReferenceCollector, VarFilter};
use closure_jscomp::reference_map::ReferenceMap;
use closure_jscomp::syntactic_scope_creator::SyntacticScopeCreator;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    harness_passes,
    jscomp_api::{CheckLevel, Compiler, CompilerOptions},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

/// ReferenceCollectorTest's `Predicate<Var> varFilter`.
type VarPredicate = Rc<dyn Fn(&Compiler, closure_jscomp::var::VarId) -> bool>;

/// The fields and overrides of ReferenceCollectorTest (getProcessor, getOptions).
struct ReferenceCollectorTest<B: Behavior + Clone + 'static> {
    ctx: Ctx,
    behavior: B,
    var_filter: VarPredicate,
    collect_hoisted_functions_in_reference_order: bool,
}

impl<B: Behavior + Clone + 'static> CompilerTestCaseHooks for ReferenceCollectorTest<B> {
    // port: ReferenceCollectorTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let behavior = self.behavior.clone();
        let var_filter = self.var_filter.clone();
        let collect_hoisted_functions_in_reference_order =
            self.collect_hoisted_functions_in_reference_order;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                let mut scope_creator = SyntacticScopeCreator::new();
                let var_filter = var_filter.clone();
                let var_filter: VarFilter<'_> = Box::new(move |c, v| var_filter(c, v));
                closure_jscomp::compiler_pass::CompilerPass::process(
                    &mut ReferenceCollector::new_with_hoisted_function_order(
                        compiler,
                        behavior.clone(),
                        &mut scope_creator,
                        var_filter,
                        collect_hoisted_functions_in_reference_order,
                    ),
                    compiler,
                    externs,
                    root,
                );
            },
        )))))
    }
    // port: ReferenceCollectorTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_warning_level(
            harness_passes::diagnostic_group("MODULE_LOAD")?,
            CheckLevel::OFF,
        );
        Ok(options)
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// ReferenceCollectorTest#setUp's field values: `varFilter = Predicates.alwaysTrue()`,
/// `collectHoistedFunctionsInReferenceOrder = true`.
fn set_up<B: Behavior + Clone + 'static>(behavior: B) -> ReferenceCollectorTest<B> {
    ReferenceCollectorTest {
        ctx: Ctx::new(
            "ReferenceCollectorTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
        behavior,
        var_filter: Rc::new(|_, _| true),
        collect_hoisted_functions_in_reference_order: true,
    }
}

// port: ReferenceCollectorTest#testBehavior
fn test_behavior<B: Behavior + Clone + 'static>(test: &mut ReferenceCollectorTest<B>, js: &str) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    if let Err(t) = harness.test_same_string(test, js) {
        panic!("{t:?}");
    }
}

/// The anonymous `Behavior` of `testProcessScopeThatsNotABasicBlock`.
struct NotABasicBlockBehavior;
impl Behavior for NotABasicBlockBehavior {
    // port: ReferenceCollectorTest#testProcessScopeThatsNotABasicBlock (Behavior#afterExitScope)
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, rm: &dyn ReferenceMap) {
        let scope = t.get_scope();
        let var = scope
            .get_var(t.get_compiler(), JsString::from("y"))
            .unwrap();
        let y = rm.get_references(var).unwrap();
        let ast = t.get_compiler();
        assert!(y.is_well_defined(ast));
        let first_basic_block = y.references[0].get_basic_block().unwrap();
        assert_eq!(first_basic_block.get_root().get_token(ast), Token::BLOCK);
        assert_eq!(
            first_basic_block
                .get_root()
                .get_parent(ast)
                .unwrap()
                .get_token(ast),
            Token::SCRIPT
        );

        // We do not create a new BasicBlock for the second { use(y); }
        let second_basic_block = y.references[1].get_basic_block().unwrap();
        assert_eq!(second_basic_block.get_root().get_token(ast), Token::BLOCK);
        assert_eq!(
            second_basic_block
                .get_root()
                .get_parent(ast)
                .unwrap()
                .get_token(ast),
            Token::IF
        );
    }
}

// port: ReferenceCollectorTest#testProcessScopeThatsNotABasicBlock
#[test]
fn test_process_scope_thats_not_a_basic_block() {
    // Tests the case where the scope we pass in is not really a basic block, but we create a new
    // basic block anyway because ReferenceCollector expects all nodes to be in a block.
    let mut case = CompilerTestCase::new("");
    case.set_up();
    let handle = case.create_compiler().unwrap();
    let mut compiler = handle.borrow_mut();
    let compiler = &mut *compiler;
    let mut syntactic_scope_creator = SyntacticScopeCreator::new();

    let js = "let x = 5; { let y = x + 1; if (true) { { use(y); } } }";
    let root = compiler.parse_test_code(js);
    let block = root.get_second_child(compiler).unwrap();

    // Java creates the scopes after constructing the collector with the same creator; the Rust
    // collector borrows the creator mutably, so the scopes are created first.
    let global_scope = syntactic_scope_creator.create_scope(compiler, root, None);
    let block_scope = syntactic_scope_creator.create_scope(compiler, block, Some(global_scope));
    let mut reference_collecting_callback = ReferenceCollector::new(
        compiler,
        NotABasicBlockBehavior,
        &mut syntactic_scope_creator,
    );
    reference_collecting_callback.process_scope(compiler, block_scope);
}

/// The anonymous `Behavior` of the two `testBehavior_hoistedFunctionsInReferenceOrder` tests.
#[derive(Clone, Default)]
struct VisitedReferenceNamesBehavior {
    visited_reference_names: Rc<RefCell<Vec<String>>>,
}
impl Behavior for VisitedReferenceNamesBehavior {
    // port: ReferenceCollectorTest#testBehavior_hoistedFunctionsInReferenceOrder_true (Behavior#afterVisitNode)
    fn after_visit_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
        added_reference: Option<&Reference>,
    ) {
        if let Some(added_reference) = added_reference {
            self.visited_reference_names.borrow_mut().push(
                added_reference
                    .get_node()
                    .get_string(t.get_compiler())
                    .to_string(),
            );
        }
    }
    // port: ReferenceCollectorTest#testBehavior_hoistedFunctionsInReferenceOrder_true (Behavior#afterExitScope)
    fn after_exit_scope(&mut self, _t: &mut NodeTraversal<'_>, _reference_map: &dyn ReferenceMap) {}
}

// port: ReferenceCollectorTest#testBehavior_hoistedFunctionsInReferenceOrder_true
#[test]
fn test_behavior_hoisted_functions_in_reference_order_true() {
    let behavior = VisitedReferenceNamesBehavior::default();
    let visited_reference_names = behavior.visited_reference_names.clone();
    let mut test = set_up(behavior);
    test.collect_hoisted_functions_in_reference_order = true;
    test_behavior(
        &mut test,
        "f();
var z = 1;
function f() {
  return z;
}
",
    );

    // When collectHoistedFunctionsInReferenceOrder is true:
    // Visiting call `f()` jumps to hoisted function `f()` via maybeJumpToHoistedFunction.
    // The declaration reference for `f` is added directly and does not pass through afterVisitNode.
    // The body of `f()` is traversed, visiting `z` inside `return z;`.
    // Then afterVisitNode runs for the referencing `f()` call.
    // Then `var z = 1;` is visited.
    // The declaration `function f() { ... }` is skipped in the outer traversal.
    assert_eq!(*visited_reference_names.borrow(), ["z", "f", "z"]);
}

// port: ReferenceCollectorTest#testBehavior_hoistedFunctionsInReferenceOrder_false
#[test]
fn test_behavior_hoisted_functions_in_reference_order_false() {
    let behavior = VisitedReferenceNamesBehavior::default();
    let visited_reference_names = behavior.visited_reference_names.clone();
    let mut test = set_up(behavior);
    test.collect_hoisted_functions_in_reference_order = false;
    test_behavior(
        &mut test,
        "f();
var z = 1;
function f() {
  return z;
}
",
    );

    // When collectHoistedFunctionsInReferenceOrder is false:
    // Traversal proceeds in pure AST order without out-of-order jumping:
    // 1. `f();` -> afterVisitNode for `f`
    // 2. `var z = 1;` -> afterVisitNode for `z`
    // 3. `function f() { return z; }` -> afterVisitNode for `f`, then afterVisitNode for `z`
    assert_eq!(*visited_reference_names.borrow(), ["f", "z", "f", "z"]);
}

/// The anonymous `Behavior` of `testBehavior_afterVisitNode_nullReferenceWhenRejectedByVarFilter`.
#[derive(Clone, Default)]
struct NullReferenceBehavior {
    non_null_reference_names: Rc<RefCell<Vec<String>>>,
    filtered_out_names: Rc<RefCell<Vec<String>>>,
}
impl Behavior for NullReferenceBehavior {
    // port: ReferenceCollectorTest#testBehavior_afterVisitNode_nullReferenceWhenRejectedByVarFilter (Behavior#afterVisitNode)
    fn after_visit_node(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
        added_reference: Option<&Reference>,
    ) {
        let ast = t.get_compiler();
        if n.is_name(ast) {
            if added_reference.is_some() {
                self.non_null_reference_names
                    .borrow_mut()
                    .push(n.get_string(ast).to_string());
            } else {
                self.filtered_out_names
                    .borrow_mut()
                    .push(n.get_string(ast).to_string());
            }
        }
    }
    // port: ReferenceCollectorTest#testBehavior_afterVisitNode_nullReferenceWhenRejectedByVarFilter (Behavior#afterExitScope)
    fn after_exit_scope(&mut self, _t: &mut NodeTraversal<'_>, _reference_map: &dyn ReferenceMap) {}
}

// port: ReferenceCollectorTest#testBehavior_afterVisitNode_nullReferenceWhenRejectedByVarFilter
#[test]
fn test_behavior_after_visit_node_null_reference_when_rejected_by_var_filter() {
    let behavior = NullReferenceBehavior::default();
    let non_null_reference_names = behavior.non_null_reference_names.clone();
    let filtered_out_names = behavior.filtered_out_names.clone();
    let mut test = set_up(behavior);
    // Filter out variable "z"
    test.var_filter = Rc::new(|compiler, v| v.get_name(compiler) != "z");
    test.collect_hoisted_functions_in_reference_order = false;
    test_behavior(
        &mut test,
        "var x = 1;
var z = 2;
",
    );

    assert_eq!(*non_null_reference_names.borrow(), ["x"]);
    assert_eq!(*filtered_out_names.borrow(), ["z"]);
}
