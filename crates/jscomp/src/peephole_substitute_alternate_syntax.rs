/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeSubstituteAlternateSyntax.java.

//! Port of `PeepholeSubstituteAlternateSyntax.java`.
//!
//! A peephole optimization that minimizes code by simplifying conditional expressions, replacing
//! IFs with HOOKs, replacing object constructors with literals, and simplifying returns.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

/// Java's `".split('.')".length()`.
const STRING_SPLIT_OVERHEAD: i32 = ".split('.')".len() as i32;

const BUILTIN_EXTERNS: &[&str] = &["Object", "Array", "Error", "RegExp", "Math"];

// String, Number, and Boolean functions return non-object types, whereas
// new String, new Number, and new Boolean return object types, so don't
// include them here.
const STANDARD_OBJECT_CONSTRUCTORS: &[&str] = &["Object", "Array", "Error"];

// Rust-only: `ImmutableSet<String>#contains` over a constant set.
fn set_contains(set: &[&str], s: &JsString) -> bool {
    set.iter().any(|e| s == e)
}

pub struct PeepholeSubstituteAlternateSyntax {
    fields: AbstractPeepholeOptimizationFields,
    late: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FoldArrayAction {
    NotSafeToFold,
    SafeToFoldWithArgs,
    SafeToFoldWithoutArgs,
}

impl PeepholeSubstituteAlternateSyntax {
    /// `late` When late is false, this mean we are currently running before most of the other
    /// optimizations. In this case we would avoid optimizations that would make the code harder
    /// to analyze (such as using string splitting, merging statements with commas, etc). When
    /// this is true, we would do anything to minimize for size.
    // port: PeepholeSubstituteAlternateSyntax#PeepholeSubstituteAlternateSyntax
    pub fn new(late: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            late,
        }
    }

    // port: PeepholeSubstituteAlternateSyntax#tryMinimizeWindowRefs
    fn try_minimize_window_refs(&self, compiler: &mut AbstractCompiler, node: NodeId) -> NodeId {
        // Normalization needs to be done to ensure there's no shadowing.
        if !self.is_ast_normalized(compiler) {
            return node;
        }

        check_argument!(node.is_get_prop(compiler));

        if node.get_first_child(compiler).unwrap().is_name(compiler) {
            let name_node = node.get_first_child(compiler).unwrap();

            // Since normalization has run we know we're referring to the global window.
            if name_node.get_string(compiler) == "window"
                && set_contains(BUILTIN_EXTERNS, &node.get_string(compiler))
            {
                let node_string = node.get_string(compiler);
                let new_name_node = IR::name(compiler, node_string);
                let parent_node = node.get_parent(compiler).unwrap();

                new_name_node.srcref(compiler, node);
                node.replace_with(compiler, new_name_node);

                if parent_node.is_call(compiler) || parent_node.is_opt_chain_call(compiler) {
                    // e.g. when converting `window.Array?.()` to `Array?.()`, ensure that the
                    // OPTCHAIN_CALL gets marked as `FREE_CALL`
                    parent_node.put_boolean_prop(compiler, NodeId::FREE_CALL, true);
                }
                self.report_change_to_enclosing_scope(compiler, parent_node);
                return new_name_node;
            }
        }

        node
    }

    // port: PeepholeSubstituteAlternateSyntax#tryRotateCommutativeOperator
    fn try_rotate_commutative_operator(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        if !self.late {
            return n;
        }
        // Transform a * (b / c) to b / c * a
        let rhs = n.get_last_child(compiler).unwrap();
        let mut lhs = n.get_first_child(compiler).unwrap();
        while lhs.get_token(compiler) == n.get_token(compiler)
            && NodeUtil::is_associative(n.get_token(compiler))
        {
            lhs = lhs.get_first_child(compiler).unwrap();
        }
        let precedence = NodeUtil::precedence(n.get_token(compiler));
        let lhs_precedence = NodeUtil::precedence(lhs.get_token(compiler));
        let rhs_precedence = NodeUtil::precedence(rhs.get_token(compiler));
        if rhs_precedence == precedence && lhs_precedence != precedence {
            rhs.detach(compiler);
            lhs.replace_with(compiler, rhs);
            n.add_child_to_back(compiler, lhs);
            self.report_change_to_enclosing_scope(compiler, n);
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryRotateAssociativeOperator
    fn try_rotate_associative_operator(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        if !self.late {
            return n;
        }
        // All commutative operators are also associative
        check_argument!(NodeUtil::is_associative(n.get_token(compiler)));
        let rhs = n.get_last_child(compiler).unwrap();
        if n.get_token(compiler) == rhs.get_token(compiler) {
            // Transform a * (b * c) to a * b * c
            let first = n.remove_first_child(compiler).unwrap();
            let second = rhs.remove_first_child(compiler).unwrap();
            let third = rhs.get_last_child(compiler).unwrap();
            third.detach(compiler);
            let n_token = n.get_token(compiler);
            let new_lhs = compiler
                .new_node_with_children2(n_token, first, second)
                .srcref_if_missing(compiler, n);
            let rhs_token = rhs.get_token(compiler);
            let new_root = compiler
                .new_node_with_children2(rhs_token, new_lhs, third)
                .srcref_if_missing(compiler, rhs);
            n.replace_with(compiler, new_root);
            self.report_change_to_enclosing_scope(compiler, new_root);
            return new_root;
        } else if NodeUtil::is_commutative(n.get_token(compiler))
            && !self.may_have_side_effects(compiler, n)
        {
            // Transform a * (b / c) to b / c * a
            return self.try_rotate_commutative_operator(compiler, n);
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryFoldSimpleFunctionCall
    fn try_fold_simple_function_call(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.is_call(compiler), "%s", n.to_string(compiler));
        let call_target = n.get_first_child(compiler);
        let Some(call_target) = call_target.filter(|t| t.is_name(compiler)) else {
            return n;
        };
        let target_name = call_target.get_string(compiler);
        if target_name == "Boolean" {
            // Fold Boolean(a) to !!a
            // http://www.ecma-international.org/ecma-262/6.0/index.html#sec-boolean-constructor-boolean-value
            // and
            // http://www.ecma-international.org/ecma-262/6.0/index.html#sec-logical-not-operator-runtime-semantics-evaluation
            let param_count = n.get_child_count(compiler) - 1;
            // only handle the single known parameter case
            if param_count == 1 {
                let value = n.get_last_child(compiler).unwrap();
                value.detach(compiler);
                let replacement = if NodeUtil::is_boolean_result(compiler, value) {
                    // If it is already a boolean do nothing.
                    value
                } else {
                    // Replace it with a "!!value"
                    let inner = IR::not(compiler, value).srcref(compiler, n);
                    IR::not(compiler, inner)
                };
                n.replace_with(compiler, replacement);
                self.report_change_to_enclosing_scope(compiler, replacement);
            }
        } else if target_name == "String" {
            // Fold String(a) to '' + (a) on immutable literals,
            // which allows further optimizations
            //
            // We can't do this in the general case, because String(a) has
            // slightly different semantics than '' + (a). See
            // https://blickly.github.io/closure-compiler-issues/#759
            let value = call_target.get_next(compiler);
            if let Some(value) = value
                && value.get_next(compiler).is_none()
                && NodeUtil::is_immutable_value(compiler, value)
            {
                let empty = IR::string(compiler, "").srcref(compiler, call_target);
                value.detach(compiler);
                let addition = IR::add(compiler, empty, value);
                n.replace_with(compiler, addition);
                self.report_change_to_enclosing_scope(compiler, addition);
                return addition;
            }
        } else {
            // nothing.
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryFoldImmediateCallToBoundFunction
    fn try_fold_immediate_call_to_bound_function(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        // Rewriting "(fn.bind(a,b))()" to "fn.call(a,b)" makes it inlinable
        check_state!(n.is_call(compiler));
        let mut call_target = n.get_first_child(compiler).unwrap();
        // Rust-only `None` registry: `checkTypes` is false, so the registry is never read.
        let bind = self.get_coding_convention(compiler).describe_function_bind(
            compiler,
            None,
            call_target,
            false,
        );
        if let Some(bind) = bind {
            // replace the call target
            bind.target.detach(compiler);
            call_target.replace_with(compiler, bind.target);
            call_target = bind.target;

            // push the parameters
            Self::add_parameter_after(compiler, bind.parameters, call_target);

            // add the this value before the parameters if necessary
            if let Some(this_value) = bind.this_value
                && !NodeUtil::is_undefined(compiler, this_value)
            {
                // rewrite from "fn(a, b)" to "fn.call(thisValue, a, b)"
                let cloned = call_target.clone_tree(compiler);
                let new_call_target = IR::getprop(compiler, cloned, "call");
                self.mark_new_scopes_changed(compiler, new_call_target);
                call_target.replace_with(compiler, new_call_target);
                self.mark_functions_deleted(compiler, call_target);
                let this_clone = this_value.clone_tree(compiler);
                this_clone.insert_after(compiler, new_call_target);
                n.put_boolean_prop(compiler, NodeId::FREE_CALL, false);
            } else {
                n.put_boolean_prop(compiler, NodeId::FREE_CALL, true);
            }
            self.report_change_to_enclosing_scope(compiler, n);
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#addParameterAfter
    fn add_parameter_after(ast: &mut Ast, parameter_list: Option<NodeId>, after: NodeId) {
        if let Some(parameter_list) = parameter_list {
            // push the last parameter to the head of the list first.
            let next = parameter_list.get_next(ast);
            Self::add_parameter_after(ast, next, after);
            let cloned = parameter_list.clone_tree(ast);
            cloned.insert_after(ast, after);
        }
    }

    /// Converts expressions of a potentially nested comma expression into a sequence of
    /// expression result statements and inserts them into the AST.
    ///
    /// `insert` Whether or not the leftmost expression is inserted into the AST. Returns the
    /// leftmost expression.
    // port: PeepholeSubstituteAlternateSyntax#splitComma
    fn split_comma(ast: &mut Ast, mut n: NodeId, insert: bool, insert_after: NodeId) -> NodeId {
        while n.is_comma(ast) {
            let left = n.get_first_child(ast).unwrap();
            let right = n.get_last_child(ast).unwrap();
            n.detach_children(ast);
            if right.is_comma(ast) {
                Self::split_comma(ast, right, true, insert_after);
            } else {
                // Add the right expression after the optimized expression.
                let new_statement = IR::expr_result(ast, right);
                new_statement.srcref_if_missing(ast, right);
                new_statement.insert_after(ast, insert_after);
            }
            n = left;
        }
        if insert {
            let new_statement = IR::expr_result(ast, n);
            new_statement.srcref_if_missing(ast, n);
            new_statement.insert_after(ast, insert_after);
            return new_statement;
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#trySplitComma
    fn try_split_comma(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        if self.late {
            return n;
        }
        check_state!(n.is_expr_result(compiler));
        if n.get_parent(compiler).unwrap().is_label(compiler) {
            // Do not split labeled comma expressions.
            return n;
        }
        let comma = n.get_first_child(compiler).unwrap();
        check_state!(comma.is_comma(compiler));
        let leftmost = Self::split_comma(compiler, comma, false, n);
        // Replace original expression with leftmost comma expression.
        n.remove_children(compiler);
        n.add_child_to_front(compiler, leftmost);
        n.srcref(compiler, leftmost);
        self.report_change_to_enclosing_scope(compiler, leftmost);
        leftmost
    }

    /// Use "void 0" in place of "undefined"
    // port: PeepholeSubstituteAlternateSyntax#tryReplaceUndefined
    fn try_replace_undefined(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        // TODO(johnlenz): consider doing this as a normalization.
        if self.is_ast_normalized(compiler)
            && NodeUtil::is_undefined(compiler, n)
            && !NodeUtil::is_l_value(compiler, n)
        {
            let replacement = NodeUtil::new_undefined_node(compiler, Some(n));
            n.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);
            return replacement;
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryReduceCall
    fn try_reduce_call(&self, compiler: &mut AbstractCompiler, node: NodeId) -> NodeId {
        let mut result = self.try_fold_literal_constructor(compiler, node);
        if result == node {
            result = self.try_fold_simple_function_call(compiler, node);
            if result == node {
                result = self.try_fold_immediate_call_to_bound_function(compiler, node);
            }
        }
        result
    }

    /// Reduce "return undefined" or "return void 0" to simply "return".
    ///
    /// Returns the original node, maybe simplified.
    // port: PeepholeSubstituteAlternateSyntax#tryReduceReturn
    fn try_reduce_return(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let result = n.get_first_child(compiler);

        if let Some(result) = result {
            match result.get_token(compiler) {
                Token::VOID => {
                    let operand = result.get_first_child(compiler).unwrap();
                    if !self.may_have_side_effects(compiler, operand) {
                        n.remove_first_child(compiler);
                        self.report_change_to_enclosing_scope(compiler, n);
                    }
                }
                Token::NAME => {
                    let name = result.get_string(compiler);
                    if name == "undefined" {
                        n.remove_first_child(compiler);
                        self.report_change_to_enclosing_scope(compiler, n);
                    }
                }
                _ => {}
            }
        }

        n
    }

    /// Fold "new Object()" to "Object()".
    // port: PeepholeSubstituteAlternateSyntax#tryFoldStandardConstructors
    fn try_fold_standard_constructors(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.is_new(compiler));

        if self.can_fold_standard_constructors(compiler, n) {
            n.set_token(compiler, Token::CALL);
            n.put_boolean_prop(compiler, NodeId::FREE_CALL, true);
            self.report_change_to_enclosing_scope(compiler, n);
        }

        n
    }

    /// Returns whether "new Object()" can be folded to "Object()" on `n`.
    // port: PeepholeSubstituteAlternateSyntax#canFoldStandardConstructors
    fn can_fold_standard_constructors(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        // If name normalization has been run then we know that
        // new Object() does in fact refer to what we think it is
        // and not some custom-defined Object().
        if self.is_ast_normalized(compiler)
            && n.get_first_child(compiler).unwrap().is_name(compiler)
        {
            let class_name = n.get_first_child(compiler).unwrap().get_string(compiler);
            if set_contains(STANDARD_OBJECT_CONSTRUCTORS, &class_name) {
                return true;
            }
            if class_name == "RegExp" {
                // Fold "new RegExp()" to "RegExp()", but only if the argument is a string.
                // See issue 1260.
                match n.get_second_child(compiler) {
                    None => return true,
                    Some(second) if second.is_string_lit(compiler) => return true,
                    Some(_) => {}
                }
            }
        }

        false
    }

    /// Replaces a new Array, Object, or RegExp node with a literal, unless the call is to a local
    /// constructor function with the same name.
    // port: PeepholeSubstituteAlternateSyntax#tryFoldLiteralConstructor
    fn try_fold_literal_constructor(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_call(compiler) || n.is_new(compiler));

        let constructor_name_node = n.get_first_child(compiler).unwrap();

        let mut new_literal_node: Option<NodeId> = None;

        // We require the AST to be normalized to ensure that, say,
        // Object() really refers to the built-in Object constructor
        // and not a user-defined constructor with the same name.

        if self.is_ast_normalized(compiler) && constructor_name_node.is_name(compiler) {
            let class_name = constructor_name_node.get_string(compiler);

            let constructor_has_args = constructor_name_node.get_next(compiler).is_some();

            if class_name == "Object" && !constructor_has_args {
                // "Object()" --> "{}"
                new_literal_node = Some(IR::objectlit(compiler, &[]));
            } else if class_name == "Array" {
                // "Array(arg0, arg1, ...)" --> "[arg0, arg1, ...]"
                let arg0 = constructor_name_node.get_next(compiler);
                let action = Self::is_safe_to_fold_array_constructor(compiler, arg0);

                if action == FoldArrayAction::SafeToFoldWithArgs
                    || action == FoldArrayAction::SafeToFoldWithoutArgs
                {
                    let literal = IR::arraylit(compiler, &[]);
                    new_literal_node = Some(literal);
                    n.remove_first_child(compiler); // discard the function name
                    let elements = n.remove_children(compiler);
                    if action == FoldArrayAction::SafeToFoldWithArgs {
                        literal.add_children_to_front(compiler, elements);
                    }
                }
            }

            if let Some(new_literal_node) = new_literal_node {
                n.replace_with(compiler, new_literal_node);
                self.report_change_to_enclosing_scope(compiler, new_literal_node);
                return new_literal_node;
            }
        }
        n
    }

    /// Checks if it is safe to fold Array() constructor into []. It can be obviously done, if the
    /// initial constructor has either no arguments or at least two. The remaining case may be
    /// unsafe since Array(number) actually reserves memory for an empty array which contains
    /// number elements.
    // port: PeepholeSubstituteAlternateSyntax#isSafeToFoldArrayConstructor
    fn is_safe_to_fold_array_constructor(ast: &Ast, arg: Option<NodeId>) -> FoldArrayAction {
        let mut action = FoldArrayAction::NotSafeToFold;

        match arg {
            None => {
                action = FoldArrayAction::SafeToFoldWithoutArgs;
            }
            Some(arg) if arg.get_next(ast).is_some() => {
                action = FoldArrayAction::SafeToFoldWithArgs;
            }
            Some(arg) => match arg.get_token(ast) {
                Token::STRINGLIT => {
                    // "Array('a')" --> "['a']"
                    action = FoldArrayAction::SafeToFoldWithArgs;
                }
                // "Array(0)" --> "[]"
                Token::NUMBER if arg.get_double(ast) == 0.0 => {
                    action = FoldArrayAction::SafeToFoldWithoutArgs;
                }
                Token::ARRAYLIT => {
                    // "Array([args])" --> "[[args]]"
                    action = FoldArrayAction::SafeToFoldWithArgs;
                }
                _ => {}
            },
        }
        action
    }

    // port: PeepholeSubstituteAlternateSyntax#reduceSubstractionAssignment
    fn reduce_substraction_assignment(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let mut right = n.get_last_child(compiler).unwrap();
        let mut is_negative = false;
        if right.is_neg(compiler) {
            is_negative = true;
            right = right.get_only_child(compiler);
        }

        if right.is_number(compiler) && right.get_double(compiler) == 1.0 {
            let left = n.remove_first_child(compiler).unwrap();
            let new_node = if is_negative {
                IR::inc(compiler, left, false)
            } else {
                IR::dec(compiler, left, false)
            };
            n.replace_with(compiler, new_node);
            self.report_change_to_enclosing_scope(compiler, new_node);
            return new_node;
        }

        n
    }

    // port: PeepholeSubstituteAlternateSyntax#reduceTrueFalse
    fn reduce_true_false(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        if self.late {
            match n.get_parent(compiler).unwrap().get_token(compiler) {
                Token::EQ | Token::GT | Token::GE | Token::LE | Token::LT | Token::NE => {
                    let value = if n.is_true(compiler) { 1.0 } else { 0.0 };
                    let number = IR::number(compiler, value);
                    n.replace_with(compiler, number);
                    self.report_change_to_enclosing_scope(compiler, number);
                    return number;
                }
                _ => {}
            }

            let value = if n.is_true(compiler) { 0.0 } else { 1.0 };
            let number = IR::number(compiler, value);
            let not = IR::not(compiler, number);
            not.srcref_tree_if_missing(compiler, n);
            n.replace_with(compiler, not);
            self.report_change_to_enclosing_scope(compiler, not);
            return not;
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryMinimizeArrayLiteral
    fn try_minimize_array_literal(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let mut all_strings = true;
        let mut cur = n.get_first_child(compiler);
        while let Some(c) = cur {
            if !c.is_string_lit(compiler) {
                all_strings = false;
            }
            cur = c.get_next(compiler);
        }

        if all_strings {
            self.try_minimize_string_array_literal(compiler, n)
        } else {
            n
        }
    }

    // port: PeepholeSubstituteAlternateSyntax#tryMinimizeStringArrayLiteral
    fn try_minimize_string_array_literal(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        if !self.late {
            return n;
        }

        let num_elements = n.get_child_count(compiler);
        // We save two bytes per element.
        let saving = num_elements * 2 - STRING_SPLIT_OVERHEAD;
        if saving <= 0 {
            return n;
        }

        let mut strings: Vec<JsString> = Vec::with_capacity(num_elements as usize);
        let mut cur = n.get_first_child(compiler);
        while let Some(c) = cur {
            strings.push(c.get_string(compiler));
            cur = c.get_next(compiler);
        }

        // These delimiters are chars that appears a lot in the program therefore
        // probably have a small Huffman encoding.
        let delimiter = Self::pick_delimiter(&strings);
        if let Some(delimiter) = delimiter {
            // Joiner.on(delimiter).join(strings)
            let delimiter_string = JsString::from(delimiter);
            let mut template: Vec<u16> = Vec::new();
            for (i, s) in strings.iter().enumerate() {
                if i > 0 {
                    template.extend_from_slice(delimiter_string.as_units());
                }
                template.extend_from_slice(s.as_units());
            }
            let template_node = IR::string(compiler, JsString::from_units(template));
            let split = IR::getprop(compiler, template_node, "split");
            let delimiter_node = IR::string(compiler, delimiter_string);
            let call = IR::call(compiler, split, &[delimiter_node]);
            call.srcref_tree_if_missing(compiler, n);
            n.replace_with(compiler, call);
            self.report_change_to_enclosing_scope(compiler, call);
            return call;
        }
        n
    }

    // port: PeepholeSubstituteAlternateSyntax#tryTurnTemplateStringsToStrings
    fn try_turn_template_strings_to_strings(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        check_state!(n.is_template_lit(compiler), "%s", n.to_string(compiler));
        if n.get_parent(compiler)
            .unwrap()
            .is_tagged_template_lit(compiler)
        {
            return n;
        }
        let Some(string) = self.get_side_effect_free_string_value(compiler, n) else {
            return n;
        };
        let string_node = IR::string(compiler, string).srcref(compiler, n);
        n.replace_with(compiler, string_node);
        self.report_change_to_enclosing_scope(compiler, string_node);
        string_node
    }

    /// Find a delimiter that does not occur in the given strings
    ///
    /// `strings` The strings that must be separated. Returns a delimiter string or `None`.
    // port: PeepholeSubstituteAlternateSyntax#pickDelimiter
    fn pick_delimiter(strings: &[JsString]) -> Option<&'static str> {
        let mut all_length1 = true;
        for s in strings {
            if s.length() != 1 {
                all_length1 = false;
                break;
            }
        }

        if all_length1 {
            return Some("");
        }

        let delimiters: [Option<&'static str>; 6] =
            [Some(" "), Some(";"), Some(","), Some("{"), Some("}"), None];
        let mut i = 0;
        'next_delimiter: while let Some(delimiter) = delimiters[i] {
            let delimiter = JsString::from(delimiter);
            for cur in strings {
                if cur.index_of(&delimiter) >= 0 {
                    i += 1;
                    continue 'next_delimiter;
                }
            }
            break;
        }
        delimiters[i]
    }
}

impl AbstractPeepholeOptimization for PeepholeSubstituteAlternateSyntax {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeSubstituteAlternateSyntax"
    }

    /// Tries apply our various peephole minimizations on the passed in node.
    // port: PeepholeSubstituteAlternateSyntax#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> Option<NodeId> {
        Some(match node.get_token(compiler) {
            Token::ASSIGN_SUB => self.reduce_substraction_assignment(compiler, node),
            Token::TRUE | Token::FALSE => self.reduce_true_false(compiler, node),
            Token::NEW => {
                let result = self.try_fold_standard_constructors(compiler, node);
                if !result.is_call(compiler) {
                    result
                } else {
                    // tryFoldStandardConstructors() may convert a NEW node into a CALL node
                    self.try_reduce_call(compiler, result)
                }
            }
            Token::CALL => self.try_reduce_call(compiler, node),
            Token::RETURN => self.try_reduce_return(compiler, node),
            Token::EXPR_RESULT => {
                if node.get_first_child(compiler).unwrap().is_comma(compiler) {
                    self.try_split_comma(compiler, node)
                } else {
                    node
                }
            }
            Token::NAME => self.try_replace_undefined(compiler, node),
            Token::ARRAYLIT => self.try_minimize_array_literal(compiler, node),
            Token::GETPROP => self.try_minimize_window_refs(compiler, node),
            Token::TEMPLATELIT => self.try_turn_template_strings_to_strings(compiler, node),
            Token::AND
            | Token::OR
            | Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::COALESCE => self.try_rotate_associative_operator(compiler, node),
            Token::MUL => {
                if self.may_have_side_effects(compiler, node) {
                    node
                } else {
                    self.try_rotate_commutative_operator(compiler, node)
                }
            }
            _ => node,
        })
    }
}
