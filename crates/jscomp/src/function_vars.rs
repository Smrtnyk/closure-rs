/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AbstractVar.java,
//   src/com/google/javascript/jscomp/DataFlowAnalysis.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/SyntacticScopeCreator.java.

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): the variables that
//! `NodeUtil::get_all_vars_declared_in_function` finds for a function, found from the `Ast` alone,
//! so that the per-function data-flow passes can analyse a function on a worker thread before
//! they create its scopes (which live in the compiler).
//!
//! `get_all_vars_declared_in_function` traverses the function (not the functions in it) and, for
//! every scope the traversal enters, lists the scope's variables in the order
//! `SyntacticScopeCreator` declared them. `FunctionVars::collect` enters the same scopes in the
//! same order and declares what `ScopeScanner` declares there, with the same tests for a
//! redeclaration. For a function with functions in it, it also finds what
//! `DataFlowAnalysis.computeEscaped` finds by closure capture: the variables of the function that
//! a name in a function below it resolves to, through the same scopes. It handles no function
//! with a CLASS or a synthetic block below it, or a name that may be an implicit variable
//! (`arguments`, `exports`, `super`, `this`): `None` for those, which the passes leave to the
//! scopes.
//!
//! The `VarId`s it hands out are not the compiler's: they stand for the variables of one function
//! (distinct variables have distinct ids), for analyses that compare variables but never look one
//! up in the compiler. They must not reach code that does.

use crate::{
    AbstractCompiler, AllVarsDeclaredInFunction, coding_convention::CodingConvention,
    live_variables_analysis::MAX_VARIABLES_TO_ANALYZE, node_util::NodeUtil, var::VarId,
};
use closure_rhino::{
    fast_hash::IndexMap,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::num::NonZeroU32;

/// One variable of a function, as `FunctionVars::collect` declared it.
pub(crate) struct FunctionVar {
    pub(crate) name: JsString,
    pub(crate) name_node: NodeId,
    /// The index of the variable's scope in `FunctionVars::scopes`.
    pub(crate) scope: usize,
    /// `AbstractVar#declarationType`.
    pub(crate) declaration_type: Token,
}

impl FunctionVar {
    // port: AbstractVar#isParam
    pub(crate) fn is_param(&self) -> bool {
        self.declaration_type == Token::PARAM_LIST
    }

    // port: AbstractVar#isCatch
    pub(crate) fn is_catch(&self) -> bool {
        self.declaration_type == Token::CATCH
    }
}

/// The variables of a function, as `get_all_vars_declared_in_function` lists them.
pub(crate) struct FunctionVars {
    /// The scope roots in the order the traversal enters them: the FUNCTION, then (for a body
    /// that is a block) the function block, then the block scopes in it.
    pub(crate) scopes: Vec<NodeId>,
    /// The number of variables of each scope (`AbstractScope#getVarCount`).
    pub(crate) scope_var_counts: Vec<usize>,
    /// `AllVarsDeclaredInFunction#getAllVariablesInOrder`.
    pub(crate) ordered: Vec<FunctionVar>,
    /// `AllVarsDeclaredInFunction#getAllVariables`, as indexes into `ordered`.
    pub(crate) all: IndexMap<JsString, usize>,
    /// The variables (indexes into `ordered`) that a name in a function below this one resolves
    /// to, in the order `computeEscaped` meets them (with repeats).
    captured: Vec<usize>,
}

impl FunctionVars {
    /// The variables of `function`, or `None` for a function this collector leaves to the scopes
    /// (see the module comment).
    pub(crate) fn collect(ast: &Ast, function: NodeId) -> Option<Self> {
        if !function.is_function(ast) {
            return None;
        }
        let collector = Collector::scan_function(ast, function)?;
        let has_nested_function = collector.has_nested_function;
        // Rust-only (performance divergence): the tables are made at their final sizes, which
        // the scan knows (only capacity changes).
        let total: usize = collector.scope_vars.iter().map(IndexMap::len).sum();
        let mut vars = FunctionVars {
            scopes: collector.scopes,
            scope_var_counts: Vec::with_capacity(collector.scope_vars.len()),
            ordered: Vec::with_capacity(total),
            all: IndexMap::with_capacity_and_hasher(total, Default::default()),
            captured: Vec::new(),
        };
        // Per scope, the index into `ordered` of each of its variables.
        let mut scope_indexes: Vec<IndexMap<JsString, usize>> =
            Vec::with_capacity(collector.scope_vars.len());
        for scope_vars in collector.scope_vars {
            vars.scope_var_counts.push(scope_vars.len());
            let mut indexes =
                IndexMap::with_capacity_and_hasher(scope_vars.len(), Default::default());
            for var in scope_vars.into_values() {
                // `NodeUtil.getAllVarsDeclaredInFunction`: `nameVarMap.put(v.getName(), v)`.
                vars.all.insert(var.name.clone(), vars.ordered.len());
                indexes.insert(var.name.clone(), vars.ordered.len());
                vars.ordered.push(var);
            }
            scope_indexes.push(indexes);
        }
        if has_nested_function {
            let mut frames: IndexMap<NodeId, Frame> = IndexMap::default();
            for (scope, indexes) in scope_indexes.into_iter().enumerate() {
                frames.insert(vars.scopes[scope], Frame::Own(indexes));
            }
            let mut chain = Vec::new();
            let mut captured = Vec::new();
            Self::capture_walk(
                ast,
                function,
                function,
                &mut frames,
                &mut chain,
                0,
                &mut captured,
            )?;
            vars.captured = captured;
        }
        Some(vars)
    }

    // port: DataFlowAnalysis#computeEscaped (the traversal)
    /// The traversal of `computeEscaped` below `function` (the function of these variables):
    /// `chain` holds the roots of the scopes it is in, innermost last, `depth` the number of
    /// functions below `function` it is in. A name below such a function (not the name of a
    /// function) that resolves, through `chain`, to a variable of `function` is captured.
    #[allow(clippy::too_many_arguments)]
    fn capture_walk(
        ast: &Ast,
        function: NodeId,
        n: NodeId,
        frames: &mut IndexMap<NodeId, Frame>,
        chain: &mut Vec<NodeId>,
        depth: usize,
        captured: &mut Vec<usize>,
    ) -> Option<()> {
        let token = n.get_token(ast);
        let mut depth = depth;
        let enters_scope = if token == Token::FUNCTION {
            if n != function {
                depth += 1;
                // The scopes of a function below: the same scan as that of `function`.
                let inner = Collector::scan_function(ast, n)?;
                for (root, scope_vars) in inner.scopes.into_iter().zip(inner.scope_vars) {
                    frames.insert(root, Frame::Inner(scope_vars.into_keys().collect()));
                }
            }
            true
        } else {
            Collector::traversal_creates_block_scope(ast, n)?
        };
        if enters_scope {
            if !frames.contains_key(&n) {
                return None;
            }
            chain.push(n);
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            Self::capture_walk(ast, function, c, frames, chain, depth, captured)?;
            child = c.get_next(ast);
        }
        if token == Token::NAME && depth > 0 && !n.get_parent(ast)?.is_function(ast) {
            // `scope.getVar(n)`: the innermost scope that declares the name.
            let name = n.get_string_ref(ast);
            for root in chain.iter().rev() {
                match &frames[root] {
                    Frame::Inner(names) => {
                        if names.contains(name) {
                            break;
                        }
                    }
                    Frame::Own(indexes) => {
                        if let Some(&index) = indexes.get(name) {
                            captured.push(index);
                            break;
                        }
                    }
                }
            }
        }
        if enters_scope {
            chain.pop();
        }
        Some(())
    }

    /// The stand-in id of `ordered[index]`.
    pub(crate) fn id(index: usize) -> VarId {
        VarId(NonZeroU32::new(u32::try_from(index + 1).unwrap()).unwrap())
    }

    /// The index into `ordered` of a stand-in id.
    pub(crate) fn index(id: VarId) -> usize {
        id.0.get() as usize - 1
    }

    /// `getAllVariables` with stand-in ids.
    pub(crate) fn all_vars_in_fn(&self) -> IndexMap<JsString, VarId> {
        self.all
            .iter()
            .map(|(name, &index)| (name.clone(), Self::id(index)))
            .collect()
    }

    /// `getAllVariablesInOrder` with stand-in ids.
    pub(crate) fn ordered_ids(&self) -> Vec<VarId> {
        (0..self.ordered.len()).map(Self::id).collect()
    }

    /// The root of the scope of `ordered[index]`.
    pub(crate) fn scope_root(&self, index: usize) -> NodeId {
        self.scopes[self.ordered[index].scope]
    }

    /// The variable count of the function scope plus that of the function block scope, as
    /// `DeadAssignmentsElimination` compares it with `MAX_VARIABLES_TO_ANALYZE`.
    pub(crate) fn function_and_block_var_count(&self) -> usize {
        self.scope_var_counts[0] + self.scope_var_counts.get(1).copied().unwrap_or(0)
    }

    /// The variable count of the function block scope.
    pub(crate) fn block_var_count(&self) -> usize {
        self.scope_var_counts.get(1).copied().unwrap_or(0)
    }

    /// What `DataFlowAnalysis.computeEscaped` finds, in its order: the captured variables, then
    /// those of `getAllVariables` that are catch variables or have an exported name.
    pub(crate) fn escaped(&self, convention: &dyn CodingConvention) -> Vec<usize> {
        let mut escaped: closure_rhino::fast_hash::IndexSet<usize> = Default::default();
        escaped.extend(self.captured.iter().copied());
        for &index in self.all.values() {
            let var = &self.ordered[index];
            if var.is_catch() || convention.is_exported_name(&var.name) {
                escaped.insert(index);
            }
        }
        escaped.into_iter().collect()
    }

    /// For `CLOSURE_RS_PARALLEL_ANALYSIS=verify`: panics unless these are the variables that the
    /// scopes give for the function: `all_vars` (`get_all_vars_declared_in_function`) and, when
    /// given, the variable counts of the function scope and the function block scope.
    pub(crate) fn verify(
        &self,
        compiler: &AbstractCompiler,
        all_vars: &AllVarsDeclaredInFunction,
        counts: Option<(i32, i32)>,
        escaped: &closure_rhino::fast_hash::IndexSet<VarId>,
    ) {
        let ordered = all_vars.get_all_variables_in_order();
        let describe = || {
            format!(
                "CLOSURE_RS_PARALLEL_ANALYSIS=verify: FunctionVars differ for {} at {}",
                self.scopes[0].to_string(compiler),
                self.scopes[0].get_lineno(compiler)
            )
        };
        assert_eq!(ordered.len(), self.ordered.len(), "{}", describe());
        for (var, mine) in ordered.iter().zip(&self.ordered) {
            assert!(
                var.get_name(compiler) == mine.name
                    && var.get_name_node(compiler) == Some(mine.name_node)
                    && var.get_scope_root(compiler) == self.scopes[mine.scope]
                    && var.is_param(compiler) == mine.is_param()
                    && var.is_catch(compiler) == mine.is_catch(),
                "{} (variable {})",
                describe(),
                mine.name
            );
        }
        let all = all_vars.get_all_variables();
        assert_eq!(all.len(), self.all.len(), "{}", describe());
        for ((name, var), (my_name, &index)) in all.iter().zip(&self.all) {
            assert!(
                name == my_name && ordered.get(index) == Some(var),
                "{} (getAllVariables {})",
                describe(),
                name
            );
        }
        if let Some((function_count, block_count)) = counts {
            assert!(
                function_count as usize == self.scope_var_counts[0]
                    && block_count as usize == self.block_var_count(),
                "{} (counts)",
                describe()
            );
        }
        // `AbstractVar#equals`: the scopes may hand out another id for the same variable.
        let key = |var: &VarId| (var.get_name(compiler), var.get_scope_root(compiler));
        let mine: closure_rhino::fast_hash::IndexSet<(JsString, NodeId)> = self
            .escaped(compiler.get_coding_convention())
            .into_iter()
            .map(|index| key(&ordered[index]))
            .collect();
        let theirs: closure_rhino::fast_hash::IndexSet<(JsString, NodeId)> =
            escaped.iter().map(key).collect();
        assert!(
            mine.len() == theirs.len() && mine.iter().all(|var| theirs.contains(var)),
            "{} (escaped: {:?}, scopes: {:?})",
            describe(),
            mine.iter().map(|var| var.0.to_string()).collect::<Vec<_>>(),
            theirs
                .iter()
                .map(|var| var.0.to_string())
                .collect::<Vec<_>>()
        );
    }

    /// Whether the counts are far enough from `MAX_VARIABLES_TO_ANALYZE` that no comparison of
    /// them with it can be off by one (callers compare in several ways).
    pub(crate) fn well_below_max(&self) -> bool {
        (self.ordered.len() as i32) < MAX_VARIABLES_TO_ANALYZE - 1
            && (self.function_and_block_var_count() as i32) < MAX_VARIABLES_TO_ANALYZE - 1
    }
}

/// The scopes of a function in `capture_walk`, by root: those of the function itself (with the
/// index into `FunctionVars::ordered` of each variable) or of a function below it.
enum Frame {
    Own(IndexMap<JsString, usize>),
    Inner(closure_rhino::fast_hash::IndexSet<JsString>),
}

/// The scopes `get_all_vars_declared_in_function` enters for one function, and their variables.
struct Collector<'a> {
    ast: &'a Ast,
    /// The scope roots, in the order the traversal enters them.
    scopes: Vec<NodeId>,
    /// The variables of each scope by name, in declaration order (`ScopeData#vars`).
    scope_vars: Vec<IndexMap<JsString, FunctionVar>>,
    /// The index of the function block scope.
    function_block: Option<usize>,
    /// Whether `walk` met a FUNCTION.
    has_nested_function: bool,
}

impl<'a> Collector<'a> {
    /// The scopes of `function`, or `None` when this collector leaves it to the scopes: when the
    /// function (not counting the functions in it) holds a CLASS, a synthetic block or a name
    /// that may be an implicit variable.
    fn scan_function(ast: &'a Ast, function: NodeId) -> Option<Self> {
        let mut collector = Collector {
            ast,
            scopes: Vec::new(),
            scope_vars: Vec::new(),
            function_block: None,
            has_nested_function: false,
        };
        let fn_name = function.get_first_child(ast)?;
        collector.check_name(fn_name)?;
        collector.enter_function_scope(function)?;
        let params = fn_name.get_next(ast)?;
        let body = params.get_next(ast)?;
        // The traversal enters no scope in the name or the parameters (no CLASS is below them,
        // and it does not enter the functions there).
        collector.walk(params)?;
        collector.walk(body)?;
        Some(collector)
    }

    /// `None` for a name that may be an implicit variable (`arguments`, `exports`, `super`,
    /// `this`): scopes resolve those differently, and looking one up may create it.
    fn check_name(&self, n: NodeId) -> Option<()> {
        let ast = self.ast;
        if n.is_name(ast) && crate::abstract_scope::ImplicitVar::of(n.get_string_ref(ast)).is_some()
        {
            return None;
        }
        Some(())
    }

    fn new_scope(&mut self, root: NodeId) -> usize {
        self.scopes.push(root);
        self.scope_vars.push(IndexMap::default());
        self.scopes.len() - 1
    }

    // port: SyntacticScopeCreator.ScopeScanner#populate (FUNCTION)
    fn enter_function_scope(&mut self, function: NodeId) -> Option<()> {
        let ast = self.ast;
        let scope = self.new_scope(function);
        let fn_name = function.get_first_child(ast)?;
        let args = fn_name.get_next(ast)?;
        if !args.is_param_list(ast) {
            return None;
        }
        self.declare_lhs(scope, args)?;
        if !fn_name.get_string_ref(ast).is_empty()
            && NodeUtil::is_function_expression(ast, function)
        {
            self.declare_var(scope, fn_name)?;
        }
        Some(())
    }

    // port: NodeTraversal#createsBlockScope
    /// `None` for a synthetic block (left to the scopes); no CLASS is below the function.
    fn traversal_creates_block_scope(ast: &Ast, n: NodeId) -> Option<bool> {
        Some(match n.get_token(ast) {
            Token::BLOCK => {
                if n.is_synthetic_block(ast) {
                    return None;
                }
                n.get_parent(ast)
                    .is_some_and(|p| !NodeUtil::is_switch_case(ast, p) && !p.is_catch(ast))
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY => true,
            _ => false,
        })
    }

    /// The traversal below the function scope (it does not enter the functions below): every
    /// scope it enters, in order (a pre-order walk; with no FUNCTION entered and no CLASS, all
    /// scope roots are statements and the traversal visits the children of each node in order).
    fn walk(&mut self, n: NodeId) -> Option<()> {
        let ast = self.ast;
        match n.get_token(ast) {
            Token::FUNCTION => {
                self.has_nested_function = true;
                return Some(());
            }
            Token::CLASS => return None,
            Token::NAME => self.check_name(n)?,
            _ => {}
        }
        if Self::traversal_creates_block_scope(ast, n)? {
            self.enter_block_scope(n)?;
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            self.walk(c)?;
            child = c.get_next(ast);
        }
        Some(())
    }

    // port: SyntacticScopeCreator.ScopeScanner#populate (BLOCK, FOR, FOR_OF, FOR_AWAIT_OF, FOR_IN,
    // SWITCH_BODY)
    fn enter_block_scope(&mut self, root: NodeId) -> Option<()> {
        let ast = self.ast;
        let scope = self.new_scope(root);
        if NodeUtil::is_function_block(ast, root) {
            self.function_block = Some(scope);
            self.scan_vars(root, Some(scope), Some(scope), root)
        } else {
            self.scan_vars(root, None, Some(scope), root)
        }
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareLHS
    fn declare_lhs(&mut self, s: usize, n: NodeId) -> Option<()> {
        let ast = self.ast;
        if n.has_one_child(ast) && n.get_first_child(ast)?.is_name(ast) {
            return self.declare_var(s, n.get_first_child(ast)?);
        }
        let mut lhs_nodes = Vec::new();
        NodeUtil::visit_lhs_nodes_in_node_readonly(ast, n, &mut |lhs| lhs_nodes.push(lhs));
        for lhs in lhs_nodes {
            self.declare_var(s, lhs)?;
        }
        Some(())
    }

    // port: SyntacticScopeCreator.ScopeScanner#scanVars
    /// `block_root` is the root of `block_scope` (when that is set).
    fn scan_vars(
        &mut self,
        n: NodeId,
        hoist_scope: Option<usize>,
        block_scope: Option<usize>,
        block_root: NodeId,
    ) -> Option<()> {
        let ast = self.ast;
        match n.get_token(ast) {
            Token::VAR => {
                if let Some(hoist_scope) = hoist_scope {
                    self.declare_lhs(hoist_scope, n)?;
                }
                return Some(());
            }
            Token::LET | Token::CONST => {
                if let Some(block_scope) = block_scope {
                    self.declare_lhs(block_scope, n)?;
                }
                return Some(());
            }
            // Never in a function body.
            Token::IMPORT | Token::EXPORT | Token::SCRIPT | Token::MODULE_BODY => return None,
            Token::FUNCTION => {
                if NodeUtil::is_function_expression(ast, n) || block_scope.is_none() {
                    return Some(());
                }
                let fn_name_node = n.get_first_child(ast)?;
                if fn_name_node.get_string_ref(ast).is_empty() {
                    return Some(());
                }
                return self.declare_var(block_scope?, fn_name_node);
            }
            // No CLASS is below the function.
            Token::CLASS => return None,
            Token::CATCH => {
                if !n.has_two_children(ast) {
                    return None;
                }
                if let Some(block_scope) = block_scope {
                    self.declare_lhs(block_scope, n)?;
                }
                let block = n.get_second_child(ast)?;
                return self.scan_vars(block, hoist_scope, block_scope, block_root);
            }
            // The goog.provide and goog.module cases need a SCRIPT parent.
            Token::EXPR_RESULT if n.get_parent(ast)?.is_script(ast) => return None,
            _ => {}
        }
        self.scan_vars_of_children(n, hoist_scope, block_scope, block_root)
    }

    // port: SyntacticScopeCreator.ScopeScanner#scanVars (the children)
    fn scan_vars_of_children(
        &mut self,
        n: NodeId,
        hoist_scope: Option<usize>,
        block_scope: Option<usize>,
        block_root: NodeId,
    ) -> Option<()> {
        let ast = self.ast;
        let is_block_start = block_scope.is_some() && n == block_root;
        let entering_new_block = !is_block_start && NodeUtil::creates_block_scope(ast, n);
        if entering_new_block && hoist_scope.is_none() {
            return Some(());
        }
        if NodeUtil::is_shallow_statement_tree(ast, Some(n)) {
            let mut child = n.get_first_child(ast);
            while let Some(current) = child {
                let next = current.get_next(ast);
                if entering_new_block {
                    self.scan_vars(current, hoist_scope, None, block_root)?;
                } else {
                    self.scan_vars(current, hoist_scope, block_scope, block_root)?;
                }
                child = next;
            }
        }
        Some(())
    }

    // port: SyntacticScopeCreator.ScopeScanner#declareVar
    fn declare_var(&mut self, s: usize, n: NodeId) -> Option<()> {
        let ast = self.ast;
        if !n.is_name(ast) {
            return None;
        }
        let name = n.get_string(ast);
        if name == VarId::ARGUMENTS {
            return None;
        }
        if let Some(existing) = self.scope_vars[s].get(&name) {
            if existing.name_node == n {
                return Some(());
            }
            // A redeclaration (the default handler does nothing).
            return Some(());
        }
        // port: SyntacticScopeCreator.ScopeScanner#isShadowingAllowed
        if Some(s) == self.function_block
            && self.scope_vars[0]
                .get(&name)
                .is_some_and(FunctionVar::is_param)
        {
            return Some(());
        }
        let declaration_type = Self::declaration_type(ast, n)?;
        self.scope_vars[s].insert(
            name.clone(),
            FunctionVar {
                name,
                name_node: n,
                scope: s,
                declaration_type,
            },
        );
        Some(())
    }

    // port: AbstractVar#declarationType
    fn declaration_type(ast: &Ast, name_node: NodeId) -> Option<Token> {
        let mut current = Some(name_node);
        while let Some(n) = current {
            let token = n.get_token(ast);
            if matches!(
                token,
                Token::VAR
                    | Token::LET
                    | Token::CONST
                    | Token::FUNCTION
                    | Token::CLASS
                    | Token::CATCH
                    | Token::IMPORT
                    | Token::PARAM_LIST
            ) {
                return Some(token);
            }
            current = n.get_parent(ast);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::FunctionVars;
    use crate::{
        Compiler,
        compiler_options::CompilerOptions,
        node_util::NodeUtil,
        syntactic_scope_creator::{SyntacticScopeCreator, ensure_ancestor_scope},
    };
    use closure_rhino::{fast_hash::IndexMap, input_id::InputId, node::NodeId};
    use std::sync::Arc;

    /// Checks `FunctionVars::collect` against the scopes for every function of `src` that it
    /// handles, and returns how many it handled and how many there are.
    fn check(src: &str) -> (usize, usize) {
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerOptions::new());
        let script = compiler.parse_test_code(src);
        script.set_input_id(&mut compiler, Some(Arc::new(InputId::new("test"))));
        assert!(compiler.get_errors().is_empty(), "{src}");
        let mut functions = Vec::new();
        let mut stack = vec![script];
        while let Some(n) = stack.pop() {
            if n.is_function(&compiler) {
                functions.push(n);
            }
            let mut children: Vec<NodeId> = n.children(&compiler).collect();
            children.reverse();
            stack.extend(children);
        }
        let mut handled = 0;
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut memo = IndexMap::default();
        for &function in &functions {
            let Some(vars) = FunctionVars::collect(&compiler.ast, function) else {
                continue;
            };
            handled += 1;
            let scope = ensure_ancestor_scope(
                &mut compiler,
                &mut scope_creator,
                &mut memo,
                script,
                function,
            );
            let all_vars = NodeUtil::get_all_vars_declared_in_function(
                &mut compiler,
                &mut scope_creator,
                scope,
            );
            let counts = NodeUtil::get_function_body(&compiler, function)
                .is_block(&compiler)
                .then(|| {
                    let body = NodeUtil::get_function_body(&compiler, function);
                    let block = scope_creator.create_scope(&mut compiler, body, Some(scope));
                    (
                        scope.get_var_count(&compiler),
                        block.get_var_count(&compiler),
                    )
                });
            let mut escaped = closure_rhino::fast_hash::IndexSet::default();
            crate::data_flow_analysis::compute_escaped(
                &mut compiler,
                scope,
                &mut escaped,
                &mut scope_creator,
                all_vars.get_all_variables(),
            );
            vars.verify(&compiler, &all_vars, counts, &escaped);
        }
        (handled, functions.len())
    }

    #[test]
    fn matches_the_scopes() {
        let cases = [
            "function f(a, b) { var c = a; let d = b; const e = c + d; return e; }",
            "function f(a) { var a; return a; }",
            "function f(a = 1, {b, c: [d, ...e]} = {}, ...g) { var b2 = a + b + d; return g; }",
            "var x = function g(a) { var g; return a; };",
            "function f() { if (x) { let a = 1; var b = a; function h() {} } else { const c = 2; } }",
            "function f() { for (let i = 0; i < 3; i++) { let j = i; } for (var k in o) {} \
             for (const v of w) { var u = v; } }",
            "function f(p) { switch (p) { case 1: let a = 1; var b = a; break; default: const c = 3; } }",
            "function f() { try { var a = 1; let b = 2; } catch ({message: m, stack}) { let c = m; \
             var d = stack; } finally { let e = 3; } }",
            "function f() { try { g(); } catch (e) { var e; } }",
            "function f() { try { g(); } catch { let x = 1; } }",
            "function f() { label: { let a = 1; break label; } do { let b; } while (c); \
             while (d) { var e; let f2; } }",
            "function f(a) { var b = 1; return function(c) { return a + b + c; }; }",
            "function f(a) { var b = 1; var c = 2; function g() { var b = 3; return b; } \
             return () => c + g(); }",
            "function f() { for (let i = 0; i < 3; i++) { setTimeout(() => i); } let j; \
             { let k; (function() { k = j; })(); } }",
            "function f(a) { return (b = a) => b + a; }",
            "function f() { var x = 1; if (y) { function x() {} } return x; }",
            "function f(a) { { let a = 1; } var b = function c() { return c; }; }",
            "function f() { let a = 1; { let b = a; { let c = b; (() => a + b + c)(); } } }",
            "function f() { var a; var a = 1; let [b, c] = [1, 2]; const {d, e: {f: g}} = h; }",
            "const f = (a, b) => a + b;",
            "function f() { var _private = 1; var $super = 2; return _private + $super; }",
            "function f(x) { x = x + 1; var y = x, z = y; return function() { return z; }; }",
        ];
        for src in cases {
            let (handled, total) = check(src);
            assert!(
                handled > 0,
                "no function handled in {src} ({total} functions)"
            );
        }
    }

    #[test]
    fn leaves_some_functions_to_the_scopes() {
        for src in [
            "function f() { return arguments.length; }",
            "function f() { class C {} return C; }",
            "function f() { return () => arguments[0]; }",
            "function f(exports) { return exports; }",
        ] {
            assert_eq!(check(src).0, 0, "{src}");
        }
    }
}
