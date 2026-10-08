/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/TypeInferencePass.java,
//   src/com/google/javascript/jscomp/TypedScopeCreator.java.

//! A compiler pass to run the type inference analysis.
//!
//! Java's `TypeInferencePass`. The Java fields `compiler` and `registry` are not stored (DESIGN
//! §6). Java shares one `TypedScopeCreator` between this pass, its NodeTraversals and every
//! TypeInference it runs; here the pass owns it in a `RefCell` (the traversal creates scopes
//! through `SharedTypedScopeCreator`, the scope callbacks borrow it between scope creations) and
//! hands it back with `into_scope_creator`.
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_scope::AbstractScopeHandle,
    coding_convention::AssertionFunctionLookup,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, LinearFlowState},
    graph::{adjacency_graph::AdjacencyGraph, annotatable::Annotatable, graph_node::GraphNode},
    node_traversal::{AbstractScopedCallback, NodeTraversal},
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    type_inference::TypeInference,
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{JSTypeNative, TypeId};
use closure_rhino::{check_state, node::NodeId, token::Token};
use indexmap::IndexMap;
use std::{cell::RefCell, sync::Arc};

/// Java's `LinkedHashMap<Integer, HashMultiset<Token>>`: (stepCount, Token) -> populationCount.
type StepCountHistogram = IndexMap<i32, IndexMap<Token, i32>>;

pub struct TypeInferencePass {
    reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
    top_scope: Option<TypedScope>,
    scope_creator: RefCell<TypedScopeCreator>,
    assertion_function_lookup: AssertionFunctionLookup,

    // (stepCount, Token) -> populationCount
    step_count_histogram: Option<RefCell<StepCountHistogram>>,
}

impl TypeInferencePass {
    // port: TypeInferencePass#TypeInferencePass
    pub fn new(
        compiler: &mut AbstractCompiler,
        reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
        scope_creator: TypedScopeCreator,
    ) -> Self {
        let assertion_function_lookup =
            AssertionFunctionLookup::of(compiler.get_coding_convention().get_assertion_functions());
        let step_count_histogram = if compiler.is_debug_logging_enabled() {
            Some(RefCell::new(IndexMap::new()))
        } else {
            None
        };
        Self {
            reverse_interpreter,
            top_scope: None,
            scope_creator: RefCell::new(scope_creator),
            assertion_function_lookup,
            step_count_histogram,
        }
    }

    /// Rust-only: returns the scope creator Java shares with the compiler.
    pub fn into_scope_creator(self) -> TypedScopeCreator {
        self.scope_creator.into_inner()
    }

    // port: TypeInferencePass#inferAllScopes
    /// Execute type inference running over part of the scope tree.
    ///
    /// Returns the top scope, either newly created, or patched by this inference.
    pub fn infer_all_scopes(
        &mut self,
        compiler: &mut AbstractCompiler,
        inference_root: NodeId,
    ) -> TypedScope {
        // Type analysis happens in two major phases.
        // 1) Finding all the symbols.
        // 2) Propagating all the inferred types.
        //
        // The order of this analysis is non-obvious. In a complete inference
        // system, we may need to backtrack arbitrarily far. But the compile-time
        // costs would be unacceptable.
        //
        // We do one pass where we do typed scope creation for all scopes
        // in pre-order.
        //
        // Then we do a second pass where we do all type inference
        // (type propagation) in pre-order.
        //
        // We use a memoized scope creator so that we never create a scope
        // more than once.
        //
        // This will allow us to handle cases like:
        // var ns = {};
        // (function() { /** JSDoc */ ns.method = function() {}; })();
        // ns.method();
        // In this code, we need to build the symbol table for the inner scope in
        // order to propagate the type of ns.method in the outer scope.
        {
            let mut closer = compiler
                .get_type_registry()
                .get_resolver()
                .open_for_definition();
            check_state!(inference_root.is_root(compiler));
            check_state!(inference_root.get_parent(compiler).is_none());
            check_state!(self.top_scope.is_none());
            let top_scope =
                self.scope_creator
                    .borrow_mut()
                    .create_scope(compiler, inference_root, None);
            self.top_scope = Some(top_scope);

            {
                let mut scope_creator = SharedTypedScopeCreator(&self.scope_creator);
                // FirstScopeBuildingCallback
                let mut callback = first_scope_building_callback();
                NodeTraversal::builder()
                    .set_compiler(compiler)
                    .set_callback(&mut callback)
                    .set_scope_creator(&mut scope_creator)
                    .build()
                    .traverse_with_scope(inference_root, top_scope);
            }
            self.scope_creator
                .borrow()
                .resolve_weak_imports_pre_resolution(compiler);
            // try-with-resources: the closer closes when the block ends.
            let (reg, ast) = compiler.get_type_registry_and_ast();
            closer.close(reg, ast);
        }
        self.scope_creator.borrow_mut().finish_and_freeze(compiler);

        {
            let top_scope = self.top_scope.unwrap();
            let mut scope_creator = SharedTypedScopeCreator(&self.scope_creator);
            let this: &TypeInferencePass = self;
            let mut callback = AbstractScopedCallback::with_scope_callbacks(
                |_t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>| {
                    // Do nothing
                },
                // port: TypeInferencePass.SecondScopeBuildingCallback#enterScope
                |t: &mut NodeTraversal<'_>| {
                    // Only infer the entry root, rather than the scope root.
                    // This ensures that incremental compilation only touches the root
                    // that's been swapped out.
                    let scope = t.get_typed_scope();
                    let compiler = t.get_compiler();
                    if scope.is_cfg_root_scope(compiler) && !scope.is_module_scope(compiler) {
                        // ignore scopes that don't have their own CFGs and module scopes, which
                        // are visited as if they were a regular script.
                        let current_node = t.get_current_node().unwrap();
                        this.infer_scope(t.get_compiler(), current_node, scope);
                    }
                },
                |_t: &mut NodeTraversal<'_>| {},
            );
            NodeTraversal::builder()
                .set_compiler(compiler)
                .set_callback(&mut callback)
                .set_scope_creator(&mut scope_creator)
                .build()
                .traverse_with_scope(inference_root, top_scope);
        }

        // Normalize TypedVars to have the '?' type instead of null after inference is complete.
        // This currently cannot be done any earlier because it breaks inference of variables
        // assigned in local scopes.
        // TODO(b/149843534): this should be a crash instead.
        let unknown_type: TypeId = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let all_symbols = self.scope_creator.borrow().get_all_symbols(compiler);
        for var in all_symbols {
            if var.get_type(compiler).is_none() {
                var.set_type(compiler, Some(unknown_type));
            }
        }

        if let Some(step_count_histogram) = &self.step_count_histogram {
            let step_count_histogram = step_count_histogram.borrow();
            let mut histogram =
                compiler.create_or_reopen_log("TypeInferencePass", "step_histogram.log", &[]);
            histogram.log(&mut || "step_count token population".to_owned());

            let mut totals = [0, 0];
            let mut step_counts: Vec<i32> = step_count_histogram.keys().copied().collect();
            step_counts.sort_by(|a, b| b.cmp(a));
            for step_count in step_counts {
                // Java's HashMultiset iterates in hash order; the entries here keep insertion
                // order before the same stable sort by count.
                let mut entries: Vec<(Token, i32)> = step_count_histogram[&step_count]
                    .iter()
                    .map(|(&token, &count)| (token, count))
                    .collect();
                entries.sort_by_key(|&(_, count)| count);
                for (token, count) in entries {
                    totals[0] += step_count * count;
                    totals[1] += count;
                    histogram.log_format(format_args!("{} {} {}", step_count, token, count));
                }
            }
            histogram.log_format(format_args!("{} TOTAL {}", totals[0], totals[1]));
        }

        self.top_scope.unwrap()
    }

    // port: TypeInferencePass#inferScope
    fn infer_scope(&self, compiler: &mut AbstractCompiler, n: NodeId, scope: TypedScope) {
        let cfg = Self::compute_cfg(compiler, n);
        let mut scope_creator = self.scope_creator.borrow_mut();
        let mut type_inference = TypeInference::new(
            compiler,
            cfg,
            Arc::clone(&self.reverse_interpreter),
            scope,
            &mut scope_creator,
            &self.assertion_function_lookup,
        );
        type_inference.analyze(compiler);

        if let Some(step_count_histogram) = &self.step_count_histogram {
            let cfg = type_inference.get_cfg();
            let mut step_count_histogram = step_count_histogram.borrow_mut();
            for node in cfg.get_nodes() {
                if node == cfg.get_implicit_return() {
                    continue;
                }

                let state = node
                    .get_annotation_as::<LinearFlowState<crate::type_inference::FlowScopeLattice>>(
                        cfg,
                    )
                    .unwrap();
                let token = node.get_value(cfg).unwrap().get_token(compiler);
                *step_count_histogram
                    .entry(state.get_step_count())
                    .or_default()
                    .entry(token)
                    .or_insert(0) += 1;
            }
        }
    }

    // port: TypeInferencePass#computeCfg
    fn compute_cfg(compiler: &mut AbstractCompiler, n: NodeId) -> ControlFlowGraph<NodeId> {
        ControlFlowAnalysis::builder()
            .set_cfg_root(n)
            .set_include_edge_annotations(true)
            .compute_cfg(compiler)
    }
}

// port: TypeInferencePass.FirstScopeBuildingCallback
fn first_scope_building_callback()
-> AbstractScopedCallback<'static, impl FnMut(&mut NodeTraversal<'_>, NodeId, Option<NodeId>)> {
    AbstractScopedCallback::with_scope_callbacks(
        // port: TypeInferencePass.FirstScopeBuildingCallback#visit
        |_t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>| {
            // Do nothing
        },
        // port: TypeInferencePass.FirstScopeBuildingCallback#enterScope
        |t: &mut NodeTraversal<'_>| {
            t.get_typed_scope();
        },
        |_t: &mut NodeTraversal<'_>| {},
    )
}

/// Rust-only: the pass's shared `TypedScopeCreator` as the NodeTraversal's scope creator
/// (Java hands the same object to `NodeTraversal.Builder#setScopeCreator`).
pub struct SharedTypedScopeCreator<'a>(pub &'a RefCell<TypedScopeCreator>);

impl ScopeCreator for SharedTypedScopeCreator<'_> {
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        let parent = parent.map(AbstractScopeHandle::from);
        self.create_abstract_scope(compiler, n, parent)
            .untyped(compiler)
    }

    // port: TypedScopeCreator#createScope(Node,AbstractScope)
    fn create_abstract_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<AbstractScopeHandle>,
    ) -> AbstractScopeHandle {
        // checkArgument(parent == null || parent instanceof TypedScope)
        let parent = parent.map(|parent| parent.typed(compiler));
        AbstractScopeHandle::Typed(self.0.borrow_mut().create_scope(compiler, n, parent))
    }
}
