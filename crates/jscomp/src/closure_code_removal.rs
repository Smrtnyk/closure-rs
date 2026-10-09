/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ClosureCodeRemoval.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `ClosureCodeRemoval.java`.
#![allow(clippy::collapsible_if)] // Keep Java's control flow.

use crate::abstract_compiler::AbstractCompiler;
use crate::coding_convention::AssertionFunctionLookup;
use crate::combined_compiler_pass::CombinedCompilerPass;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_rhino::node::{Ast, NodeId};

/// Name used to denote an abstract function
// port: ClosureCodeRemoval#ABSTRACT_METHOD_NAME
pub const ABSTRACT_METHOD_NAME: &str = "goog.abstractMethod";

/// Compiler pass that removes Closure-specific code patterns.
///
/// Currently does the following:
///
/// - Instead of setting abstract methods to a function that throws an informative error, this
///   pass allows some binary size reduction by removing these methods altogether for production
///   builds.
/// - Remove calls to assertion functions (like goog.asserts.assert). If the return value of the
///   assertion function is used, then the first argument (the asserted value) will be directly
///   inlined. Otherwise, the entire call will be removed. It is well-known that this is not
///   provably safe, much like the equivalent assert statement in Java.
pub struct ClosureCodeRemoval {
    remove_abstract_methods: bool,
    remove_assertion_calls: bool,

    /// List of names referenced in successive generations of finding referenced nodes.
    abstract_method_assignment_nodes: Vec<RemovableAssignment>,

    /// List of member function definition nodes annotated with @abstract.
    abstract_member_function_nodes: Vec<NodeId>,

    /// List of assertion functions.
    assertion_calls: Vec<NodeId>,
}

/// Utility class to track a node and its parent.
struct RemovableAssignment {
    /// The node
    node: NodeId,

    /// Its parent
    parent: NodeId,

    /// Full chain of ASSIGN ancestors
    assign_ancestors: Vec<NodeId>,

    /// The last ancestor
    last_ancestor: Option<NodeId>,
}

impl RemovableAssignment {
    /// Data structure for information about a removable assignment.
    ///
    /// `name_node` The LHS, `assign_node` The parent ASSIGN node
    // port: ClosureCodeRemoval.RemovableAssignment#RemovableAssignment
    fn new(ast: &Ast, name_node: NodeId, assign_node: NodeId) -> Self {
        let mut assign_ancestors = Vec::new();
        let mut ancestor = assign_node;
        loop {
            ancestor = ancestor.get_parent(ast).unwrap();
            assign_ancestors.push(ancestor);
            if !(ancestor.is_assign(ast)
                && ancestor
                    .get_first_child(ast)
                    .unwrap()
                    .is_qualified_name(ast))
            {
                break;
            }
        }
        let last_ancestor = ancestor.get_parent(ast);
        Self {
            node: name_node,
            parent: assign_node,
            assign_ancestors,
            last_ancestor,
        }
    }

    /// Remove this node.
    // port: ClosureCodeRemoval.RemovableAssignment#remove
    fn remove(&self, compiler: &mut AbstractCompiler) {
        let rhs = self.node.get_next(compiler).unwrap();
        let mut last = self.parent;
        for &ancestor in &self.assign_ancestors {
            if ancestor.is_expr_result(compiler) {
                ancestor.detach(compiler);
                NodeUtil::mark_functions_deleted(compiler, ancestor);
            } else {
                rhs.detach(compiler);
                last.replace_with(compiler, rhs);
            }
            last = ancestor;
        }
        compiler.report_change_to_enclosing_scope(
            self.last_ancestor.expect("ClosureCodeRemoval lastAncestor"),
        );
    }
}

/// Identifies all assignments of the abstract method to a variable and all methods annotated with
/// "@abstract" in their JSDoc.
struct FindAbstractMethods {
    abstract_method_assignment_nodes: Vec<RemovableAssignment>,
    abstract_member_function_nodes: Vec<NodeId>,
}

impl Callback for FindAbstractMethods {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ClosureCodeRemoval.FindAbstractMethods#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_assign(t) {
            let name_node = n.get_first_child(t).unwrap();
            let value_node = n.get_last_child(t).unwrap();

            if name_node.is_qualified_name(t)
                && value_node.is_qualified_name(t)
                && value_node.matches_qualified_name(t, ABSTRACT_METHOD_NAME)
            {
                // Foo.prototype.bar = goog.abstractMethod
                self.abstract_method_assignment_nodes
                    .push(RemovableAssignment::new(
                        t,
                        n.get_first_child(t).unwrap(),
                        n,
                    ));
            } else if n.get_jsdoc_info_ref(t).is_some()
                && n.get_jsdoc_info(t).unwrap().is_abstract()
                && NodeUtil::is_empty_function_expression(t, value_node)
                && !n.get_jsdoc_info(t).unwrap().is_constructor()
            {
                // @abstract
                self.abstract_method_assignment_nodes
                    .push(RemovableAssignment::new(
                        t,
                        n.get_first_child(t).unwrap(),
                        n,
                    ));
            }
        } else if n.is_member_function_def(t) && parent.unwrap().is_class_members(t) {
            if n.get_jsdoc_info_ref(t).is_some() && n.get_jsdoc_info(t).unwrap().is_abstract() {
                self.abstract_member_function_nodes.push(n);
            }
        }
    }
}

/// Identifies all assertion calls.
struct FindAssertionCalls {
    assertion_names: AssertionFunctionLookup,
    assertion_calls: Vec<NodeId>,
}

impl FindAssertionCalls {
    // port: ClosureCodeRemoval.FindAssertionCalls#FindAssertionCalls
    fn new(compiler: &AbstractCompiler) -> Self {
        Self {
            assertion_names: AssertionFunctionLookup::of(
                compiler.get_coding_convention().get_assertion_functions(),
            ),
            assertion_calls: Vec::new(),
        }
    }
}

impl Callback for FindAssertionCalls {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ClosureCodeRemoval.FindAssertionCalls#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_call(t) {
            return;
        }

        let callee = n.get_first_child(t).unwrap();

        let compiler = t.get_compiler();
        let (registry, ast) = compiler.get_type_registry_field_and_ast();
        if self
            .assertion_names
            .lookup_by_callee_with_optional_registry(ast, registry.as_deref(), callee)
            .is_some() // type-based
            || callee.get_color(ast).is_some_and(|c| c.is_closure_assert())
        {
            // color-based
            self.assertion_calls.push(n);
        }
    }
}

impl ClosureCodeRemoval {
    /// Creates a Closure code remover.
    ///
    /// `remove_abstract_methods` Remove declarations of abstract methods.
    /// `remove_assertion_calls` Remove calls to goog.assert functions.
    // port: ClosureCodeRemoval#ClosureCodeRemoval
    pub fn new(
        _compiler: &AbstractCompiler,
        remove_abstract_methods: bool,
        remove_assertion_calls: bool,
    ) -> Self {
        Self {
            remove_abstract_methods,
            remove_assertion_calls,
            abstract_method_assignment_nodes: Vec::new(),
            abstract_member_function_nodes: Vec::new(),
            assertion_calls: Vec::new(),
        }
    }
}

impl CompilerPass for ClosureCodeRemoval {
    // port: ClosureCodeRemoval#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut find_abstract_methods = FindAbstractMethods {
            abstract_method_assignment_nodes: std::mem::take(
                &mut self.abstract_method_assignment_nodes,
            ),
            abstract_member_function_nodes: std::mem::take(
                &mut self.abstract_member_function_nodes,
            ),
        };
        let mut find_assertion_calls = self.remove_assertion_calls.then(|| FindAssertionCalls {
            assertion_calls: std::mem::take(&mut self.assertion_calls),
            ..FindAssertionCalls::new(compiler)
        });
        {
            let mut passes: Vec<&mut dyn Callback> = Vec::new();
            if self.remove_abstract_methods {
                passes.push(&mut find_abstract_methods);
            }
            if let Some(pass) = find_assertion_calls.as_mut() {
                passes.push(pass);
            }
            CombinedCompilerPass::traverse(compiler, root, passes);
        }
        self.abstract_method_assignment_nodes =
            find_abstract_methods.abstract_method_assignment_nodes;
        self.abstract_member_function_nodes = find_abstract_methods.abstract_member_function_nodes;
        if let Some(pass) = find_assertion_calls {
            self.assertion_calls = pass.assertion_calls;
        }

        for assignment in &self.abstract_method_assignment_nodes {
            assignment.remove(compiler);
        }

        for &member_function in &self.abstract_member_function_nodes {
            let first = member_function.get_first_child(compiler).unwrap();
            compiler.report_function_deleted(first);
            let parent = member_function.get_parent(compiler).unwrap();
            member_function.detach(compiler);
            compiler.report_change_to_enclosing_scope(parent);
        }

        for &call in &self.assertion_calls {
            // If the assertion is an expression, just strip the whole thing.
            compiler.report_change_to_enclosing_scope(call);
            let parent = call.get_parent(compiler).unwrap();
            if parent.is_expr_result(compiler) {
                parent.detach(compiler);
                NodeUtil::mark_functions_deleted(compiler, parent);
            } else {
                // Otherwise, replace the assertion with its first argument,
                // which is the return value of the assertion.
                let first_arg = call.get_second_child(compiler);
                match first_arg {
                    None => {
                        let undefined = NodeUtil::new_undefined_node(compiler, Some(call));
                        call.replace_with(compiler, undefined);
                    }
                    Some(first_arg) => {
                        let replacement = first_arg.detach(compiler);
                        replacement.copy_type_from(compiler, call);
                        call.replace_with(compiler, replacement);
                    }
                }
                NodeUtil::mark_functions_deleted(compiler, call);
            }
        }
    }
}
