/*
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
//   src/com/google/javascript/jscomp/FlowSensitiveInlineVariables.java.

//! Port of `FlowSensitiveInlineVariables.java`.
//!
//! Inline variables when possible. Using the information from [`MaybeReachingVariableUse`] and
//! [`MustBeReachingVariableDef`], this pass attempts to inline a variable by placing the value at
//! the definition where the variable is used. The basic requirements for inlining are the
//! following:
//!
//! - There is exactly one reaching definition at the use of that variable
//! - There is exactly one use for that definition of the variable
//!
//! Other requirements can be found in `Candidate::can_inline`. Currently this pass does not
//! operate on the global scope due to compilation time.
//!
//! Rust shape: Java's per-function fields `cfg`, `candidates`, `reachingDef` and `reachingUses`
//! ("persistent in the whole execution of enter scope") are locals of `enter_scope`. The CFG is
//! owned by the analysis that annotates it: `MustBeReachingVariableDef` while candidates are
//! gathered, then `MaybeReachingVariableUse` (Java shares one graph object, and the second
//! analysis re-initializes its annotations in the same order).

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::control_flow_analysis::ControlFlowAnalysis;
use crate::control_flow_graph::{
    AbstractCfgNodeTraversal, AbstractCfgNodeTraversalCallback, ControlFlowGraph,
};
use crate::data_flow_analysis::{DataFlowAnalysis, compute_escaped};
use crate::graph::adjacency_graph::AdjacencyGraph;
use crate::graph::check_paths_between_nodes::CheckPathsBetweenNodes;
use crate::live_variables_analysis;
use crate::maybe_reaching_variable_use::MaybeReachingVariableUse;
use crate::must_be_reaching_variable_def::{Definition, MustBeReachingVariableDef};
use crate::node_traversal::{
    AbstractPostOrderCallbackInterface, AbstractShallowCallback, Callback, NodeTraversal,
    ScopedCallback,
};
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use crate::var::VarId;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_not_null, check_state};
use indexmap::IndexSet;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct FlowSensitiveInlineVariables {
    side_effect_predicate: SideEffectPredicate,
}

struct SideEffectPredicate {
    // Check if there are side effects affecting the value of any of these names
    // (but not properties defined on that name)
    names_to_check: Option<IndexSet<JsString>>,
}

impl SideEffectPredicate {
    // port: FlowSensitiveInlineVariables.SideEffectPredicate#SideEffectPredicate()
    fn new() -> Self {
        Self {
            names_to_check: None,
        }
    }

    // port: FlowSensitiveInlineVariables.SideEffectPredicate#SideEffectPredicate(Set)
    fn new_with_names(names: IndexSet<JsString>) -> Self {
        Self {
            names_to_check: Some(names),
        }
    }

    // port: FlowSensitiveInlineVariables.SideEffectPredicate#apply
    fn apply(&self, compiler: &mut AbstractCompiler, n: Option<NodeId>) -> bool {
        // When the node is null it means, we reached the implicit return
        // where the function returns (possibly without an return statement)
        let Some(n) = n else {
            return false;
        };

        if let Some(names_to_check) = &self.names_to_check
            && n.is_name(compiler)
            && names_to_check.contains(&n.get_string(compiler))
            && NodeUtil::is_l_value(compiler, n)
        {
            // the name is being written to. this is a problem, unless it is part of a top-level
            // assign chain and the write will take place after all CFG node subexpressions are
            // evaluated
            return !is_top_level_assign_target(compiler, n);
        }

        let ast_analyzer = compiler.get_ast_analyzer();
        // TODO(user): We only care about calls to functions that
        // passes one of the dependent variable to a non-side-effect free
        // function.
        if (n.is_call(compiler)
            || n.is_opt_chain_call(compiler)
            || n.is_tagged_template_lit(compiler))
            && ast_analyzer.function_call_has_side_effects(compiler, n)
        {
            return true;
        }

        if n.is_new(compiler) && ast_analyzer.constructor_call_has_side_effects(compiler, n) {
            return true;
        }

        if n.is_del_prop(compiler) {
            return true;
        }

        if (n.is_get_prop(compiler) || n.is_get_elem(compiler)) && NodeUtil::is_l_value(compiler, n)
        {
            return self.names_to_check.is_none() || !is_top_level_assign_target(compiler, n);
        }

        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            if !ControlFlowGraph::<NodeId>::is_entering_new_cfg_node(compiler, cur)
                && self.apply(compiler, Some(cur))
            {
                return true;
            }
            c = cur.get_next(compiler);
        }
        false
    }
}

/// Whether the given node is the target of a (possibly chained) assignment
// port: FlowSensitiveInlineVariables#isTopLevelAssignTarget
fn is_top_level_assign_target(ast: &Ast, n: NodeId) -> bool {
    let mut ancestor = n.get_parent(ast).unwrap();
    while ancestor.is_assign(ast) {
        ancestor = ancestor.get_parent(ast).unwrap();
    }
    ancestor.is_expr_result(ast)
}

impl Default for FlowSensitiveInlineVariables {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowSensitiveInlineVariables {
    /// Java's constructor also takes the compiler, which a pass does not store (DESIGN §6).
    // port: FlowSensitiveInlineVariables#FlowSensitiveInlineVariables
    pub fn new() -> Self {
        Self {
            side_effect_predicate: SideEffectPredicate::new(),
        }
    }

    // port: FlowSensitiveInlineVariables#isCandidateFunction
    fn is_candidate_function(ast: &Ast, fn_: NodeId) -> bool {
        let fn_body = fn_.get_last_child(ast).unwrap();
        Self::contains_candidate_expressions(ast, fn_body)
    }

    // port: FlowSensitiveInlineVariables#containsCandidateExpressions
    fn contains_candidate_expressions(ast: &Ast, n: NodeId) -> bool {
        if n.is_function(ast) {
            // don't recurse into inner functions or into expressions the can't contain
            // declarations.
            return false;
        }

        if (NodeUtil::is_name_declaration(ast, Some(n)) || Self::is_assignment_to_name(ast, n))
            // if it is a simple assignment
            && n.get_first_child(ast).unwrap().is_name(ast)
        {
            return true;
        }

        let mut c = n.get_first_child(ast);
        while let Some(cur) = c {
            if Self::contains_candidate_expressions(ast, cur) {
                return true;
            }
            c = cur.get_next(ast);
        }
        false
    }

    // port: FlowSensitiveInlineVariables#isAssignmentToName
    fn is_assignment_to_name(ast: &Ast, n: NodeId) -> bool {
        if NodeUtil::is_assignment_op(ast, n) || n.is_dec(ast) || n.is_inc(ast) {
            // if it is a simple assignment
            return n.get_first_child(ast).unwrap().is_name(ast);
        }
        false
    }
}

impl CompilerPass for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .traverse_roots(externs, root);
    }
}

impl Callback for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_script(t) || !t.get_input().unwrap().is_extern()
    }

    // port: FlowSensitiveInlineVariables#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {
        // TODO(user): While the helpers do a subtree traversal on the AST, the
        // compiler pass itself only traverse the AST to look for function
        // declarations to perform dataflow analysis on. We could combine
        // the traversal in DataFlowAnalysis's computeEscaped later to save some
        // time.
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for FlowSensitiveInlineVariables {
    // port: FlowSensitiveInlineVariables#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_scope() {
            return; // Don't even brother. All global variables are likely escaped.
        }

        let scope = t.get_scope();
        if !scope.is_function_block_scope(t.get_compiler()) {
            return; // Only want to do the following if its a function block scope.
        }

        let function_scope_root = t.get_scope_root().unwrap().get_parent(t).unwrap();

        if !Self::is_candidate_function(t, function_scope_root) {
            return;
        }

        if live_variables_analysis::MAX_VARIABLES_TO_ANALYZE < scope.get_var_count(t.get_compiler())
        {
            return;
        }

        let scope_root = t.get_scope_root().unwrap();
        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();

        // Compute the forward reaching definition.
        let cfg = ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(function_scope_root)
            .set_include_edge_annotations(true)
            .compute_cfg(compiler);

        let mut escaped: IndexSet<VarId> = IndexSet::new();
        let scope_parent = scope.get_parent(compiler).unwrap();
        let all_vars_declared_in_function =
            NodeUtil::get_all_vars_declared_in_function(compiler, scope_creator, scope_parent);
        let all_vars_in_fn = all_vars_declared_in_function.get_all_variables().clone();
        compute_escaped(
            compiler,
            scope_parent,
            &mut escaped,
            scope_creator,
            &all_vars_in_fn,
        );

        let mut reaching_def =
            MustBeReachingVariableDef::new(compiler, cfg, escaped.clone(), all_vars_in_fn.clone());
        reaching_def.analyze(compiler);
        let mut candidates: VecDeque<Candidate> = VecDeque::new();

        // Using the forward reaching definition search to find all the inline
        // candidates
        NodeTraversal::traverse(
            compiler,
            scope_root,
            &mut AbstractShallowCallback::new(GatherCandidates::new(
                &reaching_def,
                &mut candidates,
            )),
        );

        // Compute the backward reaching use. The CFG and per-function variable info can be
        // reused.
        let cfg = reaching_def.into_cfg();
        let mut reaching_uses = MaybeReachingVariableUse::new(cfg, escaped, all_vars_in_fn.clone());
        reaching_uses.analyze(compiler);
        while let Some(mut c) = candidates.pop_front() {
            let candidate_var = check_not_null!(all_vars_in_fn.get(&c.var_name).copied());
            let candidate_scope = candidate_var.get_scope(compiler);
            if c.can_inline(
                compiler,
                &mut reaching_uses,
                &self.side_effect_predicate,
                candidate_scope,
            ) {
                c.inline_variable(compiler);

                // If candidate "c" has dependencies, then inlining it may have introduced new
                // dependencies for our other inlining candidates. MustBeReachingVariableDef uses
                // a dependency graph in its analysis. Generating a new dependency graph will need
                // another CFG computation. Ideally we should iterate to a fixed point, but that
                // can be costly. Therefore, we use a conservative heuristic here: For each
                // candidate "other", we back off if its set of dependencies cannot contain all of
                // "c"'s dependencies.
                if !c.def_metadata.depends.is_empty() {
                    candidates.retain(|other| {
                        let c_var = scope.get_var(compiler, c.var_name.clone());
                        let remove = var_set_contains(compiler, &other.def_metadata.depends, c_var)
                            && !var_set_contains_all(
                                compiler,
                                &other.def_metadata.depends,
                                &c.def_metadata.depends,
                            );
                        !remove
                    });
                }
            }
        }
    }

    // port: FlowSensitiveInlineVariables#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
}

/// Java's `Set<Var>#contains`: `Var` inherits `ScopedName#equals` (name and scope root).
fn var_set_contains(
    compiler: &AbstractCompiler,
    set: &IndexSet<VarId>,
    var: Option<VarId>,
) -> bool {
    var.is_some_and(|var| set.iter().any(|&element| element.equals(compiler, var)))
}

/// Java's `Set<Var>#containsAll` with `ScopedName#equals`.
fn var_set_contains_all(
    compiler: &AbstractCompiler,
    set: &IndexSet<VarId>,
    other: &IndexSet<VarId>,
) -> bool {
    other
        .iter()
        .all(|&var| var_set_contains(compiler, set, Some(var)))
}

struct GatherCandidatesCfgNodeCallback<'a> {
    cfg_node: Option<NodeId>,
    /// The outer pass's `reachingDef` and `candidates`.
    reaching_def: &'a MustBeReachingVariableDef,
    candidates: &'a mut VecDeque<Candidate>,
}

impl GatherCandidatesCfgNodeCallback<'_> {
    // port: FlowSensitiveInlineVariables.GatherCandidatesCfgNodeCallback#setCfgNode
    fn set_cfg_node(&mut self, cfg_node: NodeId) {
        self.cfg_node = Some(cfg_node);
    }
}

impl AbstractCfgNodeTraversalCallback for GatherCandidatesCfgNodeCallback<'_> {
    // port: FlowSensitiveInlineVariables.GatherCandidatesCfgNodeCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) {
            // n.getParent() isn't null. This just the case where n is the root
            // node that gatherCb started at.
            let Some(parent) = parent else {
                return;
            };

            // Make sure that the name node is purely a read.
            if (NodeUtil::is_assignment_op(t, parent) && parent.get_first_child(t) == Some(n))
                || NodeUtil::is_name_declaration(t, Some(parent))
                || parent.is_inc(t)
                || parent.is_dec(t)
                || parent.is_param_list(t)
                || parent.is_catch(t)
                || NodeUtil::is_lhs_by_destructuring(t, n)
            {
                return;
            }

            let name = n.get_string(t);
            // This pass only runs on local scopes.
            if t.get_compiler()
                .get_coding_convention()
                .is_exported(&name, /* local= */ true)
            {
                return;
            }

            let cfg_node = self.cfg_node.unwrap();
            let def = self.reaching_def.get_def(&name, cfg_node);
            // TODO(nicksantos): We need to add some notion of @const outer
            // scope vars. We can inline those just fine.
            if let Some(def) = def
                && !self
                    .reaching_def
                    .depends_on_outer_scope_vars(t.get_compiler(), &def)
            {
                self.candidates
                    .push_back(Candidate::new(t, name, def, n, cfg_node));
            }
        }
    }
}

/// Gathers a list of possible candidates for inlining based only on information from
/// [`MustBeReachingVariableDef`]. The list will be stored in `candidates` and the validity of
/// each inlining Candidate should be later verified with `Candidate::can_inline` when
/// [`MaybeReachingVariableUse`] has been performed.
struct GatherCandidates<'a> {
    reaching_def: &'a MustBeReachingVariableDef,
    gather_cb: AbstractCfgNodeTraversal<GatherCandidatesCfgNodeCallback<'a>>,
}

impl<'a> GatherCandidates<'a> {
    fn new(
        reaching_def: &'a MustBeReachingVariableDef,
        candidates: &'a mut VecDeque<Candidate>,
    ) -> Self {
        Self {
            reaching_def,
            gather_cb: AbstractCfgNodeTraversal(GatherCandidatesCfgNodeCallback {
                cfg_node: None,
                reaching_def,
                candidates,
            }),
        }
    }
}

impl AbstractPostOrderCallbackInterface for GatherCandidates<'_> {
    // port: FlowSensitiveInlineVariables.GatherCandidates#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let graph_node = self.reaching_def.get_cfg().get_node(&Some(n));
        if graph_node.is_none() {
            // Not a CFG node.
            return;
        }
        let cfg_node = n;

        self.gather_cb.0.set_cfg_node(cfg_node);
        NodeTraversal::traverse(t.get_compiler(), cfg_node, &mut self.gather_cb);
    }
}

/// Models the connection between a definition and a use of that definition.
struct Candidate {
    // Name of the variable.
    var_name: JsString,

    // Nodes related to the definition.
    def: Option<NodeId>,
    def_metadata: Arc<Definition>,

    // Nodes related to the use.
    use_: NodeId,
    use_cfg_node: NodeId,

    // Number of uses of the variable within the current CFG node.
    num_uses_within_cfg_node: i32,
}

impl Candidate {
    // port: FlowSensitiveInlineVariables.Candidate#Candidate
    fn new(
        ast: &Ast,
        var_name: JsString,
        def_metadata: Arc<Definition>,
        use_: NodeId,
        use_cfg_node: NodeId,
    ) -> Self {
        check_argument!(use_.is_name(ast));
        Self {
            var_name,
            def: None,
            def_metadata,
            use_,
            use_cfg_node,
            num_uses_within_cfg_node: 0,
        }
    }

    // port: FlowSensitiveInlineVariables.Candidate#getDefCfgNode
    fn get_def_cfg_node(&self) -> NodeId {
        self.def_metadata.node
    }

    // port: FlowSensitiveInlineVariables.Candidate#canInline
    fn can_inline(
        &mut self,
        compiler: &mut AbstractCompiler,
        reaching_uses: &mut MaybeReachingVariableUse,
        side_effect_predicate: &SideEffectPredicate,
        scope: ScopeId,
    ) -> bool {
        // Cannot inline a parameter.
        if self.get_def_cfg_node().is_function(compiler) {
            return false;
        }

        self.get_definition(compiler, self.get_def_cfg_node());
        self.get_num_use_in_use_cfg_node(compiler, self.use_cfg_node);

        // Definition was not found.
        let Some(def) = self.def else {
            return false;
        };

        // Check that the assignment isn't used as a R-Value.
        // TODO(user): Certain cases we can still inline.
        if def.is_assign(compiler)
            && !NodeUtil::is_expr_assign(compiler, def.get_parent(compiler).unwrap())
        {
            return false;
        }

        let mut names_to_check: IndexSet<JsString> = IndexSet::new();
        for &var in &self.def_metadata.depends {
            names_to_check.insert(var.get_name(compiler));
        }

        let side_effect_predicate_with_names = SideEffectPredicate::new_with_names(names_to_check);

        // A subexpression evaluated after the variable has a side effect.
        // Example, for x:
        // x = readProp(b), modifyProp(b); print(x);
        if check_post_expressions(
            compiler,
            def,
            self.get_def_cfg_node(),
            &side_effect_predicate_with_names,
        ) {
            return false;
        }

        // Similar check as the above but this time, all the sub-expressions
        // evaluated before the variable.
        // x = readProp(b); modifyProp(b), print(x);
        if check_pre_expressions(
            compiler,
            self.use_,
            self.use_cfg_node,
            &side_effect_predicate_with_names,
        ) {
            return false;
        }

        // TODO(user): Side-effect is OK sometimes. As long as there are no
        // side-effect function down all paths to the use. Once we have all the
        // side-effect analysis tool.
        let ast_analyzer = compiler.get_ast_analyzer();
        let def_last_child = def.get_last_child(compiler).unwrap();
        if ast_analyzer.may_have_side_effects(compiler, def_last_child) {
            return false;
        }

        // TODO(user): We could inline all the uses if the expression is short.

        // Finally we have to make sure that there are no more than one use
        // in the program and in the CFG node. Even when it is semantically
        // correctly inlining twice increases code size.
        if self.num_uses_within_cfg_node != 1 {
            return false;
        }

        // Make sure that the name is not within a loop
        if NodeUtil::is_within_loop(compiler, self.use_) {
            return false;
        }

        if !Self::has_exactly_one(&reaching_uses.get_uses(&self.var_name, self.get_def_cfg_node()))
        {
            return false;
        }

        if !self.is_rhs_safe_to_inline(compiler, scope) {
            return false;
        }

        // We can skip the side effect check along the paths of two nodes if
        // they are just next to each other.
        if NodeUtil::is_statement_block(
            compiler,
            self.get_def_cfg_node().get_parent(compiler).unwrap(),
        ) && self.get_def_cfg_node().get_next(compiler) != Some(self.use_cfg_node)
        {
            // Similar side effect check as above but this time the side effect is
            // else where along the path.
            // x = readProp(b); while(modifyProp(b)) {}; print(x);
            let cfg = reaching_uses.get_cfg_mut();
            let def_graph_node = cfg.get_node(&Some(self.get_def_cfg_node())).unwrap();
            let use_graph_node = cfg.get_node(&Some(self.use_cfg_node)).unwrap();
            let compiler_cell = RefCell::new(&mut *compiler);
            let path_check = CheckPathsBetweenNodes::with_inclusive(
                def_graph_node,
                use_graph_node,
                |n: &Option<NodeId>| {
                    side_effect_predicate.apply(&mut compiler_cell.borrow_mut(), *n)
                },
                |_: &ControlFlowGraph<NodeId>, _| true,
                false,
            );
            if path_check.some_paths_satisfy_predicate(cfg) {
                return false;
            }
        }

        true
    }

    // port: FlowSensitiveInlineVariables.Candidate#hasExactlyOne
    fn has_exactly_one(iterable: &[NodeId]) -> bool {
        let mut iterator = iterable.iter();
        if iterator.next().is_some() && iterator.next().is_none() {
            return true;
        }
        false
    }

    /// Actual transformation.
    // port: FlowSensitiveInlineVariables.Candidate#inlineVariable
    fn inline_variable(&mut self, compiler: &mut AbstractCompiler) {
        let def = self.def.unwrap();
        let mut def_parent = def.get_parent(compiler).unwrap();
        let use_parent = self.use_.get_parent(compiler).unwrap();
        if def.is_assign(compiler) {
            let rhs = def.get_last_child(compiler).unwrap();
            rhs.detach(compiler);
            // Oh yes! I have grandparent to remove this.
            check_state!(def_parent.is_expr_result(compiler));
            while def_parent.get_parent(compiler).unwrap().is_label(compiler) {
                def_parent = def_parent.get_parent(compiler).unwrap();
            }
            compiler.report_change_to_enclosing_scope(def_parent);
            def_parent.detach(compiler);
            self.use_.replace_with(compiler, rhs);
        } else if NodeUtil::is_name_declaration(compiler, Some(def_parent)) {
            let rhs = def.get_last_child(compiler).unwrap();
            if def_parent.is_const(compiler) {
                // If it is a const var we don't want to remove the rhs of the variable
                let undefined = compiler.new_string_with_token(Token::NAME, "undefined");
                rhs.replace_with(compiler, undefined);
                self.use_.replace_with(compiler, rhs);
            } else {
                rhs.detach(compiler);
                self.use_.replace_with(compiler, rhs);
            }
        } else {
            panic!("No other definitions can be inlined.");
        }
        compiler.report_change_to_enclosing_scope(use_parent);
    }

    /// Set the def node
    ///
    /// `n`: A node that has a corresponding CFG node in the CFG.
    // port: FlowSensitiveInlineVariables.Candidate#getDefinition
    fn get_definition(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        struct GatherCb<'a> {
            var_name: &'a JsString,
            def: &'a mut Option<NodeId>,
        }
        impl AbstractCfgNodeTraversalCallback for GatherCb<'_> {
            // port: FlowSensitiveInlineVariables.Candidate#getDefinition.visit
            fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
                match n.get_token(t) {
                    Token::NAME if n.get_string(t) == *self.var_name && n.has_children(t) => {
                        *self.def = Some(n);
                    }
                    Token::ASSIGN => {
                        let lhs = n.get_first_child(t).unwrap();
                        if lhs.is_name(t) && lhs.get_string(t) == *self.var_name {
                            *self.def = Some(n);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut gather_cb = AbstractCfgNodeTraversal(GatherCb {
            var_name: &self.var_name,
            def: &mut self.def,
        });
        NodeTraversal::traverse(compiler, n, &mut gather_cb);
    }

    /// Computes the number of uses of the variable varName and store it in
    /// numUseWithinUseCfgNode.
    // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode
    fn get_num_use_in_use_cfg_node(&mut self, compiler: &mut AbstractCompiler, cfg_node: NodeId) {
        self.num_uses_within_cfg_node = 0;
        struct GatherCb<'a> {
            var_name: &'a JsString,
            num_uses_within_cfg_node: &'a mut i32,
            cfg_node: NodeId,
        }
        impl GatherCb<'_> {
            // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode.isAssignChain
            fn is_assign_chain(ast: &Ast, child: NodeId, ancestor: NodeId) -> bool {
                let mut n = child;
                while n != ancestor {
                    if !n.is_assign(ast) {
                        return false;
                    }
                    n = n.get_parent(ast).unwrap();
                }
                true
            }
        }
        impl AbstractCfgNodeTraversalCallback for GatherCb<'_> {
            // port: FlowSensitiveInlineVariables.Candidate#getNumUseInUseCfgNode.visit
            fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
                if n.is_name(t) && n.get_string(t) == *self.var_name {
                    // We make a special exception when the entire cfgNode is a chain
                    // of assignments, since in that case the assignment statements
                    // will happen after the inlining of the right hand side.
                    // TODO(lharker): We can probably remove the isAssignChain check, and instead
                    // use the SideEffectPredicate to look for dangerous assignments in the same
                    // CFG node
                    let parent = parent.unwrap();
                    if parent.is_assign(t)
                        && parent.get_first_child(t) == Some(n)
                        && Self::is_assign_chain(t, parent, self.cfg_node)
                    {
                        // Don't count lhs of top-level assignment chain
                    } else {
                        *self.num_uses_within_cfg_node += 1;
                    }
                }
            }
        }

        let mut gather_cb = AbstractCfgNodeTraversal(GatherCb {
            var_name: &self.var_name,
            num_uses_within_cfg_node: &mut self.num_uses_within_cfg_node,
            cfg_node,
        });
        NodeTraversal::traverse(compiler, cfg_node, &mut gather_cb);
    }

    /// Check if the definition we're considering inline has anything that makes inlining unsafe
    /// (that hasn't already been caught).
    ///
    /// `usage_scope`: The scope we will inline the variable into.
    // port: FlowSensitiveInlineVariables.Candidate#isRhsSafeToInline
    fn is_rhs_safe_to_inline(&self, compiler: &AbstractCompiler, usage_scope: ScopeId) -> bool {
        let def_last_child = self.def.unwrap().get_last_child(compiler).unwrap();
        // Don't inline definitions with an R-Value that has:
        // 1) GETELEM, OPTCHAIN_GETELEM (e.g: foo?.['bar']), GETPROP, OPTCHAIN_GETPROP (e.g:
        // foo?.bar), CLASS, ARRAYLIT,
        // OBJECTLIT, REGEXP
        // 2) anything that creates a new object.
        // Example:
        // var x = a.b.c; j.c = 1; print(x);
        // Inlining print(a.b.c) is not safe - consider if j were an alias to a.b.
        if NodeUtil::has(
            compiler,
            def_last_child,
            &|ast: &Ast, input: NodeId| {
                matches!(
                    input.get_token(ast),
                    Token::GETELEM
                        | Token::GETPROP
                        | Token::OPTCHAIN_GETPROP
                        | Token::OPTCHAIN_GETELEM
                        | Token::CLASS
                        | Token::ARRAYLIT
                        | Token::OBJECTLIT
                        | Token::REGEXP
                        | Token::NEW
                )
                // unsafe to inline.
            },
            // Recurse if the node is not a function.
            &|ast: &Ast, input: NodeId| !input.is_function(ast),
        ) {
            return false;
        }

        // Don't inline definitions with an rvalue referencing names that are not declared in the
        // usage's scope. (Unlike the above check, this includes names referenced inside function
        // expressions in the rvalue).
        // e.g. the name "a" below in the definition of "b":
        //   {
        //     let a = 3;
        //     var b = a;
        //   }
        // return b;   // "a" is not declared in this scope so we can't inline this to "return a;"
        if NodeUtil::has(
            compiler,
            def_last_child,
            &|ast: &Ast, input: NodeId| {
                if input.is_name(ast) {
                    let name = input.get_string(ast);
                    if !name.is_empty() && !usage_scope.has_slot(compiler, name) {
                        return true; // unsafe to inline.
                    }
                }
                false
            },
            &|_: &Ast, _: NodeId| true,
        ) {
            return false;
        }
        true
    }
}

/// Given an expression by its root and sub-expression n, return true if the predicate is true
/// for some expression evaluated after n.
///
/// NOTE: this doesn't correctly check destructuring patterns, because their order of evaluation
/// is different from AST traversal order, but currently this is ok because
/// FlowSensitiveInlineVariables never inlines variable assignments inside destructuring.
///
/// Example:
///
/// NotChecked(), NotChecked(), n, Checked(), Checked();
// port: FlowSensitiveInlineVariables#checkPostExpressions
fn check_post_expressions(
    compiler: &mut AbstractCompiler,
    n: NodeId,
    expression_root: NodeId,
    predicate: &SideEffectPredicate,
) -> bool {
    let mut p = n;
    while p != expression_root {
        let mut cur = p.get_next(compiler);
        while let Some(c) = cur {
            if predicate.apply(compiler, Some(c)) {
                return true;
            }
            cur = c.get_next(compiler);
        }
        p = p.get_parent(compiler).unwrap();
    }
    false
}

/// Given an expression by its root and sub-expression n, return true if the predicate is true
/// for some expression evaluated before n.
///
/// In most cases evaluation order follows left-to-right AST order. Destructuring pattern
/// evaluation is an exception.
///
/// Example:
///
/// Checked(), Checked(), n, NotChecked(), NotChecked();
// port: FlowSensitiveInlineVariables#checkPreExpressions
fn check_pre_expressions(
    compiler: &mut AbstractCompiler,
    n: NodeId,
    expression_root: NodeId,
    predicate: &SideEffectPredicate,
) -> bool {
    let mut p = n;
    while p != expression_root {
        let oldest_sibling = p
            .get_parent(compiler)
            .unwrap()
            .get_first_child(compiler)
            .unwrap();
        // Evaluate a destructuring assignment right-to-left.
        if oldest_sibling.is_destructuring_pattern(compiler) {
            if p.is_destructuring_pattern(compiler)
                && let Some(next) = p.get_next(compiler)
                && predicate.apply(compiler, Some(next))
            {
                return true;
            }
            p = p.get_parent(compiler).unwrap();
            continue;
        }
        let mut cur = oldest_sibling;
        while cur != p {
            if predicate.apply(compiler, Some(cur)) {
                return true;
            }
            cur = cur.get_next(compiler).unwrap();
        }
        p = p.get_parent(compiler).unwrap();
    }
    false
}
