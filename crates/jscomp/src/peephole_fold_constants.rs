/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeFoldConstants.java.

//! Port of `PeepholeFoldConstants.java`.
//!
//! Peephole optimization to fold constants (e.g. x + 1 + 7 --> x + 8).

use crate::colors::standard_colors as StandardColors;
use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    diagnostic_type::DiagnosticType,
    node_util::{NodeUtil, ValueType},
};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    java_lang::{self, big_integer, math as java_math},
    js_string::JsString,
    jscomp_base::{JSCompDoubles, Tri},
    node::{Ast, NodeId},
    token::Token,
};
use num_bigint::{BigInt, Sign};

// TODO(johnlenz): optimizations should not be emiting errors. Move these to
// a check pass.
// port: PeepholeFoldConstants#INVALID_GETELEM_INDEX_ERROR
pub static INVALID_GETELEM_INDEX_ERROR: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_GETELEM_INDEX_ERROR",
    "Array index not integer: {0}",
);

// port: PeepholeFoldConstants#FRACTIONAL_BITWISE_OPERAND
pub static FRACTIONAL_BITWISE_OPERAND: DiagnosticType = DiagnosticType::warning(
    "JSC_FRACTIONAL_BITWISE_OPERAND",
    "Fractional bitwise operand: {0}",
);

// port: PeepholeFoldConstants#MAX_FOLD_NUMBER
const MAX_FOLD_NUMBER: f64 = 9007199254740992.0; // Math.pow(2, 53)

/// A nullable Java `String` as Java string concatenation renders it: `null` becomes `"null"`.
fn java_string_value(s: Option<JsString>) -> JsString {
    s.unwrap_or_else(|| JsString::from("null"))
}

pub struct PeepholeFoldConstants {
    fields: AbstractPeepholeOptimizationFields,
    late: bool,
    should_use_types: bool,
}

impl PeepholeFoldConstants {
    /// `late`: When late is false, this mean we are currently running before most of the other
    /// optimizations. In this case we would avoid optimizations that would make the code harder
    /// to analyze. When this is true, we would do anything to minimize for size.
    // port: PeepholeFoldConstants#PeepholeFoldConstants
    pub fn new(late: bool, should_use_types: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            late,
            should_use_types,
        }
    }

    // port: PeepholeFoldConstants#tryFoldBinaryOperator
    fn try_fold_binary_operator(&self, compiler: &mut AbstractCompiler, subtree: NodeId) -> NodeId {
        let Some(left) = subtree.get_first_child(compiler) else {
            return subtree;
        };

        let Some(right) = left.get_next(compiler) else {
            return subtree;
        };

        // If we've reached here, node is truly a binary operator.
        match subtree.get_token(compiler) {
            Token::GETELEM | Token::OPTCHAIN_GETELEM => {
                self.try_fold_get_elem(compiler, subtree, left, right)
            }
            Token::INSTANCEOF => self.try_fold_instanceof(compiler, subtree, left, right),
            Token::AND | Token::OR => self.try_fold_and_or(compiler, subtree, left, right),
            Token::COALESCE => self.try_fold_coalesce(compiler, subtree, left, right),
            Token::LSH | Token::RSH | Token::URSH => {
                self.try_fold_shift(compiler, subtree, left, right)
            }
            Token::ASSIGN => self.try_fold_assign(compiler, subtree, left, right),
            Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_EXPONENT => self.try_unfold_assign_op(compiler, subtree, left, right),
            Token::ADD => self.try_fold_add(compiler, subtree, left, right),
            Token::SUB | Token::DIV | Token::MOD | Token::MUL | Token::EXPONENT => {
                self.try_fold_arithmetic_op(compiler, subtree, left, right)
            }
            Token::BITAND | Token::BITOR | Token::BITXOR => {
                let result = self.try_fold_arithmetic_op(compiler, subtree, left, right);
                if result != subtree {
                    return result;
                }
                self.try_fold_left_child_op(compiler, subtree, left, right)
            }
            Token::LT
            | Token::GT
            | Token::LE
            | Token::GE
            | Token::EQ
            | Token::NE
            | Token::SHEQ
            | Token::SHNE => self.try_fold_comparison(compiler, subtree, left, right),
            _ => subtree,
        }
    }

    // port: PeepholeFoldConstants#tryReduceVoid
    fn try_reduce_void(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let child = n.get_first_child(compiler).unwrap();
        if (!child.is_number(compiler) || child.get_double(compiler) != 0.0)
            && !self.may_have_side_effects(compiler, n)
        {
            let zero = IR::number(compiler, 0.0);
            child.replace_with(compiler, zero);
            self.report_change_to_enclosing_scope(compiler, n);
        }
        n
    }

    // port: PeepholeFoldConstants#tryReduceOperandsForOp
    fn try_reduce_operands_for_op(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::ADD => {
                let left = n.get_first_child(compiler).unwrap();
                let right = n.get_last_child(compiler).unwrap();
                let should_use_types = self.should_use_types;
                let (types, ast) = compiler.get_type_registry_field_and_ast();
                let types = types.as_deref();
                if !NodeUtil::may_be_string_with_type(ast, left, should_use_types, types)
                    && !NodeUtil::may_be_string_with_type(ast, right, should_use_types, types)
                {
                    self.try_convert_operands_to_number(compiler, n);
                }
            }
            Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_MOD
            | Token::ASSIGN_DIV => {
                // TODO(johnlenz): convert these to integers.
                let last = n.get_last_child(compiler).unwrap();
                self.try_convert_to_number(compiler, last);
            }
            Token::BITNOT
            | Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::LSH
            | Token::RSH
            | Token::URSH
            | Token::SUB
            | Token::MUL
            | Token::MOD
            | Token::DIV
            | Token::POS
            | Token::NEG
            | Token::EXPONENT => self.try_convert_operands_to_number(compiler, n),
            _ => {}
        }
    }

    // port: PeepholeFoldConstants#tryConvertOperandsToNumber
    fn try_convert_operands_to_number(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut c = n.get_first_child(compiler);
        while let Some(cur) = c {
            let next = cur.get_next(compiler);
            self.try_convert_to_number(compiler, cur);
            c = next;
        }
    }

    // port: PeepholeFoldConstants#tryConvertToNumber
    fn try_convert_to_number(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        match n.get_token(compiler) {
            Token::NUMBER => {
                // Nothing to do
                return;
            }
            Token::AND | Token::OR | Token::COMMA | Token::COALESCE => {
                let last = n.get_last_child(compiler).unwrap();
                self.try_convert_to_number(compiler, last);
                return;
            }
            Token::HOOK => {
                let second = n.get_second_child(compiler).unwrap();
                self.try_convert_to_number(compiler, second);
                let last = n.get_last_child(compiler).unwrap();
                self.try_convert_to_number(compiler, last);
                return;
            }
            Token::NAME if !NodeUtil::is_undefined(compiler, n) => {
                return;
            }
            _ => {}
        }

        let Some(result) = self.get_side_effect_free_number_value(compiler, n) else {
            return;
        };

        let value = result;

        let replacement = NodeUtil::number_node(compiler, value, Some(n));
        if replacement.is_equivalent_to(compiler, n) {
            return;
        }

        n.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
    }

    /// Folds 'typeof(foo)' if foo is a literal, e.g. typeof("bar") --> "string" typeof(6) -->
    /// "number"
    // port: PeepholeFoldConstants#tryFoldTypeof
    fn try_fold_typeof(
        &self,
        compiler: &mut AbstractCompiler,
        original_typeof_node: NodeId,
    ) -> NodeId {
        check_argument!(original_typeof_node.is_type_of(compiler));

        let argument_node = match original_typeof_node.get_first_child(compiler) {
            Some(argument_node) if NodeUtil::is_literal_value(compiler, argument_node, true) => {
                argument_node
            }
            _ => return original_typeof_node,
        };

        let mut type_name_string: Option<&str> = None;

        match argument_node.get_token(compiler) {
            Token::FUNCTION => type_name_string = Some("function"),
            Token::STRINGLIT => type_name_string = Some("string"),
            Token::NUMBER => type_name_string = Some("number"),
            Token::TRUE | Token::FALSE => type_name_string = Some("boolean"),
            Token::NULL | Token::OBJECTLIT | Token::ARRAYLIT => type_name_string = Some("object"),
            Token::VOID => type_name_string = Some("undefined"),
            // We assume here that programs don't change the value of the
            // keyword undefined to something other than the value undefined.
            Token::NAME if argument_node.get_string(compiler) == "undefined" => {
                type_name_string = Some("undefined");
            }
            _ => {}
        }

        if let Some(type_name_string) = type_name_string {
            let new_node = IR::string(compiler, type_name_string);
            self.report_change_to_enclosing_scope(compiler, original_typeof_node);
            original_typeof_node.replace_with(compiler, new_node);
            self.mark_functions_deleted(compiler, original_typeof_node);

            return new_node;
        }

        original_typeof_node
    }

    // port: PeepholeFoldConstants#tryFoldUnaryOperator
    fn try_fold_unary_operator(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.has_one_child(compiler), "%s", n.to_string(compiler));

        let left = n.get_first_child(compiler);
        let parent = n.get_parent(compiler);

        let Some(left) = left else {
            return n;
        };

        let left_val = self.get_side_effect_free_boolean_value(compiler, left);
        if left_val == Tri::UNKNOWN {
            return n;
        }

        match n.get_token(compiler) {
            Token::NOT => {
                // Don't fold !0 and !1 back to false.
                if self.late && left.is_number(compiler) {
                    let num_value = left.get_double(compiler);
                    if num_value == 0.0 || num_value == 1.0 {
                        return n;
                    }
                }
                let replacement_node = NodeUtil::boolean_node(compiler, !left_val.to_boolean(true));
                n.replace_with(compiler, replacement_node);
                self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                replacement_node
            }
            Token::POS => {
                if NodeUtil::is_numeric_result(compiler, left) {
                    // POS does nothing to numeric values.
                    let detached_left = left.detach(compiler);
                    n.replace_with(compiler, detached_left);
                    self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                    return left;
                }
                n
            }
            Token::NEG => {
                let mut result = None;
                if left.is_name(compiler) && left.get_string(compiler) == "NaN" {
                    result = Some(left.detach(compiler)); // "-NaN" is "NaN".
                } else if left.is_neg(compiler) {
                    let left_left = left.get_only_child(compiler);
                    if left_left.is_big_int(compiler) || left_left.is_number(compiler) {
                        result = Some(left_left.detach(compiler)); // `-(-4)` is `4`
                    }
                }

                if let Some(result) = result {
                    n.replace_with(compiler, result);
                    self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                    return result;
                }
                n
            }
            Token::BITNOT => {
                let double_val = self.get_side_effect_free_number_value(compiler, left);
                if let Some(double_val) = double_val {
                    if JSCompDoubles::is_mathematical_integer(double_val) {
                        let int_val = JSCompDoubles::ecmascript_to_int32(double_val);
                        let not_int_val_node =
                            NodeUtil::number_node(compiler, f64::from(!int_val), Some(left));
                        n.replace_with(compiler, not_int_val_node);
                        self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                        return not_int_val_node;
                    } else {
                        self.report(compiler, &FRACTIONAL_BITWISE_OPERAND, left);
                        return n;
                    }
                }

                let bigint_val = self.get_side_effect_free_big_int_value(compiler, n);
                if let Some(bigint_val) = bigint_val {
                    let bigint_not_node = Self::bigint_node(compiler, bigint_val, n);
                    n.replace_with(compiler, bigint_not_node);
                    self.report_change_to_enclosing_scope(compiler, parent.unwrap());
                    return bigint_not_node;
                }

                n
            }
            _ => n,
        }
    }

    // port: PeepholeFoldConstants#isReasonableDoubleValue
    fn is_reasonable_double_value(&self, x: Option<f64>) -> bool {
        x.is_some_and(|x| !x.is_infinite() && !x.is_nan())
    }

    // port: PeepholeFoldConstants#tryFoldInstanceof
    fn try_fold_instanceof(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        check_argument!(n.is_instance_of(compiler));

        // TODO(johnlenz) Use type information if available to fold
        // instanceof.
        if NodeUtil::is_literal_value(compiler, left, true)
            && !self.may_have_side_effects(compiler, right)
        {
            let mut replacement_node = None;

            if NodeUtil::is_immutable_value(compiler, left) {
                // Non-object types are never instances.
                replacement_node = Some(IR::false_node(compiler));
            } else if left.is_template_lit(compiler) {
                // Template literals always evaluate to strings. If they do not meet the "immutable
                // value" condition above, verify their substitutions have no side effects before
                // folding.
                if !self.may_have_side_effects(compiler, left) {
                    replacement_node = Some(IR::false_node(compiler));
                }
            } else if right.is_name(compiler) && right.get_string(compiler) == "Object" {
                replacement_node = Some(IR::true_node(compiler));
            }

            if let Some(replacement_node) = replacement_node {
                n.replace_with(compiler, replacement_node);
                self.report_change_to_enclosing_scope(compiler, replacement_node);
                self.mark_functions_deleted(compiler, n);
                return replacement_node;
            }
        }

        n
    }

    // port: PeepholeFoldConstants#tryFoldAssign
    fn try_fold_assign(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        check_argument!(n.is_assign(compiler));

        if !self.late {
            return n;
        }

        // Only certain RHS operators are supported.
        let operator = right.get_token(compiler);
        let new_type = match operator {
            Token::ADD => Some(Token::ASSIGN_ADD),
            Token::BITAND => Some(Token::ASSIGN_BITAND),
            Token::BITOR => Some(Token::ASSIGN_BITOR),
            Token::BITXOR => Some(Token::ASSIGN_BITXOR),
            Token::DIV => Some(Token::ASSIGN_DIV),
            Token::LSH => Some(Token::ASSIGN_LSH),
            Token::MOD => Some(Token::ASSIGN_MOD),
            Token::MUL => Some(Token::ASSIGN_MUL),
            Token::RSH => Some(Token::ASSIGN_RSH),
            Token::SUB => Some(Token::ASSIGN_SUB),
            Token::URSH => Some(Token::ASSIGN_URSH),
            Token::EXPONENT => Some(Token::ASSIGN_EXPONENT),
            _ => None,
        };
        let Some(new_type) = new_type else {
            return n;
        };

        // Tries to convert x = x + y -> x += y;
        if !right.has_children(compiler)
            || right.get_second_child(compiler) != right.get_last_child(compiler)
        {
            // RHS must have two children.
            return n;
        }

        if self.may_have_side_effects(compiler, left) {
            return n;
        }

        // We can fold any 'leaf' that
        // 1. is equal to the left side of the assignment
        // 2. is either the first child of the operator or any child of the operator if the
        //    operator is commutative
        // 3. Also consider recursively discoverable children of the operator if the operator is
        //    associative.
        //
        // For example, consider x = x + y + z;
        // because + is left associative, the tree looks like `(x + y) + z` so we need to look
        // deeper into the tree to find x.
        // Right now we only recurse one level deep which allows use to handle x = x + y + z but
        // not x = x + y + z + w.

        let right_first = right.get_first_child(compiler).unwrap();
        let new_right;
        if self.are_nodes_equal_for_inlining(compiler, left, right_first) {
            new_right = right.get_last_child(compiler).unwrap();
        } else if NodeUtil::is_commutative(operator)
            && !self.may_have_side_effects(compiler, right_first)
            && self.are_nodes_equal_for_inlining(
                compiler,
                left,
                right.get_last_child(compiler).unwrap(),
            )
        {
            new_right = right_first;
        } else if NodeUtil::is_associative(operator)
            && right_first.get_token(compiler) == operator
            && self.are_nodes_equal_for_inlining(
                compiler,
                left,
                right.get_first_first_child(compiler).unwrap(),
            )
        {
            let new_left = right_first
                .get_last_child(compiler)
                .unwrap()
                .detach(compiler);
            right.remove_first_child(compiler);
            right.add_child_to_front(compiler, new_left);
            new_right = right;
        } else {
            return n;
        }

        let detached_left = left.detach(compiler);
        let detached_right = new_right.detach(compiler);
        let new_node = compiler.new_node_with_children2(new_type, detached_left, detached_right);
        n.replace_with(compiler, new_node);

        self.report_change_to_enclosing_scope(compiler, new_node);

        new_node
    }

    // port: PeepholeFoldConstants#tryUnfoldAssignOp
    fn try_unfold_assign_op(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        if self.late {
            return n;
        }

        if !n.has_children(compiler) || n.get_second_child(compiler) != n.get_last_child(compiler) {
            return n;
        }

        if self.may_have_side_effects(compiler, left) {
            return n;
        }

        // Tries to convert x += y -> x = x + y;
        let op = NodeUtil::get_op_from_assignment_op(compiler, n);
        let detached_left = left.detach(compiler);
        let left_clone = left.clone_tree(compiler);
        let detached_right = right.detach(compiler);
        let op_node = compiler
            .new_node_with_children2(op, left_clone, detached_right)
            .srcref(compiler, n);
        let replacement = IR::assign(compiler, detached_left, op_node);
        n.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
        replacement
    }

    /// Try to fold a AND/OR node.
    // port: PeepholeFoldConstants#tryFoldAndOr
    #[allow(clippy::collapsible_if)]
    fn try_fold_and_or(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let parent = n.get_parent(compiler);

        let mut result = None;
        let mut dropped = None;

        let r#type = n.get_token(compiler);

        let left_val = NodeUtil::get_boolean_value(compiler, left);

        if left_val != Tri::UNKNOWN {
            let lval = left_val.to_boolean(true);

            // (TRUE || x) => TRUE (also, (3 || x) => 3)
            // (FALSE && x) => FALSE
            if if lval {
                r#type == Token::OR
            } else {
                r#type == Token::AND
            } {
                result = Some(left);
                dropped = Some(right);
            } else if !self.may_have_side_effects(compiler, left) {
                // (FALSE || x) => x
                // (TRUE && x) => x
                result = Some(right);
                dropped = Some(left);
            } else {
                // Left side may have side effects, but we know its boolean value.
                // e.g. true_with_sideeffects || foo() => true_with_sideeffects, foo()
                // or: false_with_sideeffects && foo() => false_with_sideeffects, foo()
                // This, combined with PeepholeRemoveDeadCode, helps reduce expressions
                // like "x() || false || z()".
                n.detach_children(compiler);
                result = Some(IR::comma(compiler, left, right));
                dropped = None;
            }
        } else if parent.unwrap().get_token(compiler) == r#type
            && Some(n) == parent.unwrap().get_first_child(compiler)
        {
            let right_value = NodeUtil::get_boolean_value(compiler, right);
            if !self.may_have_side_effects(compiler, right) {
                if (right_value == Tri::FALSE && r#type == Token::OR)
                    || (right_value == Tri::TRUE && r#type == Token::AND)
                {
                    result = Some(left);
                    dropped = Some(right);
                }
            }
        }

        // Note: Right hand side folding is handled by
        // PeepholeMinimizeConditions#tryMinimizeCondition

        if let Some(result) = result {
            // Fold it!
            n.detach_children(compiler);
            n.replace_with(compiler, result);
            self.report_change_to_enclosing_scope(compiler, result);
            if let Some(dropped) = dropped {
                self.mark_functions_deleted(compiler, dropped);
            }
            result
        } else {
            n
        }
    }

    /// Try to fold a COALESCE node.
    // port: PeepholeFoldConstants#tryFoldCoalesce
    fn try_fold_coalesce(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let mut result = None;

        let left_val = NodeUtil::get_known_value_type(compiler, left);

        match left_val {
            ValueType::NULL | ValueType::VOID => {
                // nullish condition => this expression evaluates to the right side.
                if !self.may_have_side_effects(compiler, left) {
                    result = Some(right);
                    self.mark_functions_deleted(compiler, left);
                } else {
                    // e.g. `(a(), null) ?? 1` => `(a(), null, 1)`
                    n.detach_children(compiler);
                    result = Some(IR::comma(compiler, left, right));
                }
            }
            ValueType::NUMBER
            | ValueType::BIGINT
            | ValueType::STRING
            | ValueType::BOOLEAN
            | ValueType::OBJECT => {
                // non-nullish condition => this expression evaluates to the left side.
                result = Some(left);
                self.mark_functions_deleted(compiler, right);
            }
            ValueType::UNDETERMINED => {}
        }

        if let Some(result) = result {
            // Fold!
            n.detach_children(compiler);
            n.replace_with(compiler, result);
            self.report_change_to_enclosing_scope(compiler, result);
            result
        } else {
            n
        }
    }

    /// Takes a subtree representing an expression of chained addition between expressions and
    /// folds adjacent string addition when possible and safe.
    ///
    /// `n`: root node of ADD expression to be optimized; `left`: left child of n being added;
    /// `right`: right child of n being added. Returns the AST subtree starting from n that has
    /// been optimized to collapse the rightmost left child leaf node which must be a string
    /// literal and the leftmost right child leaf node if possible to do so in a type safe way.
    // port: PeepholeFoldConstants#tryFoldAdjacentLiteralLeaves
    fn try_fold_adjacent_literal_leaves(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut left: NodeId,
        mut right: NodeId,
    ) -> NodeId {
        // Find left child's rightmost leaf
        let mut left_parent = n;
        while left.is_add(compiler) {
            // This had better be in a chain of '+' operations
            left_parent = left;
            left = left.get_second_child(compiler).unwrap();
        }

        // Find right child's leftmost leaf
        let mut right_parent = n;
        while right.is_add(compiler) {
            // This had better be in a chain of '+' operations
            right_parent = right;
            right = right.get_first_child(compiler).unwrap();
        }

        // Try to fold if of the form:
        // ... + <STRINGLITERAL> + <LITERAL>
        // ... + <STRINGLITERAL> + (<LITERAL> + ...)
        // aka. when the literal might be collapsible into the string
        if left_parent.is_add(compiler)
            && left.is_string_lit(compiler)
            && right_parent.is_add(compiler)
            && NodeUtil::is_literal_value(compiler, right, /* include_functions= */ false)
        {
            let right_grandparent = right_parent.get_parent(compiler);
            // `rr` is righthand side term that `right` is being added to and is null if right is
            // still the second child of `n` being added to `left` and non-null if it is in a
            // nested add expr
            let rr = right.get_next(compiler);
            let fold_is_type_safe =
                // If right.getNext() is already a string, folding won't disturb any typecasting
                rr.is_some_and(|rr| NodeUtil::is_string_result(compiler, rr))
                    || (rr.is_some()
                        // If right.getNext() isn't a string result, right must be a string to be
                        // folded
                        && right.is_string_lit(compiler)
                        // Access the right grandparent safely...
                        && right_grandparent.is_some_and(|right_grandparent| {
                            right_grandparent.is_add(compiler)
                                // The second child of `rightGrandparent` is what `right + rr` is
                                // being added to. If it exists, `right` can only be safely removed
                                // if `rr` is still treated as a string in the resulting addition.
                                && NodeUtil::is_string_result(
                                    compiler,
                                    right_grandparent.get_second_child(compiler).unwrap(),
                                )
                        }))
                    // Dangling literal term has no typecasting side effects; fold it!
                    || rr.is_none();
            if fold_is_type_safe {
                let result = left.get_string(compiler).concat(&java_string_value(
                    NodeUtil::get_string_value(compiler, right),
                ));
                // If the right parent is the root, shift the left parent up so as not to
                // overwrite the tree. Otherwise, shift the right parent up
                if right_parent.get_second_child(compiler) == Some(right) {
                    let string_node = IR::string(compiler, result);
                    left.replace_with(compiler, string_node);
                    let clone = right_parent
                        .get_first_child(compiler)
                        .unwrap()
                        .clone_tree_with_type_exprs(compiler, true);
                    self.replace(compiler, right_parent, clone);
                } else {
                    let string_node = IR::string(compiler, result);
                    left.replace_with(compiler, string_node);
                    let clone = right_parent
                        .get_second_child(compiler)
                        .unwrap()
                        .clone_tree_with_type_exprs(compiler, true);
                    self.replace(compiler, right_parent, clone);
                }
            }
        }
        n
    }

    /// Try to fold an ADD node with constant operands
    // port: PeepholeFoldConstants#tryFoldAddConstantString
    fn try_fold_add_constant_string(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        if left.is_string_lit(compiler)
            || right.is_string_lit(compiler)
            || left.is_array_lit(compiler)
            || right.is_array_lit(compiler)
        {
            // Add strings.
            let left_string = self.get_side_effect_free_string_value(compiler, left);
            let right_string = self.get_side_effect_free_string_value(compiler, right);
            if let (Some(left_string), Some(right_string)) = (left_string, right_string) {
                let new_string_node = IR::string(compiler, left_string.concat(&right_string));
                n.replace_with(compiler, new_string_node);
                self.report_change_to_enclosing_scope(compiler, new_string_node);
                return new_string_node;
            }
        }

        n
    }

    /// Try to fold arithmetic binary operators
    // port: PeepholeFoldConstants#tryFoldArithmeticOp
    fn try_fold_arithmetic_op(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let result = self.perform_arithmetic_op(compiler, n, left, right);
        if let Some(result) = result {
            result.srcref_tree_if_missing(compiler, n);
            self.report_change_to_enclosing_scope(compiler, n);
            n.replace_with(compiler, result);
            return result;
        }
        n
    }

    /// Try to fold arithmetic binary operators
    // port: PeepholeFoldConstants#performArithmeticOp
    #[allow(clippy::needless_late_init, clippy::collapsible_if)]
    fn perform_arithmetic_op(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> Option<NodeId> {
        // Unlike other operations, ADD operands are not always converted
        // to Number.
        if n.is_add(compiler) {
            let should_use_types = self.should_use_types;
            let (types, ast) = compiler.get_type_registry_field_and_ast();
            let types = types.as_deref();
            if NodeUtil::may_be_string_with_type(ast, left, should_use_types, types)
                || NodeUtil::may_be_string_with_type(ast, right, should_use_types, types)
            {
                return None;
            }
        }

        if self.is_big_int(compiler, left) && self.is_big_int(compiler, right) {
            return self.perform_big_int_arithmetic_op(compiler, n, left, right);
        }

        let result: f64;

        // TODO(johnlenz): Handle NaN with unknown value. BIT ops convert NaN
        // to zero so this is a little awkward here.

        let l_val_obj = self.get_side_effect_free_number_value(compiler, left);
        let r_val_obj = self.get_side_effect_free_number_value(compiler, right);
        // at least one of the two operands must have a value and both must be numeric
        if (l_val_obj.is_none() && r_val_obj.is_none())
            || !self.is_numeric(compiler, left)
            || !self.is_numeric(compiler, right)
        {
            return None;
        }
        // handle the operations that have algebraic identities, since we can simplify the tree
        // without actually knowing the value statically.
        match n.get_token(compiler) {
            Token::ADD => {
                if let (Some(l), Some(r)) = (l_val_obj, r_val_obj) {
                    return self.maybe_replace_binary_op_with_numeric_result(compiler, l + r, l, r);
                }
                if l_val_obj.is_some_and(|l| l == 0.0) {
                    return Some(right.clone_tree_with_type_exprs(compiler, true));
                } else if r_val_obj.is_some_and(|r| r == 0.0) {
                    return Some(left.clone_tree_with_type_exprs(compiler, true));
                }
                return None;
            }
            Token::SUB => {
                if let (Some(l), Some(r)) = (l_val_obj, r_val_obj) {
                    return self.maybe_replace_binary_op_with_numeric_result(compiler, l - r, l, r);
                }
                if l_val_obj.is_some_and(|l| l == 0.0) {
                    // 0 - x -> -x
                    // NOTE: this optimization has the subtle side effect of changing `0` to `-0`
                    // because `0-0 -> 0` but `-0 -> -0`.
                    let clone = right.clone_tree_with_type_exprs(compiler, true);
                    return Some(IR::neg(compiler, clone));
                } else if r_val_obj.is_some_and(|r| r == 0.0) {
                    // x - 0 -> x
                    return Some(left.clone_tree_with_type_exprs(compiler, true));
                }
                return None;
            }
            Token::MUL => {
                if let (Some(l), Some(r)) = (l_val_obj, r_val_obj) {
                    return self.maybe_replace_binary_op_with_numeric_result(compiler, l * r, l, r);
                }
                // NOTE: 0*x != 0 for all x, if x==0, then it is NaN.  So we can't take advantage
                // of that without some kind of non-NaN proof.  So the special cases here only
                // deal with 1*x
                if let Some(l) = l_val_obj {
                    if l == 1.0 {
                        return Some(right.clone_tree_with_type_exprs(compiler, true));
                    }
                } else if r_val_obj.unwrap() == 1.0 {
                    return Some(left.clone_tree_with_type_exprs(compiler, true));
                }
                return None;
            }
            Token::DIV => {
                if let (Some(l), Some(r)) = (l_val_obj, r_val_obj) {
                    if r == 0.0 {
                        return None;
                    }
                    return self.maybe_replace_binary_op_with_numeric_result(compiler, l / r, l, r);
                }
                // NOTE: 0/x != 0 for all x, if x==0, then it is NaN
                if let Some(r) = r_val_obj {
                    if r == 1.0 {
                        // x/1->x
                        return Some(left.clone_tree_with_type_exprs(compiler, true));
                    }
                }
                return None;
            }
            Token::EXPONENT => {
                if let (Some(l), Some(r)) = (l_val_obj, r_val_obj) {
                    return self.maybe_replace_binary_op_with_numeric_result(
                        compiler,
                        java_math::pow(l, r),
                        l,
                        r,
                    );
                }
                return None;
            }
            _ => {}
        }
        let (Some(lval), Some(rval)) = (l_val_obj, r_val_obj) else {
            return None;
        };
        match n.get_token(compiler) {
            Token::BITAND => {
                result = f64::from(
                    JSCompDoubles::ecmascript_to_int32(lval)
                        & JSCompDoubles::ecmascript_to_int32(rval),
                );
            }
            Token::BITOR => {
                result = f64::from(
                    JSCompDoubles::ecmascript_to_int32(lval)
                        | JSCompDoubles::ecmascript_to_int32(rval),
                );
            }
            Token::BITXOR => {
                result = f64::from(
                    JSCompDoubles::ecmascript_to_int32(lval)
                        ^ JSCompDoubles::ecmascript_to_int32(rval),
                );
            }
            Token::MOD => {
                if rval == 0.0 {
                    return None;
                }
                result = lval % rval;
            }
            _ => panic!("Unexpected arithmetic operator: {}", n.get_token(compiler)),
        }
        self.maybe_replace_binary_op_with_numeric_result(compiler, result, lval, rval)
    }

    // port: PeepholeFoldConstants#performBigIntArithmeticOp
    #[allow(clippy::collapsible_match)]
    fn perform_big_int_arithmetic_op(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> Option<NodeId> {
        let l_val = self.get_side_effect_free_big_int_value(compiler, left);
        let r_val = self.get_side_effect_free_big_int_value(compiler, right);
        if let (Some(l_val), Some(r_val)) = (&l_val, &r_val) {
            match n.get_token(compiler) {
                Token::ADD => return Some(Self::bigint_node(compiler, l_val + r_val, n)),
                Token::SUB => return Some(Self::bigint_node(compiler, l_val - r_val, n)),
                Token::MUL => return Some(Self::bigint_node(compiler, l_val * r_val, n)),
                Token::DIV => {
                    if *r_val != BigInt::ZERO {
                        return Some(Self::bigint_node(compiler, l_val / r_val, n));
                    } else {
                        return None;
                    }
                }
                Token::EXPONENT => {
                    let pow = big_integer::int_value_exact(r_val)
                        .and_then(|exponent| big_integer::pow(l_val, exponent));
                    match pow {
                        Ok(pow) => return Some(Self::bigint_node(compiler, pow, n)),
                        Err(_exception) => return None,
                    }
                }
                Token::MOD => {
                    // BigInteger#mod throws an ArithmeticException for a modulus that is not
                    // positive; Java does not catch it here.
                    let modulo = big_integer::r#mod(l_val, r_val)
                        .unwrap_or_else(|e| panic!("java.lang.ArithmeticException: {}", e.0));
                    return Some(Self::bigint_node(compiler, modulo, n));
                }
                Token::BITAND => return Some(Self::bigint_node(compiler, l_val & r_val, n)),
                Token::BITOR => return Some(Self::bigint_node(compiler, l_val | r_val, n)),
                Token::BITXOR => return Some(Self::bigint_node(compiler, l_val ^ r_val, n)),
                _ => return None,
            }
        }

        // handle the operations that have algebraic identities, since we can simplify the tree
        // without actually knowing the value statically.
        match n.get_token(compiler) {
            Token::ADD => {
                if l_val.as_ref().is_some_and(|l| *l == BigInt::ZERO) {
                    return Some(
                        right.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                } else if r_val.as_ref().is_some_and(|r| *r == BigInt::ZERO) {
                    return Some(
                        left.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                }
                None
            }
            Token::SUB => {
                if l_val.as_ref().is_some_and(|l| *l == BigInt::ZERO) {
                    // 0n - x -> -x
                    let clone = right
                        .clone_tree_with_type_exprs(compiler, /* clone_type_exprs= */ true);
                    return Some(IR::neg(compiler, clone));
                } else if r_val.as_ref().is_some_and(|r| *r == BigInt::ZERO) {
                    // x - 0n -> x
                    return Some(
                        left.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                }
                None
            }
            Token::MUL => {
                if l_val.as_ref().is_some_and(|l| *l == BigInt::from(1)) {
                    return Some(
                        right.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                } else if r_val.as_ref().is_some_and(|r| *r == BigInt::from(1)) {
                    return Some(
                        left.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                }
                None
            }
            Token::DIV => {
                if r_val.as_ref().is_some_and(|r| *r == BigInt::from(1)) {
                    return Some(
                        left.clone_tree_with_type_exprs(
                            compiler, /* clone_type_exprs= */ true,
                        ),
                    );
                }
                None
            }
            _ => None,
        }
    }

    // port: PeepholeFoldConstants#isNumeric
    fn is_numeric(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        if NodeUtil::is_numeric_result(compiler, n) {
            return true;
        }
        if self.should_use_types {
            return n
                .get_color(compiler)
                .is_some_and(|color| color == *StandardColors::NUMBER);
        }
        false
    }

    // port: PeepholeFoldConstants#isBigInt
    fn is_big_int(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        if NodeUtil::is_big_int_result(compiler, n) {
            return true;
        }
        if self.should_use_types {
            return n
                .get_color(compiler)
                .is_some_and(|color| color == *StandardColors::BIGINT);
        }
        false
    }

    // port: PeepholeFoldConstants#maybeReplaceBinaryOpWithNumericResult
    fn maybe_replace_binary_op_with_numeric_result(
        &self,
        compiler: &mut AbstractCompiler,
        result: f64,
        lval: f64,
        rval: f64,
    ) -> Option<NodeId> {
        // TODO(johnlenz): consider removing the result length check.
        // length of the left and right value plus 1 byte for the operator.
        if (java_lang::double_to_string(result).len()
            <= java_lang::double_to_string(lval).len() + java_lang::double_to_string(rval).len() + 1

            // Do not try to fold arithmetic for numbers > 2^53. After that
            // point, fixed-point math starts to break down and become inaccurate.
            && result.abs() <= MAX_FOLD_NUMBER)
            || result.is_nan()
            || result == f64::INFINITY
            || result == f64::NEG_INFINITY
        {
            return Some(NodeUtil::number_node(compiler, result, None));
        }
        None
    }

    /// Expressions such as [foo() && 10 && 20] generate parse trees where no node has two const
    /// children ((foo() && 10) && 20), so performArithmeticOp() won't fold it:
    /// tryFoldLeftChildOp() will.
    ///
    /// Specifically, this folds associative expressions where:
    ///
    /// - The left child is also an associative expression of the same type.
    /// - The right child is a BIGINT or NUMBER constant.
    /// - The left child's right child is a BIGINT or NUMBER constant.
    // port: PeepholeFoldConstants#tryFoldLeftChildOp
    fn try_fold_left_child_op(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let op_type = n.get_token(compiler);
        check_state!(NodeUtil::is_associative(op_type) && NodeUtil::is_commutative(op_type));

        // Use getNumberValue to handle constants like "NaN" and "Infinity"
        // other values are converted to numbers elsewhere.
        let right_val_obj = self.get_side_effect_free_number_value(compiler, right);
        let right_big_int = self.get_side_effect_free_big_int_value(compiler, right);
        if (right_val_obj.is_some() || right_big_int.is_some())
            && left.get_token(compiler) == op_type
        {
            check_state!(left.has_two_children(compiler));

            let ll = left.get_first_child(compiler).unwrap();
            let lr = ll.get_next(compiler).unwrap();

            let mut value_to_combine = ll;
            let mut replacement = self.perform_arithmetic_op(compiler, n, value_to_combine, right);
            if replacement.is_none() {
                value_to_combine = lr;
                replacement = self.perform_arithmetic_op(compiler, n, value_to_combine, right);
            }
            if let Some(replacement) = replacement {
                // Remove the child that has been combined
                value_to_combine.detach(compiler);
                // Replace the left op with the remaining child.
                let remaining = left.remove_first_child(compiler).unwrap();
                left.replace_with(compiler, remaining);
                // New "-Infinity" node need location info explicitly
                // added.
                replacement.srcref_tree_if_missing(compiler, right);
                right.replace_with(compiler, replacement);
                self.report_change_to_enclosing_scope(compiler, n);
            }
        }

        n
    }

    /// Folds additions of template literals with placeholders/substitutions like `${foo} and
    /// ${bar}` + `${baz}`.
    ///
    /// This code path is only hit if the left and right are both template literals WITH
    /// placeholders/substitutions. If one of them did not have placeholders/substitutions, it
    /// would have already have been optimized to a string.
    // port: PeepholeFoldConstants#foldAddTemplateLiterals
    fn fold_add_template_literals(
        &self,
        compiler: &mut AbstractCompiler,
        old_node: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        // All template literal nodes with placeholders/substitutions will have their first child
        // and last child as a string node.
        // Merge the last left child string node with the first right child node.
        let last_left = left.get_last_child(compiler).unwrap();
        let first_right = right.get_first_child(compiler).unwrap();
        if self.ends_with_unescaped_dollar(&last_left.get_raw_string(compiler))
            && first_right
                .get_raw_string(compiler)
                .starts_with(&JsString::from("{"))
        {
            return old_node;
        }
        let cooked = java_string_value(last_left.get_cooked_string(compiler))
            .concat(&java_string_value(first_right.get_cooked_string(compiler)));
        let raw = last_left
            .get_raw_string(compiler)
            .concat(&first_right.get_raw_string(compiler));
        let merge_node = IR::template_literal_string(compiler, Some(cooked), raw);
        merge_node.srcref_tree_if_missing(compiler, last_left);

        // Detach the lastLeft and firstRight children and append all right children to the left
        // node.
        last_left.detach(compiler);
        first_right.detach(compiler);
        left.add_child_to_back(compiler, merge_node);
        let right_children = right.remove_children(compiler);
        left.add_children_to_back(compiler, right_children);

        // Detach the updated left node and replace the old node with it.
        left.detach(compiler);
        self.replace(compiler, old_node, left)
    }

    /// Simplify escaping and value calculations by avoiding problematic characters when merging
    /// strings into template literals.
    // port: PeepholeFoldConstants#isSimpleTemplateStringValue
    fn is_simple_template_string_value(&self, s: &JsString) -> bool {
        for i in 0..s.length() {
            match s.char_at(i) {
                // See: https://tc39.es/ecma262/#prod-LineTerminator
                0x60 /* '`' */ | 0x24 /* '$' */ | 0x7B /* '{' */ | 0x5C /* '\\' */
                | 0x0A /* '\n' */ | 0x0D /* '\r' */ | 0x2028 | 0x2029 => {
                    return false;
                }
                _ => {}
            }
        }
        true
    }

    /// returns true if the raw string ends with an unescaped backslash
    // port: PeepholeFoldConstants#endsWithUnescapedBackslash
    fn ends_with_unescaped_backslash(&self, raw: &JsString) -> bool {
        let mut backslash_count = 0;
        let mut i = raw.length() as i32 - 1;
        while i >= 0 {
            if raw.char_at(i as usize) == u16::from(b'\\') {
                backslash_count += 1;
            } else {
                break;
            }
            i -= 1;
        }
        backslash_count % 2 != 0
    }

    /// returns true if the raw string ends with an unescaped dollar sign
    // port: PeepholeFoldConstants#endsWithUnescapedDollar
    fn ends_with_unescaped_dollar(&self, raw: &JsString) -> bool {
        if !raw.ends_with(&JsString::from("$")) {
            return false;
        }
        let mut backslash_count = 0;
        let mut i = raw.length() as i32 - 2;
        while i >= 0 {
            if raw.char_at(i as usize) == u16::from(b'\\') {
                backslash_count += 1;
            } else {
                break;
            }
            i -= 1;
        }
        backslash_count % 2 == 0
    }

    // port: PeepholeFoldConstants#tryFoldTemplateLiteralSubstitutions
    #[allow(clippy::collapsible_if)]
    fn try_fold_template_literal_substitutions(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        if n.get_parent(compiler)
            .is_some_and(|parent| parent.is_tagged_template_lit(compiler))
        {
            return n;
        }

        let mut changed = false;
        let mut cur = n.get_first_child(compiler);
        if cur.is_none() {
            return n;
        }

        while let Some(cur_node) = cur {
            let Some(next) = cur_node.get_next(compiler) else {
                break;
            };

            if cur_node.is_template_lit_string(compiler) && next.is_template_lit_sub(compiler) {
                let expr = next.get_first_child(compiler);
                if let Some(expr) = expr
                    && (expr.is_string_lit(compiler)
                        || expr.is_number(compiler)
                        || expr.is_true(compiler)
                        || expr.is_false(compiler))
                {
                    let str_value = self.get_side_effect_free_string_value(compiler, expr);
                    if let Some(str_value) = str_value
                        && self.is_simple_template_string_value(&str_value)
                    {
                        let next_next = next.get_next(compiler);
                        if let Some(next_next) = next_next
                            && next_next.is_template_lit_string(compiler)
                        {
                            let cur_cooked =
                                java_string_value(cur_node.get_cooked_string(compiler));
                            let next_next_cooked =
                                java_string_value(next_next.get_cooked_string(compiler));

                            // Avoid creating a new `${` sequence when folding.
                            if str_value.is_empty()
                                && self
                                    .ends_with_unescaped_dollar(&cur_node.get_raw_string(compiler))
                                && next_next
                                    .get_raw_string(compiler)
                                    .starts_with(&JsString::from("{"))
                            {
                                cur = Some(next);
                                continue;
                            }

                            let combined_cooked =
                                cur_cooked.concat(&str_value).concat(&next_next_cooked);
                            let combined_raw = cur_node
                                .get_raw_string(compiler)
                                .concat(&str_value)
                                .concat(&next_next.get_raw_string(compiler));

                            // Avoid creating a restricted octal escape sequence (e.g. \07)
                            if self.is_restricted_octal_escape(
                                &cur_node.get_raw_string(compiler),
                                &str_value,
                            ) || self.is_restricted_octal_escape(
                                &cur_node.get_raw_string(compiler).concat(&str_value),
                                &next_next.get_raw_string(compiler),
                            ) {
                                cur = Some(next);
                                continue;
                            }

                            let new_string_node = IR::template_literal_string(
                                compiler,
                                Some(combined_cooked),
                                combined_raw,
                            );
                            new_string_node.srcref_tree(compiler, cur_node);

                            cur_node.replace_with(compiler, new_string_node);
                            next.detach(compiler);
                            next_next.detach(compiler);

                            cur = Some(new_string_node);
                            changed = true;
                            continue;
                        }
                    }
                }
            }
            cur = cur_node.get_next(compiler);
        }

        if changed {
            self.report_change_to_enclosing_scope(compiler, n);
        }
        n
    }

    // port: PeepholeFoldConstants#isRestrictedOctalEscape
    fn is_restricted_octal_escape(&self, prefix_raw: &JsString, suffix_raw: &JsString) -> bool {
        if suffix_raw.is_empty() {
            return false;
        }
        let first_char = suffix_raw.char_at(0);
        if first_char < u16::from(b'0') || first_char > u16::from(b'9') {
            return false;
        }

        // Check if prefix ends with an unescaped \0
        if prefix_raw.ends_with(&JsString::from("0")) {
            let before_zero = prefix_raw.substring(0, prefix_raw.length() - 1);
            return self.ends_with_unescaped_backslash(&before_zero);
        }
        false
    }

    // port: PeepholeFoldConstants#tryFoldAdd
    fn try_fold_add(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        check_argument!(node.is_add(compiler));

        let should_use_types = self.should_use_types;
        let may_be_string = {
            let (types, ast) = compiler.get_type_registry_field_and_ast();
            NodeUtil::may_be_string_with_type(ast, node, should_use_types, types.as_deref())
        };
        if may_be_string {
            if left.is_template_lit(compiler) && right.is_template_lit(compiler) {
                return self.fold_add_template_literals(compiler, node, left, right);
            } else if left.is_template_lit(compiler) {
                let right_str = self.get_side_effect_free_string_value(compiler, right);
                if let Some(right_str) = right_str
                    && self.is_simple_template_string_value(&right_str)
                {
                    return self
                        .fold_template_literal_and_constant(compiler, node, left, &right_str);
                }
            } else if right.is_template_lit(compiler) {
                let left_str = self.get_side_effect_free_string_value(compiler, left);
                if let Some(left_str) = left_str
                    && self.is_simple_template_string_value(&left_str)
                {
                    return self
                        .fold_constant_and_template_literal(compiler, node, &left_str, right);
                }
            }

            if NodeUtil::is_literal_value(compiler, left, false)
                && NodeUtil::is_literal_value(compiler, right, false)
            {
                // '6' + 7
                self.try_fold_add_constant_string(compiler, node, left, right)
            } else if left.is_string_lit(compiler)
                && left.get_string(compiler).is_empty()
                && self.is_string_typed(compiler, right)
            {
                let clone = right.clone_tree_with_type_exprs(compiler, true);
                self.replace(compiler, node, clone)
            } else if right.is_string_lit(compiler)
                && right.get_string(compiler).is_empty()
                && self.is_string_typed(compiler, left)
            {
                let clone = left.clone_tree_with_type_exprs(compiler, true);
                self.replace(compiler, node, clone)
            } else {
                // a + 7 or 6 + a
                self.try_fold_adjacent_literal_leaves(compiler, node, left, right)
            }
        } else {
            // Try arithmetic add
            self.try_fold_arithmetic_op(compiler, node, left, right)
        }
    }

    // port: PeepholeFoldConstants#foldTemplateLiteralAndConstant
    fn fold_template_literal_and_constant(
        &self,
        compiler: &mut AbstractCompiler,
        old_node: NodeId,
        left: NodeId,
        right_str: &JsString,
    ) -> NodeId {
        let last_left = left.get_last_child(compiler).unwrap();
        check_state!(last_left.is_template_lit_string(compiler));

        if self.is_restricted_octal_escape(&last_left.get_raw_string(compiler), right_str) {
            return old_node;
        }

        let cooked = java_string_value(last_left.get_cooked_string(compiler)).concat(right_str);
        let raw = last_left.get_raw_string(compiler).concat(right_str);
        let merge_node = IR::template_literal_string(compiler, Some(cooked), raw);
        merge_node.srcref_tree_if_missing(compiler, last_left);

        last_left.replace_with(compiler, merge_node);
        left.detach(compiler);
        self.replace(compiler, old_node, left)
    }

    // port: PeepholeFoldConstants#foldConstantAndTemplateLiteral
    fn fold_constant_and_template_literal(
        &self,
        compiler: &mut AbstractCompiler,
        old_node: NodeId,
        left_str: &JsString,
        right: NodeId,
    ) -> NodeId {
        let first_right = right.get_first_child(compiler).unwrap();
        check_state!(first_right.is_template_lit_string(compiler));

        if self.is_restricted_octal_escape(left_str, &first_right.get_raw_string(compiler)) {
            return old_node;
        }

        let cooked = left_str.concat(&java_string_value(first_right.get_cooked_string(compiler)));
        let raw = left_str.concat(&first_right.get_raw_string(compiler));
        let merge_node = IR::template_literal_string(compiler, Some(cooked), raw);
        merge_node.srcref_tree_if_missing(compiler, first_right);

        first_right.replace_with(compiler, merge_node);
        right.detach(compiler);
        self.replace(compiler, old_node, right)
    }

    // port: PeepholeFoldConstants#replace
    fn replace(
        &self,
        compiler: &mut AbstractCompiler,
        old_node: NodeId,
        new_node: NodeId,
    ) -> NodeId {
        old_node.replace_with(compiler, new_node);
        self.report_change_to_enclosing_scope(compiler, new_node);
        new_node
    }

    // port: PeepholeFoldConstants#isStringTyped
    fn is_string_typed(&self, compiler: &AbstractCompiler, n: NodeId) -> bool {
        // We could also accept !String, but it is unlikely to be very common.
        if NodeUtil::is_string_result(compiler, n) {
            return true;
        }
        if self.should_use_types {
            return n
                .get_color(compiler)
                .is_some_and(|color| color == *StandardColors::STRING);
        }
        false
    }

    /// Try to fold shift operations
    // port: PeepholeFoldConstants#tryFoldShift
    fn try_fold_shift(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let left_val = self.get_side_effect_free_number_value(compiler, left);
        let right_val = self.get_side_effect_free_number_value(compiler, right);

        if !self.is_reasonable_double_value(left_val) || !self.is_reasonable_double_value(right_val)
        {
            return n;
        }
        let (left_val, right_val) = (left_val.unwrap(), right_val.unwrap());
        if !JSCompDoubles::is_mathematical_integer(left_val) {
            self.report(compiler, &FRACTIONAL_BITWISE_OPERAND, left);
            return n;
        }
        if !JSCompDoubles::is_mathematical_integer(right_val) {
            self.report(compiler, &FRACTIONAL_BITWISE_OPERAND, right);
            return n;
        }

        // only the lower 5 bits are used when shifting, so don't do anything
        // if the shift amount is outside [0,32)
        #[allow(clippy::manual_range_contains)]
        if !(0.0 <= right_val && right_val < 32.0) {
            return n;
        }

        let rval_int = right_val as i32;
        let bits = JSCompDoubles::ecmascript_to_int32(left_val);

        let result: f64 = match n.get_token(compiler) {
            Token::LSH => f64::from(bits.wrapping_shl(rval_int as u32)),
            Token::RSH => f64::from(bits.wrapping_shr(rval_int as u32)),
            Token::URSH => {
                // JavaScript always treats the result of >>> as unsigned.
                // We must force Java to do the same here.
                (0xffffffff_i64 & i64::from(((bits as u32) >> rval_int) as i32)) as f64
            }
            _ => panic!("Unknown shift operator: {}", n.get_token(compiler)),
        };

        let new_number = NodeUtil::number_node(compiler, result, Some(n));
        self.report_change_to_enclosing_scope(compiler, n);
        n.replace_with(compiler, new_number);

        new_number
    }

    /// Try to fold comparison nodes, e.g ==
    // port: PeepholeFoldConstants#tryFoldComparison
    fn try_fold_comparison(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        let op = n.get_token(compiler);
        let result = Self::evaluate_comparison(self, compiler, op, left, right);
        if result == Tri::UNKNOWN {
            return n;
        }

        let new_node = NodeUtil::boolean_node(compiler, result.to_boolean(true));
        self.report_change_to_enclosing_scope(compiler, n);
        n.replace_with(compiler, new_node);
        self.mark_functions_deleted(compiler, n);

        new_node
    }

    // port: PeepholeFoldConstants#tryAbstractRelationalComparison
    fn try_abstract_relational_comparison(
        peephole_optimization: &dyn AbstractPeepholeOptimization,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
        will_negate: bool,
    ) -> Tri {
        let left_value_type = NodeUtil::get_known_value_type(compiler, left);
        let right_value_type = NodeUtil::get_known_value_type(compiler, right);
        // First, check for a string comparison.
        if left_value_type == ValueType::STRING && right_value_type == ValueType::STRING {
            let lv_str = peephole_optimization.get_side_effect_free_string_value(compiler, left);
            let rv_str = peephole_optimization.get_side_effect_free_string_value(compiler, right);
            if let (Some(lv_str), Some(rv_str)) = (&lv_str, &rv_str) {
                // In JS, browsers parse \v differently. So do not compare strings if one contains
                // \v.
                if lv_str.index_of_char(0x000B) != -1 || rv_str.index_of_char(0x000B) != -1 {
                    return Tri::UNKNOWN;
                } else {
                    return Tri::for_boolean(lv_str.compare_to(rv_str) < 0);
                }
            } else if left.is_type_of(compiler)
                && right.is_type_of(compiler)
                && left.get_first_child(compiler).unwrap().is_name(compiler)
                && right.get_first_child(compiler).unwrap().is_name(compiler)
                && left.get_first_child(compiler).unwrap().get_string(compiler)
                    == right
                        .get_first_child(compiler)
                        .unwrap()
                        .get_string(compiler)
            {
                // Special case: `typeof a < typeof a` is always false.
                return Tri::FALSE;
            }
        }

        // Next, handle comparisons between BigInt and String.
        // Per ECMA-262, comparing a BigInt with a String converts the String to a BigInt via
        // StringToBigInt(string). If StringToBigInt returns undefined (invalid BigInt syntax like
        // "foo", "1e2", "1.5"), all relational comparisons (<, >, <=, >=) return false.
        if left_value_type == ValueType::BIGINT && right_value_type == ValueType::STRING {
            let rv_str = peephole_optimization.get_side_effect_free_string_value(compiler, right);
            if let Some(rv_str) = rv_str {
                let rv_big = NodeUtil::get_string_big_int_value(&rv_str);
                let Some(rv_big) = rv_big else {
                    return Tri::for_boolean(will_negate);
                };
                let lv_big =
                    peephole_optimization.get_side_effect_free_big_int_value(compiler, left);
                if let Some(lv_big) = lv_big {
                    return Tri::for_boolean(lv_big < rv_big);
                }
            }
            return Tri::UNKNOWN;
        }
        if left_value_type == ValueType::STRING && right_value_type == ValueType::BIGINT {
            let lv_str = peephole_optimization.get_side_effect_free_string_value(compiler, left);
            if let Some(lv_str) = lv_str {
                let lv_big = NodeUtil::get_string_big_int_value(&lv_str);
                let Some(lv_big) = lv_big else {
                    return Tri::for_boolean(will_negate);
                };
                let rv_big =
                    peephole_optimization.get_side_effect_free_big_int_value(compiler, right);
                if let Some(rv_big) = rv_big {
                    return Tri::for_boolean(lv_big < rv_big);
                }
            }
            return Tri::UNKNOWN;
        }

        // Next, try to evaluate based on the value of the node. Try comparing as BigInts first.
        let lv_big = peephole_optimization.get_side_effect_free_big_int_value(compiler, left);
        let rv_big = peephole_optimization.get_side_effect_free_big_int_value(compiler, right);
        if let (Some(lv_big), Some(rv_big)) = (&lv_big, &rv_big) {
            return Tri::for_boolean(lv_big < rv_big);
        }

        // Then, try comparing as Numbers.
        let lv_num = peephole_optimization.get_side_effect_free_number_value(compiler, left);
        let rv_num = peephole_optimization.get_side_effect_free_number_value(compiler, right);
        if let (Some(lv_num), Some(rv_num)) = (lv_num, rv_num) {
            if lv_num.is_nan() || rv_num.is_nan() {
                return Tri::for_boolean(will_negate);
            } else {
                return Tri::for_boolean(lv_num < rv_num);
            }
        }

        // Finally, try comparisons between BigInt and Number.
        if let (Some(lv_big), Some(rv_num)) = (&lv_big, rv_num) {
            return Self::bigint_less_than_double(lv_big, rv_num, Tri::FALSE, will_negate);
        }
        if let (Some(lv_num), Some(rv_big)) = (lv_num, &rv_big) {
            return Self::bigint_less_than_double(rv_big, lv_num, Tri::TRUE, will_negate);
        }

        // Special case: `x < x` is always false.
        // TODO(moz): If we knew the named value wouldn't be NaN, it would be nice to handle
        // LE and GE. We should use type information if available here.
        if !will_negate
            && left.is_name(compiler)
            && right.is_name(compiler)
            && left.get_string(compiler) == right.get_string(compiler)
        {
            return Tri::FALSE;
        }

        Tri::UNKNOWN
    }

    // port: PeepholeFoldConstants#bigintLessThanDouble
    fn bigint_less_than_double(
        bigint: &BigInt,
        number: f64,
        invert: Tri,
        will_negate: bool,
    ) -> Tri {
        // if invert is false, then the number is on the right in tryAbstractRelationalComparison
        // if it's true, then the number is on the left
        if number.is_nan() {
            return Tri::for_boolean(will_negate);
        } else if number == f64::INFINITY {
            return Tri::TRUE.xor(invert);
        } else if number == f64::NEG_INFINITY {
            return Tri::FALSE.xor(invert);
        } else if !JSCompDoubles::is_at_least_integer_precision(number) {
            return Tri::UNKNOWN;
        }

        // long can hold all values within [-2^53, 2^53]
        let number_as_big_int = BigInt::from(number as i64);
        let negative_means_bigint_smaller = bigint.cmp(&number_as_big_int);
        if negative_means_bigint_smaller.is_lt() {
            Tri::TRUE.xor(invert)
        } else if negative_means_bigint_smaller.is_gt() {
            Tri::FALSE.xor(invert)
        } else if JSCompDoubles::is_exact_int64(number) {
            Tri::FALSE // This is the == case, don't invert.
        } else {
            Tri::for_boolean(JSCompDoubles::is_positive(number)).xor(invert)
        }
    }

    /// http://www.ecma-international.org/ecma-262/6.0/#sec-abstract-equality-comparison
    // port: PeepholeFoldConstants#tryAbstractEqualityComparison
    fn try_abstract_equality_comparison(
        peephole_optimization: &dyn AbstractPeepholeOptimization,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
    ) -> Tri {
        // Evaluate based on the general type.
        let left_value_type = NodeUtil::get_known_value_type(compiler, left);
        let right_value_type = NodeUtil::get_known_value_type(compiler, right);
        if left_value_type != ValueType::UNDETERMINED && right_value_type != ValueType::UNDETERMINED
        {
            // Delegate to strict equality comparison for values of the same type.
            if left_value_type == right_value_type {
                return Self::try_strict_equality_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                );
            }

            if (left_value_type == ValueType::NULL && right_value_type == ValueType::VOID)
                || (left_value_type == ValueType::VOID && right_value_type == ValueType::NULL)
            {
                return Tri::TRUE;
            }

            if (left_value_type == ValueType::NUMBER && right_value_type == ValueType::STRING)
                || right_value_type == ValueType::BOOLEAN
            {
                let rv = peephole_optimization.get_side_effect_free_number_value(compiler, right);
                return match rv {
                    None => Tri::UNKNOWN,
                    Some(rv) => {
                        let rv_node = NodeUtil::number_node(compiler, rv, Some(right));
                        Self::try_abstract_equality_comparison(
                            peephole_optimization,
                            compiler,
                            left,
                            rv_node,
                        )
                    }
                };
            }
            if (left_value_type == ValueType::STRING && right_value_type == ValueType::NUMBER)
                || left_value_type == ValueType::BOOLEAN
            {
                let lv = peephole_optimization.get_side_effect_free_number_value(compiler, left);
                return match lv {
                    None => Tri::UNKNOWN,
                    Some(lv) => {
                        let lv_node = NodeUtil::number_node(compiler, lv, Some(left));
                        Self::try_abstract_equality_comparison(
                            peephole_optimization,
                            compiler,
                            lv_node,
                            right,
                        )
                    }
                };
            }

            if left_value_type == ValueType::BIGINT && right_value_type == ValueType::STRING {
                let rv_str =
                    peephole_optimization.get_side_effect_free_string_value(compiler, right);
                if let Some(rv_str) = rv_str {
                    let rv_big = NodeUtil::get_string_big_int_value(&rv_str);
                    let Some(rv_big) = rv_big else {
                        return Tri::FALSE;
                    };
                    let lv_big =
                        peephole_optimization.get_side_effect_free_big_int_value(compiler, left);
                    return match lv_big {
                        Some(lv_big) => Tri::for_boolean(lv_big == rv_big),
                        None => Tri::UNKNOWN,
                    };
                }
                return Tri::UNKNOWN;
            }
            if left_value_type == ValueType::STRING && right_value_type == ValueType::BIGINT {
                let lv_str =
                    peephole_optimization.get_side_effect_free_string_value(compiler, left);
                if let Some(lv_str) = lv_str {
                    let lv_big = NodeUtil::get_string_big_int_value(&lv_str);
                    let Some(lv_big) = lv_big else {
                        return Tri::FALSE;
                    };
                    let rv_big =
                        peephole_optimization.get_side_effect_free_big_int_value(compiler, right);
                    return match rv_big {
                        Some(rv_big) => Tri::for_boolean(lv_big == rv_big),
                        None => Tri::UNKNOWN,
                    };
                }
                return Tri::UNKNOWN;
            }

            if left_value_type == ValueType::BIGINT || right_value_type == ValueType::BIGINT {
                let lv = peephole_optimization.get_side_effect_free_big_int_value(compiler, left);
                let rv = peephole_optimization.get_side_effect_free_big_int_value(compiler, right);
                if let (Some(lv), Some(rv)) = (&lv, &rv) {
                    return Tri::for_boolean(lv == rv);
                }
                if left_value_type == ValueType::NULL
                    || left_value_type == ValueType::VOID
                    || right_value_type == ValueType::NULL
                    || right_value_type == ValueType::VOID
                {
                    return Tri::FALSE;
                }
                if left_value_type == ValueType::STRING
                    && peephole_optimization
                        .get_side_effect_free_string_value(compiler, left)
                        .is_some()
                    && lv.is_none()
                {
                    return Tri::FALSE;
                }
                if right_value_type == ValueType::STRING
                    && peephole_optimization
                        .get_side_effect_free_string_value(compiler, right)
                        .is_some()
                    && rv.is_none()
                {
                    return Tri::FALSE;
                }
                return Tri::UNKNOWN;
            }

            if (left_value_type == ValueType::STRING || left_value_type == ValueType::NUMBER)
                && right_value_type == ValueType::OBJECT
            {
                return Tri::UNKNOWN;
            }
            if left_value_type == ValueType::OBJECT
                && (right_value_type == ValueType::STRING || right_value_type == ValueType::NUMBER)
            {
                return Tri::UNKNOWN;
            }

            return Tri::FALSE;
        }
        // In general, the rest of the cases cannot be folded.
        Tri::UNKNOWN
    }

    /// http://www.ecma-international.org/ecma-262/6.0/#sec-strict-equality-comparison
    // port: PeepholeFoldConstants#tryStrictEqualityComparison
    fn try_strict_equality_comparison(
        peephole_optimization: &dyn AbstractPeepholeOptimization,
        compiler: &mut AbstractCompiler,
        left: NodeId,
        right: NodeId,
    ) -> Tri {
        // First, try to evaluate based on the general type.
        let left_value_type = NodeUtil::get_known_value_type(compiler, left);
        let right_value_type = NodeUtil::get_known_value_type(compiler, right);
        if left_value_type != ValueType::UNDETERMINED && right_value_type != ValueType::UNDETERMINED
        {
            // Strict equality can only be true for values of the same type.
            if left_value_type != right_value_type {
                return Tri::FALSE;
            }
            match left_value_type {
                ValueType::VOID | ValueType::NULL => {
                    return Tri::TRUE;
                }
                ValueType::NUMBER => {
                    if NodeUtil::is_nan(compiler, left) {
                        return Tri::FALSE;
                    }
                    if NodeUtil::is_nan(compiler, right) {
                        return Tri::FALSE;
                    }
                    let lv =
                        peephole_optimization.get_side_effect_free_number_value(compiler, left);
                    let rv =
                        peephole_optimization.get_side_effect_free_number_value(compiler, right);
                    if let (Some(lv), Some(rv)) = (lv, rv) {
                        return Tri::for_boolean(lv == rv);
                    }
                }
                ValueType::STRING => {
                    let lv =
                        peephole_optimization.get_side_effect_free_string_value(compiler, left);
                    let rv =
                        peephole_optimization.get_side_effect_free_string_value(compiler, right);
                    if let (Some(lv), Some(rv)) = (&lv, &rv) {
                        // In JS, browsers parse \v differently. So do not consider strings
                        // equal if one contains \v.
                        if lv.index_of_char(0x000B) != -1 || rv.index_of_char(0x000B) != -1 {
                            return Tri::UNKNOWN;
                        } else {
                            return if lv == rv { Tri::TRUE } else { Tri::FALSE };
                        }
                    } else if left.is_type_of(compiler)
                        && right.is_type_of(compiler)
                        && left.get_first_child(compiler).unwrap().is_name(compiler)
                        && right.get_first_child(compiler).unwrap().is_name(compiler)
                        && left.get_first_child(compiler).unwrap().get_string(compiler)
                            == right
                                .get_first_child(compiler)
                                .unwrap()
                                .get_string(compiler)
                    {
                        // Special case, typeof a == typeof a is always true.
                        return Tri::TRUE;
                    }
                }
                ValueType::BOOLEAN => {
                    let lv =
                        peephole_optimization.get_side_effect_free_boolean_value(compiler, left);
                    let rv =
                        peephole_optimization.get_side_effect_free_boolean_value(compiler, right);
                    return lv.and(rv).or(lv.not().and(rv.not()));
                }
                ValueType::BIGINT => {
                    let lv =
                        peephole_optimization.get_side_effect_free_big_int_value(compiler, left);
                    let rv =
                        peephole_optimization.get_side_effect_free_big_int_value(compiler, right);
                    if let (Some(lv), Some(rv)) = (lv, rv) {
                        return Tri::for_boolean(lv == rv);
                    }
                }
                _ => {
                    // Symbol and Object cannot be folded in the general case.
                    return Tri::UNKNOWN;
                }
            }
        }

        // Then, try to evaluate based on the value of the node. There's only one special case:
        // Any strict equality comparison against NaN returns false.
        if NodeUtil::is_nan(compiler, left) || NodeUtil::is_nan(compiler, right) {
            return Tri::FALSE;
        }
        Tri::UNKNOWN
    }

    // port: PeepholeFoldConstants#evaluateComparison
    pub fn evaluate_comparison(
        peephole_optimization: &dyn AbstractPeepholeOptimization,
        compiler: &mut AbstractCompiler,
        op: Token,
        left: NodeId,
        right: NodeId,
    ) -> Tri {
        // Don't try to minimize side-effects here.
        if peephole_optimization.may_have_side_effects(compiler, left)
            || peephole_optimization.may_have_side_effects(compiler, right)
        {
            return Tri::UNKNOWN;
        }

        match op {
            Token::EQ => {
                return Self::try_abstract_equality_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                );
            }
            Token::NE => {
                return Self::try_abstract_equality_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                )
                .not();
            }
            Token::SHEQ => {
                return Self::try_strict_equality_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                );
            }
            Token::SHNE => {
                return Self::try_strict_equality_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                )
                .not();
            }
            Token::LT => {
                return Self::try_abstract_relational_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                    false,
                );
            }
            Token::GT => {
                return Self::try_abstract_relational_comparison(
                    peephole_optimization,
                    compiler,
                    right,
                    left,
                    false,
                );
            }
            Token::LE => {
                return Self::try_abstract_relational_comparison(
                    peephole_optimization,
                    compiler,
                    right,
                    left,
                    true,
                )
                .not();
            }
            Token::GE => {
                return Self::try_abstract_relational_comparison(
                    peephole_optimization,
                    compiler,
                    left,
                    right,
                    true,
                )
                .not();
            }
            _ => {}
        }
        panic!("Unexpected operator for comparison");
    }

    // port: PeepholeFoldConstants#tryFoldCtorCall
    fn try_fold_ctor_call(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_new(compiler));

        // we can remove this for GETELEM calls (anywhere else?)
        if Self::in_forced_string_context(compiler, n) {
            return self.try_fold_in_forced_string_context(compiler, n);
        }
        n
    }

    /// Remove useless calls: Object.defineProperties(o, {}) -> o
    // port: PeepholeFoldConstants#tryFoldUselessObjectDotDefinePropertiesCall
    fn try_fold_useless_object_dot_define_properties_call(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler) || n.is_opt_chain_call(compiler));

        if NodeUtil::is_object_define_properties_definition(compiler, n) {
            let src_obj = n.get_last_child(compiler).unwrap();
            if src_obj.is_object_lit(compiler) && !src_obj.has_children(compiler) {
                let parent = n.get_parent(compiler).unwrap();
                let dest_obj = n.get_second_child(compiler).unwrap().detach(compiler);
                n.replace_with(compiler, dest_obj);
                self.report_change_to_enclosing_scope(compiler, parent);
            }
        }
        n
    }

    /// Returns whether this node must be coerced to a string.
    // port: PeepholeFoldConstants#inForcedStringContext
    fn in_forced_string_context(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        if parent.is_get_elem(ast) && parent.get_last_child(ast) == Some(n) {
            return true;
        }

        // we can fold in the case "" + new String("")
        parent.is_add(ast)
    }

    // port: PeepholeFoldConstants#tryFoldInForcedStringContext
    fn try_fold_in_forced_string_context(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        // For now, we only know how to fold ctors.
        check_argument!(n.is_new(compiler));

        let object_type = n.get_first_child(compiler).unwrap();
        if !object_type.is_name(compiler) {
            return n;
        }

        if object_type.get_string(compiler) == "String" {
            let value = object_type.get_next(compiler);
            let string_value = match value {
                None => Some(JsString::from("")),
                Some(value) => self.get_side_effect_free_string_value(compiler, value),
            };

            let Some(string_value) = string_value else {
                return n;
            };

            let parent = n.get_parent(compiler).unwrap();
            let new_string = IR::string(compiler, string_value);

            n.replace_with(compiler, new_string);
            new_string.srcref_if_missing(compiler, parent);
            self.report_change_to_enclosing_scope(compiler, parent);

            return new_string;
        }
        n
    }

    /// For element access using GETLEM/OPTCHAIN_GETELEM on object literals, arrays or strings,
    /// tries to fold the prop access. e.g. folds array-element [1, 2, 3][1];
    // port: PeepholeFoldConstants#tryFoldGetElem
    #[allow(clippy::collapsible_if)]
    fn try_fold_get_elem(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        check_argument!(n.is_get_elem(compiler) || n.is_opt_chain_get_elem(compiler));

        if left.is_object_lit(compiler) {
            if right.is_string_lit(compiler) {
                let name = right.get_string(compiler);
                return self.try_fold_object_prop_access(compiler, n, left, &name);
            }
        } else if left.is_array_lit(compiler) {
            return self.try_fold_array_access(compiler, n, left, right);
        } else if left.is_string_lit(compiler) {
            return self.try_fold_string_array_access(compiler, n, left, right);
        }
        n
    }

    /// For prop access using GETPROP/OPTCHAIN_GETPROP on object literals, tries to fold their
    /// property access. For prop access on arrays, only tries to fold array-length. e.g [1, 2,
    /// 3].length ==> 3, [x, y].length ==> 2
    // port: PeepholeFoldConstants#tryFoldGetProp
    fn try_fold_get_prop(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_get_prop(compiler) || n.is_opt_chain_get_prop(compiler));
        let left = n.get_first_child(compiler).unwrap();
        let name = n.get_string(compiler);

        if left.is_object_lit(compiler) {
            return self.try_fold_object_prop_access(compiler, n, left, &name);
        }

        if name == "length" {
            if NodeUtil::is_l_value(compiler, n) {
                return n;
            }
            let known_length: i32;
            match left.get_token(compiler) {
                Token::ARRAYLIT => {
                    if self.may_have_side_effects(compiler, left) {
                        // Nope, can't fold this, without handling the side-effects.
                        return n;
                    }
                    let mut child = left.get_first_child(compiler);
                    while let Some(c) = child {
                        if c.is_spread(compiler) {
                            return n;
                        }
                        child = c.get_next(compiler);
                    }
                    known_length = left.get_child_count(compiler);
                }
                Token::STRINGLIT => known_length = left.get_string(compiler).length() as i32,
                _ => {
                    // Not a foldable case, forget it.
                    return n;
                }
            }

            check_state!(known_length != -1);
            let length_node = IR::number(compiler, f64::from(known_length));
            self.report_change_to_enclosing_scope(compiler, n);
            n.replace_with(compiler, length_node);

            return length_node;
        }

        n
    }

    // port: PeepholeFoldConstants#tryFoldArrayAccess
    fn try_fold_array_access(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        // If GETPROP/GETELEM is used as assignment target the array literal is
        // acting as a temporary we can't fold it here:
        //    "[][0] += 1"
        if NodeUtil::is_l_value(compiler, n) {
            return n;
        }

        let index = self.get_side_effect_free_number_value(compiler, right);
        if !self.is_reasonable_double_value(index) {
            // Sometimes people like to use complex expressions to index into
            // arrays, or strings to index into array methods.
            return n;
        }
        let index = index.unwrap();

        if !JSCompDoubles::is_exact_int32(index) {
            // Ideally this should be caught in the check passes.
            self.report(compiler, &INVALID_GETELEM_INDEX_ERROR, right);
            return n;
        }

        let int_index = index as i32;
        let mut current = left.get_first_child(compiler);
        let mut elem = None;
        let mut i = 0;
        while let Some(cur) = current {
            if cur.is_spread(compiler) {
                // The only time we can fold getelems with spread is for spread arrays literals,
                // and `tryFlattenArray` already flattens those.
                return n;
            }
            if i != int_index {
                if self.may_have_side_effects(compiler, cur) {
                    return n;
                }
            } else {
                elem = Some(cur);
            }

            current = cur.get_next(compiler);
            i += 1;
        }

        let elem = match elem {
            None => {
                // If the index was out of bounds
                NodeUtil::new_undefined_node(compiler, Some(left))
            }
            Some(elem) if elem.is_empty(compiler) => {
                NodeUtil::new_undefined_node(compiler, Some(elem))
            }
            Some(elem) => elem.detach(compiler),
        };

        // Replace the entire GETELEM with the value
        n.replace_with(compiler, elem);
        self.report_change_to_enclosing_scope(compiler, elem);
        elem
    }

    /// Fold any occurrences of spread of array literals e.g. `...[1,2,3]` to `1,2,3`
    // port: PeepholeFoldConstants#tryFoldSpread
    fn try_fold_spread(&self, compiler: &mut AbstractCompiler, spread: NodeId) -> NodeId {
        check_state!(spread.is_spread(compiler));
        let parent = spread.get_parent(compiler).unwrap();
        let child = spread.get_only_child(compiler);
        if child.is_array_lit(compiler)
            && Self::is_safe_to_flatten_array_lit_spread(compiler, child)
        {
            let children = child.remove_children(compiler);
            parent.add_children_after(compiler, children, Some(spread));
            spread.detach(compiler);
            self.report_change_to_enclosing_scope(compiler, parent);
        }
        parent
    }

    /// Flattens array- or object-literals that contain spreads of other literals.
    ///
    /// Does not recurse into nested spreads because this method is already called as part of a
    /// postorder traversal and nested spreads will already have been flattened.
    ///
    /// Example: `[0, ...[1, 2, 3], 4, ...[5]]` => `[0, 1, 2, 3, 4, 5]`
    // port: PeepholeFoldConstants#tryFlattenArrayOrObjectLit
    fn try_flatten_array_or_object_lit(
        &self,
        compiler: &mut AbstractCompiler,
        parent_lit: NodeId,
    ) -> NodeId {
        let mut child = parent_lit.get_first_child(compiler);
        while let Some(c) = child {
            // We have to store the next element here because nodes may be inserted below.
            let spread = c;
            child = c.get_next(compiler);

            if !spread.is_spread(compiler) {
                continue;
            }

            let inner_lit = spread.get_only_child(compiler);
            if parent_lit.get_token(compiler) != inner_lit.get_token(compiler) {
                continue; // We only want to inline arrays into arrays and objects into objects.
            }

            if parent_lit.is_object_lit(compiler)
                && !Self::is_safe_to_flatten_object_lit_spread(compiler, inner_lit)
            {
                continue;
            }

            if parent_lit.is_array_lit(compiler)
                && !Self::is_safe_to_flatten_array_lit_spread(compiler, inner_lit)
            {
                continue;
            }

            let children = inner_lit.remove_children(compiler);
            parent_lit.add_children_after(compiler, children, Some(spread));
            spread.detach(compiler);
            self.report_change_to_enclosing_scope(compiler, parent_lit);
        }
        parent_lit
    }

    // port: PeepholeFoldConstants#tryFoldStringArrayAccess
    fn try_fold_string_array_access(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        // If GETPROP/GETELEM is used as assignment target the array literal is
        // acting as a temporary we can't fold it here:
        //    "[][0] += 1"
        if NodeUtil::is_l_value(compiler, n) {
            return n;
        }

        let index = self.get_side_effect_free_number_value(compiler, right);
        if !self.is_reasonable_double_value(index) {
            // Sometimes people like to use complex expressions to index into
            // arrays, or strings to index into array methods.
            return n;
        }
        let index = index.unwrap();

        let int_index = index as i32;
        if f64::from(int_index) != index {
            self.report(compiler, &INVALID_GETELEM_INDEX_ERROR, right);
            return n;
        }

        check_state!(left.is_string_lit(compiler));
        let value = left.get_string(compiler);
        if int_index < 0 || int_index >= value.length() as i32 {
            let undefined = NodeUtil::new_undefined_node(compiler, Some(left));
            n.replace_with(compiler, undefined);
            self.report_change_to_enclosing_scope(compiler, undefined);
            return undefined;
        }

        let mut c: u16 = 0;
        // Note: For now skip the strings with unicode
        // characters as I don't understand the differences
        // between Java and JavaScript.
        for i in 0..=int_index {
            c = value.char_at(i as usize);
            #[allow(clippy::manual_range_contains)]
            if c < 32 || c > 127 {
                return n;
            }
        }
        let elem = IR::string(compiler, JsString::from_units(vec![c]));

        // Replace the entire GETELEM with the value
        n.replace_with(compiler, elem);
        self.report_change_to_enclosing_scope(compiler, elem);
        elem
    }

    /// Tries to fold the node `n` that's accessing an object literal's property. Bails out
    /// (skips folding and returns the same `n` node) with certainty when any of the following
    /// holds true:
    ///
    /// - the access `n` is L-value
    /// - the property references super
    /// - the object has side-effects other than those preserved by folding the property
    /// - property accessed does not exist on the object (might exist on prototype)
    ///
    /// Examples of folding:
    /// - `({a() { return 1; }})?.a` ---> `(function() { return 1; })`
    /// - `({a() { return 1; }}).a()` ---> `(function() { return 1; }())`
    // port: PeepholeFoldConstants#tryFoldObjectPropAccess
    fn try_fold_object_prop_access(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        name: &JsString,
    ) -> NodeId {
        check_argument!(NodeUtil::is_normal_or_opt_chain_get(compiler, n));

        if !left.is_object_lit(compiler) {
            return n;
        }

        if NodeUtil::is_l_value(compiler, n) {
            // If GETPROP/GETELEM is used as assignment target the object literal is
            // acting as a temporary we can't fold it here:
            //    "{a:x}.a += 1" is not "x += 1"
            check_state!(!NodeUtil::is_opt_chain_node(compiler, n)); // optional chains can not be targets of assign
            return n;
        }

        // find the last definition in the object literal
        let mut key = None;
        let mut value = None;
        let mut c = left.get_first_child(compiler);
        while let Some(cur) = c {
            match cur.get_token(compiler) {
                Token::SETTER_DEF => {}
                Token::OBJECT_SPREAD => {
                    // Reset the search because spread could overwrite any previous result.
                    key = None;
                    value = None;
                }
                Token::COMPUTED_PROP => {
                    // don't handle computed properties unless the input is a simple string
                    let prop = cur.get_first_child(compiler).unwrap();
                    if !prop.is_string_lit(compiler) {
                        return n;
                    }
                    if prop.get_string(compiler) == *name
                        && !cur.get_boolean_prop(compiler, NodeId::COMPUTED_PROP_SETTER)
                    {
                        key = Some(cur);
                        value = cur.get_second_child(compiler);
                    }
                }
                Token::GETTER_DEF | Token::STRING_KEY | Token::MEMBER_FUNCTION_DEF => {
                    if cur.get_string(compiler) == *name {
                        key = Some(cur);
                        value = cur.get_first_child(compiler);
                    }
                }
                // throw new IllegalStateException()
                _ => panic!(""),
            }
            c = cur.get_next(compiler);
        }

        // Didn't find a definition of the name in the object literal, it might
        // be coming from the Object prototype.
        let Some(mut value) = value else {
            return n;
        };
        let key = key.unwrap();

        /* `super` captures a hidden reference to the declaring objectlit, so we can't fold it away. */
        if NodeUtil::references_super(compiler, value) {
            return n;
        }

        // Check to see if there are any side-effects to this object-literal.
        //
        // We remove the value we're going to use because its side-effects will be preserved.
        let temp_value = IR::null_node(compiler);
        value.replace_with(compiler, temp_value);
        let has_side_effect_besides_value = self.may_have_side_effects(compiler, left);
        temp_value.replace_with(compiler, value);
        if has_side_effect_besides_value {
            return n;
        }

        let key_is_getter = NodeUtil::is_get_or_set_key(compiler, key);
        let n_is_invoked = NodeUtil::is_invocation_target(compiler, n);

        if key_is_getter || n_is_invoked {
            // When the code looks like:
            //   {x: f}.x();
            //   {get x() {...}}.x;
            //   {get ['x']() {...}}.x;

            // It's not safe, in general, to convert that to just a function call, because the
            // receiver value will be wrong.
            //
            // However, there are some cases where it's ok which we check here.
            if !value.is_function(compiler) || NodeUtil::references_own_receiver(compiler, value) {
                return n;
            }
        }

        value.detach(compiler);

        let parent = n.get_parent(compiler).unwrap();

        if NodeUtil::is_opt_chain_node(compiler, parent)
            && n.is_first_child_of(compiler, Some(parent))
            && !parent.is_optional_chain_start(compiler)
        {
            // If the chain continues after `n`, simply doing `n.replaceWith(value)` below would
            // leave the subsequent nodes in the current chain segment optional, with their start
            // `n` replaced.
            //
            // So, we must ensure that all nodes in the chain's current segment are made
            // non-optional.
            //
            // This can happen for e.g.
            //
            // - `({a() { return 1; }})?.a()` ---> `(function() { return 1; })()`. Here, parent
            //   OPTCHAIN_CALL node must be converted to CALL.
            // - `({a() { return 1; }})?.a().b.c?.d` ---> `(function() { return 1; })().b.c?.d`.
            //   Here, all nodes upto `({a() { return 1; }})?.a().b.c` must become non-optional
            let end_of_current_chain = NodeUtil::get_end_of_opt_chain_segment(compiler, parent);
            NodeUtil::convert_to_non_optional_chain_segment(compiler, end_of_current_chain);
        }
        if key_is_getter {
            value = IR::call(compiler, value, &[]);
            value.put_boolean_prop(compiler, NodeId::FREE_CALL, true);
        } else if n_is_invoked {
            n.get_parent(compiler)
                .unwrap()
                .put_boolean_prop(compiler, NodeId::FREE_CALL, true);
        }

        n.replace_with(compiler, value);
        self.report_change_to_enclosing_scope(compiler, value);
        self.mark_functions_deleted(compiler, n);
        n
    }

    // port: PeepholeFoldConstants#bigintNode
    fn bigint_node(ast: &mut Ast, val: BigInt, srcref: NodeId) -> NodeId {
        let tree = if val.sign() == Sign::Minus {
            let negated = IR::bigint(ast, -val);
            IR::neg(ast, negated)
        } else {
            IR::bigint(ast, val)
        };
        tree.srcref_tree(ast, srcref)
    }

    // port: PeepholeFoldConstants#isSafeToFlattenObjectLitSpread
    fn is_safe_to_flatten_object_lit_spread(ast: &Ast, inner_lit: NodeId) -> bool {
        let mut prop = inner_lit.get_first_child(ast);
        while let Some(p) = prop {
            // Object spread converts all properties to regular data properties. This step
            // evaluates any getters and omits any setters. See
            // https://tc39.es/ecma262/#sec-copydataproperties.
            // For simplicity, we just back off and do not flatten spread in this case.
            if NodeUtil::is_get_or_set_key(ast, p) {
                return false;
            }
            prop = p.get_next(ast);
        }
        true
    }

    // port: PeepholeFoldConstants#isSafeToFlattenArrayLitSpread
    fn is_safe_to_flatten_array_lit_spread(ast: &Ast, inner_lit: NodeId) -> bool {
        let mut n = inner_lit.get_first_child(ast);
        while let Some(cur) = n {
            if cur.is_empty(ast) {
                return false;
            }
            n = cur.get_next(ast);
        }
        true
    }
}

impl AbstractPeepholeOptimization for PeepholeFoldConstants {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeFoldConstants"
    }

    // port: PeepholeFoldConstants#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        Some(match subtree.get_token(compiler) {
            Token::OPTCHAIN_CALL | Token::CALL => {
                self.try_fold_useless_object_dot_define_properties_call(compiler, subtree)
            }
            Token::NEW => self.try_fold_ctor_call(compiler, subtree),
            Token::TYPEOF => self.try_fold_typeof(compiler, subtree),
            Token::ITER_SPREAD => self.try_fold_spread(compiler, subtree),
            Token::ARRAYLIT | Token::OBJECTLIT => {
                self.try_flatten_array_or_object_lit(compiler, subtree)
            }
            Token::NOT | Token::POS | Token::NEG | Token::BITNOT => {
                self.try_reduce_operands_for_op(compiler, subtree);
                self.try_fold_unary_operator(compiler, subtree)
            }
            Token::VOID => self.try_reduce_void(compiler, subtree),
            Token::OPTCHAIN_GETPROP | Token::GETPROP => self.try_fold_get_prop(compiler, subtree),
            Token::TEMPLATELIT => self.try_fold_template_literal_substitutions(compiler, subtree),
            _ => {
                self.try_reduce_operands_for_op(compiler, subtree);
                self.try_fold_binary_operator(compiler, subtree)
            }
        })
    }
}
