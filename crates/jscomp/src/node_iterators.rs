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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/NodeIterators.java.

//! Port of NodeIterators.java: a utility class for some useful iterators over the AST.
//!
//! Java's iterators hold their nodes; the arena convention passes the `Ast` (or the compiler) to
//! `has_next`/`next`. `remove` is not provided (Java throws UnsupportedOperationException).
use crate::abstract_compiler::AbstractCompiler;
use crate::node_util::NodeUtil;
use closure_rhino::check_argument;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::collections::VecDeque;

/// Traverses the local scope, skipping all function nodes.
///
/// Also, will not traverse the RHS of a declaration or assignment node if it is a function.
// port: NodeIterators.FunctionlessLocalScope
pub struct FunctionlessLocalScope {
    ancestors: VecDeque<NodeId>,
}

impl FunctionlessLocalScope {
    /// The `ancestors` are the current node followed by its ancestors, innermost first.
    // port: NodeIterators.FunctionlessLocalScope#FunctionlessLocalScope
    pub fn new(ast: &Ast, ancestors: &[NodeId]) -> Self {
        check_argument!(!ancestors.is_empty());
        let mut deque = VecDeque::new();
        for &n in ancestors {
            if n.is_function(ast) {
                break;
            }
            deque.push_front(n);
        }
        Self { ancestors: deque }
    }

    // port: NodeIterators.FunctionlessLocalScope#hasNext
    pub fn has_next(&self, ast: &Ast) -> bool {
        // Check if the current node has any nodes after it.
        !(self.ancestors.len() == 1 && self.ancestors.back().unwrap().get_next(ast).is_none())
    }

    // port: NodeIterators.FunctionlessLocalScope#next
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self, ast: &Ast) -> NodeId {
        let mut current = self.ancestors.pop_back().unwrap();
        if current.get_next(ast).is_none() {
            current = *self.ancestors.back().unwrap();

            // If this is a function node, skip it.
            if current.is_function(ast) {
                return self.next(ast);
            }
        } else {
            current = current.get_next(ast).unwrap();
            self.ancestors.push_back(current);

            // If this is a function node, skip it.
            if current.is_function(ast) {
                return self.next(ast);
            }

            while current.has_children(ast) {
                current = current.get_first_child(ast).unwrap();
                self.ancestors.push_back(current);

                // If this is a function node, skip it.
                if current.is_function(ast) {
                    return self.next(ast);
                }
            }
        }

        current
    }

    // port: NodeIterators.FunctionlessLocalScope#current
    pub fn current(&self) -> NodeId {
        *self.ancestors.back().unwrap()
    }

    // port: NodeIterators.FunctionlessLocalScope#currentParent
    pub fn current_parent(&self, ast: &Ast) -> Option<NodeId> {
        if self.ancestors.len() >= 2 {
            self.current().get_parent(ast)
        } else {
            None
        }
    }

    // port: NodeIterators.FunctionlessLocalScope#currentAncestors
    pub fn current_ancestors(&self) -> Vec<NodeId> {
        let mut list: Vec<NodeId> = self.ancestors.iter().copied().collect();
        list.reverse();
        list
    }
}

/// An iterator to help with variable inlining. Given a variable declaration, find all the nodes
/// in post-order where the variable is guaranteed to retain its original value.
// port: NodeIterators.LocalVarMotion
pub struct LocalVarMotion {
    value_has_side_effects: bool,
    iterator: FunctionlessLocalScope,
    var_name: closure_rhino::js_string::JsString,
    look_ahead: Option<NodeId>,
}

impl LocalVarMotion {
    /// `name` is the name node of the declaration, `var` its VAR/LET/CONST statement and
    /// `block` the block containing the statement.
    // port: NodeIterators.LocalVarMotion#forVar
    pub fn for_var(
        compiler: &mut AbstractCompiler,
        name: NodeId,
        var: NodeId,
        block: NodeId,
    ) -> Self {
        check_argument!(NodeUtil::is_name_declaration(compiler, Some(var)));
        check_argument!(NodeUtil::is_statement(compiler, var));
        // The FunctionlessLocalScope must start at "name" as this may be used
        // before the Normalize pass, and thus the VAR node may define multiple
        // names and the "name" node may have siblings.  The actual assigned
        // value is skipped as it is a child of name.
        let iterator = FunctionlessLocalScope::new(compiler, &[name, var, block]);
        Self::new(compiler, name, iterator)
    }

    /// `name` is the name node of the assignment target, `assign` the ASSIGN, `expr` the
    /// EXPR_RESULT and `block` the block containing the statement.
    // port: NodeIterators.LocalVarMotion#forAssign
    pub fn for_assign(
        compiler: &mut AbstractCompiler,
        name: NodeId,
        assign: NodeId,
        expr: NodeId,
        block: NodeId,
    ) -> Self {
        check_argument!(assign.is_assign(compiler));
        check_argument!(expr.is_expr_result(compiler));
        // The FunctionlessLocalScope must start at "assign", to skip the value
        // assigned to "name" (which would be its sibling).
        let iterator = FunctionlessLocalScope::new(compiler, &[assign, expr, block]);
        Self::new(compiler, name, iterator)
    }

    // port: NodeIterators.LocalVarMotion#LocalVarMotion
    fn new(
        compiler: &mut AbstractCompiler,
        name_node: NodeId,
        iterator: FunctionlessLocalScope,
    ) -> Self {
        check_argument!(name_node.is_name(compiler));
        let value_node = NodeUtil::get_assigned_value(compiler, name_node);
        // checkNotNull(compiler): a Rust reference is never null.
        let var_name = name_node.get_string(compiler);
        let value_has_side_effects = value_node.is_some_and(|value_node| {
            compiler
                .get_ast_analyzer()
                .may_have_side_effects(compiler, value_node)
        });
        let mut motion = Self {
            value_has_side_effects,
            iterator,
            var_name,
            look_ahead: None,
        };
        motion.advance_look_ahead(compiler, true);
        motion
    }

    // port: NodeIterators.LocalVarMotion#hasNext
    pub fn has_next(&self) -> bool {
        self.look_ahead.is_some()
    }

    // port: NodeIterators.LocalVarMotion#next
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self, compiler: &mut AbstractCompiler) -> Option<NodeId> {
        let next = self.look_ahead;
        self.advance_look_ahead(compiler, false);
        next
    }

    // port: NodeIterators.LocalVarMotion#advanceLookAhead
    fn advance_look_ahead(&mut self, compiler: &mut AbstractCompiler, at_start: bool) {
        if !at_start {
            if self.look_ahead.is_none() {
                return;
            }

            // Don't advance past a reference to the variable that we're trying
            // to inline.
            let cur_node = self.iterator.current();
            if cur_node.is_name(compiler) && self.var_name == cur_node.get_string(compiler) {
                self.look_ahead = None;
                return;
            }
        }

        if !self.iterator.has_next(compiler) {
            self.look_ahead = None;
            return;
        }

        let next_node = self.iterator.next(compiler);
        let next_parent = self.iterator.current_parent(compiler);
        let type_ = next_node.get_token(compiler);

        if self.value_has_side_effects {
            // Reject anything that might read state
            let mut reads_state = false;

            if
            // Any read of a different variable.
            (next_node.is_name(compiler) && self.var_name != next_node.get_string(compiler))
                // Any read of a property.
                || (next_node.is_get_prop(compiler)
                    || next_node.is_get_elem(compiler)
                    || next_node.is_opt_chain_get_prop(compiler)
                    || next_node.is_opt_chain_get_elem(compiler))
            {
                // If this is a simple assign, we'll be ok.
                if next_parent.is_none()
                    || !NodeUtil::is_name_decl_or_simple_assign_lhs(
                        compiler,
                        next_node,
                        next_parent.unwrap(),
                    )
                {
                    reads_state = true;
                }
            } else if next_node.is_call(compiler)
                || next_node.is_new(compiler)
                || next_node.is_opt_chain_call(compiler)
            {
                // This isn't really an important case. In most cases when we use
                // CALL or NEW, we're invoking it on a NAME or a GETPROP. And in the
                // few cases where we're not, it's because we have an anonymous
                // function that escapes the variable we're worried about. But we
                // include this for completeness.
                reads_state = true;
            }

            if reads_state {
                self.look_ahead = None;
                return;
            }
        }

        // Reject anything that might modify relevant state. We assume that
        // nobody relies on variables being undeclared, which will break
        // constructions like:
        //   var a = b;
        //   var b = 3;
        //   alert(a);
        if (compiler
            .get_ast_analyzer()
            .node_type_may_have_side_effects(compiler, next_node)
            && type_ != Token::NAME)
            || (type_ == Token::NAME && next_parent.unwrap().is_catch(compiler))
        {
            self.look_ahead = None;
            return;
        }

        self.look_ahead = Some(next_node);
    }
}
