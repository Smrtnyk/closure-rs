/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeReplaceKnownMethods.java.

//! Port of `PeepholeReplaceKnownMethods.java`.
//!
//! Just to fold known methods when they are called with constants.

use crate::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::{
        AbstractPeepholeOptimization, AbstractPeepholeOptimizationFields,
    },
    inline_cost_estimator::InlineCostEstimator,
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    java_lang::{self, big_integer::BigIntegerParseError, math, regex::Pattern},
    js_string::JsString,
    jscomp_base::js_comp_doubles::JSCompDoubles,
    jscomp_colors::standard_colors,
    node::{Ast, NodeId},
    token::Token,
};

pub struct PeepholeReplaceKnownMethods {
    fields: AbstractPeepholeOptimizationFields,
    late: bool,
    use_types: bool,
}

/// Java's anonymous `ConcatFunctionCall` subclass.
#[derive(Clone, Copy)]
struct ConcatFunctionCall {
    call_node: NodeId,
    callee_node: NodeId,
    first_argument_node: Option<NodeId>,
}

impl ConcatFunctionCall {
    // port: PeepholeReplaceKnownMethods.ConcatFunctionCall#ConcatFunctionCall
    fn new(call_node: NodeId, callee_node: NodeId, first_argument_node: Option<NodeId>) -> Self {
        Self {
            call_node,
            callee_node,
            first_argument_node,
        }
    }
}

impl PeepholeReplaceKnownMethods {
    /// `late` When late is true, this mean we are currently running after most of the other
    /// optimizations. In this case we avoid changes that make the code larger (but otherwise
    /// easier to analyze - such as using string splitting).
    // port: PeepholeReplaceKnownMethods#PeepholeReplaceKnownMethods
    pub fn new(late: bool, use_types: bool) -> Self {
        Self {
            fields: AbstractPeepholeOptimizationFields::new(),
            late,
            use_types,
        }
    }

    // port: PeepholeReplaceKnownMethods#tryFoldKnownMethods
    fn try_fold_known_methods(&self, compiler: &mut AbstractCompiler, subtree: NodeId) -> NodeId {
        // For now we only support string methods .join(),
        // .indexOf(), .substring() and .substr()
        // array method concat()
        // and numeric methods parseInt() and parseFloat().

        check_argument!(subtree.is_call(compiler), "%s", subtree.to_string(compiler));
        let mut subtree = self.try_fold_array_join(compiler, subtree);
        // tryFoldArrayJoin may return a string literal instead of a CALL node
        if subtree.is_call(compiler) {
            subtree = self.try_to_fold_array_concat(compiler, subtree);
            check_state!(subtree.is_call(compiler), "%s", subtree.to_string(compiler));
            let call_target = subtree.get_first_child(compiler);
            check_not_null!(call_target);
            let call_target = call_target.unwrap();

            if call_target.is_get_prop(compiler) {
                if self.is_ast_normalized(compiler)
                    && call_target
                        .get_first_child(compiler)
                        .unwrap()
                        .is_qualified_name(compiler)
                {
                    let qualified_name = call_target
                        .get_first_child(compiler)
                        .unwrap()
                        .get_qualified_name(compiler)
                        .unwrap();
                    match qualified_name.to_string_lossy().as_str() {
                        "Array" => {
                            return self.try_fold_known_array_methods(
                                compiler,
                                subtree,
                                call_target,
                            );
                        }
                        "Math" => {
                            return self.try_fold_known_math_methods(
                                compiler,
                                subtree,
                                call_target,
                            );
                        }
                        "Number" => {
                            return self.try_fold_known_number_methods(
                                compiler,
                                subtree,
                                call_target,
                            );
                        }
                        _ => {}
                    }
                }
                subtree = self.try_fold_known_string_methods(compiler, subtree, call_target);
            } else if call_target.is_name(compiler) {
                subtree = self.try_fold_known_numeric_methods(compiler, subtree, call_target);
            }
        }

        subtree
    }

    /// Tries to evaluate a method on the Array object
    // port: PeepholeReplaceKnownMethods#tryFoldKnownArrayMethods
    fn try_fold_known_array_methods(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        call_target: NodeId,
    ) -> NodeId {
        check_argument!(subtree.is_call(compiler) && call_target.is_get_prop(compiler));

        // Method node might not be a string if callTarget is a GETELEM.
        // e.g. Array[something]()
        if call_target.get_string_ref(compiler) != "of" {
            return subtree;
        }

        subtree.remove_first_child(compiler);

        let arraylit = compiler.new_node(Token::ARRAYLIT);
        let children = subtree.remove_children(compiler);
        arraylit.add_children_to_back(compiler, children);
        subtree.replace_with(compiler, arraylit);
        self.report_change_to_enclosing_scope(compiler, arraylit);
        arraylit
    }

    // port: PeepholeReplaceKnownMethods#tryFoldKnownNumberMethods
    fn try_fold_known_number_methods(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        call_target: NodeId,
    ) -> NodeId {
        check_argument!(subtree.is_call(compiler) && call_target.is_get_prop(compiler));
        let method_name = call_target.get_string(compiler);
        let method_name_str = method_name.to_string_lossy();
        match method_name_str.as_str() {
            "parseInt" | "parseFloat" => {
                let first_arg = call_target.get_next(compiler);
                // Java dereferences a null firstArg in isNumericLiteral (NullPointerException).
                if first_arg.is_some_and(|a| a.is_string_lit(compiler))
                    || Self::is_numeric_literal(compiler, first_arg.unwrap())
                {
                    return self.try_fold_parse_number(
                        compiler,
                        subtree,
                        &method_name,
                        first_arg.unwrap(),
                    );
                }
                return subtree;
            }
            _ => {}
        }

        // Get the only number arg as a double as long as it is the only argument
        let value_node = call_target.get_next(compiler);
        let Some(value_node) = value_node.filter(|v| v.get_next(compiler).is_none()) else {
            return subtree;
        };
        let only_arg = self.get_side_effect_free_number_value_no_conversion(compiler, value_node);
        let mut replacement: Option<NodeId> = None;
        if let Some(only_arg) = only_arg {
            match method_name_str.as_str() {
                "isFinite" => {
                    replacement = Some(NodeUtil::boolean_node(compiler, only_arg.is_finite()))
                }
                "isNaN" => replacement = Some(NodeUtil::boolean_node(compiler, only_arg.is_nan())),
                "isSafeInteger" => {
                    replacement = Some(NodeUtil::boolean_node(
                        compiler,
                        only_arg < math::pow(2.0, 53.0)
                            && only_arg >= -(math::pow(2.0, 53.0) - 1.0)
                            && (only_arg as i64) as f64 == only_arg,
                    ))
                }
                _ => {}
            }
        } else if !self.may_have_side_effects(compiler, value_node)
            && (NodeUtil::is_immutable_value(compiler, value_node)
                || NodeUtil::is_literal_value(compiler, value_node, false))
        {
            match method_name_str.as_str() {
                "isFinite" | "isNaN" | "isSafeInteger" => {
                    replacement = Some(NodeUtil::boolean_node(compiler, false))
                }
                _ => {}
            }
        }
        if let Some(replacement) = replacement {
            subtree.replace_with(compiler, replacement);
            self.report_change_to_enclosing_scope(compiler, replacement);
            return replacement;
        }
        subtree
    }

    /// Tries to evaluate a method on the Math object
    // port: PeepholeReplaceKnownMethods#tryFoldKnownMathMethods
    fn try_fold_known_math_methods(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        call_target: NodeId,
    ) -> NodeId {
        check_argument!(subtree.is_call(compiler) && call_target.is_get_prop(compiler));

        // first collect the arguments, if they are all numbers then we proceed
        let mut args: Vec<f64> = Vec::new();
        let mut arg = call_target.get_next(compiler);
        while let Some(a) = arg {
            let d = self.get_side_effect_free_number_value(compiler, a);
            if let Some(d) = d {
                args.push(d);
            } else {
                return subtree;
            }
            arg = a.get_next(compiler);
        }
        let mut replacement: Option<f64> = None;
        let method_name = call_target.get_string(compiler).to_string_lossy();
        // NOTE: the standard does not define precision for these methods, but we are
        // conservative, so for now we only implement the methods that are guaranteed to not
        // increase the size of the numeric constants.
        if args.len() == 1 {
            let arg = args[0];
            match method_name.as_str() {
                "abs" => replacement = Some(arg.abs()),
                "ceil" => replacement = Some(arg.ceil()),
                "floor" => replacement = Some(arg.floor()),
                "fround" => {
                    if arg.is_nan() || arg.is_infinite() || arg == 0.0 {
                        replacement = Some(arg);
                        // if the double is exactly representable as a float, then just cast
                        // since no rounding is involved
                    } else if f64::from(arg as f32) == arg {
                        // TODO(b/155511629): This condition is always true after J2CL
                        // transpilation.
                        replacement = Some(f64::from(arg as f32));
                    } else {
                        // (float) arg does not necessarily use the correct rounding mode, so
                        // don't do anything
                        replacement = None;
                    }
                }
                "round" => {
                    if arg.is_nan() || arg.is_infinite() {
                        replacement = Some(arg);
                    } else {
                        replacement = Some(math::round(arg) as f64);
                    }
                }
                "sign" => replacement = Some(math::signum(arg)),
                "trunc" => {
                    if arg.is_nan() || arg.is_infinite() {
                        replacement = Some(arg);
                    } else {
                        replacement = Some(math::signum(arg) * arg.abs().floor());
                    }
                }
                "clz32" => {
                    replacement = Some(f64::from(
                        (JSCompDoubles::ecmascript_to_uint32(arg) as u32).leading_zeros(),
                    ))
                }
                _ => {}
            }
        }
        // handle the variadic functions now if we haven't already
        // For each of these we could allow for some of the values to be unknown and either
        // reduce to NaN or simplify the existing args. e.g. Math.max(3, x, 2) -> Math.max(3, x)
        if replacement.is_none() {
            match method_name.as_str() {
                "max" => {
                    let mut result = f64::NEG_INFINITY;
                    for &d in &args {
                        result = math::max(result, d);
                    }
                    replacement = Some(result);
                }
                "min" => {
                    let mut result = f64::INFINITY;
                    for &d in &args {
                        result = math::min(result, d);
                    }
                    replacement = Some(result);
                }
                "imul" => {
                    if args.len() < 2 {
                        replacement = Some(0.0);
                    } else {
                        // Ignore args3+
                        replacement = Some(f64::from(
                            JSCompDoubles::ecmascript_to_int32(args[0])
                                .wrapping_mul(JSCompDoubles::ecmascript_to_int32(args[1])),
                        ));
                    }
                }
                "pow" if args.len() == 2 => {
                    replacement = Some(math::pow(args[0], args[1]));
                }
                _ => {}
            }
        }

        if let Some(replacement) = replacement {
            let number_node = NodeUtil::number_node(compiler, replacement, Some(subtree));
            subtree.replace_with(compiler, number_node);
            self.report_change_to_enclosing_scope(compiler, number_node);
            return number_node;
        }
        subtree
    }

    /// Try to evaluate known String methods .indexOf(), .substr(), .substring()
    // port: PeepholeReplaceKnownMethods#tryFoldKnownStringMethods
    fn try_fold_known_string_methods(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        call_target: NodeId,
    ) -> NodeId {
        check_argument!(subtree.is_call(compiler) && call_target.is_get_prop(compiler));

        // check if this is a call on a string method
        // then dispatch to specific folding method.
        let string_node = call_target.get_first_child(compiler).unwrap();

        let is_string_literal = string_node.is_string_lit(compiler);
        let function_name_string = call_target.get_string(compiler);
        let function_name = function_name_string.to_string_lossy();
        let first_arg = call_target.get_next(compiler);
        if is_string_literal {
            if function_name == "split" {
                return self.try_fold_string_split(compiler, subtree, string_node, first_arg);
            } else if let Some(first_arg) = first_arg {
                if NodeUtil::is_immutable_value(compiler, first_arg) {
                    match function_name.as_str() {
                        "indexOf" | "lastIndexOf" => {
                            return self.try_fold_string_index_of(
                                compiler,
                                subtree,
                                &function_name_string,
                                string_node,
                                first_arg,
                            );
                        }
                        "substr" => {
                            return self.try_fold_string_substr(
                                compiler,
                                subtree,
                                string_node,
                                first_arg,
                            );
                        }
                        "substring" | "slice" => {
                            return self.try_fold_string_substring_or_slice(
                                compiler,
                                subtree,
                                string_node,
                                first_arg,
                            );
                        }
                        "charAt" => {
                            return self.try_fold_string_char_at(
                                compiler,
                                subtree,
                                string_node,
                                Some(first_arg),
                            );
                        }
                        "charCodeAt" => {
                            return self.try_fold_string_char_code_at(
                                compiler,
                                subtree,
                                string_node,
                                Some(first_arg),
                            );
                        }
                        "replace" => {
                            return self.try_fold_string_replace(
                                compiler,
                                subtree,
                                string_node,
                                first_arg,
                            );
                        }
                        "replaceAll" => {
                            return self.try_fold_string_replace_all(
                                compiler,
                                subtree,
                                string_node,
                                first_arg,
                            );
                        }
                        _ => {}
                    }
                }
            } else {
                match function_name.as_str() {
                    "toLowerCase" => {
                        return self.try_fold_string_to_lower_case(compiler, subtree, string_node);
                    }
                    "toUpperCase" => {
                        return self.try_fold_string_to_upper_case(compiler, subtree, string_node);
                    }
                    "trim" => {
                        return self.try_fold_string_trim(compiler, subtree, string_node);
                    }
                    _ => {}
                }
            }
        }
        if self.use_types
            && let Some(first_arg) = first_arg
            && (is_string_literal
                || string_node
                    .get_color(compiler)
                    .is_some_and(|c| c == *standard_colors::STRING))
            && subtree.has_x_children(compiler, 3)
        {
            let maybe_start = self.get_side_effect_free_number_value(compiler, first_arg);
            if let Some(maybe_start) = maybe_start {
                let start = maybe_start as i32;
                let second_arg = first_arg.get_next(compiler).unwrap();
                let maybe_length_or_end =
                    self.get_side_effect_free_number_value(compiler, second_arg);
                if let Some(maybe_length_or_end) = maybe_length_or_end {
                    match function_name.as_str() {
                        "substr" => {
                            let length = maybe_length_or_end as i32;
                            if start >= 0 && length == 1 {
                                return self.replace_with_char_at(
                                    compiler,
                                    subtree,
                                    call_target,
                                    first_arg,
                                );
                            }
                        }
                        "substring" | "slice" => {
                            let end = maybe_length_or_end as i32;
                            // unlike slice and substring, chatAt can not be used with negative
                            // indexes
                            if start >= 0 && end.wrapping_sub(start) == 1 {
                                return self.replace_with_char_at(
                                    compiler,
                                    subtree,
                                    call_target,
                                    first_arg,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        subtree
    }

    /// Try to evaluate known Numeric methods parseInt(), parseFloat()
    // port: PeepholeReplaceKnownMethods#tryFoldKnownNumericMethods
    fn try_fold_known_numeric_methods(
        &self,
        compiler: &mut AbstractCompiler,
        mut subtree: NodeId,
        call_target: NodeId,
    ) -> NodeId {
        check_argument!(subtree.is_call(compiler));

        if self.is_ast_normalized(compiler) {
            // check if this is a call on a string method
            // then dispatch to specific folding method.
            let function_name_string = call_target.get_string(compiler);
            let first_argument = call_target.get_next(compiler);
            if let Some(first_argument) = first_argument
                && (first_argument.is_string_lit(compiler)
                    || Self::is_numeric_literal(compiler, first_argument))
                && (function_name_string == "parseInt" || function_name_string == "parseFloat")
            {
                subtree = self.try_fold_parse_number(
                    compiler,
                    subtree,
                    &function_name_string,
                    first_argument,
                );
            }
        }
        subtree
    }

    /// Returns true both for number literals and their negations (e.g. `-12.3`).
    // port: PeepholeReplaceKnownMethods#isNumericLiteral
    fn is_numeric_literal(ast: &Ast, n: NodeId) -> bool {
        n.is_number(ast) || (n.is_neg(ast) && n.get_only_child(ast).is_number(ast))
    }

    /// Returns The lowered string Node.
    ///
    /// This method is believed to be correct independent of the locale of the compiler and the
    /// JSVM executing the compiled code, assuming both are implementations of Unicode are
    /// correct.
    // port: PeepholeReplaceKnownMethods#tryFoldStringToLowerCase
    fn try_fold_string_to_lower_case(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        string_node: NodeId,
    ) -> NodeId {
        let lowered = java_lang::string::to_lower_case_root(&string_node.get_string(compiler));
        let replacement = IR::string(compiler, lowered);
        subtree.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
        replacement
    }

    /// Returns The upped string Node.
    ///
    /// This method is believed to be correct independent of the locale of the compiler and the
    /// JSVM executing the compiled code, assuming both are implementations of Unicode are
    /// correct.
    // port: PeepholeReplaceKnownMethods#tryFoldStringToUpperCase
    fn try_fold_string_to_upper_case(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        string_node: NodeId,
    ) -> NodeId {
        let upped = java_lang::string::to_upper_case_root(&string_node.get_string(compiler));
        let replacement = IR::string(compiler, upped);
        subtree.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
        replacement
    }

    /// Returns The trimmed string Node.
    // port: PeepholeReplaceKnownMethods#tryFoldStringTrim
    fn try_fold_string_trim(
        &self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
        string_node: NodeId,
    ) -> NodeId {
        // See ECMA 15.5.4.20, 7.2, and 7.3
        // All Unicode 10.0 whitespace + BOM
        let whitespace = "[ \t\n-\r\\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000\\uFEFF]+";
        let trimmed = Self::replace_all(
            &string_node.get_string(compiler),
            &format!("^{whitespace}|{whitespace}$"),
            &JsString::from(""),
        );
        let replacement = IR::string(compiler, trimmed);
        subtree.replace_with(compiler, replacement);
        self.report_change_to_enclosing_scope(compiler, replacement);
        replacement
    }

    /// Rust-only: Java's `String#replaceAll(String regex, String replacement)` for a replacement
    /// without `$` or `\` (Matcher#replaceAll's find/append loop).
    fn replace_all(s: &JsString, regex: &str, replacement: &JsString) -> JsString {
        let pattern = Pattern::compile(regex);
        let mut matcher = pattern.matcher(s.clone());
        let units = s.as_units();
        let mut result: Vec<u16> = Vec::new();
        let mut last_append_position = 0;
        while matcher.find() {
            result.extend_from_slice(&units[last_append_position..matcher.start()]);
            result.extend_from_slice(replacement.as_units());
            last_append_position = matcher.end();
        }
        result.extend_from_slice(&units[last_append_position..]);
        JsString::from_units(result)
    }

    /// `input` string representation of a number. Returns string with leading and trailing
    /// zeros removed
    // port: PeepholeReplaceKnownMethods#normalizeNumericString
    fn normalize_numeric_string(input: &JsString) -> JsString {
        if input.is_empty() {
            return input.clone();
        }

        let mut start_index: i32 = 0;
        let mut end_index: i32 = input.length() as i32 - 1;

        // Remove leading zeros
        while start_index < input.length() as i32
            && input.char_at(start_index as usize) == u16::from(b'0')
            && input.char_at(start_index as usize) != u16::from(b'.')
        {
            start_index += 1;
        }

        // Remove trailing zeros only after the decimal
        if input.index_of_char(u16::from(b'.')) >= 0 {
            while end_index >= 0 && input.char_at(end_index as usize) == u16::from(b'0') {
                end_index -= 1;
            }
            if input.char_at(end_index as usize) == u16::from(b'.') {
                end_index -= 1;
            }
        }
        if start_index >= end_index {
            return input.clone();
        }

        input.substring(start_index as usize, (end_index + 1) as usize)
    }

    /// Try to evaluate parseInt, parseFloat:
    ///
    /// ```text
    ///     parseInt("1") -> 1
    ///     parseInt("1", 10) -> 1
    ///     parseFloat("1.11") -> 1.11
    /// ```
    // port: PeepholeReplaceKnownMethods#tryFoldParseNumber
    fn try_fold_parse_number(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_name: &JsString,
        first_arg: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));

        let is_parse_int = function_name == "parseInt";
        let second_arg = first_arg.get_next(compiler);

        // Second argument is only used as the radix for parseInt
        let mut radix: i32 = 0;
        if let Some(second_arg) = second_arg {
            if !is_parse_int {
                return n;
            }

            // Third-argument and non-numeric second arg are problematic. Discard.
            if second_arg.get_next(compiler).is_some() || !second_arg.is_number(compiler) {
                return n;
            } else {
                let tmp_radix = second_arg.get_double(compiler);
                if tmp_radix != f64::from(tmp_radix as i32) {
                    return n;
                }
                radix = tmp_radix as i32;
                if radix < 0 || radix == 1 || radix > 36 {
                    return n;
                }
            }
        }

        if !is_parse_int {
            // parseFloat logic
            let mut string_val: JsString;
            if Self::is_numeric_literal(compiler, first_arg) {
                let check_val = self.get_side_effect_free_number_value(compiler, first_arg);
                let numeric_node = NodeUtil::number_node(compiler, check_val.unwrap(), Some(n));
                n.replace_with(compiler, numeric_node);
                self.report_change_to_enclosing_scope(compiler, numeric_node);
                return numeric_node;
            } else {
                let Some(value) = self.get_side_effect_free_string_value(compiler, first_arg)
                else {
                    return n;
                };
                string_val = value;
                let check_val = NodeUtil::get_string_number_value(&string_val);
                if check_val.is_none() {
                    return n;
                }
                string_val = NodeUtil::trim_js_white_space(&string_val);
                if string_val.is_empty() {
                    return n;
                }
                if string_val == "0" {
                    let new_node = IR::number(compiler, 0.0);
                    n.replace_with(compiler, new_node);
                    self.report_change_to_enclosing_scope(compiler, new_node);
                    return new_node;
                }
                let normalized_new_val;
                match java_lang::parse_double(&string_val) {
                    Ok(new_val) => {
                        let new_node = NodeUtil::number_node(compiler, new_val, Some(n));
                        normalized_new_val = Self::normalize_numeric_string(&JsString::from(
                            java_lang::double_to_string(new_val).as_str(),
                        ));
                        if Self::normalize_numeric_string(&string_val) != normalized_new_val {
                            return n;
                        }
                        n.replace_with(compiler, new_node);
                        self.report_change_to_enclosing_scope(compiler, new_node);
                        return new_node;
                    }
                    Err(_) => {
                        return n;
                    }
                }
            }
        }

        // parseInt logic following ECMA-262 semantics
        let Some(mut string_val) = self.get_side_effect_free_string_value(compiler, first_arg)
        else {
            return n;
        };

        string_val = NodeUtil::trim_js_white_space(&string_val);
        if string_val.is_empty() {
            return n;
        }

        let mut is_negative = false;
        if string_val.starts_with(&JsString::from("-")) {
            is_negative = true;
            string_val = string_val.substring_from(1);
        } else if string_val.starts_with(&JsString::from("+")) {
            string_val = string_val.substring_from(1);
        }

        if radix == 0 || radix == 16 {
            if string_val.length() > 1
                && Self::ascii_equals_ignore_case(&string_val.substring(0, 2), "0x")
            {
                radix = 16;
                string_val = string_val.substring_from(2);
            } else if radix == 0 {
                if !self.is_ecma_script5_or_greater(compiler)
                    && string_val.starts_with(&JsString::from("0"))
                {
                    return n;
                }
                radix = 10;
            }
        }

        let mut end_digit_index = 0;
        while end_digit_index < string_val.length() {
            let c = string_val.char_at(end_digit_index);
            if Self::get_radix_digit(c, radix) < 0 {
                break;
            }
            end_digit_index += 1;
        }

        if end_digit_index == 0 {
            return n;
        }

        let digits = string_val.substring(0, end_digit_index);
        let mut new_val = match java_lang::parse_big_integer(&digits, radix) {
            Ok(bi) => java_lang::big_integer::double_value(&bi),
            Err(BigIntegerParseError::NumberFormatException(_)) => {
                return n;
            }
            Err(e) => panic!("{}: {}", e.class(), e.message()),
        };

        if is_negative {
            new_val = -new_val;
        }

        let new_node = NodeUtil::number_node(compiler, new_val, Some(n));
        n.replace_with(compiler, new_node);
        self.report_change_to_enclosing_scope(compiler, new_node);
        new_node
    }

    /// Rust-only: Guava's `Ascii.equalsIgnoreCase(CharSequence, CharSequence)`.
    fn ascii_equals_ignore_case(s1: &JsString, s2: &str) -> bool {
        let s2 = JsString::from(s2);
        let length = s1.length();
        if length != s2.length() {
            return false;
        }
        // Guava's Ascii#getAlphaIndex: (char) ((c | 0x20) - 'a')
        let get_alpha_index = |c: u16| (c | 0x20).wrapping_sub(u16::from(b'a'));
        for i in 0..length {
            let c1 = s1.char_at(i);
            let c2 = s2.char_at(i);
            if c1 == c2 {
                continue;
            }
            let alpha_index = get_alpha_index(c1);
            if alpha_index < 26 && alpha_index == get_alpha_index(c2) {
                continue;
            }
            return false;
        }
        true
    }

    // port: PeepholeReplaceKnownMethods#getRadixDigit
    fn get_radix_digit(c: u16, radix: i32) -> i32 {
        if c >= u16::from(b'0') && c <= u16::from(b'9') {
            let d = i32::from(c) - i32::from(b'0');
            return if d < radix { d } else { -1 };
        }
        if c >= u16::from(b'a') && c <= u16::from(b'z') {
            let d = i32::from(c) - i32::from(b'a') + 10;
            return if d < radix { d } else { -1 };
        }
        if c >= u16::from(b'A') && c <= u16::from(b'Z') {
            let d = i32::from(c) - i32::from(b'A') + 10;
            return if d < radix { d } else { -1 };
        }
        -1
    }

    /// Rust-only: Java's `String#lastIndexOf(String, int)`.
    fn last_index_of_from(s: &JsString, target: &JsString, mut from_index: i32) -> i32 {
        let right_index = s.length() as i32 - target.length() as i32;
        if from_index > right_index {
            from_index = right_index;
        }
        if from_index < 0 {
            return -1;
        }
        let units = s.as_units();
        let target_units = target.as_units();
        let mut i = from_index;
        while i >= 0 {
            let start = i as usize;
            if &units[start..start + target_units.len()] == target_units {
                return i;
            }
            i -= 1;
        }
        -1
    }

    /// Try to evaluate String.indexOf/lastIndexOf:
    ///
    /// ```text
    ///     "abcdef".indexOf("bc") -> 1
    ///     "abcdefbc".indexOf("bc", 3) -> 6
    /// ```
    // port: PeepholeReplaceKnownMethods#tryFoldStringIndexOf
    fn try_fold_string_index_of(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        function_name: &JsString,
        lstring_node: NodeId,
        first_arg: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(lstring_node.is_string_lit(compiler));

        let lstring = lstring_node.get_string(compiler);
        let is_index_of = function_name == "indexOf";
        let second_arg = first_arg.get_next(compiler);
        let search_value = self.get_side_effect_free_string_value(compiler, first_arg);
        // searchValue must be a valid string.
        let Some(search_value) = search_value else {
            return n;
        };
        let mut from_index: i32 = if is_index_of {
            0
        } else {
            lstring.length() as i32
        };
        if let Some(second_arg) = second_arg {
            // Third-argument and non-numeric second arg are problematic. Discard.
            if second_arg.get_next(compiler).is_some() || !second_arg.is_number(compiler) {
                return n;
            } else {
                from_index = second_arg.get_double(compiler) as i32;
            }
        }
        let index_val = if is_index_of {
            lstring.index_of_from(&search_value, from_index)
        } else {
            Self::last_index_of_from(&lstring, &search_value, from_index)
        };
        let new_node = NodeUtil::number_node(compiler, f64::from(index_val), Some(n));
        n.replace_with(compiler, new_node);
        self.report_change_to_enclosing_scope(compiler, new_node);

        new_node
    }

    /// Try to fold an array join: ['a', 'b', 'c'].join('') -> 'abc';
    // port: PeepholeReplaceKnownMethods#tryFoldArrayJoin
    fn try_fold_array_join(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_state!(n.is_call(compiler), "%s", n.to_string(compiler));
        let call_target = n.get_first_child(compiler);

        let Some(call_target) = call_target.filter(|c| c.is_get_prop(compiler)) else {
            return n;
        };

        let right = call_target.get_next(compiler);
        if let Some(right) = right
            && (right.get_next(compiler).is_some()
                || !NodeUtil::is_immutable_value(compiler, right))
        {
            return n;
        }

        let array_node = call_target.get_first_child(compiler).unwrap();

        if !array_node.is_array_lit(compiler) || call_target.get_string_ref(compiler) != "join" {
            return n;
        }

        if let Some(right) = right
            && (NodeUtil::is_undefined(compiler, right)
                || (right.is_string_lit(compiler) && right.get_string_ref(compiler) == ","))
        {
            // "," is the default, it doesn't need to be explicit
            right.detach(compiler);
            self.report_change_to_enclosing_scope(compiler, n);
        }

        // logic above ensures that `right` is immutable, so no need to check for
        // side effects with getSideEffectFreeStringValue(right)
        let join_string: JsString = match right {
            Some(right) if !NodeUtil::is_undefined(compiler, right) => {
                NodeUtil::get_string_value(compiler, right).unwrap()
            }
            _ => JsString::from(","),
        };
        let mut array_folded_children: Vec<NodeId> = Vec::new();
        let mut sb: Option<Vec<u16>> = None;
        let mut folded_size: i32 = 0;
        let mut prev: Option<NodeId> = None;
        let mut elem = array_node.get_first_child(compiler);
        // Merges adjacent String nodes.
        while let Some(e) = elem {
            if NodeUtil::is_immutable_value(compiler, e) || e.is_empty(compiler) {
                match sb.as_mut() {
                    None => sb = Some(Vec::new()),
                    Some(sb) => sb.extend_from_slice(join_string.as_units()),
                }
                let element_str = NodeUtil::get_array_element_string_value(compiler, e);
                let Some(element_str) = element_str else {
                    return n; // TODO(nickreid): Is this ever null?
                };
                sb.as_mut()
                    .unwrap()
                    .extend_from_slice(element_str.as_units());
            } else {
                if let Some(s) = sb.take() {
                    check_not_null!(prev);
                    // + 2 for the quotes.
                    folded_size += s.len() as i32 + 2;
                    let string = IR::string(compiler, JsString::from_units(s))
                        .srcref_if_missing(compiler, prev.unwrap());
                    array_folded_children.push(string);
                }
                folded_size += InlineCostEstimator::get_cost(compiler, e);
                array_folded_children.push(e);
            }
            prev = Some(e);
            elem = e.get_next(compiler);
        }

        if let Some(s) = sb.take() {
            check_not_null!(prev);
            // + 2 for the quotes.
            folded_size += s.len() as i32 + 2;
            let string = IR::string(compiler, JsString::from_units(s))
                .srcref_if_missing(compiler, prev.unwrap());
            array_folded_children.push(string);
        }
        // one for each comma.
        folded_size += array_folded_children.len() as i32 - 1;

        let original_size = InlineCostEstimator::get_cost(compiler, n);
        match array_folded_children.len() {
            0 => {
                let empty_string_node = IR::string(compiler, "");
                n.replace_with(compiler, empty_string_node);
                self.report_change_to_enclosing_scope(compiler, empty_string_node);
                return empty_string_node;
            }
            1 => {
                let folded_string_node = array_folded_children.remove(0);
                // The spread isn't valid outside any array literal (or would change meaning)
                // so don't try to fold it.
                if folded_string_node.is_spread(compiler) || folded_size > original_size {
                    return n;
                }
                if folded_string_node.is_string_lit(compiler) {
                    array_node.detach_children(compiler);
                    n.replace_with(compiler, folded_string_node);
                    self.report_change_to_enclosing_scope(compiler, folded_string_node);
                    return folded_string_node;
                } else {
                    // Because of special case behavior for `null` and `undefined` values,
                    // there's no safe way to convert `[someNonStringValue].join()` to something
                    // shorter. e.g. String(someNonStringValue) would turn `null` into `"null"`,
                    // which isn't right.
                    return n;
                }
            }
            _ => {
                if array_node.has_x_children(compiler, array_folded_children.len() as i32) {
                    // No folding could actually be performed.
                    return n;
                }
                let k_join_overhead = "[].join()".len() as i32;
                folded_size += k_join_overhead;
                folded_size += match right {
                    Some(right) => InlineCostEstimator::get_cost(compiler, right),
                    None => 0,
                };
                if folded_size > original_size {
                    return n;
                }
                array_node.detach_children(compiler);
                for node in array_folded_children {
                    array_node.add_child_to_back(compiler, node);
                }
                self.report_change_to_enclosing_scope(compiler, array_node);
            }
        }

        n
    }

    /// Try to fold .substr() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringSubstr
    fn try_fold_string_substr(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let start: i32;
        let length: i32;
        let string_as_string = string_node.get_string(compiler);
        let string_length = string_as_string.length() as i32;

        let maybe_start = self.get_side_effect_free_number_value(compiler, arg1);
        if let Some(maybe_start) = maybe_start {
            start = maybe_start as i32;
        } else {
            return n;
        }

        let arg2 = arg1.get_next(compiler);
        if let Some(arg2) = arg2 {
            let maybe_length = self.get_side_effect_free_number_value(compiler, arg2);
            if let Some(maybe_length) = maybe_length {
                length = maybe_length as i32;
            } else {
                return n;
            }

            if arg2.get_next(compiler).is_some() {
                // If we got more args than we expected, bail out.
                return n;
            }
        } else {
            // parameter 2 not passed
            length = string_length.wrapping_sub(start);
        }

        // Don't handle these cases. The specification actually does
        // specify the behavior in some of these cases, but we haven't
        // done a thorough investigation that it is correctly implemented
        // in all browsers.
        if start.wrapping_add(length) > string_length || (length < 0) || (start < 0) {
            return n;
        }

        let result = string_as_string.substring(start as usize, (start + length) as usize);
        let result_node = IR::string(compiler, result);

        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    /// Try to fold .substring() or .slice() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringSubstringOrSlice
    fn try_fold_string_substring_or_slice(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let start: i32;
        let end: i32;
        let string_as_string = string_node.get_string(compiler);
        let string_length = string_as_string.length() as i32;

        let maybe_start = self.get_side_effect_free_number_value(compiler, arg1);
        if let Some(maybe_start) = maybe_start {
            start = maybe_start as i32;
        } else {
            return n;
        }

        let arg2 = arg1.get_next(compiler);
        if let Some(arg2) = arg2 {
            let maybe_end = self.get_side_effect_free_number_value(compiler, arg2);
            if let Some(maybe_end) = maybe_end {
                end = maybe_end as i32;
            } else {
                return n;
            }

            if arg2.get_next(compiler).is_some() {
                // If we got more args than we expected, bail out.
                return n;
            }
        } else {
            // parameter 2 not passed
            end = string_length;
        }

        // Don't handle these cases. The specification actually does
        // specify the behavior in some of these cases, but we haven't
        // done a thorough investigation that it is correctly implemented
        // in all browsers.
        if (end > string_length)
            || (start > string_length)
            || (start < 0)
            || (end < 0)
            || (start > end)
        {
            return n;
        }

        let result = string_as_string.substring(start as usize, end as usize);
        let result_node = IR::string(compiler, result);

        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    // port: PeepholeReplaceKnownMethods#replaceWithCharAt
    fn replace_with_char_at(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        call_target: NodeId,
        first_arg: NodeId,
    ) -> NodeId {
        // TODO(moz): Maybe correct the arity of the function type here.
        call_target.set_string(compiler, "charAt");
        first_arg.get_next(compiler).unwrap().detach(compiler);
        self.report_change_to_enclosing_scope(compiler, first_arg);
        n
    }

    /// Try to fold .charAt() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringCharAt
    fn try_fold_string_char_at(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: Option<NodeId>,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let index: i32;
        let string_as_string = string_node.get_string(compiler);

        if let Some(arg1) = arg1
            && arg1.is_number(compiler)
            && arg1.get_next(compiler).is_none()
        {
            index = arg1.get_double(compiler) as i32;
        } else {
            return n;
        }

        if index < 0 || string_as_string.length() as i32 <= index {
            // http://es5.github.com/#x15.5.4.4 says "" is returned when index is
            // out of bounds but we bail.
            return n;
        }

        let result = string_as_string.substring(index as usize, index as usize + 1);
        let result_node = IR::string(compiler, result);
        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    /// Try to fold .replace() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringReplace
    fn try_fold_string_replace(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let arg2 = arg1.get_next(compiler);
        let Some(arg2) = arg2.filter(|a| a.get_next(compiler).is_none()) else {
            // too few or too many parameters
            return n;
        };

        if !arg1.is_string_lit(compiler) || !arg2.is_string_lit(compiler) {
            // only string literals are supported for folding.
            return n;
        }

        let look_for_pattern = arg1.get_string(compiler);
        let replacement_pattern = arg2.get_string(compiler);
        if replacement_pattern.index_of(&JsString::from("$")) >= 0 {
            // 'special' replacements aren't supported yet.
            return n;
        }

        let original = string_node.get_string(compiler);

        let index = original.index_of(&look_for_pattern);
        if index == -1 {
            return n;
        }

        // Java "replace" acts like JavaScript's "replaceAll" here we only want to replace the
        // first instance of the string
        let new_string = original
            .substring(0, index as usize)
            .concat(&replacement_pattern)
            .concat(&original.substring_from(index as usize + look_for_pattern.length()));

        let result_node = IR::string(compiler, new_string).srcref(compiler, string_node);
        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    /// Try to fold .replaceAll() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringReplaceAll
    fn try_fold_string_replace_all(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: NodeId,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let arg2 = arg1.get_next(compiler);
        let Some(arg2) = arg2.filter(|a| a.get_next(compiler).is_none()) else {
            // too few or too many parameters
            return n;
        };

        if !arg1.is_string_lit(compiler) || !arg2.is_string_lit(compiler) {
            // only string literals are supported for folding.
            return n;
        }

        let replacement_pattern = arg2.get_string(compiler);
        if replacement_pattern.index_of(&JsString::from("$")) >= 0 {
            // 'special' replacements aren't supported yet.
            return n;
        }

        // Java "replace" acts like JavaScript's "replaceAll" and replaces all occurrences.
        let original = string_node.get_string(compiler);
        let new_string = original.replace(&arg1.get_string(compiler), &replacement_pattern);

        let result_node = IR::string(compiler, new_string).srcref(compiler, string_node);
        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    /// Try to fold .charCodeAt() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringCharCodeAt
    fn try_fold_string_char_code_at(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: Option<NodeId>,
    ) -> NodeId {
        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let index: i32;
        let string_as_string = string_node.get_string(compiler);

        if let Some(arg1) = arg1
            && arg1.is_number(compiler)
            && arg1.get_next(compiler).is_none()
        {
            index = arg1.get_double(compiler) as i32;
        } else {
            return n;
        }

        if index < 0 || string_as_string.length() as i32 <= index {
            // http://es5.github.com/#x15.5.4.5 says NaN is returned when index is
            // out of bounds but we bail.
            return n;
        }

        let result_node = IR::number(
            compiler,
            f64::from(string_as_string.char_at(index as usize)),
        );
        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, result_node);
        self.report_change_to_enclosing_scope(compiler, parent);
        result_node
    }

    /// Support function for jsSplit, find the first occurrence of separator within stringValue
    /// starting at startIndex.
    // port: PeepholeReplaceKnownMethods#jsSplitMatch
    fn js_split_match(string_value: &JsString, start_index: i32, separator: &JsString) -> i32 {
        if start_index + separator.length() as i32 > string_value.length() as i32 {
            return -1;
        }

        let match_index = string_value.index_of_from(separator, start_index);

        if match_index < 0 {
            return -1;
        }

        match_index
    }

    /// Implement the JS String.split method using a string separator.
    // port: PeepholeReplaceKnownMethods#jsSplit
    fn js_split(
        &self,
        string_value: &JsString,
        separator: Option<&JsString>,
        limit: i32,
    ) -> Vec<JsString> {
        check_argument!(limit >= 0);

        // For limits of 0, return an empty array
        if limit == 0 {
            return Vec::new();
        }

        // If a separator is not specified, return the entire string as
        // the only element of an array.
        let Some(separator) = separator else {
            return vec![string_value.clone()];
        };

        let mut split_strings: Vec<JsString> = Vec::new();

        // If an empty string is specified for the separator, split apart each
        // character of the string.
        if separator.is_empty() {
            let mut i = 0;
            while i < string_value.length() as i32 && i < limit {
                split_strings.push(string_value.substring(i as usize, i as usize + 1));
                i += 1;
            }
        } else {
            let mut start_index: i32 = 0;
            loop {
                let match_index = Self::js_split_match(string_value, start_index, separator);
                if !(match_index >= 0 && (split_strings.len() as i32) < limit) {
                    break;
                }
                split_strings
                    .push(string_value.substring(start_index as usize, match_index as usize));

                start_index = match_index + separator.length() as i32;
            }

            if (split_strings.len() as i32) < limit {
                if start_index < string_value.length() as i32 {
                    split_strings.push(string_value.substring_from(start_index as usize));
                } else {
                    split_strings.push(JsString::from(""));
                }
            }
        }

        split_strings
    }

    /// Try to fold .split() calls on strings
    // port: PeepholeReplaceKnownMethods#tryFoldStringSplit
    fn try_fold_string_split(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        string_node: NodeId,
        arg1: Option<NodeId>,
    ) -> NodeId {
        if self.late {
            return n;
        }

        check_argument!(n.is_call(compiler));
        check_argument!(string_node.is_string_lit(compiler));

        let mut separator: Option<JsString> = None;
        let string_value = string_node.get_string(compiler);

        // Maximum number of possible splits
        let mut limit = string_value.length() as i32 + 1;

        if let Some(arg1) = arg1 {
            if arg1.is_string_lit(compiler) {
                separator = Some(arg1.get_string(compiler));
            } else if !arg1.is_null(compiler) {
                return n;
            }

            let arg2 = arg1.get_next(compiler);
            if let Some(arg2) = arg2 {
                if arg2.is_number(compiler) {
                    limit = std::cmp::min(arg2.get_double(compiler) as i32, limit);
                    if limit < 0 {
                        return n;
                    }
                } else {
                    return n;
                }
            }
        }

        // Split the string and convert the returned array into JS nodes
        let string_array = self.js_split(&string_value, separator.as_ref(), limit);
        let array_of_strings = IR::arraylit(compiler, &[]);
        for element in string_array {
            let string = IR::string(compiler, element).srcref(compiler, string_node);
            array_of_strings.add_child_to_back(compiler, string);
        }

        let parent = n.get_parent(compiler).unwrap();
        n.replace_with(compiler, array_of_strings);
        self.report_change_to_enclosing_scope(compiler, parent);
        array_of_strings
    }

    // port: PeepholeReplaceKnownMethods#tryToFoldArrayConcat
    fn try_to_fold_array_concat(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        check_argument!(n.is_call(compiler), "%s", n.to_string(compiler));

        if !self.is_ast_normalized(compiler) || !self.use_types {
            return n;
        }
        let concat_function_call = Self::create_concat_function_call_for_node(compiler, n);
        let Some(concat_function_call) = concat_function_call else {
            return n;
        };
        let concat_function_call =
            self.try_to_remove_array_literal_from_front_of_concat(compiler, concat_function_call);
        check_not_null!(concat_function_call);
        self.try_to_fold_concat_chaining(compiler, concat_function_call.unwrap())
    }

    /// Check if we have this code pattern `[].concat(exactlyArrayArgument,...*)` and if yes
    /// replace empty array literal from the front of concatenation by the first argument of
    /// concat function call `[].concat(arr,1)` -> `arr.concat(1)`.
    // port: PeepholeReplaceKnownMethods#tryToRemoveArrayLiteralFromFrontOfConcat
    fn try_to_remove_array_literal_from_front_of_concat(
        &self,
        compiler: &mut AbstractCompiler,
        concat_function_call: ConcatFunctionCall,
    ) -> Option<ConcatFunctionCall> {
        let call_node = concat_function_call.call_node;
        let array_literal_to_remove = concat_function_call.callee_node;
        if !array_literal_to_remove.is_array_lit(compiler)
            || array_literal_to_remove.has_children(compiler)
        {
            return Some(concat_function_call);
        }
        let first_arg = concat_function_call.first_argument_node;
        if !Self::contains_exactly_array(compiler, first_arg) {
            return Some(concat_function_call);
        }
        let first_arg = first_arg.unwrap();

        first_arg.detach(compiler);
        array_literal_to_remove.replace_with(compiler, first_arg);

        self.report_change_to_enclosing_scope(compiler, call_node);
        Self::create_concat_function_call_for_node(compiler, call_node)
    }

    /// Check if we have this code pattern `array.concat(...*).concat(sideEffectFreeArguments)`
    /// and if yes fold chained concat functions, so `arr.concat(a).concat(b)` will be fold into
    /// `arr.concat(a,b)`.
    // port: PeepholeReplaceKnownMethods#tryToFoldConcatChaining
    fn try_to_fold_concat_chaining(
        &self,
        compiler: &mut AbstractCompiler,
        concat_function_call: ConcatFunctionCall,
    ) -> NodeId {
        let concat_call_node = concat_function_call.call_node;

        let maybe_function_call = concat_function_call.callee_node;
        if !maybe_function_call.is_call(compiler) {
            return concat_call_node;
        }
        let previous_concat_function_call =
            Self::create_concat_function_call_for_node(compiler, maybe_function_call);
        let Some(previous_concat_function_call) = previous_concat_function_call else {
            return concat_call_node;
        };
        // make sure that arguments in second concat function call can't change the array
        // so we can fold chained concat functions
        // to clarify, consider this code
        // here we can't fold concatenation
        // var a = [];
        // a.concat(1).concat(a.push(1)); -> [1,1]
        // a.concat(1,a.push(1)); -> [1,1,1]
        let mut arg = concat_function_call.first_argument_node;
        while let Some(a) = arg {
            if self.may_have_side_effects(compiler, a) {
                return concat_call_node;
            }
            arg = a.get_next(compiler);
        }

        // perform folding
        let previous_concat_call_node = previous_concat_function_call.call_node;
        let mut arg = concat_function_call.first_argument_node;
        while let Some(current_arg) = arg {
            arg = current_arg.get_next(compiler);
            current_arg.detach(compiler);
            previous_concat_call_node.add_child_to_back(compiler, current_arg);
        }
        previous_concat_call_node.detach(compiler);
        concat_call_node.replace_with(compiler, previous_concat_call_node);
        self.report_change_to_enclosing_scope(compiler, previous_concat_call_node);
        previous_concat_call_node
    }

    /// If the argument node is a call to `Array.prototype.concat`, then return a
    /// `ConcatFunctionCall` object for it, otherwise return `None`.
    // port: PeepholeReplaceKnownMethods#createConcatFunctionCallForNode
    fn create_concat_function_call_for_node(ast: &Ast, n: NodeId) -> Option<ConcatFunctionCall> {
        check_argument!(n.is_call(ast), "%s", n.to_string(ast));
        let call_target = n.get_first_child(ast);
        check_not_null!(call_target);
        let call_target = call_target.unwrap();
        if !call_target.is_get_prop(ast) || call_target.get_string_ref(ast) != "concat" {
            return None;
        }
        let callee_node = call_target.get_first_child(ast);
        if !Self::contains_exactly_array(ast, callee_node) {
            return None;
        }
        let first_argument_node = n.get_second_child(ast);
        Some(ConcatFunctionCall::new(
            n,
            callee_node.unwrap(),
            first_argument_node,
        ))
    }

    /// Check if a node is a known array. Checks for array literals and nested .concat calls
    // port: PeepholeReplaceKnownMethods#containsExactlyArray
    fn contains_exactly_array(ast: &Ast, n: Option<NodeId>) -> bool {
        let Some(n) = n else {
            return false;
        };

        if n.is_array_lit(ast) {
            return true;
        }

        // Check for "[].concat(1)"
        if !n.is_call(ast) {
            return false;
        }
        let callee = n.get_first_child(ast).unwrap();
        callee.is_get_prop(ast)
            && callee.get_string_ref(ast) == "concat"
            && Self::contains_exactly_array(ast, callee.get_first_child(ast))
    }
}

impl AbstractPeepholeOptimization for PeepholeReplaceKnownMethods {
    fn fields(&self) -> &AbstractPeepholeOptimizationFields {
        &self.fields
    }

    fn fields_mut(&mut self) -> &mut AbstractPeepholeOptimizationFields {
        &mut self.fields
    }

    fn get_class_name(&self) -> &'static str {
        "com.google.javascript.jscomp.PeepholeReplaceKnownMethods"
    }

    // port: PeepholeReplaceKnownMethods#optimizeSubtree
    fn optimize_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        if NodeUtil::is_goog_weak_usage_call(compiler, subtree) && self.late {
            // goog.weakUsage(x) -> x.
            let name = subtree.get_second_child(compiler).unwrap().detach(compiler);
            subtree.replace_with(compiler, name);
            self.report_change_to_enclosing_scope(compiler, name);
            return Some(name);
        } else if subtree.is_call(compiler) {
            return Some(self.try_fold_known_methods(compiler, subtree));
        }
        Some(subtree)
    }
}
