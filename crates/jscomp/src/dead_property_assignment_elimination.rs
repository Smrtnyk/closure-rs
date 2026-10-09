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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/DeadPropertyAssignmentElimination.java.

//! Port of `DeadPropertyAssignmentElimination.java`.
//!
//! An optimization pass that finds and removes dead property assignments within functions and
//! classes.
//!
//! This pass does not currently use the control-flow graph. It makes the following assumptions:
//!
//! - Functions with inner functions are not processed.
//! - All properties are read whenever entering a block node. Dead assignments within a block are
//!   processed.
//! - Hook nodes are not processed (it's assumed they read everything)
//! - Switch blocks are not processed (it's assumed they read everything)
//! - Any reference to a property getter/setter is treated like a call that escapes all props.
//! - If there's an Object.definePropert{y,ies} call where the object or property name is aliased
//!   then the optimization does not run at all.
//! - Properties names defined in externs will not be pruned.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{AbstractChangedScopeCallback, Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state, js_string::JsString, node::NodeId, token::Token,
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
        if compiler.get_extern_properties().is_none() || compiler.get_accessor_summary().is_none() {
            return;
        }

        // Sets.union(accessors.keySet(), externProperties): only membership is queried.
        let mut skiplisted_prop_names: IndexSet<JsString> = IndexSet::<_>::default();
        for name in compiler
            .get_accessor_summary()
            .unwrap()
            .get_accessors()
            .keys()
        {
            skiplisted_prop_names.insert(name.clone());
        }
        for name in compiler.get_extern_properties().unwrap() {
            skiplisted_prop_names.insert(JsString::from(name.as_str()));
        }

        let js_root = compiler.get_js_root().unwrap();
        let mut function_visitor = FunctionVisitor::new(skiplisted_prop_names);
        let mut callback =
            AbstractChangedScopeCallback::new(|compiler: &mut AbstractCompiler, root| {
                function_visitor.enter_changed_scope_root(compiler, root)
            });
        NodeTraversal::traverse(compiler, js_root, &mut callback);
    }
}

struct FunctionVisitor {
    /// A set of properties names that are potentially unsafe to remove duplicate writes to.
    skiplisted_prop_names: IndexSet<JsString>,
}

impl FunctionVisitor {
    // port: DeadPropertyAssignmentElimination.FunctionVisitor#FunctionVisitor
    fn new(skiplisted_prop_names: IndexSet<JsString>) -> Self {
        Self {
            skiplisted_prop_names,
        }
    }

    // port: DeadPropertyAssignmentElimination.FunctionVisitor#enterChangedScopeRoot
    fn enter_changed_scope_root(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        if !root.is_function(compiler) {
            return;
        }

        let body = NodeUtil::get_function_body(compiler, root);
        if !body.has_children(compiler)
            || NodeUtil::has(compiler, body, &|ast, n| n.is_function(ast), &|_, _| true)
        {
            return;
        }

        let mut traversal = FindCandidateAssignmentTraversal::new(&self.skiplisted_prop_names);
        NodeTraversal::traverse(compiler, body, &mut traversal);

        // Any candidate property assignment can have a write removed if that write is never read
        // and it's written to at least one more time.
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
                    let lhs = property_write.assigned_at;
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
    fn add_write(&mut self, compiler: &AbstractCompiler, lhs: NodeId) {
        check_argument!(lhs.is_qualified_name(compiler));
        self.writes.push_back(PropertyWrite::new(compiler, lhs));
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
    fn new(compiler: &AbstractCompiler, assigned_at: NodeId) -> Self {
        check_argument!(assigned_at.is_qualified_name(compiler));
        Self {
            assigned_at,
            is_read: false,
            qualified_name: assigned_at.get_qualified_name(compiler),
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

    /// A set of properties names that are potentially unsafe to remove duplicate writes to.
    skiplisted_prop_names: &'a IndexSet<JsString>,
}

impl<'a> FindCandidateAssignmentTraversal<'a> {
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#FindCandidateAssignmentTraversal
    fn new(skiplisted_prop_names: &'a IndexSet<JsString>) -> Self {
        Self {
            property_map: IndexMap::<_, _>::default(),
            properties: Vec::new(),
            skiplisted_prop_names,
        }
    }

    /// Gets a `Property` given the node that references it; the `Property` is created if it does
    /// not already exist.
    ///
    /// Returns a `Property`, or null if the provided node is not a qualified name.
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#getOrCreateProperty
    fn get_or_create_property(
        &mut self,
        compiler: &AbstractCompiler,
        prop_node: NodeId,
    ) -> Option<PropertyIndex> {
        if !prop_node.is_qualified_name(compiler) {
            return None;
        }

        let prop_name = if prop_node.is_get_prop(compiler) {
            prop_node.get_string(compiler)
        } else {
            prop_node.get_qualified_name(compiler).unwrap()
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
        if prop_node.is_get_prop(compiler) {
            let parent_property =
                self.get_or_create_property(compiler, prop_node.get_first_child(compiler).unwrap());
            if let Some(parent_property) = parent_property {
                self.properties[parent_property].children.insert(property);
            }
        }

        Some(property)
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visitBlock
    fn visit_block(&mut self, compiler: &AbstractCompiler, block_node: NodeId) {
        check_argument!(block_node.is_block(compiler));

        // We don't do flow analysis yet so we're going to assume everything written up to this
        // block is read.
        if block_node.has_children(compiler) {
            self.mark_all_props_read();
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#isConditionalExpression
    fn is_conditional_expression(compiler: &AbstractCompiler, n: NodeId) -> bool {
        matches!(
            n.get_token(compiler),
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
    fn visit_assignment_lhs(&mut self, compiler: &AbstractCompiler, lhs: NodeId) {
        let Some(property) = self.get_or_create_property(compiler, lhs) else {
            return;
        };

        if !lhs.is_get_prop(compiler) {
            self.properties[property].mark_last_write_read();
            Property::mark_children_read(&mut self.properties, property);
            return;
        }

        let assign_node = lhs.get_parent(compiler).unwrap();

        // If it's mutating assignment (+=, *=, etc.) then mark the last assignment read first.
        if !assign_node.is_assign(compiler) {
            self.properties[property].mark_last_write_read();
        }

        // Reassignment of a qualified name prefix might change what child properties are
        // referenced later on, so consider children properties as read.
        // Ex. a.b.c = 10; a.b = other; a.b.c = 20;
        Property::mark_children_read(&mut self.properties, property);
        self.properties[property].add_write(compiler, lhs);

        // Now we need to go up the prop chain and mark those as read.
        let mut child = lhs.get_first_child(compiler);
        while let Some(c) = child {
            let Some(child_property) = self.get_or_create_property(compiler, c) else {
                break;
            };
            self.properties[child_property].mark_last_write_read();
            child = c.get_first_child(compiler);
        }
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visitNode
    fn visit_node(
        &mut self,
        compiler: &AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(compiler) {
            Token::GETPROP => {
                // Handle potential getters/setters.
                if n.is_get_prop(compiler)
                    && self.skiplisted_prop_names.contains(&n.get_string(compiler))
                {
                    // We treat getters/setters as if they were a call, thus we mark all
                    // properties as read.
                    self.mark_all_props_read();
                    return true;
                }

                let parent = parent.unwrap();
                if NodeUtil::is_assignment_op(compiler, parent)
                    && parent.get_first_child(compiler) == Some(n)
                {
                    // This is a write to the property, so skip the read handling below.
                    // We'll handle this write in the visit() method
                    // We need to continue traversing, because there could be a function call
                    // child.
                    return true;
                }
                let property = self.get_or_create_property(compiler, n);
                if let Some(property) = property {
                    // Mark all children properties as read.
                    self.properties[property].mark_last_write_read();

                    // Only mark children properties as read if we're at the end of the referenced
                    // property chain.
                    // Ex. A read of "a.b.c" should mark a, a.b, a.b.c, and a.b.c.* as read, but
                    // not a.d
                    if !parent.is_get_prop(compiler) {
                        Property::mark_children_read(&mut self.properties, property);
                    }
                }
                true
            }
            Token::THIS | Token::NAME => {
                let name_prop = check_not_null!(self.get_or_create_property(compiler, n));
                self.properties[name_prop].mark_last_write_read();
                if !parent.unwrap().is_get_prop(compiler) {
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
                let mut child = n.get_first_child(compiler);
                while let Some(c) = child {
                    if c.is_string_key(compiler) {
                        let prop_name = c.get_string(compiler);
                        let property = self.property_map.get(&prop_name).copied();
                        if let Some(property) = property {
                            self.properties[property].mark_last_write_read();
                            Property::mark_children_read(&mut self.properties, property);
                        }
                    } else {
                        self.mark_all_props_read();
                        break;
                    }
                    child = c.get_next(compiler);
                }
                true
            }
            Token::BLOCK => {
                self.visit_block(compiler, n);
                true
            }
            _ => {
                if Self::is_conditional_expression(compiler, n) {
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
}

impl Callback for FindCandidateAssignmentTraversal<'_> {
    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        self.visit_node(t.get_compiler(), n, parent)
    }

    // port: DeadPropertyAssignmentElimination.FindCandidateAssignmentTraversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let compiler = t.get_compiler();
        // Visit the LHS of an assignment in post-order
        if NodeUtil::is_assignment_op(compiler, n) {
            self.visit_assignment_lhs(compiler, n.get_first_child(compiler).unwrap());
        }

        // Assume that all properties may be read when control flow leaves the function
        if NodeUtil::is_invocation(compiler, n) || n.is_yield(compiler) || n.is_await(compiler) {
            self.mark_all_props_read();
        }

        // Mark all properties as read when leaving a block since we haven't proven that the block
        // will execute.
        if n.is_block(compiler) {
            self.visit_block(compiler, n);
        }
    }
}
