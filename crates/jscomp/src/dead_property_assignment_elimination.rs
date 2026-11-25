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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/DeadPropertyAssignmentElimination.java.

//! Port of `DeadPropertyAssignmentElimination.java`.
//!
//! An optimization pass that finds and removes dead property assignments within functions and
//! classes.
//!
//! This pass does not currently use the control-flow graph. It makes the following assumptions:
//!
//! - Functions with inner functions or non-static class field initializers are not processed.
//! - All properties are read whenever entering a block node. Dead assignments within a block are
//!   processed.
//! - Hook nodes are not processed (it's assumed they read everything)
//! - Switch blocks are not processed (it's assumed they read everything)
//! - Any reference to a property getter/setter is treated like a call that escapes all props.
//! - If there's an Object.definePropert{y,ies} call where the object or property name is aliased
//!   then the optimization does not run at all.
//! - Properties names defined in externs will not be pruned.
use crate::{
    AbstractCompiler, accessor_summary::PropertyAccessKind, change_tracker::ChangeTracker,
    compiler_pass::CompilerPass, node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::collections::VecDeque;

pub struct DeadPropertyAssignmentElimination;

impl Default for DeadPropertyAssignmentElimination {
    fn default() -> Self {
        Self::new()
    }
}

impl DeadPropertyAssignmentElimination {
    // port: DeadPropertyAssignmentElimination#DeadPropertyAssignmentElimination
    pub fn new() -> Self {
        Self
    }
}

impl CompilerPass for DeadPropertyAssignmentElimination {
    // port: DeadPropertyAssignmentElimination#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        // GatherExternProperties must be enabled for this pass to safely know what property
        // writes are eligible for removal.
        let Some(extern_props) = compiler.get_extern_property_set_js().cloned() else {
            return;
        };
        if compiler.get_accessor_summary().is_none() {
            return;
        }

        let js_root = compiler.get_js_root().unwrap();
        // Rust-only (performance divergence): borrow the cached interned extern property set and
        // accessor map directly (avoiding rebuilding a 10K-entry set every run), collect changed
        // function bodies directly in pre-order, pre-screen functions with fewer than two GETPROP
        // assignments or containing inner functions, analyze eligible leaf bodies in parallel on
        // `&Ast`, and apply removals sequentially in exact AST order.
        let mut candidates = Vec::new();
        collect_changed_function_bodies(compiler, js_root, &mut candidates);
        if candidates.is_empty() {
            return;
        }

        let threads = crate::parallel_parse::parallel_thread_count(candidates.len(), 64, 8);
        let ast = &compiler.ast;
        let accessors = compiler.get_accessor_summary().unwrap().get_accessors();
        let extern_props_ref: &IndexSet<JsString> = &extern_props;
        let worker_batches = crate::parallel_parse::run_scoped_indexed(
            "closure-dpae",
            threads,
            &candidates,
            |_idx, &body| analyze_function_body(ast, body, extern_props_ref, accessors),
        );

        let mut ordered: Vec<Option<Vec<NodeId>>> = (0..candidates.len()).map(|_| None).collect();
        for batch in worker_batches {
            for (idx, writes) in batch {
                ordered[idx] = Some(writes);
            }
        }
        for writes in ordered.into_iter().flatten() {
            for lhs in writes {
                let rhs = lhs.get_next(compiler).unwrap();
                let assign_node = lhs.get_parent(compiler).unwrap();
                if assign_node.is_assign(compiler) {
                    // replace "a.b.c = <expr>" with "<expr>"
                    rhs.detach(compiler);
                    assign_node.replace_with(compiler, rhs);
                    compiler.report_change_to_enclosing_scope(rhs);
                } else {
                    check_state!(NodeUtil::is_assignment_op(compiler, assign_node));
                    // replace "a.b.c += <expr>" with "a.b.c + expr"
                    let op_type = NodeUtil::get_op_from_assignment_op(compiler, assign_node);
                    assign_node.set_token(compiler, op_type);
                    compiler.report_change_to_enclosing_scope(assign_node);
                }
            }
        }
    }
}

/// Rust-only (performance divergence): mirrors `AbstractChangedScopeCallback` pre-order collection
/// for changed `FUNCTION` nodes with non-empty bodies.
fn collect_changed_function_bodies(compiler: &AbstractCompiler, n: NodeId, out: &mut Vec<NodeId>) {
    let ast = &compiler.ast;
    if ChangeTracker::is_change_scope_root(ast, n)
        && compiler.has_scope_changed(n)
        && n.is_function(ast)
    {
        let body = NodeUtil::get_function_body(ast, n);
        if body.has_children(ast) {
            out.push(body);
        }
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        collect_changed_function_bodies(compiler, c, out);
        child = c.get_next(ast);
    }
}

/// Rust-only (performance divergence): single-pass check that `body` contains neither an inner
/// function nor a non-static class field initializer, and counts `GETPROP` assignment LHS nodes so
/// bodies with fewer than two property writes skip full analysis.
fn prescan_body(ast: &Ast, n: NodeId, getprop_assigns: &mut u32) -> bool {
    if NodeUtil::is_function_or_non_static_class_field_initializer(ast, n) {
        return false;
    }
    if NodeUtil::is_assignment_op(ast, n)
        && n.get_first_child(ast)
            .is_some_and(|lhs| lhs.is_get_prop(ast))
    {
        *getprop_assigns = getprop_assigns.saturating_add(1);
    }
    let mut child = n.get_first_child(ast);
    while let Some(c) = child {
        if !prescan_body(ast, c, getprop_assigns) {
            return false;
        }
        child = c.get_next(ast);
    }
    true
}

// port: DeadPropertyAssignmentElimination.FunctionVisitor#enterChangedScopeRoot
fn analyze_function_body(
    ast: &Ast,
    body: NodeId,
    extern_props: &IndexSet<JsString>,
    accessors: &IndexMap<JsString, PropertyAccessKind>,
) -> Option<Vec<NodeId>> {
    let mut getprop_assigns = 0u32;
    if !prescan_body(ast, body, &mut getprop_assigns) || getprop_assigns < 2 {
        return None;
    }

    let mut traversal = FindCandidateAssignmentTraversal::new(extern_props, accessors);
    traversal.walk(ast, body, body.get_parent(ast));

    // Any candidate property assignment can have a write removed if that write is never read
    // and it's written to at least one more time.
    let mut removable = Vec::new();
    for &property in traversal.property_map.values() {
        let property = &traversal.properties[property];
        if property.writes.len() <= 1 {
            continue;
        }
        // Iterators.peekingIterator(property.writes.iterator())
        let mut iter = property.writes.iter().peekable();
        while let Some(property_write) = iter.next() {
            if let Some(&next) = iter.peek()
                && property_write.is_safe_to_remove(Some(next))
            {
                removable.push(property_write.assigned_at);
            }
        }
    }
    (!removable.is_empty()).then_some(removable)
}

// Rust-only: Properties reference each other (children), so they live in an arena owned by the
// traversal and are addressed by index; Java's identity sets of Property become index sets.
type PropertyIndex = usize;

struct Property {
    name: JsString,

    // This pass doesn't use a control-flow graph; this field contains a rough approximation
    // of the control flow. For writes in the same block, they appear in this list in
    // program-execution order.
    // All writes in a list are to the same property name, but the full qualified names may
    // differ, eg, a.b.c and e.d.c can be in the list. Consecutive writes to the same qname
    // may mean that the first write can be removed (see isSafeToRemove).
    writes: VecDeque<PropertyWrite>,

    children: IndexSet<PropertyIndex>,
}

impl Property {
    // port: DeadPropertyAssignmentElimination.Property#Property
    fn new(name: JsString) -> Self {
        Self {
            name,
            writes: VecDeque::new(),
            children: IndexSet::<_>::default(),
        }
    }

    // port: DeadPropertyAssignmentElimination.Property#markLastWriteRead
    fn mark_last_write_read(&mut self) {
        if !self.writes.is_empty() {
            self.writes.back_mut().unwrap().mark_read();
        }
    }

    /// Marks all children of this property as read.
    // port: DeadPropertyAssignmentElimination.Property#markChildrenRead
    fn mark_children_read(properties: &mut [Property], this: PropertyIndex) {
        // If a property is in propertiesSet, it has been added to the queue and processed,
        // it will not be added to the queue again.
        let mut properties_set: IndexSet<PropertyIndex> = properties[this].children.clone();
        let mut property_queue: VecDeque<PropertyIndex> = properties_set.iter().copied().collect();

        // Ensure we don't process ourselves.
        properties_set.insert(this);

        while let Some(child_property) = property_queue.pop_front() {
            properties[child_property].mark_last_write_read();
            for &grandchild_property in &properties[child_property].children {
                if properties_set.insert(grandchild_property) {
                    property_queue.push_back(grandchild_property);
                }
            }
        }
    }

    // port: DeadPropertyAssignmentElimination.Property#addWrite
    fn add_write(&mut self, ast: &Ast, lhs: NodeId) {
        check_argument!(lhs.is_qualified_name(ast));
        self.writes.push_back(PropertyWrite::new(ast, lhs));
    }
}

impl std::fmt::Display for Property {
    // port: DeadPropertyAssignmentElimination.Property#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Property {}", self.name.to_string_lossy())
    }
}

struct PropertyWrite {
    assigned_at: NodeId,
    is_read: bool,
    qualified_name: Option<JsString>,
}

impl PropertyWrite {
    // port: DeadPropertyAssignmentElimination.PropertyWrite#PropertyWrite
    fn new(ast: &Ast, assigned_at: NodeId) -> Self {
        check_argument!(assigned_at.is_qualified_name(ast));
        Self {
            assigned_at,
            is_read: false,
            qualified_name: assigned_at.get_qualified_name(ast),
        }
    }

    // port: DeadPropertyAssignmentElimination.PropertyWrite#isSafeToRemove
    fn is_safe_to_remove(&self, next_write: Option<&PropertyWrite>) -> bool {
        !self.is_read
            && next_write.is_some()
            && self.qualified_name == next_write.unwrap().qualified_name
    }

    // port: DeadPropertyAssignmentElimination.PropertyWrite#markRead
    fn mark_read(&mut self) {
        self.is_read = true;
    }
}

/// A NodeTraversal that operates within a function block and collects candidate properties
/// assignments.
struct FindCandidateAssignmentTraversal<'a> {
    /// A map of property names to their nodes.
    ///
    /// Note: the references `a.b` and `c.b` will assume that it's the same b, because a and c may
    /// be aliased, and we don't track aliasing.
    property_map: IndexMap<JsString, PropertyIndex>,
    properties: Vec<Property>,

    /// Sets of property names that are potentially unsafe to remove duplicate writes to
    /// (union of extern properties and getter/setter accessors).
    extern_props: &'a IndexSet<JsString>,
    accessors: &'a IndexMap<JsString, PropertyAccessKind>,
}

impl<'a> FindCandidateAssignmentTraversal<'a> {
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#FindCandidateAssignmentTraversal
    fn new(
        extern_props: &'a IndexSet<JsString>,
        accessors: &'a IndexMap<JsString, PropertyAccessKind>,
    ) -> Self {
        Self {
            property_map: IndexMap::<_, _>::default(),
            properties: Vec::new(),
            extern_props,
            accessors,
        }
    }

    #[inline]
    fn is_skiplisted(&self, name: &JsString) -> bool {
        self.extern_props.contains(name) || self.accessors.contains_key(name)
    }

    /// Gets a `Property` given the node that references it; the `Property` is created if it does
    /// not already exist.
    ///
    /// Returns a `Property`, or null if the provided node is not a qualified name.
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#getOrCreateProperty
    fn get_or_create_property(&mut self, ast: &Ast, prop_node: NodeId) -> Option<PropertyIndex> {
        if !prop_node.is_qualified_name(ast) {
            return None;
        }

        let prop_name = if prop_node.is_get_prop(ast) {
            prop_node.get_string(ast)
        } else {
            prop_node.get_qualified_name(ast).unwrap()
        };

        // propertyMap.computeIfAbsent(propName, Property::new)
        let property = match self.property_map.get(&prop_name) {
            Some(&property) => property,
            None => {
                let property = self.properties.len();
                self.properties.push(Property::new(prop_name.clone()));
                self.property_map.insert(prop_name, property);
                property
            }
        };

        /* Using the GETPROP chain, build out the tree of children properties.

           For example, from a.b.c and a.c we can build:
                   a
                  / \
                 b   c
                /
               c

          Note: c is the same Property in this tree.
        */
        if prop_node.is_get_prop(ast) {
            let parent_property =
                self.get_or_create_property(ast, prop_node.get_first_child(ast).unwrap());
            if let Some(parent_property) = parent_property {
                self.properties[parent_property].children.insert(property);
            }
        }

        Some(property)
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visitBlock
    fn visit_block(&mut self, ast: &Ast, block_node: NodeId) {
        check_argument!(block_node.is_block(ast));

        // We don't do flow analysis yet so we're going to assume everything written up to this
        // block is read.
        if block_node.has_children(ast) {
            self.mark_all_props_read();
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#isConditionalExpression
    fn is_conditional_expression(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::AND
                | Token::OR
                | Token::HOOK
                | Token::COALESCE
                | Token::OPTCHAIN_CALL
                | Token::OPTCHAIN_GETELEM
                | Token::OPTCHAIN_GETPROP
        )
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visitAssignmentLhs
    fn visit_assignment_lhs(&mut self, ast: &Ast, lhs: NodeId) {
        let Some(property) = self.get_or_create_property(ast, lhs) else {
            return;
        };

        if !lhs.is_get_prop(ast) {
            self.properties[property].mark_last_write_read();
            Property::mark_children_read(&mut self.properties, property);
            return;
        }

        let assign_node = lhs.get_parent(ast).unwrap();

        // If it's mutating assignment (+=, *=, etc.) then mark the last assignment read first.
        if !assign_node.is_assign(ast) {
            self.properties[property].mark_last_write_read();
        }

        // Reassignment of a qualified name prefix might change what child properties are
        // referenced later on, so consider children properties as read.
        // Ex. a.b.c = 10; a.b = other; a.b.c = 20;
        Property::mark_children_read(&mut self.properties, property);
        self.properties[property].add_write(ast, lhs);

        // Now we need to go up the prop chain and mark those as read.
        let mut child = lhs.get_first_child(ast);
        while let Some(c) = child {
            let Some(child_property) = self.get_or_create_property(ast, c) else {
                break;
            };
            self.properties[child_property].mark_last_write_read();
            child = c.get_first_child(ast);
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visitNode
    fn visit_node(&mut self, ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        match n.get_token(ast) {
            Token::GETPROP => {
                // Handle potential getters/setters.
                if self.is_skiplisted(n.get_string_ref(ast)) {
                    // We treat getters/setters as if they were a call, thus we mark all
                    // properties as read.
                    self.mark_all_props_read();
                    return true;
                }

                let parent = parent.unwrap();
                if NodeUtil::is_assignment_op(ast, parent) && parent.get_first_child(ast) == Some(n)
                {
                    // This is a write to the property, so skip the read handling below.
                    // We'll handle this write in the visit() method
                    // We need to continue traversing, because there could be a function call
                    // child.
                    return true;
                }
                let property = self.get_or_create_property(ast, n);
                if let Some(property) = property {
                    // Mark all children properties as read.
                    self.properties[property].mark_last_write_read();

                    // Only mark children properties as read if we're at the end of the referenced
                    // property chain.
                    // Ex. A read of "a.b.c" should mark a, a.b, a.b.c, and a.b.c.* as read, but
                    // not a.d
                    if !parent.is_get_prop(ast) {
                        Property::mark_children_read(&mut self.properties, property);
                    }
                }
                true
            }
            Token::THIS | Token::NAME => {
                let name_prop = check_not_null!(self.get_or_create_property(ast, n));
                self.properties[name_prop].mark_last_write_read();
                if !parent.unwrap().is_get_prop(ast) {
                    Property::mark_children_read(&mut self.properties, name_prop);
                }
                true
            }
            Token::THROW
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::ITER_SPREAD
            | Token::ARRAY_PATTERN
            | Token::SWITCH => {
                // Loops and switch statements may execute out of order, and implicit iteration
                // operations (for-of, for-await-of, spread, array destructuring) invoke arbitrary
                // JS iterator methods which can read properties.
                self.mark_all_props_read();
                false
            }
            Token::OBJECT_PATTERN => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if c.is_string_key(ast) {
                        let property = self.property_map.get(c.get_string_ref(ast)).copied();
                        if let Some(property) = property {
                            self.properties[property].mark_last_write_read();
                            Property::mark_children_read(&mut self.properties, property);
                        }
                    } else {
                        self.mark_all_props_read();
                        break;
                    }
                    child = c.get_next(ast);
                }
                true
            }
            Token::BLOCK => {
                self.visit_block(ast, n);
                true
            }
            _ => {
                if Self::is_conditional_expression(ast, n) {
                    self.mark_all_props_read();
                    return false;
                }
                true
            }
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#markAllPropsRead
    fn mark_all_props_read(&mut self) {
        for &property in self.property_map.values() {
            let property = &mut self.properties[property];
            if property.writes.is_empty() {
                continue;
            }

            property.mark_last_write_read();
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#shouldTraverse
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visit
    fn walk(&mut self, ast: &Ast, n: NodeId, parent: Option<NodeId>) {
        if !self.visit_node(ast, n, parent) {
            return;
        }
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            self.walk(ast, c, Some(n));
            child = c.get_next(ast);
        }
        // Visit the LHS of an assignment in post-order
        if NodeUtil::is_assignment_op(ast, n) {
            self.visit_assignment_lhs(ast, n.get_first_child(ast).unwrap());
        }

        // Assume that all properties may be read when control flow leaves the function
        if NodeUtil::is_invocation(ast, n) || n.is_yield(ast) || n.is_await(ast) {
            self.mark_all_props_read();
        }

        // Mark all properties as read when leaving a block since we haven't proven that the block
        // will execute.
        if n.is_block(ast) {
            self.visit_block(ast, n);
        }
    }
}
