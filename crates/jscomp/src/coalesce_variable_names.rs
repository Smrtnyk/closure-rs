/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CoalesceVariableNames.java,
//   src/com/google/javascript/jscomp/MemoizedScopeCreator.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `CoalesceVariableNames.java`.
//!
//! Reuse variable names if possible.
//!
//! For example, from `var x = 1; print(x); var y = 2; print(y);` to
//! `var x = 1; print(x); x = 2; print(x)`. The benefits are slightly shorter code because of the
//! removed `var` declaration, less unique variables in hope for better renaming, and finally
//! better gzip compression.
//!
//! The pass operates similar to a typical register allocator found in an optimizing compiler by
//! first computing live ranges with `LiveVariablesAnalysis` and a variable interference graph.
//! Then it uses graph coloring in `GraphColoring` to determine which two variables can be merge
//! together safely.
use crate::{
    AbstractCompiler, AllVarsDeclaredInFunction,
    abstract_compiler::LifeCycleStage,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    control_flow_analysis::ControlFlowAnalysis,
    control_flow_graph::ControlFlowGraph,
    data_flow_analysis::{DataFlowAnalysis, LinearFlowState},
    graph::{
        adjacency_graph::AdjacencyGraph,
        annotatable::Annotatable,
        graph::Graph,
        graph_coloring::{GraphColoring, GreedyGraphColoring},
        graph_node::GraphNode,
        linked_undirected_graph::LinkedUndirectedGraph,
    },
    live_variables_analysis::{
        LiveVariableLattice, LiveVariablesAnalysis, MAX_VARIABLES_TO_ANALYZE,
    },
    memoized_scope_creator::MemoizedScopeCreator,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    scope::ScopeId,
    scope_creator::ScopeCreator,
    syntactic_scope_creator::SyntacticScopeCreator,
    var::VarId,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_not_null, check_state, ir::IR, java_util::bit_set::BitSet, js_string::JsString,
    node::NodeId, token::Token,
};
use std::{cell::RefCell, collections::BTreeSet, collections::VecDeque, rc::Rc};

// Rust-only: Java shares one MemoizedScopeCreator object between this pass (the field) and its
// NodeTraversal. The traversal borrows its creator mutably while it runs, so both hold this
// shared handle.
#[derive(Clone)]
struct SharedScopeCreator(Rc<RefCell<MemoizedScopeCreator<'static>>>);

impl ScopeCreator for SharedScopeCreator {
    // port: MemoizedScopeCreator#createScope
    fn create_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        self.0.borrow_mut().create_scope(compiler, n, parent)
    }
}

// The interference graph and its coloring; Java's GraphColoring holds its graph.
struct Coloring {
    graph: LinkedUndirectedGraph<VarId, ()>,
    coloring: GreedyGraphColoring<VarId, ()>,
}

pub struct CoalesceVariableNames {
    scope_creator: SharedScopeCreator,
    colorings: VecDeque<Coloring>,
    // Java's `liveness` field is always the head of `liveAnalyses` (it is set to the new analysis
    // right before the push and to `liveAnalyses.peek()` after each pop).
    live_analyses: VecDeque<LiveVariablesAnalysis>,
    use_pseudo_names: bool,
    ast_factory: AstFactory,
    /// A stack of shouldOptimizeScope results.
    should_optimize_scope_stack: VecDeque<bool>,
    // Rust-only: the cfg-root stack of the NodeTraversal.AbstractCfgCallback superclass.
    cfgs: VecDeque<NodeId>,
}

impl CoalesceVariableNames {
    /// `use_pseudo_names`: For debug purposes, when merging variable foo and bar to foo, rename
    /// both variable to foo_bar.
    // port: CoalesceVariableNames#CoalesceVariableNames
    pub fn new(compiler: &mut AbstractCompiler, use_pseudo_names: bool) -> Self {
        // The code is normalized at this point in the compilation process. This allows us to use
        // the fact that all variables have been given unique names. We can hoist coalesced
        // variables to VARS because we know that shadowing can't occur.
        check_state!(compiler.get_life_cycle_stage().is_normalized());

        Self {
            colorings: VecDeque::new(),
            live_analyses: VecDeque::new(),
            use_pseudo_names,
            ast_factory: compiler.create_ast_factory(),
            scope_creator: SharedScopeCreator(Rc::new(RefCell::new(MemoizedScopeCreator::new(
                Box::new(SyntacticScopeCreator::new()),
            )))),
            should_optimize_scope_stack: VecDeque::new(),
            cfgs: VecDeque::new(),
        }
    }

    fn liveness(&self) -> &LiveVariablesAnalysis {
        self.live_analyses.front().unwrap()
    }

    // port: NodeTraversal.AbstractCfgCallback#getControlFlowGraph
    fn get_control_flow_graph(
        &mut self,
        compiler: &mut AbstractCompiler,
    ) -> ControlFlowGraph<NodeId> {
        let cfg_root = self.cfgs.front().copied();
        check_state!(cfg_root.is_some());
        ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(cfg_root.unwrap())
            .set_include_edge_annotations(true)
            .compute_cfg(compiler)
    }

    /// Returns populated AllVarsDeclaredInFunction object iff shouldOptimizeScope is true.
    // port: CoalesceVariableNames#shouldOptimizeScope
    fn should_optimize_scope(
        t: &mut NodeTraversal<'_>,
        scope_creator: &mut dyn ScopeCreator,
    ) -> Option<AllVarsDeclaredInFunction> {
        // TODO(user): We CAN do this in the global scope, just need to be
        // careful when something is exported. Liveness uses bit-vector for live
        // sets so I don't see compilation time will be a problem for running this
        // pass in the global scope.

        if t.get_scope_root().unwrap().is_function(t) {
            let scope = t.get_scope();
            let all_vars_declared_in_function =
                NodeUtil::get_all_vars_declared_in_function(t.get_compiler(), scope_creator, scope);
            if MAX_VARIABLES_TO_ANALYZE
                > all_vars_declared_in_function
                    .get_all_variables_in_order()
                    .len() as i32
            {
                return Some(all_vars_declared_in_function);
            }
        }

        None
    }

    // port: CoalesceVariableNames#enterScopeWithCfg
    fn enter_scope_with_cfg(&mut self, t: &mut NodeTraversal<'_>) {
        let mut scope_creator = self.scope_creator.clone();
        let Some(all_vars_declared_in_function) =
            Self::should_optimize_scope(t, &mut scope_creator)
        else {
            self.should_optimize_scope_stack.push_front(false);
            return;
        };
        self.should_optimize_scope_stack.push_front(true);

        let scope = t.get_scope();
        check_state!(
            scope.is_function_scope(t.get_compiler()),
            "%s",
            scope.to_string(t.get_compiler())
        );

        // live variables analysis is based off of the control flow graph
        let cfg = self.get_control_flow_graph(t.get_compiler());

        let compiler = t.get_compiler();
        let mut liveness = LiveVariablesAnalysis::new(
            compiler,
            cfg,
            scope,
            None,
            &mut scope_creator,
            all_vars_declared_in_function,
        );

        if FeatureSet::ES3.contains(compiler.get_options().get_output_feature_set()) {
            // If the function has exactly 2 params, mark them as escaped. This is a work-around
            // for a bug in IE 8 and below, where it throws an exception if you write to the
            // parameters of the callback in a sort(). See
            // http://blickly.github.io/closure-compiler-issues/#58 and
            // https://www.zachleat.com/web/array-sort/
            let enclosing_function = scope.get_root_node(compiler);
            if NodeUtil::get_function_parameters(compiler, enclosing_function)
                .has_two_children(compiler)
            {
                liveness.mark_all_parameters_escaped(compiler);
            }
        }

        liveness.analyze(compiler);
        self.live_analyses.push_front(liveness);

        // The interference graph has the function's variables as its nodes and any interference
        // between the variables as the edges. Interference between two variables means that they
        // are alive at overlapping times, which means that their variable names cannot be
        // coalesced.
        let mut interference_graph = self.compute_variable_names_interference_graph(compiler);

        // Color any interfering variables with different colors and any variables that can be
        // safely coalesced wih the same color.
        // coloringTieBreaker: comparingInt((Var arg) -> liveness.getVarIndex(arg.getName()))
        let liveness = self.liveness();
        let var_index: IndexMap<VarId, i32> = liveness
            .get_all_variables_in_order()
            .iter()
            .map(|&v| (v, liveness.get_var_index(&v.get_name(compiler))))
            .collect();
        let mut coloring =
            GreedyGraphColoring::with_tie_breaker(Some(Box::new(move |a: &VarId, b: &VarId| {
                var_index[a].cmp(&var_index[b])
            })));
        coloring.color(&mut interference_graph);
        self.colorings.push_front(Coloring {
            graph: interference_graph,
            coloring,
        });
    }

    // port: CoalesceVariableNames#exitScopeWithCfg
    fn exit_scope_with_cfg(&mut self, _t: &mut NodeTraversal<'_>) {
        if !self.should_optimize_scope_stack.pop_front().unwrap() {
            return;
        }
        self.colorings.pop_front();
        self.live_analyses.pop_front();
        // liveness = liveAnalyses.peek();
    }

    /// Updates declarations of the given name node and the coalesced variable. Firstly, it
    /// converts the coalesced variable's declaration into a `var` if it is a `const` or a `let`.
    /// Secondly, it removes the declaration of the given name node if it is under a parent which
    /// is a name declaration or if the given name is a destructuring LHS name under a
    /// destructuring LHS declaration.
    ///
    /// For example, when coalescing the names `a` and `b` below, it changes:
    ///
    /// ```text
    ///   const a = 1;
    ///   const [{prop: b} = {prop: undefined}] = [{prop: 10}]; // declares `b`
    /// ```
    ///
    /// to
    ///
    /// ```text
    ///   var a = 1;
    ///   [{prop: a} = {prop: undefined}] = [{prop: 10}]; // coalescing renames `b` to `a` and this
    ///                                                   // method undeclares it by removing the
    ///                                                   // `const`
    /// ```
    // port: CoalesceVariableNames#updateDeclarationsPostCoalescing
    fn update_declarations_post_coalescing(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        coalesced_var: VarId,
        parent: NodeId,
    ) {
        check_state!(
            n.is_name(compiler),
            "trying to update the declaration of a non-name node"
        );
        if NodeUtil::is_name_declaration(compiler, Some(parent))
            || (NodeUtil::get_enclosing_type(compiler, n, Token::DESTRUCTURING_LHS).is_some()
                && NodeUtil::is_lhs_by_destructuring(compiler, n)
                // Consider this to be a destructuring LHS declaration only if we don't hit an
                // assignment like `[a,b] = [3,4]` first.
                && !self.is_name_inside_destructuring_assignment(compiler, n))
        {
            // convert the coalesced variable's declaration into a `var` if it is a `const` or a
            // `let`
            self.make_declaration_var(compiler, coalesced_var);
            // remove the declaration of the given name node as it has been coalesced with
            // coalescedVar
            Self::remove_var_declaration(compiler, n);
        }
    }

    /// Is the given name node inside the LHS of a destructuring pattern assignment like
    /// `[a,b,c] = [1,2,3]`.
    ///
    /// Importantly, for this pass, it means that the name is not inside a declaration like
    /// `const [a,b,c] = [1,2,3]` and must not be undeclared.
    // port: CoalesceVariableNames#isNameInsideDestructuringAssignment
    fn is_name_inside_destructuring_assignment(
        &self,
        compiler: &AbstractCompiler,
        n: NodeId,
    ) -> bool {
        let enclosing_target = NodeUtil::get_root_target(compiler, n);
        if enclosing_target.is_destructuring_pattern(compiler)
            && enclosing_target
                .get_parent(compiler)
                .unwrap()
                .is_assign(compiler)
        {
            // This is a destructuring pattern assignment.
            return true;
        }
        false
    }

    /// In order to determine when it is appropriate to coalesce two variables, we use a live
    /// variables analysis to make sure they are not alive at the same time. We take every CFG
    /// node and determine which pairs of variables are alive at the same time. These pairs are
    /// set to true in a bit map. We take every pairing of variables and use the bit map to check
    /// if the two variables are alive at the same time. If two variables are alive at the same
    /// time, we create an edge between them in the interference graph. The interference graph is
    /// the input to a graph coloring algorithm that ensures any interfering variables are marked
    /// in different color groups, while variables that can safely be coalesced are assigned the
    /// same color group.
    ///
    /// The cfg and the escaped set (we don't want to coalesce any escaped variables) are those of
    /// `liveness`, which owns them in Rust.
    // port: CoalesceVariableNames#computeVariableNamesInterferenceGraph
    fn compute_variable_names_interference_graph(
        &self,
        compiler: &mut AbstractCompiler,
    ) -> LinkedUndirectedGraph<VarId, ()> {
        let liveness = self.liveness();
        let cfg = liveness.get_cfg();
        let escaped = liveness.get_escaped_locals();
        // LinkedUndirectedGraph.create(): node and edge annotations on. A Var's toString needs the
        // compiler, so the graph prints the handle (only used for debugging output).
        let mut interference_graph = LinkedUndirectedGraph::<VarId, ()>::new_with_value_to_strings(
            true,
            true,
            |v| format!("{v:?}"),
            |_| "null".to_string(),
        );

        // First create a node for each non-escaped variable. We add these nodes in the order in
        // which they appear in the code because we want the names that appear earlier in the code
        // to be used when coalescing to variables that appear later in the code.
        let ordered_variables: Vec<VarId> = liveness.get_all_variables_in_order().to_vec();

        // index i in interferenceGraphNodes is set to true when interferenceGraph
        // has node orderedVariables[i]
        let mut interference_graph_nodes = BitSet::new(64);

        // interferenceBitSet[i] = indices of all variables that should have an edge with
        // orderedVariables[i]
        let mut interference_bit_set: Vec<BitSet> = (0..ordered_variables.len())
            .map(|_| BitSet::new(64))
            .collect();

        let mut v_index: i32 = -1;
        for &v in &ordered_variables {
            v_index += 1;
            // Java Set<Var>.contains uses the inherited ScopedName equality.
            if escaped.iter().any(|e| e.equals(compiler, v)) {
                continue;
            }

            // NOTE(user): In theory, we CAN coalesce function names just like any variables. Our
            // Liveness analysis captures this just like it as described in the specification.
            // However, we saw some zipped and unzipped size increase after this. We are not
            // totally sure why that is but, for now, we will respect the dead functions and not
            // play around with it
            if v.get_parent_node(compiler).unwrap().is_function(compiler) {
                continue;
            }

            // NOTE: we skip class declarations for a combination of two reasons:
            // 1. they are block-scoped, so we would need to rewrite them as class expressions
            //      e.g. `class C {}` -> `var C = class {}` to avoid incorrect semantics
            //      (see testDontCoalesceClassDeclarationsWithDestructuringDeclaration).
            //    This is possible but increases pre-gzip code size and complexity.
            // 2. since function declaration coalescing seems to cause a size regression (as
            //    discussed above) we assume that coalescing class names may cause a similar size
            //    regression.
            if v.get_parent_node(compiler).unwrap().is_class(compiler) {
                continue;
            }

            // Skip lets and consts that have multiple variables declared in them, otherwise this
            // produces incorrect semantics. See test case "testCapture".
            // Skipping vars technically isn't needed for correct semantics, but works around a
            // Safari bug for var redeclarations
            // (https://github.com/google/closure-compiler/issues/3164)
            if Self::is_in_multiple_lvalue_decl(compiler, v) {
                continue;
            }

            interference_graph.create_node(v);
            interference_graph_nodes.set(v_index);
        }

        // Go through every CFG node in the program and look at variables that are live.
        // Set the pair of live variables in interferenceBitSet so we can add an edge between them.
        for cfg_node in cfg.get_nodes() {
            if cfg.is_implicit_return(cfg_node) {
                continue;
            }

            let state = cfg_node
                .get_annotation_as::<LinearFlowState<LiveVariableLattice>>(cfg)
                .unwrap();

            // Check the live states and add edge when possible. An edge between two variables
            // means that they are alive at overlapping times, which means that their
            // variable names cannot be coalesced.
            let livein = state.get_in();
            let mut i = livein.next_set_bit(0);
            while i >= 0 {
                let mut j = livein.next_set_bit(i);
                while j >= 0 {
                    interference_bit_set[i as usize].set(j);
                    j = livein.next_set_bit(j + 1);
                }
                i = livein.next_set_bit(i + 1);
            }
            let liveout = state.get_out();
            let mut i = liveout.next_set_bit(0);
            while i >= 0 {
                let mut j = liveout.next_set_bit(i);
                while j >= 0 {
                    interference_bit_set[i as usize].set(j);
                    j = liveout.next_set_bit(j + 1);
                }
                i = liveout.next_set_bit(i + 1);
            }

            let root = cfg_node.get_value(cfg).unwrap();
            let mut live_range_checker = LiveRangeChecker::new(root, &ordered_variables, state);
            live_range_checker.check(compiler, root);
            live_range_checker.set_crossing_variables(&mut interference_bit_set);
        }

        // Go through each variable and try to connect them.
        let mut v1_index: i32 = -1;
        for &v1 in &ordered_variables {
            v1_index += 1;

            let mut v2_index: i32 = -1;
            for &v2 in &ordered_variables {
                v2_index += 1;
                // Skip duplicate pairs. Also avoid merging a variable with itself.
                if v1_index > v2_index {
                    continue;
                }

                if !interference_graph_nodes.get(v1_index)
                    || !interference_graph_nodes.get(v2_index)
                {
                    // Skip nodes that were not added. They are globals and escaped locals.
                    continue;
                }

                if (v1.is_param(compiler) && v2.is_param(compiler))
                    || interference_bit_set[v1_index as usize].get(v2_index)
                {
                    // Add an edge between variable pairs that are both parameters
                    // because we don't want parameters to share a name.
                    interference_graph.connect_if_not_found(v1, (), v2);
                }
            }
        }
        interference_graph
    }

    /// Returns whether this variable's declaration also declares other names.
    ///
    /// For example, this would return true for `x` in `let [x, y, z] = []`;
    // port: CoalesceVariableNames#isInMultipleLvalueDecl
    fn is_in_multiple_lvalue_decl(compiler: &mut AbstractCompiler, v: VarId) -> bool {
        let declaration_type = v.declaration_type(compiler);
        match declaration_type {
            Some(Token::LET | Token::CONST | Token::VAR) => {
                let name_decl = NodeUtil::get_enclosing_node(
                    compiler,
                    v.get_node(compiler).unwrap(),
                    &|ast, n| NodeUtil::is_name_declaration(ast, Some(n)),
                )
                .unwrap();

                let mut count = 0; // for lambda access
                NodeUtil::visit_lhs_nodes_in_node(compiler, name_decl, &mut |_, _lhs| count += 1);
                count > 1
            }
            _ => false,
        }
    }

    /// Remove variable declaration if the variable has been coalesced with another variable that
    /// has already been declared.
    ///
    /// A precondition is that if the variable has already been declared, it must be the only
    /// lvalue in said declaration. For example, this method will not accept `var x = 1, y = 2`.
    /// In theory we could leave in the `var` declaration, but var shadowing of params triggers a
    /// Safari bug: https://bugs.webkit.org/show_bug.cgi?id=182414 Another
    ///
    /// `name`: name node of the variable being coalesced
    // port: CoalesceVariableNames#removeVarDeclaration
    fn remove_var_declaration(compiler: &mut AbstractCompiler, name: NodeId) {
        let var = NodeUtil::get_enclosing_node(compiler, name, &|ast, n| {
            NodeUtil::is_name_declaration(ast, Some(n))
        })
        .unwrap();
        let parent = var.get_parent(compiler).unwrap();

        if var
            .get_first_child(compiler)
            .unwrap()
            .is_destructuring_lhs(compiler)
        {
            // convert `const [x] = arr` to `[x] = arr`
            // a precondition for this method is that `x` is the only lvalue in the destructuring
            // pattern
            let destructuring_lhs = var.get_first_child(compiler).unwrap();
            let pattern = destructuring_lhs.remove_first_child(compiler).unwrap();
            if NodeUtil::is_enhanced_for(compiler, parent) {
                var.replace_with(compiler, pattern);
            } else {
                let rvalue = var
                    .get_first_first_child(compiler)
                    .unwrap()
                    .detach(compiler);
                let assign = IR::assign(compiler, pattern, rvalue).srcref(compiler, var);
                let expr = NodeUtil::new_expr(compiler, assign);
                var.replace_with(compiler, expr);
            }
        } else if NodeUtil::is_enhanced_for(compiler, parent) {
            // convert `for (let x of ...` to `for (x of ...`
            let detached = name.detach(compiler);
            var.replace_with(compiler, detached);
        } else {
            // either `var x = 0;` or `var x;`
            check_state!(
                var.has_one_child(compiler) && var.get_first_child(compiler) == Some(name),
                "%s",
                var.to_string(compiler)
            );
            if name.has_children(compiler) {
                // convert `let x = 0;` to `x = 0;`
                let value = name.remove_first_child(compiler).unwrap();
                name.detach(compiler);
                let mut assign = IR::assign(compiler, name, value).srcref(compiler, name);

                // We don't need to wrapped it with EXPR node if it is within a FOR.
                if !parent.is_vanilla_for(compiler) {
                    assign = NodeUtil::new_expr(compiler, assign);
                }
                var.replace_with(compiler, assign);
            } else {
                // convert `let x;` to ``
                // and `for (let x;;) {}` to `for (;;) {}`
                // We can expect uninitialized declarations at this point and it's okay to remove
                // them and they've been coalesced with another declaration.
                NodeUtil::remove_child(compiler, parent, var);
            }
        }
    }

    /// Convert `const` or `let` declarations to `var` declarations.
    ///
    /// This method should be called on the first declared variable of a group that are being
    /// coalesced.
    ///
    /// Because the code has already been normalized by the time this pass runs, we can safely
    /// redeclare any let and const coalesced variables as vars
    // port: CoalesceVariableNames#makeDeclarationVar
    fn make_declaration_var(&self, compiler: &mut AbstractCompiler, coalesced_name: VarId) {
        if coalesced_name.is_const(compiler) || coalesced_name.is_let(compiler) {
            let name_node = check_not_null!(
                coalesced_name.get_name_node(compiler),
                "%s",
                coalesced_name.to_string(compiler)
            );
            if Self::is_uninitialized_let_name_in_loop_body(compiler, name_node) {
                // We need to make sure that within a loop:
                //
                // `let x;`
                // becomes
                // `var x = void 0;`
                //
                // If we don't we won't be correctly resetting the variable to undefined on each
                // loop iteration once we turn it into a var declaration.
                //
                // Note that all other cases will already have an initializer.
                // const x = 1; // constant requires an initializer
                // let {x, y} = obj; // destructuring requires an initializer
                // let [x, y] = iterable; // destructuring requires an initializer
                let undefined_value = self
                    .ast_factory
                    .create_undefined_value(compiler)
                    .srcref_tree(compiler, name_node);
                name_node.add_child_to_front(compiler, undefined_value);
            }
            // find the declaration node in a way that works normal and destructuring
            // declarations.
            let decl_node = NodeUtil::get_enclosing_node(
                compiler,
                name_node.get_parent(compiler).unwrap(),
                &|ast, n| NodeUtil::is_name_declaration(ast, Some(n)),
            )
            .unwrap();
            // normalization ensures that all variables in a function are uniquely named, so it's
            // OK to turn a `const` or `let` into a `var`.
            decl_node.set_token(compiler, Token::VAR);
        }
    }

    // port: CoalesceVariableNames#isUninitializedLetNameInLoopBody
    fn is_uninitialized_let_name_in_loop_body(
        compiler: &AbstractCompiler,
        name_node: NodeId,
    ) -> bool {
        check_state!(
            name_node.is_name(compiler),
            "%s",
            name_node.to_string(compiler)
        );
        let let_node = name_node.get_parent(compiler).unwrap();
        if !let_node.is_let(compiler) {
            // We're looking for `let name;`
            // Note that in the case of destructuring an initializer always exists.
            // `let {name} = initializerRequiredHere;
            return false;
        }
        if name_node.has_one_child(compiler) {
            // `let name = child;` has an initializer
            return false;
        }

        let let_parent = let_node.get_parent(compiler).unwrap();
        if NodeUtil::is_loop_structure(compiler, let_parent) {
            // `for (let x; ...`
            // `for (let x in ...`
            // `for (let x of ...`
            // `for await (let x of ...`
            // In all these cases the variable gets initialized on each loop iteration
            return false;
        }
        // Inside a loop body, but not the loop control node itself
        NodeUtil::is_within_loop(compiler, let_parent)
    }
}

impl CompilerPass for CoalesceVariableNames {
    // port: CoalesceVariableNames#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // checkNotNull(externs) and checkNotNull(root): a NodeId is never null.
        let mut scope_creator = self.scope_creator.clone();
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut scope_creator)
            .traverse(root);
        compiler.set_life_cycle_stage(LifeCycleStage::RAW);
    }
}

impl Callback for CoalesceVariableNames {
    // port: NodeTraversal.AbstractCfgCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CoalesceVariableNames#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if self.colorings.is_empty() || !n.is_name(t) || parent.unwrap().is_function(t) {
            // Don't rename named functions.
            return;
        }
        let parent = parent.unwrap();

        let name = n.get_string(t);
        let var = self.liveness().get_all_variables().get(&name).copied();
        let coloring = self.colorings.front_mut().unwrap();
        let v_node = var.and_then(|var| coloring.graph.get_node(&var));
        let Some(v_node) = v_node else {
            // This is not a local.
            return;
        };
        let var = var.unwrap();
        let coalesced_var = coloring
            .coloring
            .get_partition_super_node(&coloring.graph, var);
        let v_node_value = *v_node.get_value(&coloring.graph);

        if !self.use_pseudo_names {
            if v_node_value.equals(t.get_compiler(), coalesced_var) {
                // The coalesced name is itself, nothing to do.
                return;
            }

            // Rename.
            let coalesced_name = coalesced_var.get_name(t.get_compiler());
            n.set_string(t, coalesced_name);
            t.get_compiler().report_change_to_enclosing_scope(n);
            self.update_declarations_post_coalescing(t.get_compiler(), n, coalesced_var, parent);
        } else {
            // This code block is slow but since usePseudoName is for debugging,
            // we should not sacrifice performance for non-debugging compilation to
            // make this fast.
            let mut all_merged_names: BTreeSet<JsString> = BTreeSet::new();
            let coloring = self.colorings.front().unwrap();
            for &i_var in self.liveness().get_all_variables_in_order() {
                // Look for all the variables that can be merged (in the graph by now)
                // and it is merged with the current coalescedVar.
                if coloring
                    .coloring
                    .get_graph(&coloring.graph)
                    .get_node(&i_var)
                    .is_some()
                    && coloring
                        .coloring
                        .have_same_color(&coloring.graph, &i_var, &coalesced_var)
                {
                    all_merged_names.insert(i_var.get_name(t.get_compiler()));
                }
            }

            // Keep its original name.
            if all_merged_names.len() == 1 {
                return;
            }

            // Joiner.on("_").join(allMergedNames)
            let mut pseudo_name = JsString::from("");
            for (i, merged_name) in all_merged_names.iter().enumerate() {
                if i > 0 {
                    pseudo_name = pseudo_name.concat(&JsString::from("_"));
                }
                pseudo_name = pseudo_name.concat(merged_name);
            }

            loop {
                let scope = t.get_scope();
                if !scope.has_slot(t.get_compiler(), &pseudo_name) {
                    break;
                }
                pseudo_name = pseudo_name.concat(&JsString::from("$"));
            }

            // Rename.
            n.set_string(t, pseudo_name);
            t.get_compiler().report_change_to_enclosing_scope(n);

            if v_node_value.equals(t.get_compiler(), coalesced_var) {
                return;
            }
            self.update_declarations_post_coalescing(t.get_compiler(), n, coalesced_var, parent);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CoalesceVariableNames {
    // port: NodeTraversal.AbstractCfgCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            self.cfgs.push_front(current_scope_root);
        }
        self.enter_scope_with_cfg(t);
    }

    // port: NodeTraversal.AbstractCfgCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        self.exit_scope_with_cfg(t);
        let current_scope_root = check_not_null!(t.get_scope_root());
        if NodeUtil::is_valid_cfg_root(t, current_scope_root) {
            check_not_null!(self.cfgs.pop_front());
        }
    }
}

/// Used to find written and read variables in the same CFG node so that the variable pairs can
/// be marked as interfering in an interference bit map. Indices of written and read variables are
/// put in a list. These two lists are used to mark each written variable as "crossing" all read
/// variables.
struct LiveRangeChecker<'a> {
    root: NodeId,
    state: &'a LinearFlowState<LiveVariableLattice>,
    ordered_variables: &'a [VarId],
    is_assign_to_list: Vec<i32>, // indices of written variables
    is_read_from_list: Vec<i32>, // indices of read variables
}

impl<'a> LiveRangeChecker<'a> {
    // port: CoalesceVariableNames.LiveRangeChecker#LiveRangeChecker
    fn new(
        root: NodeId,
        ordered_variables: &'a [VarId],
        state: &'a LinearFlowState<LiveVariableLattice>,
    ) -> Self {
        Self {
            root,
            ordered_variables,
            state,
            is_assign_to_list: Vec::new(),
            is_read_from_list: Vec::new(),
        }
    }

    // port: CoalesceVariableNames.LiveRangeChecker#check
    fn check(&mut self, compiler: &AbstractCompiler, n: NodeId) {
        // For most AST nodes, traverse the subtree in postorder because that's how the
        // expressions are evaluated.
        if n == self.root || !ControlFlowGraph::is_entering_new_cfg_node(compiler, n) {
            if (n.is_destructuring_lhs(compiler) && n.has_two_children(compiler))
                || (n.is_assign(compiler)
                    && n.get_first_child(compiler)
                        .unwrap()
                        .is_destructuring_pattern(compiler))
                || n.is_default_value(compiler)
            {
                // Evaluate the rhs of a destructuring assignment/declaration before the lhs.
                self.check(compiler, n.get_second_child(compiler).unwrap());
                self.check(compiler, n.get_first_child(compiler).unwrap());
            } else {
                let mut c = n.get_first_child(compiler);
                while let Some(cur) = c {
                    self.check(compiler, cur);
                    c = cur.get_next(compiler);
                }
            }
            if Self::should_visit(compiler, n) {
                self.visit(compiler, n, n.get_parent(compiler).unwrap());
            }
        }
    }

    // port: CoalesceVariableNames.LiveRangeChecker#visit
    fn visit(&mut self, compiler: &AbstractCompiler, n: NodeId, parent: NodeId) {
        for i_var in 0..self.ordered_variables.len() {
            if Self::is_assign_to(compiler, self.ordered_variables[i_var], n, parent) {
                self.is_assign_to_list.push(i_var as i32);
            }
        }
        if !self.is_assign_to_list.is_empty() {
            for i_var in 0..self.ordered_variables.len() {
                let var_out_live = self.state.get_out().is_live(i_var as i32);
                if var_out_live || Self::is_read_from(compiler, self.ordered_variables[i_var], n) {
                    self.is_read_from_list.push(i_var as i32);
                }
            }
        }
    }

    // port: CoalesceVariableNames.LiveRangeChecker#setCrossingVariables
    fn set_crossing_variables(&self, interference_bit_set: &mut [BitSet]) {
        for &i_written_var in &self.is_assign_to_list {
            for &i_read_var in &self.is_read_from_list {
                interference_bit_set[i_written_var as usize].set(i_read_var);
                interference_bit_set[i_read_var as usize].set(i_written_var);
            }
        }
    }

    /// Returns whether any LiveRangeChecker would be interested in the node.
    // port: CoalesceVariableNames.LiveRangeChecker#shouldVisit
    fn should_visit(compiler: &AbstractCompiler, n: NodeId) -> bool {
        n.is_name(compiler)
            || (n.has_children(compiler) && n.get_first_child(compiler).unwrap().is_name(compiler))
    }

    // port: CoalesceVariableNames.LiveRangeChecker#isAssignTo
    fn is_assign_to(compiler: &AbstractCompiler, var: VarId, n: NodeId, parent: NodeId) -> bool {
        if n.is_name(compiler) {
            if parent.is_param_list(compiler) {
                // In a function declaration, the formal parameters are assigned.
                return var.get_name(compiler) == n.get_string(compiler);
            } else if NodeUtil::is_name_declaration(compiler, Some(parent))
                && n.has_children(compiler)
            {
                // If this is a VAR declaration, if the name node has a child, we are
                // assigning to that name.
                return var.get_name(compiler) == n.get_string(compiler);
            } else if NodeUtil::is_lhs_by_destructuring(compiler, n) {
                return var.get_name(compiler) == n.get_string(compiler);
            }
        } else if NodeUtil::is_assignment_op(compiler, n) {
            // Lastly, any assignmentOP is also an assign.
            let name = n.get_first_child(compiler).unwrap();
            return name.is_name(compiler) && var.get_name(compiler) == name.get_string(compiler);
        }
        false // Definitely a read.
    }

    // port: CoalesceVariableNames.LiveRangeChecker#isReadFrom
    fn is_read_from(compiler: &AbstractCompiler, var: VarId, name: NodeId) -> bool {
        name.is_name(compiler)
            && var.get_name(compiler) == name.get_string(compiler)
            && !NodeUtil::is_name_decl_or_simple_assign_lhs(
                compiler,
                name,
                name.get_parent(compiler).unwrap(),
            )
    }
}
