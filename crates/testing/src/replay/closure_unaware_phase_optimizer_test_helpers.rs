/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/ClosureUnawarePhaseOptimizerTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper
//! `oracle/replay/helpers/.../ClosureUnawarePhaseOptimizerTest_Helpers.java`, itself copied from
//! ClosureUnawarePhaseOptimizerTest.java: the holder fields `shadowNodes` and `aditionalPasses`,
//! the constants, the inner class `ChangedScopeNodesIterativePass`, `getProcessor` and the two
//! test-method prologues.
use crate::{
    replay::{
        native_registry::NativePhaseOptimizer,
        replay_dsl::{Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    pass_factory::PassFactory,
    phase_optimizer::PhaseOptimizer,
    validity_check::ValidityCheck,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_state, node::Ast, node::NodeId};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

const HOLDER: &str = "com.google.javascript.jscomp.ClosureUnawarePhaseOptimizerTest_Helpers";
const ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE: &str =
    "arbitraryNumericChangeClosureUnawareCode";
const ARBITRARY_NUMERIC_CHANGE_MAIN_AST: &str = "arbitraryNumericChangeMainAST";

/// `List<Node> shadowNodes`, shared between the holder and the passes its factories create.
type ShadowNodes = Arc<Mutex<Vec<NodeId>>>;

/// The helper holder (one test instance).
pub struct ClosureUnawarePhaseOptimizerTestHelpers {
    shadow_nodes: ShadowNodes,
    aditional_passes: Vec<PassFactory>,
}

impl NativeObject for ClosureUnawarePhaseOptimizerTestHelpers {
    // port: ReplayDsl#invoke (runtime declaring class)
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        let shadow_nodes = self.shadow_nodes.lock().expect("shadowNodes");
        fields.insert(
            "shadowNodes".into(),
            DslValue::List(shadow_nodes.iter().map(|n| DslValue::Node(*n)).collect()),
        );
        Ok(fields)
    }
    // port: ReplayDsl#invoke (receiver cast)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The post-order callback of `ChangedScopeNodesIterativePass#process`.
struct NumericChangeCallback;

impl Callback for NumericChangeCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }
    // port: ClosureUnawarePhaseOptimizerTest_Helpers.ChangedScopeNodesIterativePass#process (visit)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        if node.is_number(compiler) {
            let value = node.get_double(compiler);
            if value <= 3.0 {
                node.set_double(compiler, 4.0);
                compiler.report_change_to_enclosing_scope(node);
            }
        }
    }
}

/// `private final class ChangedScopeNodesIterativePass implements CompilerPass`.
struct ChangedScopeNodesIterativePass {
    pass_name: String,
    shadow_nodes: ShadowNodes,
}

impl ChangedScopeNodesIterativePass {
    // port: ClosureUnawarePhaseOptimizerTest_Helpers.ChangedScopeNodesIterativePass#ChangedScopeNodesIterativePass
    fn new(pass_name: &str, shadow_nodes: ShadowNodes) -> Self {
        Self {
            pass_name: pass_name.to_string(),
            shadow_nodes,
        }
    }
}

impl CompilerPass for ChangedScopeNodesIterativePass {
    // port: ClosureUnawarePhaseOptimizerTest_Helpers.ChangedScopeNodesIterativePass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut cb = NumericChangeCallback;
        let mut changed_scope_nodes = compiler
            .get_change_tracker()
            .get_changed_scope_nodes_for_pass(&self.pass_name);
        while changed_scope_nodes
            .as_ref()
            .is_none_or(|nodes| !nodes.is_empty())
        {
            // changedScopeNodes is only null if this is the first run of this pass.
            if let Some(nodes) = &changed_scope_nodes {
                NodeTraversal::traverse_scope_roots(compiler, nodes, &mut cb, false);
            } else if self.pass_name == ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE {
                let shadow_nodes = self.shadow_nodes.lock().expect("shadowNodes").clone();
                NodeTraversal::traverse_scope_roots(compiler, &shadow_nodes, &mut cb, false);
            } else {
                NodeTraversal::traverse(compiler, root, &mut cb);
            }
            changed_scope_nodes = compiler
                .get_change_tracker()
                .get_changed_scope_nodes_for_pass(&self.pass_name);
        }
    }
}

/// The lambda pass of the prologues that bumps NUMBER nodes 1 and 2 by one under `root` (the
/// main AST) or under every gathered shadow node.
// port: ClosureUnawarePhaseOptimizerTest_Helpers#testClosureUnawareCodePassModifiesMainAST_incorrectly (anonymous CompilerPass#process)
fn increment_one_and_two(compiler: &mut AbstractCompiler, root: NodeId) {
    let (tracker, ast) = compiler.get_change_tracker_and_ast();
    NodeUtil::visit_pre_order(ast, root, &mut |ast: &mut Ast, node: NodeId| {
        if node.is_number(ast) {
            let value = node.get_double(ast);
            if value == 1.0 || value == 2.0 {
                node.set_double(ast, value + 1.0);
                tracker.report_change_to_enclosing_scope(ast, node);
            }
        }
    });
}

impl ClosureUnawarePhaseOptimizerTestHelpers {
    // port: ClosureUnawarePhaseOptimizerTest_Helpers#getProcessor
    fn get_processor(&self, compiler: &mut AbstractCompiler) -> DslValue {
        let mut phaseopt = PhaseOptimizer::new(compiler, None);
        let mut passes: Vec<PassFactory> = vec![];
        let shadow_nodes = self.shadow_nodes.clone();
        passes.push(
            PassFactory::builder()
                .set_name("gatherShadowNodes")
                .set_internal_factory(Arc::new(move |_c: &mut AbstractCompiler| {
                    let shadow_nodes = shadow_nodes.clone();
                    let pass: Box<dyn CompilerPass> = Box::new(
                        move |compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId| {
                            NodeUtil::visit_pre_order(
                                compiler,
                                root,
                                &mut |ast: &mut Ast, node: NodeId| {
                                    let shadow = node.get_closure_unaware_shadow(ast);
                                    if let Some(shadow) = shadow {
                                        /*
                                         * shadow is a ROOT node that follows the pattern:
                                         * ROOT -> SCRIPT -> EXPR_RESULT -> CALL -> FUNCTION.
                                         */
                                        let expr_result =
                                            shadow.get_first_first_child(ast).expect("EXPR_RESULT");
                                        check_state!(
                                            expr_result.is_expr_result(ast),
                                            "%s",
                                            expr_result.to_string(ast)
                                        );
                                        let shadow_function = expr_result
                                            .get_first_child(ast)
                                            .and_then(|call| call.get_second_child(ast))
                                            .expect("FUNCTION");
                                        check_state!(
                                            shadow_function.is_function(ast),
                                            "%s",
                                            shadow_function.to_string(ast)
                                        );
                                        shadow_nodes
                                            .lock()
                                            .expect("shadowNodes")
                                            .push(shadow_function);
                                    }
                                },
                            );
                        },
                    );
                    pass
                }))
                .build(),
        );
        passes.extend(self.aditional_passes.iter().cloned());
        phaseopt.consume(passes);
        phaseopt.set_validity_check(
            compiler,
            PassFactory::builder()
                .set_name("validityCheck")
                .set_run_in_fixed_point_loop(true)
                .set_internal_factory(Arc::new(|c: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(ValidityCheck::new(c));
                    pass
                }))
                .build(),
        );
        DslValue::Native(Rc::new(RefCell::new(NativePhaseOptimizer(phaseopt))))
    }

    // port: ClosureUnawarePhaseOptimizerTest_Helpers#testClosureUnawareCodePassModifiesMainAST_incorrectly
    fn test_closure_unaware_code_pass_modifies_main_ast_incorrectly(&mut self) {
        let shadow_nodes = self.shadow_nodes.clone();
        self.aditional_passes.push(
            PassFactory::builder()
                .set_name(ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE)
                .set_internal_factory(Arc::new(move |_c: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(ChangedScopeNodesIterativePass::new(
                            ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE,
                            shadow_nodes.clone(),
                        ));
                    pass
                }))
                .set_run_in_fixed_point_loop(true)
                .build(),
        );

        self.aditional_passes.push(
            PassFactory::builder()
                .set_name(ARBITRARY_NUMERIC_CHANGE_MAIN_AST)
                .set_internal_factory(Arc::new(|_c: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> = Box::new(
                        |compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId| {
                            increment_one_and_two(compiler, root);
                        },
                    );
                    pass
                }))
                .set_run_in_fixed_point_loop(true)
                .build(),
        );
    }

    // port: ClosureUnawarePhaseOptimizerTest_Helpers#testMainASTPassModifiesClosureUnawareCode_incorrectly
    fn test_main_ast_pass_modifies_closure_unaware_code_incorrectly(&mut self) {
        let shadow_nodes = self.shadow_nodes.clone();
        self.aditional_passes.push(
            PassFactory::builder()
                .set_name(ARBITRARY_NUMERIC_CHANGE_MAIN_AST)
                .set_internal_factory(Arc::new(move |_c: &mut AbstractCompiler| {
                    let pass: Box<dyn CompilerPass> =
                        Box::new(ChangedScopeNodesIterativePass::new(
                            ARBITRARY_NUMERIC_CHANGE_MAIN_AST,
                            shadow_nodes.clone(),
                        ));
                    pass
                }))
                .set_run_in_fixed_point_loop(true)
                .build(),
        );

        let shadow_nodes = self.shadow_nodes.clone();
        self.aditional_passes.push(
            PassFactory::builder()
                .set_name(ARBITRARY_NUMERIC_CHANGE_CLOSURE_UNAWARE_CODE)
                .set_internal_factory(Arc::new(move |_c: &mut AbstractCompiler| {
                    let shadow_nodes = shadow_nodes.clone();
                    let pass: Box<dyn CompilerPass> = Box::new(
                        move |compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId| {
                            let shadow_roots = shadow_nodes.lock().expect("shadowNodes").clone();
                            for shadow_root in shadow_roots {
                                increment_one_and_two(compiler, shadow_root);
                            }
                        },
                    );
                    pass
                }))
                .set_run_in_fixed_point_loop(true)
                .build(),
        );
    }
}

// port: ClosureUnawarePhaseOptimizerTest_Helpers#ClosureUnawarePhaseOptimizerTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ClosureUnawarePhaseOptimizerTestHelpers {
            shadow_nodes: Arc::new(Mutex::new(Vec::new())),
            aditional_passes: Vec::new(),
        },
    ))))
}

// port: ReplayDsl#invoke (receiver cast)
fn with_holder<R>(
    receiver: &DslValue,
    f: impl FnOnce(&mut ClosureUnawarePhaseOptimizerTestHelpers) -> R,
) -> Result<R, Throwable> {
    let DslValue::Native(receiver) = receiver else {
        return Err(bad());
    };
    let mut receiver = receiver.borrow_mut();
    let holder = receiver
        .as_any_mut()
        .downcast_mut::<ClosureUnawarePhaseOptimizerTestHelpers>()
        .ok_or_else(bad)?;
    Ok(f(holder))
}

// port: ClosureUnawarePhaseOptimizerTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [receiver, DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    with_holder(receiver, |holder| {
        holder.get_processor(&mut compiler.borrow_mut())
    })
}

// port: ClosureUnawarePhaseOptimizerTest_Helpers#testClosureUnawareCodePassModifiesMainAST_incorrectly
pub fn test_closure_unaware_code_pass_modifies_main_ast_incorrectly(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [receiver] = args.as_slice() else {
        return Err(bad());
    };
    with_holder(receiver, |holder| {
        holder.test_closure_unaware_code_pass_modifies_main_ast_incorrectly()
    })?;
    // return this;
    Ok(receiver.clone())
}

// port: ClosureUnawarePhaseOptimizerTest_Helpers#testMainASTPassModifiesClosureUnawareCode_incorrectly
pub fn test_main_ast_pass_modifies_closure_unaware_code_incorrectly(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [receiver] = args.as_slice() else {
        return Err(bad());
    };
    with_holder(receiver, |holder| {
        holder.test_main_ast_pass_modifies_closure_unaware_code_incorrectly()
    })?;
    // return this;
    Ok(receiver.clone())
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
