/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/GuardedCallback.java.

//! An AST traverser that keeps track of whether access to a generic resource are "guarded" or not.
//!
//! Port of `GuardedCallback.java`. Java's abstract class becomes the state struct
//! [`GuardedCallback`] plus the [`GuardedCallbackSubclass`] trait (the template method
//! `visitGuarded` and access to the inherited state); every subclass is a
//! [`Callback`] through the final `shouldTraverse`/`visit` methods below.
use crate::{
    abstract_compiler::AbstractCompiler, node_traversal::Callback, node_traversal::NodeTraversal,
    node_util::NodeUtil,
};
use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::{hash::Hash, rc::Rc};

pub struct GuardedCallback<T> {
    // Map from short-circuiting conditional nodes (AND, OR, COALESCE, IF, and HOOK) to
    // the set of resources each node guards.  This is saved separately from
    // just `guarded` because the guard must not go into effect until after
    // traversal of the first child is complete.  Before traversing the second
    // child any node, its values in this map are moved into `guarded` and
    // `installedGuards` (the latter allowing removal at the correct time).
    registered_guards: IndexMap<NodeId, IndexSet<T>>,
    // Set of currently-guarded resources.  Elements are added to this set
    // just before traversing the second or later (i.e. "then" or "else")
    // child of a short-circuiting conditional node, and then removed after
    // traversing the last child.  It is a multiset so that multiple adds
    // of the same resource require the same number of removals before the
    // resource becomes unguarded.
    guarded: IndexMap<T, usize>,
    // Resources that are currently installed as guarded but will need to
    // be removed from `guarded` after visiting all the key nodes' children.
    installed_guards: IndexMap<NodeId, Vec<T>>,
    // A stack of `Context` objects describing the current node's context:
    // specifically, whether it is inherently safe, and a link to one or
    // more conditional nodes in the current statement directly above it
    // (for registering safe resources as guards).
    context_stack: Vec<Rc<Context>>,
}

impl<T: Clone + Eq + Hash> Default for GuardedCallback<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The subclass side of Java's abstract `GuardedCallback<T>`: the template method and access to
/// the inherited state. Every implementor is a [`Callback`] whose `shouldTraverse` and `visit`
/// are GuardedCallback's final methods.
pub trait GuardedCallbackSubclass {
    type Resource: Clone + Eq + Hash;

    /// The inherited GuardedCallback state.
    fn guarded_callback(&mut self) -> &mut GuardedCallback<Self::Resource>;

    /// Performs specific traversal behavior. Should call {@link #isGuarded}
    /// at least once.
    // port: GuardedCallback#visitGuarded
    fn visit_guarded(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    );

    /// Determines if the given resource is guarded, either intrinsically or
    /// conditionally.  If the former, any ancestor conditional nodes are
    /// registered as feature-testing the resource.
    // port: GuardedCallback#isGuarded
    fn is_guarded(&mut self, resource: Self::Resource) -> bool {
        self.guarded_callback().is_guarded(resource)
    }
}

impl<C: GuardedCallbackSubclass> Callback for C {
    // port: GuardedCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        self.guarded_callback()
            .should_traverse(traversal.get_compiler(), n, parent)
    }

    // port: GuardedCallback#visit
    fn visit(&mut self, traversal: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.guarded_callback()
            .visit_before_guarded(traversal, n, parent);

        // Finally continue on to whatever the traversal would normally do.
        self.visit_guarded(traversal, n, parent);

        // After children have been traversed, pop the top of the conditional stack.
        self.guarded_callback().context_stack.pop();
    }
}

impl<T: Clone + Eq + Hash> GuardedCallback<T> {
    // port: GuardedCallback#GuardedCallback
    pub fn new() -> Self {
        Self {
            registered_guards: IndexMap::new(),
            guarded: IndexMap::new(),
            installed_guards: IndexMap::new(),
            context_stack: Vec::new(),
        }
    }

    // port: GuardedCallback#shouldTraverse
    pub fn should_traverse(
        &mut self,
        compiler: &AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // Note that shouldTraverse() operates primarily on `parent`, while visit()
        // uses `n`.  This is intentional.  To see why, consider traversing the
        // following tree:
        //
        //    if (x) y; else z;
        //
        // 1. shouldTraverse(`if`):
        //    a. parent is null, so pushes an EMPTY onto the context stack.
        // 2. shouldTraverse(`x`):
        //    a. parent is `if`, so pushes Context(`if`, true); guards is empty.
        // 3. visit(`x`)
        //    a. guarded and installedGuards are both empty, so nothing is removed.
        //    b. visitGuarded(`x`) will call isGuarded("x"), which looks at the top
        //       of the stack and sees that the context is safe (true) and that
        //       there is a linked conditional node (the `if`); adds {`if`: "x"}
        //       to registeredGuards.
        //    b. Context(`if`, true) is popped off the stack.
        // 4. shouldTraverse(`y`):
        //    a. parent is still `if`, but since `y` is the second child it is
        //       no longer safe, so another EMPTY is pushed.
        //    b. the {`if`: "x"} guard is moved from registered to installed.
        // 5. visit(`y`):
        //    a. nothing is installed on `y` so no guards are removed.
        //    b. visitGuarded(`y`) will call isGuarded("y"), which will return
        //       false since "y" is neither intrinsically or conditionally guarded;
        //       if we'd called isResourceRequired("x"), it would return false
        //       because "x" is currently an element of guarded.
        //    c. one empty context is popped.
        // 6. shouldTraverse(`z`), visit(`z`)
        //    a. see steps 4-5, nothing really changes here.
        // 7. visit(`if`)
        //    a. the installed {`if`: "x"} guard is removed.
        //    c. pop the final empty context from the stack.

        match parent {
            None => {
                // The first node gets an empty context.
                self.context_stack.push(Context::empty());
            }
            Some(parent) => {
                // Before traversing any children, we update the stack
                let top = self.context_stack.last().expect("context stack").clone();
                self.context_stack
                    .push(Context::descend(&top, compiler, parent, n));

                // If the parent has any guards registered on it, then add them to both
                // `guarded` and `installedGuards`.
                if CAN_HAVE_GUARDS.contains(&parent.get_token(compiler))
                    && self.registered_guards.contains_key(&parent)
                {
                    let resources = self
                        .registered_guards
                        .shift_remove(&parent)
                        .unwrap_or_default();
                    for resource in resources {
                        *self.guarded.entry(resource.clone()).or_insert(0) += 1;
                        self.installed_guards
                            .entry(parent)
                            .or_default()
                            .push(resource);
                    }
                }
            }
        }
        true
    }

    // The part of GuardedCallback#visit that runs before visitGuarded.
    // port: GuardedCallback#visit
    fn visit_before_guarded(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        // Remove any guards registered on this node by its children, which are no longer
        // relevant.  This happens first because these were registered on a "parent", but
        // now this is that parent (i.e. `n` here vs `parent` in isGuarded).
        if parent.is_some()
            && CAN_HAVE_GUARDS.contains(&n.get_token(traversal))
            && self.installed_guards.contains_key(&n)
        {
            // Multiset#removeAll removes every occurrence of each element.
            for resource in self.installed_guards.shift_remove(&n).unwrap_or_default() {
                self.guarded.shift_remove(&resource);
            }
        }

        // Check for abrupt returns (`return` and `throw`).
        if Self::is_abrupt(traversal, n) {
            // If found, any guards installed on a parent IF should be promoted to the
            // grandparent.  This allows a small amount of flow-sensitivity, in that
            //   if (!x) return; x();
            // has the guard for `x` promoted from the `if` to the outer block, so that
            // it guards the next statement.
            self.promote_abrupt_returns(traversal, parent.expect("abrupt node has a parent"));
        }
    }

    // port: GuardedCallback#promoteAbruptReturns
    fn promote_abrupt_returns(&mut self, ast: &Ast, mut parent: NodeId) {
        // If the parent is a BLOCK (e.g. `if (x) { return; }`) then go up one level.
        if parent.is_block(ast) {
            parent = parent.get_parent(ast).expect("block parent");
        }
        // If there were any guards registered the parent IF, then promote them up one level.
        if parent.is_if(ast) && self.installed_guards.contains_key(&parent) {
            let grandparent = parent.get_parent(ast).expect("if parent");
            if grandparent.is_block(ast) || grandparent.is_script(ast) {
                let installed = self.installed_guards[&parent].clone();
                let registered = self.registered_guards.entry(grandparent).or_default();
                for resource in installed {
                    registered.insert(resource);
                }
            }
        }
    }

    /// Determines if the given resource is guarded, either intrinsically or
    /// conditionally.  If the former, any ancestor conditional nodes are
    /// registered as feature-testing the resource.
    // port: GuardedCallback#isGuarded
    pub fn is_guarded(&mut self, resource: T) -> bool {
        // Check if this polyfill is already guarded.  If so, return true right away.
        if self.guarded.contains_key(&resource) {
            return true;
        }

        // If not, see if this is itself a feature check guard.  This is
        // defined as a usage of the polyfill in such a way that throws
        // away the actual value and only cares about its truthiness or
        // typeof.  We walk up the ancestor tree through a small set of
        // node types and if this is detected to be a guard, then the
        // conditional node is marked as a guard for this polyfill.
        let mut context = Some(self.context_stack.last().expect("context stack").clone());
        if !context.as_ref().unwrap().safe {
            return false;
        }

        // Loop over all the linked conditionals and register this as a guard.
        while let Some(c) = context.as_ref().filter(|c| c.conditional.is_some()) {
            self.registered_guards
                .entry(c.conditional.unwrap())
                .or_default()
                .insert(resource.clone());
            context = c.linked.clone();
        }
        true
    }

    // port: GuardedCallback#isAbrupt
    fn is_abrupt(ast: &Ast, n: NodeId) -> bool {
        n.is_return(ast) || n.is_throw(ast)
    }
}

// The context of a node, keeping track of whether it is safe for
// possibly-undefined values, and whether there are any conditionals
// upstream in the tree.
struct Context {
    // The most recent conditional.
    conditional: Option<NodeId>,
    // Whether this position is safe for an undefined type.
    safe: bool,
    // A very naive linked list for storing additional conditional nodes.
    linked: Option<Rc<Context>>,
}

thread_local! {
    // An empty instance: unsafe and with no linked conditional nodes.
    // port: GuardedCallback.Context#EMPTY
    static EMPTY: Rc<Context> = Rc::new(Context::new(None, false, None));
}

impl Context {
    // port: GuardedCallback.Context#Context
    fn new(conditional: Option<NodeId>, safe: bool, linked: Option<Rc<Context>>) -> Self {
        Self {
            conditional,
            safe,
            linked,
        }
    }

    // port: GuardedCallback.Context#EMPTY
    fn empty() -> Rc<Context> {
        EMPTY.with(Rc::clone)
    }

    // Returns a new Context with a new conditional node and safety status.
    // If the current context already has a conditional, then it is linked
    // so that both can be marked when necessary.
    // port: GuardedCallback.Context#link
    fn link(this: &Rc<Context>, new_conditional: NodeId, new_safe: bool) -> Rc<Context> {
        Rc::new(Context::new(
            Some(new_conditional),
            new_safe,
            if this.conditional.is_some() {
                Some(this.clone())
            } else {
                None
            },
        ))
    }

    // Returns a new Context with a different safety bit, but doesn't
    // change anything else.
    // port: GuardedCallback.Context#propagate
    fn propagate(this: &Rc<Context>, new_safe: bool) -> Rc<Context> {
        if new_safe == this.safe {
            this.clone()
        } else {
            Rc::new(Context::new(
                this.conditional,
                new_safe,
                this.linked.clone(),
            ))
        }
    }

    // Returns a new context given the current context and the next parent
    // node.  Child is only used to determine whether we're looking at the
    // first child or not.
    // port: GuardedCallback.Context#descend
    fn descend(
        this: &Rc<Context>,
        compiler: &AbstractCompiler,
        parent: NodeId,
        child: NodeId,
    ) -> Rc<Context> {
        let first = Some(child) == parent.get_first_child(compiler);
        match parent.get_token(compiler) {
            Token::CAST => {
                // Casts are irrelevant.
                this.clone()
            }
            Token::COMMA => {
                // `Promise, whatever` is safe.
                // `whatever, Promise` is same as outer context.
                if Some(child) == parent.get_last_child(compiler) {
                    this.clone()
                } else {
                    Self::propagate(this, true)
                }
            }
            Token::AND => {
                // `Promise && whatever` never returns Promise itself, so it is safe.
                // `whatever && Promise` may return Promise, so return outer context.
                if first {
                    Self::link(this, parent, true)
                } else {
                    this.clone()
                }
            }
            Token::OR | Token::COALESCE => {
                // `Promise || whatever` and `Promise ?? whatever`
                // may return Promise (unsafe), but is itself a conditional.
                // `whatever || Promise` and `whatever ?? Promise`
                // is same as outer context.
                if first {
                    Self::link(this, parent, false)
                } else {
                    this.clone()
                }
            }
            Token::HOOK => {
                // `Promise ? whatever : whatever` is a safe conditional.
                // `whatever ? Promise : whatever` (etc) is same as outer context.
                if first {
                    Self::link(this, parent, true)
                } else {
                    this.clone()
                }
            }
            Token::IF => {
                // `if (Promise) whatever` is a safe conditional.
                // `if (whatever) { ... }` is nothing.
                // TODO(sdh): Handle do/while/for/for-of/for-in?
                if first {
                    Self::link(this, parent, true)
                } else {
                    Self::empty()
                }
            }
            Token::INSTANCEOF | Token::ASSIGN => {
                // `Promise instanceof whatever` is safe, `whatever instanceof Promise` is not.
                // `Promise = whatever` is a bad idea, but it's safe w.r.t. polyfills.
                Self::propagate(this, first)
            }
            Token::TYPEOF | Token::NOT | Token::EQ | Token::NE | Token::SHEQ | Token::SHNE => {
                // `typeof Promise` is always safe, as is `Promise == whatever`, etc.
                Self::propagate(this, true)
            }
            Token::CALL => {
                // `String(Promise)` is safe, `Promise(whatever)` or `whatever(Promise)` is not.
                Self::propagate(this, !first && is_property_test_function(compiler, parent))
            }
            Token::ROOT => {
                // This case causes problems for isStatement() so handle it separately.
                Self::empty()
            }
            Token::OPTCHAIN_CALL | Token::OPTCHAIN_GETELEM | Token::OPTCHAIN_GETPROP => {
                if first {
                    // thisNode?.rest.of.chain
                    // OR firstChild?.thisNode.rest.of.chain
                    // For the first case `thisNode` should be considered intrinsically guarded.
                    Self::link(this, parent, parent.is_optional_chain_start(compiler))
                } else {
                    // `first?.(thisNode)`
                    // or `first?.[thisNode]`
                    // or `first?.thisNode`
                    Self::propagate(this, false)
                }
            }
            _ => {
                // Expressions propagate linked conditionals; statements do not.
                if NodeUtil::is_statement(compiler, parent) {
                    Self::empty()
                } else {
                    Self::propagate(this, false)
                }
            }
        }
    }
}

// Extend the coding convention's idea of property test functions to also
// include String() and Boolean().
// port: GuardedCallback#isPropertyTestFunction
fn is_property_test_function(compiler: &AbstractCompiler, n: NodeId) -> bool {
    if compiler
        .get_coding_convention()
        .is_property_test_function(compiler, n)
    {
        return true;
    }
    let target = n.get_first_child(compiler).expect("call target");
    if target.is_name(compiler) {
        let name = target.get_string(compiler);
        return name == "String" || name == "Boolean";
    }
    false
}

// Tokens that are allowed to have guards on them (no point doing a hash lookup on
// any other type of node).
// port: GuardedCallback#CAN_HAVE_GUARDS
const CAN_HAVE_GUARDS: [Token; 10] = [
    Token::AND,
    Token::OR,
    Token::COALESCE,
    Token::HOOK,
    Token::IF,
    Token::BLOCK,
    Token::SCRIPT,
    Token::OPTCHAIN_CALL,
    Token::OPTCHAIN_GETELEM,
    Token::OPTCHAIN_GETPROP,
];
