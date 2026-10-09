/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteLogicalAssignmentOperatorsHelper.java.

//! Port of `RewriteLogicalAssignmentOperatorsHelper.java`.
// Preserve Java's declaration of locals assigned in later branches.
#![allow(clippy::needless_late_init)]
use crate::{ast_factory::AstFactory, node_traversal::NodeTraversal, node_util::NodeUtil};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{check_not_null, check_state, ir::IR, node::NodeId};
use std::rc::Rc;

const TEMP_VAR_NAME_PREFIX: &str = "$jscomp$logical$assign$tmp";
const TEMP_INDEX_VAR_NAME_PREFIX: &str = "$jscomp$logical$assign$tmpindex";

/// Helper class for RewriteLogicalAssignmentOperators
///
/// Java keeps the compiler and `compiler.getUniqueIdSupplier()` (every caller passes the
/// compiler's own supplier); here both are reached through the traversal's compiler (DESIGN §6).
pub struct RewriteLogicalAssignmentOperatorsHelper {
    ast_factory: Rc<AstFactory>,
}

impl RewriteLogicalAssignmentOperatorsHelper {
    // port: RewriteLogicalAssignmentOperatorsHelper#RewriteLogicalAssignmentOperatorsHelper
    pub fn new(ast_factory: Rc<AstFactory>) -> Self {
        Self { ast_factory }
    }

    // port: RewriteLogicalAssignmentOperatorsHelper#visitLogicalAssignmentOperator
    pub fn visit_logical_assignment_operator(
        &self,
        t: &mut NodeTraversal<'_>,
        logical_assignment: NodeId,
    ) {
        let enclosing_function = NodeUtil::get_enclosing_function(t, logical_assignment);
        if let Some(enclosing_function) = enclosing_function
            && !NodeUtil::get_function_body(t, enclosing_function).is_block(t)
        {
            let return_value = NodeUtil::get_function_body(t, enclosing_function);
            let detached = return_value.detach(t);
            let return_node = IR::return_node_with_expression(t, detached);
            let body = IR::block_with_child(t, return_node).srcref_tree_if_missing(t, return_value);
            enclosing_function.add_child_to_back(t, body);
            t.get_compiler().report_change_to_enclosing_scope(body);
        }

        let enclosing_statement = NodeUtil::get_enclosing_statement(t, logical_assignment);

        let mut left = check_not_null!(logical_assignment.remove_first_child(t));
        let right = logical_assignment.get_last_child(t).unwrap().detach(t);

        let replacement;

        while left.is_cast(t) {
            // This pass runs after type checking, which is what requires the CAST nodes.
            // They aren't needed after that, but they are still in the AST until
            // Normalization removes them. So, it's safe to just remove this CAST now to simplify
            // things. If this pass moves after Normalization, then it will no longer be necessary
            // to check for CAST nodes.
            left = check_not_null!(left.remove_first_child(t));
        }

        if left.is_name(t) {
            replacement = self.handle_lhs_name(t, logical_assignment, left, right);
        } else {
            check_state!(
                left.is_get_prop(t) || left.is_get_elem(t),
                "%s",
                left.to_string(t)
            );
            replacement = self.handle_lhs_property_reference(
                t,
                logical_assignment,
                left,
                right,
                check_not_null!(enclosing_statement),
            );
        }

        logical_assignment.replace_with(t, replacement);

        t.get_compiler()
            .report_change_to_enclosing_scope(check_not_null!(enclosing_statement));
    }

    // port: RewriteLogicalAssignmentOperatorsHelper#handleLHSName
    pub fn handle_lhs_name(
        &self,
        t: &mut NodeTraversal<'_>,
        logical_assignment: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        // handle name case
        // e.g. convert `name ??= something()` to
        // `name ?? (name = something())`
        let assign_to_rhs = self
            .ast_factory
            .create_assign(t.get_compiler(), left, right)
            .srcref(t, right);

        // TODO(bradfordcsmith): We should have an AstFactory method for this.
        let op = NodeUtil::get_op_from_assignment_op(t, logical_assignment);
        let left_clone = left.clone_node(t);
        t.new_node_with_children2(op, left_clone, assign_to_rhs)
            .copy_type_from(t, logical_assignment)
            .srcref(t, logical_assignment)
    }

    // port: RewriteLogicalAssignmentOperatorsHelper#handleLHSPropertyReference
    pub fn handle_lhs_property_reference(
        &self,
        t: &mut NodeTraversal<'_>,
        logical_assignment: NodeId,
        left: NodeId,
        right: NodeId,
        enclosing_statement: NodeId,
    ) -> NodeId {
        // handle getprop case and getelem case
        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let temp_var_name = format!("{TEMP_VAR_NAME_PREFIX}{unique_id}");

        let assign_to_rhs;
        let new_lhs;

        let object_node = check_not_null!(left.remove_first_child(t));

        let r#let = self
            .ast_factory
            .create_single_let_name_declaration(t.get_compiler(), temp_var_name.as_str())
            .srcref_tree(t, logical_assignment);
        r#let.insert_before(t, enclosing_statement);

        let object_type = AstFactory::type_node(object_node);
        let temp_name = self
            .ast_factory
            .create_name(t.get_compiler(), temp_var_name.as_str(), object_type)
            .srcref(t, object_node);
        let assign_temp = self
            .ast_factory
            .create_assign(t.get_compiler(), temp_name, object_node)
            .srcref(t, object_node); // (tmp = someExpression)

        if left.is_get_prop(t) {
            // handle getprop case
            // e.g. convert `(someExpression).property ??= something()` to
            // let tmp;
            // (tmp = someExpression).property ?? (tmp.property = something());
            let property_name = left.get_string(t);

            let temp_name_clone = temp_name.clone_node(t);
            let left_type = AstFactory::type_node(left);
            let temp_prop = self
                .ast_factory
                .create_get_prop(
                    t.get_compiler(),
                    temp_name_clone,
                    property_name.clone(),
                    left_type,
                )
                .srcref(t, right); // (tmp.property)
            assign_to_rhs = self
                .ast_factory
                .create_assign(t.get_compiler(), temp_prop, right)
                .srcref(t, right); // (tmp.property = something())
            let left_type = AstFactory::type_node(left);
            new_lhs = self
                .ast_factory
                .create_get_prop(t.get_compiler(), assign_temp, property_name, left_type)
                .srcref(t, left); // ((tmp = someExpression).property)
        } else {
            // handle getelem case
            // e.g. convert `someExpression[indexExpression] ??= something()` to
            // let tmp;
            // let tmpIndex;
            // (tmp = someExpression)[tmpIndex = indexExpression] ?? (tmp[tmpIndex] = something());
            check_state!(left.is_get_elem(t), "%s", left.to_string(t));
            let temp_index_var_name = format!("{TEMP_INDEX_VAR_NAME_PREFIX}{unique_id}");

            let index_expr_node = left.get_last_child(t).unwrap().detach(t);

            let let_index = self
                .ast_factory
                .create_single_let_name_declaration(t.get_compiler(), temp_index_var_name.as_str())
                .srcref_tree(t, logical_assignment);
            let_index.insert_before(t, enclosing_statement);

            let index_type = AstFactory::type_node(index_expr_node);
            let temp_index_name = self
                .ast_factory
                .create_name(t.get_compiler(), temp_index_var_name.as_str(), index_type)
                .srcref(t, index_expr_node); // tmpIndex

            let assign_temp_index = self
                .ast_factory
                .create_assign(t.get_compiler(), temp_index_name, index_expr_node)
                .srcref(t, index_expr_node); // [tmpIndex = indexExpression]

            let temp_name_clone = temp_name.clone_node(t);
            let temp_index_name_clone = temp_index_name.clone_node(t);
            let temp_elem = self
                .ast_factory
                .create_get_elem(t.get_compiler(), temp_name_clone, temp_index_name_clone)
                .copy_type_from(t, left)
                .srcref(t, right); // (tmp[tmpIndex])

            assign_to_rhs = self
                .ast_factory
                .create_assign(t.get_compiler(), temp_elem, right)
                .srcref(t, right); // (tmp[tmpIndex] = something())
            new_lhs = self
                .ast_factory
                .create_get_elem(t.get_compiler(), assign_temp, assign_temp_index)
                .copy_type_from(t, left)
                .srcref(t, left); // (tmp = someExpression)[tmpIndex = indexExpression]
        }

        let current_script = check_not_null!(t.get_current_script());
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            current_script,
            Feature::LET_DECLARATIONS,
        );

        // TODO(bradfordcsmith): We should have an AstFactory method for this.
        let op = NodeUtil::get_op_from_assignment_op(t, logical_assignment);
        t.new_node_with_children2(op, new_lhs, assign_to_rhs)
            .copy_type_from(t, logical_assignment)
            .srcref(t, logical_assignment)
    }
}
