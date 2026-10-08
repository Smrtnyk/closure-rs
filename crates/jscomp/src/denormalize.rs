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
//   src/com/google/javascript/jscomp/Denormalize.java.

//! Port of Denormalize.java.
//!
//! The goal with this pass is to reverse the simplifications done in the normalization pass that
//! are not handled by other passes (such as CollapseVariableDeclarations) to avoid making the
//! resulting code larger.
//!
//! Currently this pass only does a few things:
//!
//! 1. Push statements into for-loop initializer. This: `var a = 0; for(;a<0;a++) {}` becomes:
//!    `for(var a = 0;a<0;a++) {}`
//!
//! 2. Fold assignments like `x = x + 1` into `x += 1`
//!
//! 3. Inline 'var' keyword. For instance: `var x; if (y) { x = 0; }` becomes
//!    `if (y) { var x = 0; }`, effectively undoing what HoistVarsOutOfBlocks does.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    reference_collector::{Behavior, ReferenceCollector},
    reference_map::ReferenceMap,
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_state,
    ir::IR,
    node::{Ast, NodeId},
    token::Token,
};

pub struct Denormalize {
    output_feature_set: FeatureSet,
}

impl Denormalize {
    // port: Denormalize#Denormalize
    pub fn new(_compiler: &AbstractCompiler, output_feature_set: FeatureSet) -> Self {
        Self { output_feature_set }
    }

    /// Rust-only: the `outputFeatureSet` field the test harness reads reflectively.
    pub fn get_output_feature_set(&self) -> FeatureSet {
        self.output_feature_set
    }

    // port: Denormalize#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        // Don't inline the VAR declaration if this compilation involves old-style ctemplates.
        if compiler
            .get_options()
            .get_synthetic_block_start_marker()
            .is_none()
        {
            let mut scope_creator = SyntacticScopeCreator::new();
            ReferenceCollector::new(compiler, &mut *self, &mut scope_creator)
                .process(compiler, root);
        }
    }

    /// Collapse VARs and EXPR_RESULT node into FOR loop initializers where possible.
    // port: Denormalize#maybeCollapseIntoForStatements
    fn maybe_collapse_into_for_statements(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        // Only SCRIPT, BLOCK, and LABELs can have FORs that can be collapsed into.
        // LABELs are not supported here.
        let Some(parent) = parent else {
            return;
        };
        if !NodeUtil::is_statement_block(t, parent) {
            return;
        }

        // Is the current node something that can be in a for loop initializer?
        if !n.is_expr_result(t) && !n.is_var(t) {
            return;
        }

        // Is the next statement a valid FOR?
        let Some(next_sibling) = n.get_next(t) else {
            return;
        };
        if next_sibling.is_for_in(t) || next_sibling.is_for_of(t) {
            let for_node = next_sibling;
            let for_var = for_node.get_first_child(t).unwrap();
            if for_var.is_name(t) && n.is_var(t) && n.has_one_child(t) {
                let name = n.get_first_child(t).unwrap();
                if !name.has_children(t) && for_var.get_string(t) == name.get_string(t) {
                    // OK, the names match, and the var declaration does not have an
                    // initializer. Move it into the loop.
                    n.detach(t);
                    for_var.replace_with(t, n);
                    t.get_compiler().report_change_to_enclosing_scope(parent);
                }
            }
        } else if next_sibling.is_vanilla_for(t)
            && next_sibling.get_first_child(t).unwrap().is_empty(t)
        {
            // Does the current node contain an in operator?  If so, embedding
            // the expression in a for loop can cause some JavaScript parsers (such
            // as the PlayStation 3's browser based on Access's NetFront
            // browser) to fail to parse the code.
            // See bug 1778863 for details.
            if NodeUtil::has(t, n, &|ast, node| node.is_in(ast), &|_, _| true) {
                return;
            }

            // Move the current node into the FOR loop initializer.
            let for_node = next_sibling;
            let old_initializer = for_node.get_first_child(t).unwrap();
            n.detach(t);

            let new_initializer = if n.is_var(t) {
                n
            } else {
                // Extract the expression from EXPR_RESULT node.
                check_state!(n.has_one_child(t), "%s", n.to_string(t));
                let new_initializer = n.get_first_child(t).unwrap();
                new_initializer.detach(t);
                new_initializer
            };

            old_initializer.replace_with(t, new_initializer);

            t.get_compiler().report_change_to_enclosing_scope(for_node);
        }
    }

    // port: Denormalize#maybeCollapseAssignShorthand
    fn maybe_collapse_assign_shorthand(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if !Self::is_collapsable_assign(t, n) {
            return;
        }

        let op = n.get_last_child(t).unwrap();
        let assign_op = Self::get_assign_op_from_op(t, op);
        if n.get_first_child(t).unwrap().get_string(t)
            == op.get_first_child(t).unwrap().get_string(t)
        {
            op.set_token(t, assign_op);
            let op_detached = op.detach(t);
            let info = n.get_jsdoc_info(t);
            op_detached.set_jsdoc_info(t, info);
            n.replace_with(t, op_detached);
            t.get_compiler()
                .report_change_to_enclosing_scope(parent.unwrap());
        }
    }

    // port: Denormalize#maybeCollapseLogicalAssignShorthand
    fn maybe_collapse_logical_assign_shorthand(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        if !self.is_collapsable_logical_assign(t, n) {
            return;
        }

        let op = n.get_last_child(t).unwrap();
        let assign_op = Self::get_assign_op_from_op(t, n);
        if n.get_first_child(t).unwrap().get_string(t)
            == op.get_first_child(t).unwrap().get_string(t)
        {
            op.set_token(t, assign_op);
            let op_detached = op.detach(t);
            let info = n.get_jsdoc_info(t);
            op_detached.set_jsdoc_info(t, info);
            n.replace_with(t, op_detached);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::LOGICAL_ASSIGNMENT);
            t.get_compiler()
                .report_change_to_enclosing_scope(parent.unwrap());
        }
    }

    // port: Denormalize#isCollapsableAssign
    fn is_collapsable_assign(ast: &Ast, n: NodeId) -> bool {
        n.is_assign(ast)
            && n.get_first_child(ast).unwrap().is_name(ast)
            && Self::has_corresponding_assignment_op(ast, n.get_last_child(ast).unwrap())
            && n.get_last_child(ast)
                .unwrap()
                .get_first_child(ast)
                .unwrap()
                .is_name(ast)
    }

    // port: Denormalize#isCollapsableLogicalAssign
    fn is_collapsable_logical_assign(&self, ast: &Ast, n: NodeId) -> bool {
        self.output_feature_set.has(Feature::LOGICAL_ASSIGNMENT)
            && (n.is_or(ast) || n.is_and(ast) || n.is_nullish_coalesce(ast))
            && n.get_first_child(ast).unwrap().is_name(ast)
            && n.get_last_child(ast).unwrap().is_assign(ast)
            && n.get_last_child(ast)
                .unwrap()
                .get_first_child(ast)
                .unwrap()
                .is_name(ast)
    }

    // port: Denormalize#getAssignOpFromOp
    fn get_assign_op_from_op(ast: &Ast, n: NodeId) -> Token {
        match n.get_token(ast) {
            Token::BITOR => Token::ASSIGN_BITOR,
            Token::BITXOR => Token::ASSIGN_BITXOR,
            Token::BITAND => Token::ASSIGN_BITAND,
            Token::LSH => Token::ASSIGN_LSH,
            Token::RSH => Token::ASSIGN_RSH,
            Token::URSH => Token::ASSIGN_URSH,
            Token::ADD => Token::ASSIGN_ADD,
            Token::SUB => Token::ASSIGN_SUB,
            Token::MUL => Token::ASSIGN_MUL,
            Token::EXPONENT => Token::ASSIGN_EXPONENT,
            Token::DIV => Token::ASSIGN_DIV,
            Token::MOD => Token::ASSIGN_MOD,
            Token::OR => Token::ASSIGN_OR,
            Token::AND => Token::ASSIGN_AND,
            Token::COALESCE => Token::ASSIGN_COALESCE,
            _ => panic!(
                "IllegalStateException: Unexpected operator: {}",
                n.to_string(ast)
            ),
        }
    }

    // port: Denormalize#hasCorrespondingAssignmentOp
    fn has_corresponding_assignment_op(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::BITOR
                | Token::BITXOR
                | Token::BITAND
                | Token::LSH
                | Token::RSH
                | Token::URSH
                | Token::ADD
                | Token::SUB
                | Token::MUL
                | Token::DIV
                | Token::MOD
        )
    }
}

impl CompilerPass for Denormalize {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Denormalize::process(self, compiler, externs, root);
    }
}

impl Callback for Denormalize {
    // port: Denormalize#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Denormalize#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        self.maybe_collapse_into_for_statements(t, n, parent);
        self.maybe_collapse_logical_assign_shorthand(t, n, parent);
        self.maybe_collapse_assign_shorthand(t, n, parent);
    }
}

impl Behavior for Denormalize {
    /// Implements step 3 (inlining the var keyword).
    // port: Denormalize#afterExitScope
    fn after_exit_scope(&mut self, t: &mut NodeTraversal<'_>, reference_map: &dyn ReferenceMap) {
        let scope_root = t.get_scope_root().unwrap();
        if scope_root.is_block(t) && scope_root.get_parent(t).unwrap().is_function(t) {
            let mut changed = false;
            let scope = t.get_scope();
            for v in scope.get_var_iterable(t.get_compiler()) {
                let references = reference_map
                    .get_references(v)
                    .expect("NullPointerException");
                let mut declaration = None;
                let mut assign = None;
                for r in references {
                    if r.is_var_declaration(t)
                        && NodeUtil::is_statement(t, r.get_node().get_parent(t).unwrap())
                        && !r.is_initializing_declaration(t)
                    {
                        declaration = Some(r);
                    } else if assign.is_none()
                        && r.is_simple_assignment_to_name(t)
                        && r.get_scope()
                            .unwrap()
                            .get_closest_hoist_scope(t.get_compiler())
                            == Some(scope)
                    {
                        assign = Some(r);
                    }
                }

                if let (Some(declaration), Some(assign)) = (declaration, assign) {
                    let lhs = assign.get_node();
                    let assign_node = lhs.get_parent(t).unwrap();
                    if assign_node.get_parent(t).unwrap().is_expr_result(t) {
                        let rhs = lhs.get_next(t).unwrap();
                        let expr_result = assign_node.get_parent(t).unwrap();
                        let lhs = lhs.detach(t);
                        let rhs = rhs.detach(t);
                        let new_var = IR::var_with_value(t, lhs, rhs);
                        expr_result.replace_with(t, new_var);
                        let var = declaration.get_node().get_parent(t).unwrap();
                        check_state!(var.is_var(t), "%s", var.to_string(t));
                        NodeUtil::remove_child(t, var, declaration.get_node());
                        changed = true;
                    }
                }
            }

            if changed {
                t.report_code_change();
            }
        }
    }
}
