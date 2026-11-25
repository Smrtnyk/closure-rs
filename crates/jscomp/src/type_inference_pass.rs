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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
    data_flow_analysis::DataFlowAnalysis,
    graph::{adjacency_graph::AdjacencyGraph, graph_node::GraphNode},
    node_traversal::{Callback, NodeTraversal, ScopedCallback, TypedScopeEntry, WalkBranch},
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    source_file::declarations,
    type_inference::TypeInference,
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{JSTypeNative, TypeId};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    node::{NodeId, SnapshotUse},
    token::Token,
};
use std::{cell::RefCell, sync::Arc};

/// Java's `LinkedHashMap<Integer, HashMultiset<Token>>`: (stepCount, Token) -> populationCount.
type StepCountHistogram = IndexMap<i32, IndexMap<Token, i32>>;

pub struct TypeInferencePass {
    reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
    use_linear_flow: bool,
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
            Some(RefCell::new(IndexMap::<_, _>::default()))
        } else {
            None
        };
        let use_linear_flow = step_count_histogram.is_none()
            && compiler.owns_reverse_abstract_interpreter(&reverse_interpreter);
        Self {
            reverse_interpreter,
            use_linear_flow,
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
        let mut cfg_scopes = Vec::new();
        let mut deferred_extern_stubs = Vec::new();
        let omit_empty_extern_stub_scopes =
            self.scope_creator.borrow().omit_empty_extern_stub_scopes()
                && self.step_count_histogram.is_none();
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
                let mut callback = FirstScopeBuildingCallback {
                    cfg_scopes: &mut cfg_scopes,
                    deferred_extern_stubs: &mut deferred_extern_stubs,
                    omit_empty_extern_stub_scopes,
                };
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

        // Rust-only (performance divergence): assign resolved parameter types directly on
        // unchanged bundled extern stubs whose empty function/block TypedScopes were omitted.
        for stub in deferred_extern_stubs {
            TypeInference::infer_extern_stub_parameter_nodes_only(compiler, stub);
        }

        // port: TypeInferencePass.SecondScopeBuildingCallback#enterScope
        // Rust-only (performance divergence): type resolution leaves the scope roots and AST
        // structure in place. Run the CFGs in their observed preorder after types are frozen.
        NodeTraversal::replay_typed_scopes(
            compiler,
            inference_root,
            cfg_scopes,
            |compiler, node, scope| self.infer_scope(compiler, node, scope),
        );

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
        let mut scope_creator = self.scope_creator.borrow_mut();
        if self.use_linear_flow
            && n == scope.get_root_node(compiler)
            && let Some((body, statements)) = ControlFlowAnalysis::linear_function(compiler, n)
        {
            TypeInference::infer_linear_function(
                compiler,
                Arc::clone(&self.reverse_interpreter),
                scope,
                &mut scope_creator,
                &self.assertion_function_lookup,
                body,
                &statements,
            );
            return;
        }
        let (cfg, linear_scripts) =
            if self.use_linear_flow && n.is_root(compiler) && n.get_parent(compiler).is_none() {
                let (cfg, scripts) = ControlFlowAnalysis::global_inference_cfg(compiler, n);
                (cfg, Some(scripts))
            } else {
                (Self::compute_cfg(compiler, n), None)
            };
        let mut type_inference = TypeInference::new(
            compiler,
            cfg,
            Arc::clone(&self.reverse_interpreter),
            scope,
            &mut scope_creator,
            &self.assertion_function_lookup,
        );
        type_inference.linear_scripts = linear_scripts;
        type_inference.analyze_indexed(compiler);

        if let Some(step_count_histogram) = &self.step_count_histogram {
            let cfg = type_inference.get_cfg();
            let mut step_count_histogram = step_count_histogram.borrow_mut();
            for node in cfg.get_nodes() {
                if node == cfg.get_implicit_return() {
                    continue;
                }

                let state = type_inference.flow_state(node);
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
struct FirstScopeBuildingCallback<'a> {
    cfg_scopes: &'a mut Vec<TypedScopeEntry>,
    deferred_extern_stubs: &'a mut Vec<NodeId>,
    omit_empty_extern_stub_scopes: bool,
}

impl Callback for FirstScopeBuildingCallback<'_> {
    fn should_traverse(&mut self, _: &mut NodeTraversal<'_>, _: NodeId, _: Option<NodeId>) -> bool {
        true
    }

    fn should_traverse_script(
        &mut self,
        t: &mut NodeTraversal<'_>,
        script: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let Some((facts, suffix)) = t.bundled_script_prefix(script, SnapshotUse::SyntaxAndNames)
        else {
            return true;
        };
        let mut child = script.get_first_child(t);
        for &record in facts.declarations {
            let statement = child.expect("recorded extern statement");
            child = statement.get_next(t);
            if record == declarations::OTHER {
                self.walk_branch(t, statement, Some(script));
            } else if record & declarations::HAS_FUNCTION != 0 {
                // Only the stub and its body introduce scopes in this declaration.
                let stub = declarations::initializer(t, statement, record).unwrap();
                if self.omit_empty_extern_stub_scopes
                    && declarations::is_plain_stub_no_scope(record)
                    && !stub.get_parent(t).is_some_and(|p| {
                        matches!(
                            p.get_token(t),
                            Token::FUNCTION | Token::FOR_OF | Token::FOR_AWAIT_OF
                        )
                    })
                {
                    self.deferred_extern_stubs.push(stub);
                } else {
                    self.walk_branch(t, stub, stub.get_parent(t));
                }
            }
        }
        debug_assert_eq!(child, suffix);
        t.traverse_script_suffix(script, parent, suffix, self);
        false
    }

    // port: TypeInferencePass.FirstScopeBuildingCallback#visit
    fn visit(&mut self, _: &mut NodeTraversal<'_>, _: NodeId, _: Option<NodeId>) {}

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for FirstScopeBuildingCallback<'_> {
    // port: TypeInferencePass.FirstScopeBuildingCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let scope = t.get_typed_scope();
        let compiler = t.get_compiler();
        if scope.is_cfg_root_scope(compiler) && !scope.is_module_scope(compiler) {
            self.cfg_scopes.push(t.typed_scope_entry(scope));
        }
    }

    fn exit_scope(&mut self, _: &mut NodeTraversal<'_>) {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Compiler, flow_scope::FlowScope, typed_var::TypedVar};
    use closure_jstype::prelude::JSType;
    use closure_rhino::{ir::IR, outcome::Outcome};
    use std::sync::Mutex;

    struct RecordingInterpreter {
        inner: Arc<dyn ReverseAbstractInterpreter>,
        calls: Mutex<Vec<(NodeId, Outcome)>>,
    }

    impl ReverseAbstractInterpreter for RecordingInterpreter {
        fn get_preciser_scope_knowing_condition_outcome(
            &self,
            compiler: &mut AbstractCompiler,
            condition: NodeId,
            scope: Arc<dyn FlowScope>,
            outcome: Outcome,
        ) -> Arc<dyn FlowScope> {
            self.calls.lock().unwrap().push((condition, outcome));
            self.inner
                .get_preciser_scope_knowing_condition_outcome(compiler, condition, scope, outcome)
        }
    }

    #[derive(Debug, PartialEq)]
    struct InferenceResult {
        nodes: Vec<(NodeId, Option<TypeId>, Option<String>)>,
        slots: Vec<(TypedVar, TypedScope, Option<TypeId>, String, bool)>,
        diagnostics: Vec<String>,
    }

    fn infer_with_recording(record: bool) -> InferenceResult {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code(
            r#"
            class C {}
            /** @param {number} declared */ function empty(declared) {}
            (function (arg, {n = 3} = {}, ...rest) {
                function nested(x = arg ? 1 : 2) { return x && n; }
                var unbound;
                let result = nested() || arg;
                rest[0] = result;
                C.prototype = {};
                return unbound ?? result;
            })('abc', {n: 5}, 7);
        "#,
        );
        let declarations = compiler.parse_synthetic_code(
            "declarations",
            "var value = 1; function hoisted() {} value = 'narrowed';",
        );
        let prefix = compiler.parse_synthetic_code(
            "prefix",
            "var result = value && value.length; result = result || 3;",
        );
        let externs = IR::root(&mut compiler, &[declarations]);
        let sources = IR::root(&mut compiler, &[prefix, script]);
        let root = IR::root(&mut compiler, &[externs, sources]);
        let native = compiler.get_reverse_abstract_interpreter();
        let recording = Arc::new(RecordingInterpreter {
            inner: Arc::clone(&native),
            calls: Mutex::new(Vec::new()),
        });
        let interpreter: Arc<dyn ReverseAbstractInterpreter> =
            if record { recording.clone() } else { native };
        let creator = TypedScopeCreator::new(&mut compiler);
        let mut pass = TypeInferencePass::new(&mut compiler, interpreter, creator);
        assert_eq!(pass.use_linear_flow, !record);
        pass.infer_all_scopes(&mut compiler, root);
        if record {
            assert!(!recording.calls.lock().unwrap().is_empty());
        }

        results(compiler, root, pass)
    }

    fn results(mut compiler: Compiler, root: NodeId, pass: TypeInferencePass) -> InferenceResult {
        let mut nodes = Vec::new();
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            let ty = node.get_jstype(&compiler);
            let name = ty.map(|ty| {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                ty.to_string(reg, ast)
            });
            nodes.push((node, ty, name));
            pending.extend(node.children(&compiler));
        }
        let slots = pass
            .into_scope_creator()
            .get_all_symbols(&compiler)
            .into_iter()
            .map(|v| {
                (
                    v,
                    v.get_scope(&compiler),
                    v.get_type(&compiler),
                    v.to_string(&mut compiler),
                    v.is_type_inferred(&compiler),
                )
            })
            .collect();
        let diagnostics = compiler
            .get_errors()
            .into_iter()
            .chain(compiler.get_warnings())
            .map(|e| format!("{} {:?} {}", e.type_.key, e.node, e))
            .collect();
        InferenceResult {
            nodes,
            slots,
            diagnostics,
        }
    }

    #[test]
    fn bundled_declarations_match_live_scope_and_type_construction() {
        use crate::{
            compiler_options::{CompilerOptions, LanguageMode},
            source_file::{SourceFile, bundled_script::BundledScript},
        };
        use closure_resources::compiler_options::Environment;
        use closure_rhino::{node::SnapshotUse, static_source_file::StaticSourceFile};

        #[derive(Clone, Copy, PartialEq)]
        enum Edit {
            Append,
            JsDoc,
            BodyAndProperty,
            Transplant,
        }

        fn run(bundled: bool, edit: Edit) -> InferenceResult {
            let mut compiler = Compiler::new();
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            let file = SourceFile::bundled_externs(Environment::BROWSER)
                .into_iter()
                .find(|f| f.get_name().ends_with("/w3c_abort.js"))
                .unwrap();
            let name = file.get_name().to_owned();
            let file = if bundled {
                file
            } else {
                SourceFile::from_code(&name, file.get_code().unwrap())
            };
            let src = SourceFile::from_code(
                "app.js",
                "/** @constructor */ function SourceOnly() {}
                 /** @typedef {number} */ var SourceNumber;
                 var result = AbortSignal.later(new SourceOnly());",
            );
            compiler.init(&[Arc::new(file)], &[Arc::new(src)], options);
            let root = compiler.parse_inputs().unwrap();
            let script = compiler.get_script_node(&name).unwrap();
            let extra = compiler.parse_synthetic_code(
                "appended",
                "/** @type {SourceNumber} */ AbortSignal.extra;
                 /** @param {!SourceOnly} value
                  * @return {!SourceOnly} */
                 AbortSignal.later = function(value) {};
                 function appended(arg) { function nested() { return arg; } return nested(); }",
            );
            while let Some(n) = extra.get_first_child(&compiler) {
                n.detach(&mut compiler);
                script.add_child_to_back(&mut compiler, n);
            }
            let prop = script
                .children(&compiler)
                .find_map(|n| {
                    let expr = n.get_first_child(&compiler)?;
                    expr.matches_qualified_name(&compiler, "AbortSignal.prototype.aborted")
                        .then_some(expr)
                })
                .unwrap();
            if matches!(edit, Edit::JsDoc | Edit::BodyAndProperty) {
                let source_typedef = compiler
                    .get_script_node("app.js")
                    .unwrap()
                    .get_second_child(&compiler)
                    .unwrap()
                    .get_jsdoc_info(&compiler);
                assert!(source_typedef.as_ref().unwrap().has_typedef_type());
                prop.set_jsdoc_info(&mut compiler, source_typedef);
            }
            if edit == Edit::BodyAndProperty {
                let stub = script
                    .children(&compiler)
                    .find_map(|n| {
                        let assign = n.get_first_child(&compiler)?;
                        let lhs = assign.get_first_child(&compiler)?;
                        lhs.matches_qualified_name(&compiler, "AbortSignal.timeout")
                            .then(|| assign.get_second_child(&compiler).unwrap())
                    })
                    .unwrap();
                let body = stub.get_last_child(&compiler).unwrap();
                let ret = IR::return_node(&mut compiler);
                body.add_child_to_back(&mut compiler, ret);
                prop.set_string(&mut compiler, "alternate");
            }
            if edit == Edit::Transplant {
                // CompilerInput retains its own numeric script root when another compiler's
                // complete arena replaces this one. The donor's watched record travels with it.
                let other = SourceFile::bundled_externs(Environment::BROWSER)
                    .into_iter()
                    .find(|f| f.get_name().ends_with("/w3c_selection.js"))
                    .unwrap();
                let other_name = other.get_name().to_owned();
                let mut owner = Compiler::new();
                let mut options = CompilerOptions::new();
                options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
                owner.init(&[Arc::new(other)], &[], options);
                owner.parse_inputs().unwrap();
                let old = owner.get_script_node(&other_name).unwrap();
                assert_eq!(old, script);
                let id = old.get_input_id(&owner).unwrap();
                let records = owner
                    .get_input(&id)
                    .unwrap()
                    .bundled_declarations(old)
                    .unwrap();
                if bundled {
                    let (facts, _) = compiler
                        .parsed_script_record::<BundledScript>(script, SnapshotUse::SyntaxAndNames)
                        .unwrap();
                    assert_ne!(facts.declarations.len(), records.len());
                }
                std::mem::swap(&mut owner.ast, &mut compiler.ast);
                compiler = owner;
                script.set_input_id(&mut compiler, Some(id));
            }
            assert_eq!(
                compiler
                    .parsed_script_record::<BundledScript>(script, SnapshotUse::SyntaxAndNames)
                    .is_some(),
                bundled && edit != Edit::BodyAndProperty
            );
            assert_eq!(
                compiler
                    .parsed_script_record::<BundledScript>(script, SnapshotUse::SyntaxNamesAndJsDoc)
                    .is_some(),
                bundled && matches!(edit, Edit::Append | Edit::Transplant)
            );
            let creator = TypedScopeCreator::new(&mut compiler);
            let interpreter = compiler.get_reverse_abstract_interpreter();
            let mut pass = TypeInferencePass::new(&mut compiler, interpreter, creator);
            pass.infer_all_scopes(&mut compiler, root);
            results(compiler, root, pass)
        }

        for edit in [
            Edit::Append,
            Edit::JsDoc,
            Edit::BodyAndProperty,
            Edit::Transplant,
        ] {
            assert_eq!(run(true, edit), run(false, edit));
        }
    }

    #[test]
    fn extern_parameter_types_and_slots_match_the_full_flow_graph() {
        use crate::{
            compiler_options::{CompilerOptions, LanguageMode},
            source_file::SourceFile,
        };
        fn run(force_graph: bool) -> InferenceResult {
            let mut compiler = Compiler::new();
            let mut options = CompilerOptions::new();
            options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
            let ext = SourceFile::from_code(
                "extern.js",
                r#"
                /** @constructor */ function API() {}
                /** @param {!Later} second
                 * @param {boolean} missingFirst
                 * @param {number=} first
                 * @param {boolean} missingLast
                 * @return {!Later} */
                API.prototype.read = function(first, second, extra) {};
                /** @param {T} value
                 * @return {T}
                 * @template T */
                var identity = function(value) {};
                /** @param {...string} values */ function collect(values) {}
                function inline(/** number */ n) {}
                function noParams() {}
                /** @param {{key: number}} object */ function unpack({key}) {}
                function defaulted(x = 3, ...rest) {}
            "#,
            );
            let src = SourceFile::from_code(
                "user.js",
                r#"
                /** @constructor */ function Later() {}
                this.result = identity(new API().read(1, new Later()));
            "#,
            );
            compiler.init(&[Arc::new(ext)], &[Arc::new(src)], options);
            let root = compiler.parse_inputs().unwrap();
            let script = compiler.get_script_node("extern.js").unwrap();
            assert!(
                script
                    .get_first_child(&compiler)
                    .unwrap()
                    .is_from_externs(&compiler)
            );
            let creator = TypedScopeCreator::new(&mut compiler);
            let interpreter = compiler.get_reverse_abstract_interpreter();
            let mut pass = TypeInferencePass::new(&mut compiler, interpreter, creator);
            if force_graph {
                pass.use_linear_flow = false;
            }
            pass.infer_all_scopes(&mut compiler, root);
            results(compiler, root, pass)
        }
        let direct = run(false);
        let unused: Vec<_> = direct
            .diagnostics
            .iter()
            .filter(|d| d.starts_with("JSC_INEXISTENT_PARAM"))
            .collect();
        assert_eq!(unused.len(), 2);
        assert!(unused[0].contains("missingFirst"));
        assert!(unused[1].contains("missingLast"));
        assert_eq!(direct, run(true));
    }

    #[test]
    fn linear_flow_matches_graph_types_slots_and_diagnostics() {
        let direct = infer_with_recording(false);
        assert!(
            direct
                .diagnostics
                .iter()
                .any(|d| d.starts_with("JSC_REASSIGN_CLASS_PROTOTYPE"))
        );
        assert_eq!(direct, infer_with_recording(true));
    }
}
