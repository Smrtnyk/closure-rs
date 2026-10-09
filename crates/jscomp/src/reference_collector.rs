/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReferenceCollector.java.

//! Port of `ReferenceCollector.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::reference_collection::ReferenceCollection;
use crate::reference_map::ReferenceMap;
use crate::scope::ScopeId;
use crate::scope_creator::ScopeCreator;
use crate::var::VarId;
use closure_rhino::check_state;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::collections::VecDeque;
use std::sync::Arc;

/// A helper class for passes that want to access all information about where a variable is
/// referenced and declared at once and then make a decision as to how it should be handled,
/// possibly inlining, reordering, or generating warnings. Callers do this by providing
/// [`Behavior`] and then calling `process(Node, Node)`.
///
/// Java's inner `CollectorCallback` reads the outer instance's fields; here the traversal state
/// lives in [`CollectorCallback`] and the scope creator beside it, so a traversal can borrow
/// both (DESIGN §6: the compiler is not stored).
pub struct ReferenceCollector<'a> {
    callback: CollectorCallback<'a>,
    scope_creator: &'a mut dyn ScopeCreator,
}

/// Java's `Predicate<Var>`.
pub type VarFilter<'a> = Box<dyn Fn(&AbstractCompiler, VarId) -> bool + 'a>;

/// The fields of `ReferenceCollector` that its inner `CollectorCallback` reads and writes.
pub struct CollectorCallback<'a> {
    /// Maps a given variable to a collection of references to that name. Note that Var objects
    /// are not stable across multiple traversals (unlike scope root or name).
    reference_map: IndexMap<VarId, ReferenceCollection>,

    /// The stack of basic blocks and scopes the current traversal is in.
    block_stack: VecDeque<Arc<BasicBlock>>,

    /// Source of behavior at various points in the traversal.
    behavior: Box<dyn Behavior + 'a>,

    /// Only collect references for filtered variables.
    var_filter: VarFilter<'a>,

    collected_hoisted_functions: IndexSet<NodeId>,

    narrow_scope: Option<ScopeId>,
}

impl<'a> ReferenceCollector<'a> {
    pub const DO_NOTHING_BEHAVIOR: DoNothingBehavior = DoNothingBehavior;

    /// Constructor initializes block stack.
    // port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator)
    pub fn new(
        compiler: &mut AbstractCompiler,
        behavior: impl Behavior + 'a,
        creator: &'a mut dyn ScopeCreator,
    ) -> Self {
        Self::new_with_filter(compiler, behavior, creator, Box::new(|_, _| true))
    }

    /// Constructor only collects references that match the given variable.
    ///
    /// The test for Var equality uses reference equality, so it's necessary to inject a scope
    /// when you traverse.
    // port: ReferenceCollector#ReferenceCollector(AbstractCompiler,Behavior,ScopeCreator,Predicate)
    pub fn new_with_filter(
        _compiler: &mut AbstractCompiler,
        behavior: impl Behavior + 'a,
        creator: &'a mut dyn ScopeCreator,
        var_filter: VarFilter<'a>,
    ) -> Self {
        Self {
            callback: CollectorCallback {
                reference_map: IndexMap::<_, _>::default(),
                block_stack: VecDeque::new(),
                behavior: Box::new(behavior),
                var_filter,
                collected_hoisted_functions: IndexSet::<_>::default(),
                narrow_scope: None,
            },
            scope_creator: creator,
        }
    }

    // port: ReferenceCollector#process(Node)
    pub fn process(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        let Self {
            callback,
            scope_creator,
        } = self;
        create_traversal_builder(compiler, callback, &mut **scope_creator).traverse(root);
    }

    /// Targets reference collection to a particular scope.
    // port: ReferenceCollector#processScope
    pub fn process_scope(&mut self, compiler: &mut AbstractCompiler, scope: ScopeId) {
        let should_add_to_block_stack = !scope.is_hoist_scope(compiler);
        self.callback.narrow_scope = Some(scope);
        if should_add_to_block_stack {
            self.callback
                .push_new_block(compiler, scope.get_root_node(compiler));
        }
        {
            let Self {
                callback,
                scope_creator,
            } = self;
            create_traversal_builder(compiler, callback, &mut **scope_creator)
                .traverse_at_scope(scope);
        }
        if should_add_to_block_stack {
            self.callback
                .pop_last_block(compiler, scope.get_root_node(compiler));
        }
        self.callback.narrow_scope = None;
    }

    /// Gets the variables that were referenced in this callback.
    // port: ReferenceCollector#getAllSymbols
    pub fn get_all_symbols(&self) -> Vec<VarId> {
        self.callback.reference_map.keys().copied().collect()
    }

    // port: ReferenceCollector#getScope
    pub fn get_scope(&self, compiler: &AbstractCompiler, var: VarId) -> ScopeId {
        var.get_scope(compiler)
    }

    /// Gets the reference collection for the given variable.
    // port: ReferenceCollector#getReferences
    pub fn get_references(&self, v: VarId) -> Option<&ReferenceCollection> {
        self.callback.reference_map.get(&v)
    }
}

impl CompilerPass for ReferenceCollector<'_> {
    /// Convenience method for running this pass over a tree with this class as a callback.
    // port: ReferenceCollector#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let Self {
            callback,
            scope_creator,
        } = self;
        create_traversal_builder(compiler, callback, &mut **scope_creator)
            .traverse_roots(externs, root);
    }
}

impl ReferenceMap for ReferenceCollector<'_> {
    fn get_references(&self, var: VarId) -> Option<&ReferenceCollection> {
        ReferenceCollector::get_references(self, var)
    }
}

/// Same as process but only runs on a part of AST associated to one script.
// port: ReferenceCollector#createTraversalBuilder
fn create_traversal_builder<'b>(
    compiler: &'b mut AbstractCompiler,
    callback: &'b mut dyn Callback,
    scope_creator: &'b mut dyn ScopeCreator,
) -> crate::node_traversal::Builder<'b> {
    let mut builder = NodeTraversal::builder();
    builder
        .set_compiler(compiler)
        .set_callback(callback)
        .set_scope_creator(scope_creator)
        .set_obey_destructuring_and_default_value_execution_order(true);
    builder
}

impl CollectorCallback<'_> {
    // port: ReferenceCollector#maybeJumpToHoistedFunction
    fn maybe_jump_to_hoisted_function(&mut self, var: VarId, t: &mut NodeTraversal<'_>) {
        let fn_node = var.get_parent_node(t.get_compiler());
        let var_scope = var.get_scope(t.get_compiler());
        let Some(fn_node) = fn_node else {
            return;
        };
        if !NodeUtil::is_hoisted_function_declaration(t.get_compiler(), fn_node)
            // If we're only traversing a narrow scope, do not try to climb outside.
            || self.narrow_scope.is_some_and(|narrow_scope| {
                narrow_scope.get_depth(t.get_compiler()) > var_scope.get_depth(t.get_compiler())
            })
            || self.collected_hoisted_functions.contains(&fn_node)
        {
            return;
        }

        // Replace the block stack with a new one matching the hoist position.
        //
        // This algorithm only works because we know hoisted functions cannot be inside loops. It
        // will have to change if we ever do general function continuations.
        //
        // This is tricky to compute because of the weird traverseAtScope call for
        // AggressiveInlineAliases.
        let old_block_stack = std::mem::take(&mut self.block_stack);
        if var_scope.is_global(t.get_compiler()) {
            self.block_stack
                .push_back(old_block_stack.front().expect("getFirst").clone());
        } else {
            let var_scope_root = var_scope.get_root_node(t.get_compiler());
            for b in &old_block_stack {
                self.block_stack.push_back(b.clone());
                if b.get_root() == var_scope_root {
                    break;
                }
            }
        }

        // Record the function declaration reference explicitly because it is a bleeding name.
        // The reference must be recorded with the containing scope, and traverseAtScope skips
        // bleeding function NAME nodes
        let fn_name = fn_node.get_first_child(t.get_compiler()).unwrap();
        self.add_reference(var, fn_name, t);

        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
        let fn_scope = scope_creator.create_scope(compiler, fn_node, Some(var_scope));
        create_traversal_builder(compiler, self, scope_creator).traverse_at_scope(fn_scope);
        self.block_stack = old_block_stack;
    }

    // port: ReferenceCollector#pushNewBlock
    fn push_new_block(&mut self, ast: &Ast, root: NodeId) {
        let block = BasicBlock::new(ast, self.block_stack.back().cloned(), root);
        self.block_stack.push_back(block);
    }

    // port: ReferenceCollector#popLastBlock
    fn pop_last_block(&mut self, ast: &Ast, root: NodeId) {
        let last = self.block_stack.pop_back().expect("removeLast");
        // Verfiy that the stack is unwound correctly.
        check_state!(root == last.get_root(), &root.to_string(ast));
    }

    // port: ReferenceCollector#addReference
    fn add_reference(&mut self, v: VarId, n: NodeId, t: &mut NodeTraversal<'_>) {
        if !(self.var_filter)(t.get_compiler(), v) {
            return;
        }

        // Create collection if none already
        let block = self.block_stack.back().expect("getLast").clone();
        let reference = Reference::new(n, t, block);
        let collection = self.reference_map.entry(v).or_default();

        // Add this particular reference
        collection.add(reference);
    }
}

/// Returns true if this node marks the start of a new basic block
// port: ReferenceCollector#isBlockBoundary
fn is_block_boundary(ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
    if let Some(parent) = parent {
        match parent.get_token(ast) {
            Token::DO
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::TRY
            | Token::WHILE
            | Token::WITH
            | Token::CLASS
            | Token::SWITCH_BODY => {
                // NOTE: TRY has up to 3 child blocks:
                // TRY
                //   BLOCK
                //   BLOCK
                //     CATCH
                //   BLOCK
                // Note that there is an explicit CATCH token but no explicit
                // FINALLY token. For simplicity, we consider each BLOCK
                // a separate basic BLOCK.
                return true;
            }
            Token::AND
            | Token::HOOK
            | Token::IF
            | Token::OR
            | Token::COALESCE
            | Token::OPTCHAIN_GETPROP
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_CALL
            | Token::DEFAULT_VALUE => {
                // The first child of a conditional is not a boundary,
                // but all the rest of the children are.
                return Some(n) != parent.get_first_child(ast);
            }
            _ => {}
        }
    }
    n.is_case(ast)
}

impl Callback for CollectorCallback<'_> {
    /// Updates block stack.
    // port: ReferenceCollector.CollectorCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // We automatically traverse a hoisted function body when that function
        // is first referenced, so that the reference lists are in the right order.
        //
        // TODO(nicksantos): Maybe generalize this to a continuation mechanism
        // like in RemoveUnusedCode.
        #[allow(clippy::collapsible_if)] // Retain Java control flow.
        if NodeUtil::is_hoisted_function_declaration(node_traversal, n) {
            if !self.collected_hoisted_functions.insert(n) {
                return false;
            }
        }

        // If node is a new basic block, put on basic block stack
        if is_block_boundary(node_traversal, n, parent) {
            self.push_new_block(node_traversal, n);
        }
        true
    }

    /// For each node, update the block stack and reference collection as appropriate.
    // port: ReferenceCollector.CollectorCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_name(t) || n.is_import_star(t) {
            let p = parent.unwrap();
            if (p.is_import_spec(t) && Some(n) != p.get_last_child(t))
                || (p.is_export_spec(t) && Some(n) != p.get_first_child(t))
            {
                // The n in `import {n as x}` or `export {x as n}` are not references, even though
                // they are represented in the AST as NAME nodes.
                return;
            }

            let name = n.get_string(t);
            let scope = t.get_scope();
            let v = scope.get_var(t.get_compiler(), name);

            if let Some(v) = v {
                self.add_reference(v, n, t);
                self.maybe_jump_to_hoisted_function(v, t);
            }
        }

        if is_block_boundary(t, n, parent) {
            self.pop_last_block(t, n);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CollectorCallback<'_> {
    /// Updates block stack and invokes any additional behavior.
    // port: ReferenceCollector.CollectorCallback#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        // Don't add all ES6 scope roots to blockStack, only those that are also scopes according
        // to the ES5 scoping rules. Other nodes that ought to be considered the root of a
        // BasicBlock are added in shouldTraverse() or processScope() and removed in visit().
        if t.is_hoist_scope() {
            let root = t.get_scope_root().unwrap();
            self.push_new_block(t, root);
        }
    }

    /// Updates block stack and invokes any additional behavior.
    // port: ReferenceCollector.CollectorCallback#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.is_hoist_scope() {
            let root = t.get_scope_root().unwrap();
            self.pop_last_block(t, root);
        }
        let Self {
            behavior,
            reference_map,
            ..
        } = self;
        behavior.after_exit_scope(t, &ReferenceMapWrapper::new(reference_map));
    }
}

pub struct ReferenceMapWrapper<'m> {
    reference_map: &'m IndexMap<VarId, ReferenceCollection>,
}

impl<'m> ReferenceMapWrapper<'m> {
    // port: ReferenceCollector.ReferenceMapWrapper#ReferenceMapWrapper
    pub fn new(reference_map: &'m IndexMap<VarId, ReferenceCollection>) -> Self {
        Self { reference_map }
    }

    // port: ReferenceCollector.ReferenceMapWrapper#getRawReferenceMap
    pub fn get_raw_reference_map(&self) -> &'m IndexMap<VarId, ReferenceCollection> {
        self.reference_map
    }

    // port: ReferenceCollector.ReferenceMapWrapper#toString
    pub fn to_string(&self, compiler: &mut AbstractCompiler) -> String {
        let entries = self
            .reference_map
            .iter()
            .map(|(var, collection)| {
                format!(
                    "{}={}",
                    var.to_string(compiler),
                    collection.to_string(compiler)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{{entries}}}")
    }
}

impl ReferenceMap for ReferenceMapWrapper<'_> {
    // port: ReferenceCollector.ReferenceMapWrapper#getReferences
    fn get_references(&self, var: VarId) -> Option<&ReferenceCollection> {
        self.reference_map.get(&var)
    }
}

/// Way for callers to add specific behavior during traversal that utilizes the built-up
/// reference information.
pub trait Behavior {
    /// Called after we finish with a scope.
    // port: ReferenceCollector.Behavior#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap);
}

// Rust-only forwarding so callers can lend a behavior they keep using afterwards.
impl<T: Behavior + ?Sized> Behavior for &mut T {
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        (**self).after_exit_scope(t, reference_map);
    }
}

impl<T: Behavior + ?Sized> Behavior for Box<T> {
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        (**self).after_exit_scope(t, reference_map);
    }
}

/// Java's anonymous `DO_NOTHING_BEHAVIOR`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DoNothingBehavior;

impl Behavior for DoNothingBehavior {
    // port: ReferenceCollector.DO_NOTHING_BEHAVIOR#afterExitScope
    fn after_exit_scope(&mut self, _t: &mut NodeTraversal<'_>, _reference_map: &dyn ReferenceMap) {}
}
