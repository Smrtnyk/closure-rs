/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckSuspiciousCode.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_jstype::prelude::JSType;
use closure_rhino::{check_state, jscomp_base::tri::Tri, node::NodeId, token::Token};

// port: CheckSuspiciousCode#SUSPICIOUS_SEMICOLON
pub static SUSPICIOUS_SEMICOLON: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_SEMICOLON",
    "If this if/for/while really shouldn''t have a body, use '{}'",
);

// port: CheckSuspiciousCode#SUSPICIOUS_COMPARISON_WITH_NAN
pub static SUSPICIOUS_COMPARISON_WITH_NAN: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_NAN",
    "Comparison against NaN is always false. Did you mean isNaN()?",
);

// port: CheckSuspiciousCode#SUSPICIOUS_IN_OPERATOR
pub static SUSPICIOUS_IN_OPERATOR: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_IN",
    "Use of the \"in\" keyword on non-object types throws an exception.",
);

// port: CheckSuspiciousCode#SUSPICIOUS_INSTANCEOF_LEFT_OPERAND
pub static SUSPICIOUS_INSTANCEOF_LEFT_OPERAND: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_INSTANCEOF_LEFT",
    "\"instanceof\" with left non-object operand is always false.",
);

// port: CheckSuspiciousCode#SUSPICIOUS_LEFT_OPERAND_OF_LOGICAL_OPERATOR
pub static SUSPICIOUS_LEFT_OPERAND_OF_LOGICAL_OPERATOR: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_LEFT_OPERAND_OF_LOGICAL_OPERATOR",
    "Left operand of {0} operator is always {1}.",
);

// port: CheckSuspiciousCode#SUSPICIOUS_NEGATED_LEFT_OPERAND_OF_IN_OPERATOR
pub static SUSPICIOUS_NEGATED_LEFT_OPERAND_OF_IN_OPERATOR: DiagnosticType = DiagnosticType::warning(
    "JSC_SUSPICIOUS_NEGATED_LEFT_OPERAND_OF_IN_OPERATOR",
    "Suspicious negated left operand of 'in' operator.",
);

// port: CheckSuspiciousCode#SUSPICIOUS_BREAKING_OUT_OF_OPTIONAL_CHAIN
pub static SUSPICIOUS_BREAKING_OUT_OF_OPTIONAL_CHAIN: DiagnosticType = DiagnosticType::warning(
    "SUSPICIOUS_BREAKING_OUT_OF_OPTIONAL_CHAIN",
    "Suspicious breaking out of optional chain. May result in TypeError if optional chain is undefined.",
);

/// Checks for common errors, such as misplaced semicolons:
///
/// ```text
/// if (x); act_now();
/// ```
///
/// or comparison against NaN:
///
/// ```text
/// if (x === NaN) act();
/// ```
///
/// and generates warnings.
pub struct CheckSuspiciousCode;

impl CheckSuspiciousCode {
    // port: CheckSuspiciousCode#CheckSuspiciousCode
    pub fn new() -> Self {
        Self
    }

    // port: CheckSuspiciousCode#checkMissingSemicolon
    fn check_missing_semicolon(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        match n.get_token(t) {
            Token::IF => {
                let true_case = n.get_second_child(t).unwrap();
                Self::report_if_was_empty(t, true_case);
                let else_case = true_case.get_next(t);
                if let Some(else_case) = else_case {
                    Self::report_if_was_empty(t, else_case);
                }
            }
            Token::WHILE | Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let block = NodeUtil::get_loop_code_block(t, n).unwrap();
                Self::report_if_was_empty(t, block);
            }
            _ => {}
        }
    }

    // port: CheckSuspiciousCode#reportIfWasEmpty
    fn report_if_was_empty(t: &mut NodeTraversal<'_>, block: NodeId) {
        check_state!(block.is_block(t));

        // A semicolon is distinguished from a block without children by
        // annotating it with EMPTY_BLOCK.  Blocks without children are
        // usually intentional, especially with loops.
        if !block.has_children(t) && block.is_added_block(t) {
            let error = JSError::make(t, block, &SUSPICIOUS_SEMICOLON, &[]);
            t.get_compiler().report(error);
        }
    }

    // port: CheckSuspiciousCode#checkNaN
    fn check_nan(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        match n.get_token(t) {
            Token::EQ
            | Token::GE
            | Token::GT
            | Token::LE
            | Token::LT
            | Token::NE
            | Token::SHEQ
            | Token::SHNE => {
                let first = n.get_first_child(t).unwrap();
                Self::report_if_nan(t, first);
                let last = n.get_last_child(t).unwrap();
                Self::report_if_nan(t, last);
            }
            _ => {}
        }
    }

    // port: CheckSuspiciousCode#reportIfNaN
    fn report_if_nan(t: &mut NodeTraversal<'_>, n: NodeId) {
        if NodeUtil::is_nan(t, n) {
            let parent = n.get_parent(t).unwrap();
            let error = JSError::make(t, parent, &SUSPICIOUS_COMPARISON_WITH_NAN, &[]);
            t.get_compiler().report(error);
        }
    }

    // port: CheckSuspiciousCode#checkInvalidIn
    fn check_invalid_in(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_in(t) {
            let last = n.get_last_child(t).unwrap();
            Self::report_if_non_object(t, last, &SUSPICIOUS_IN_OPERATOR);
        }
    }

    // port: CheckSuspiciousCode#checkNonObjectInstanceOf
    fn check_non_object_instance_of(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_instance_of(t) {
            let first = n.get_first_child(t).unwrap();
            Self::report_if_non_object(t, first, &SUSPICIOUS_INSTANCEOF_LEFT_OPERAND);
        }
    }

    // port: CheckSuspiciousCode#reportIfNonObject
    fn report_if_non_object(
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        diagnostic_type: &'static DiagnosticType,
    ) -> bool {
        if n.is_add(t) || !NodeUtil::may_be_object(t, n) {
            let parent = n.get_parent(t).unwrap();
            t.report(parent, diagnostic_type, &[]);
            return true;
        }
        false
    }

    // port: CheckSuspiciousCode#checkNegatedLeftOperandOfInOperator
    fn check_negated_left_operand_of_in_operator(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_in(t) && n.get_first_child(t).unwrap().is_not(t) {
            let first = n.get_first_child(t).unwrap();
            t.report(first, &SUSPICIOUS_NEGATED_LEFT_OPERAND_OF_IN_OPERATOR, &[]);
        }
    }

    /// Check for the LHS of a logical operator (&amp;&amp; and ||) being deterministically truthy
    /// or falsy, using both syntactic and type information. This is always suspicious (though for
    /// different reasons: "truthy and" means the LHS is always ignored and should be removed,
    /// "falsy and" means the RHS is dead code, and vice versa for "or". However, there are a number
    /// of legitimate use cases where we need to back off from using type information (see
    /// [`Self::get_boolean_value_with_types`] for more details on these back offs).
    // port: CheckSuspiciousCode#checkLeftOperandOfLogicalOperator
    fn check_left_operand_of_logical_operator(&self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if n.is_or(t) || n.is_and(t) {
            let operator = if n.is_or(t) { "||" } else { "&&" };
            let first = n.get_first_child(t).unwrap();
            let v = self.get_boolean_value_with_types(t, first);
            if v != Tri::UNKNOWN {
                let result = if v == Tri::TRUE { "truthy" } else { "falsy" };
                t.report(
                    n,
                    &SUSPICIOUS_LEFT_OPERAND_OF_LOGICAL_OPERATOR,
                    &[operator, result],
                );
            }
        }
    }

    /// Checks for breaking out of optional chain. (e.g. `(a?.b).c)` There is no reason to break
    /// out of optional chains and doing so may cause a TypeError if `a` is nullish. Using `a?.b.c`
    /// is safer and will have the same effect when `a` is not nullish.
    // port: CheckSuspiciousCode#checkSuspiciousBreakingOutOfOptionalChain
    fn check_suspicious_breaking_out_of_optional_chain(t: &mut NodeTraversal<'_>, n: NodeId) {
        if NodeUtil::is_opt_chain_node(t, n) {
            let parent = n.get_parent(t).unwrap();
            if n.is_first_child_of(t, Some(parent))
                && (parent.is_get_prop(t) || parent.is_get_elem(t) || parent.is_call(t))
            {
                t.report(n, &SUSPICIOUS_BREAKING_OUT_OF_OPTIONAL_CHAIN, &[]);
            }
        }
    }

    /// Returns the possible boolean values of a node. This is a combination of
    /// `NodeUtil#getBooleanValue` and `JSType#getPossibleToBooleanOutcomes`, with some additional
    /// backoff for situations that are known to be less accurate (to wit, qualified names and
    /// truthy return types, which return UNKNOWN even if the type information seems conclusive).
    /// Another difference from the NodeUtil and JSTyoe methods is that we include some specific
    /// optimization to avoid quadratic behavior of large nested logical operations.
    ///
    /// The specifics of when this method returns UNKNOWN instead of TRUE or FALSE is as follows:
    ///
    /// - We should not determine that a qualified name (expanded slightly to include computed
    ///   properties) is always truthy or always falsy. There are many cases where an extern
    ///   defines a name to be truthy, but it still makes sense to feature-test in the browser.
    /// - We should not determine that a getelem or a function call is always truthy (though we
    ///   expand it to simply never complain about always-truthy based only on types) since the
    ///   standard externs lie about the return type of `Map.prototype.get` and array accesses,
    ///   which can both return undefined despite what the externs say.
    ///
    /// We do not back off from boolean literals (e.g. "`true &&`"), though they appear to be
    /// common in generated code. Instead, such code should suppress "suspiciousCode". We also do
    /// not back off from always-falsy function call results, since it provides a valuable check
    /// and lies in this direction are much less common.
    // port: CheckSuspiciousCode#getBooleanValueWithTypes
    fn get_boolean_value_with_types(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> Tri {
        match n.get_token(t) {
            Token::ASSIGN | Token::COMMA => {
                let last = n.get_last_child(t).unwrap();
                return self.get_boolean_value_with_types(t, last);
            }
            Token::NOT => {
                let last = n.get_last_child(t).unwrap();
                return self.get_boolean_value_with_types(t, last).not();
            }
            Token::AND => {
                // Assume the left-hand side is unknown. If it's not then we'll report it
                // elsewhere. This prevents revisiting deeper nodes repeatedly, which would result
                // in O(n^2) performance.
                let last = n.get_last_child(t).unwrap();
                return Tri::UNKNOWN.and(self.get_boolean_value_with_types(t, last));
            }
            Token::OR => {
                // Assume the left-hand side is unknown. If it's not then we'll report it
                // elsewhere. This prevents revisiting deeper nodes repeatedly, which would result
                // in O(n^2) performance.
                let last = n.get_last_child(t).unwrap();
                return Tri::UNKNOWN.or(self.get_boolean_value_with_types(t, last));
            }
            Token::HOOK => {
                let second = n.get_second_child(t).unwrap();
                let true_value = self.get_boolean_value_with_types(t, second);
                let last = n.get_last_child(t).unwrap();
                let false_value = self.get_boolean_value_with_types(t, last);
                return if true_value == false_value {
                    true_value
                } else {
                    Tri::UNKNOWN
                };
            }
            Token::FUNCTION | Token::CLASS | Token::NEW | Token::ARRAYLIT | Token::OBJECTLIT => {
                return Tri::TRUE;
            }
            Token::VOID => {
                return Tri::FALSE;
            }
            Token::GETPROP | Token::GETELEM | Token::OPTCHAIN_GETELEM | Token::OPTCHAIN_GETPROP => {
                // Assume that type information on getprops and getelems are likely to be wrong.
                // This prevents spurious warnings from not including undefined in getelem's
                // return value, from existence checks of symbols the externs define as certainly
                // true, or from default initialization of globals (`x.y = x.y || {}`).
                return Tri::UNKNOWN;
            }
            _ => {}
        }
        // If we reach this point then all the composite structures that we can decompose have
        // already been handled, leaving only qualified names and type-aware checks to handle
        // below. Note that much of the switch above in fact duplicates the logic in
        // getImpureBooleanValue, though with some subtle differences.  Important differences
        // include (1) avoiding recursion into the left-hand-side of nested logical operators,
        // instead treating them as unknown since they would have already been reported elsewhere
        // in the traversal had they been otherwise (this guarantees we visit each node once,
        // rather than quadratically repeating work); (2) it propagates our unique amalgam of
        // syntax-based and type-based checks to work when more deeply nested (i.e. recursively).
        // These differences rely on assumptions that are very specific to this use case, so it
        // does not make sense to upstream them.
        let literal_value = NodeUtil::get_boolean_value(t, n);
        if literal_value != Tri::UNKNOWN || n.is_name(t) {
            // If the truthiness is determinstic from the syntax then return that immediately.
            // Alternatively, NAME nodes also get a pass since we don't trust the type
            // information.
            return literal_value;
        }
        let type_ = n.get_jstype(t);
        if let Some(type_) = type_ {
            // Distrust types we think are always truthy, since sometimes the types lie, even for
            // results of function calls (e.g. Map.prototype.get), so it's still important to
            // check.  But always-falsy values are a little more obviously wrong and there should
            // be no reason for those type annotations to be lies.  ANDing with UNKNOWN ensures we
            // never return TRUE.
            let registry = t.get_compiler().get_type_registry();
            return Tri::UNKNOWN.and(type_.get_possible_to_boolean_outcomes(registry).to_tri());
        }
        Tri::UNKNOWN
    }
}

impl Default for CheckSuspiciousCode {
    fn default() -> Self {
        Self::new()
    }
}

impl Callback for CheckSuspiciousCode {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckSuspiciousCode#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        self.check_missing_semicolon(t, n);
        self.check_nan(t, n);
        self.check_invalid_in(t, n);
        self.check_non_object_instance_of(t, n);
        self.check_negated_left_operand_of_in_operator(t, n);
        self.check_left_operand_of_logical_operator(t, n);
        Self::check_suspicious_breaking_out_of_optional_chain(t, n);
    }
}
