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
//   src/com/google/javascript/jscomp/NodeUtil.java.

#![allow(clippy::match_like_matches_macro)] // Retain Java switch bodies.
use crate::{abstract_compiler::AbstractCompiler, scope::ScopeId};
use closure_jstype::js_type::JSType as _;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    dtoa::d_to_a,
    input_id::InputId,
    ir::IR,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jscomp_base::{js_comp_doubles::JSCompDoubles, tri::Tri},
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    static_source_file::StaticSourceFile,
    token::Token,
    token_util::TokenUtil,
};
use num_bigint::BigInt;
use std::sync::{Arc, LazyLock};
pub struct NodeUtil;
impl NodeUtil {
    pub const MAX_POSITIVE_INTEGER_NUMBER: i64 = (1i64 << 53) - 1;
    pub const JSC_PROPERTY_NAME_FN: &'static str = "JSCompiler_renameProperty";
    pub const LARGEST_BASIC_LATIN: u16 = 0x7f;
}
static GOOG_MODULE_DECLARE_LEGACY_NAMESPACE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.declareLegacyNamespace"));
static GOOG_SET_TEST_ONLY: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.setTestOnly"));
static GOOG_PROVIDE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.provide"));
static GOOG_MODULE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.module"));
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
static GOOG_REQUIRE_TYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
static GOOG_FORWARD_DECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));
static GOOG_REQUIRE_DYNAMIC: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireDynamic"));
static GOOG_WEAK_USAGE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.weakUsage"));
static GOOG_WEAK_USAGE_MANGLED: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog$weakUsage"));
impl NodeUtil {
    // port: NodeUtil#NodeUtil
    #[allow(dead_code)] // Java utility class has a private constructor.
    fn new() -> Self {
        Self
    }
    // port: NodeUtil#getBooleanValue
    pub fn get_boolean_value(ast: &Ast, n: NodeId) -> Tri {
        match n.get_token(ast) {
            Token::NULL | Token::FALSE | Token::VOID => Tri::FALSE,
            Token::TRUE
            | Token::REGEXP
            | Token::FUNCTION
            | Token::CLASS
            | Token::NEW
            | Token::ARRAYLIT
            | Token::OBJECTLIT => Tri::TRUE,
            Token::TEMPLATELIT => {
                if n.has_one_child(ast) {
                    let template_lit_string = n.get_only_child(ast);
                    check_state!(
                        template_lit_string.is_template_lit_string(ast),
                        "%s",
                        template_lit_string.to_string(ast)
                    );
                    Tri::for_boolean(
                        template_lit_string
                            .get_cooked_string(ast)
                            .is_some_and(|s| !s.is_empty()),
                    )
                } else {
                    Tri::UNKNOWN
                }
            }
            Token::STRINGLIT => Tri::for_boolean(n.get_string_ref(ast).length() > 0),
            Token::NUMBER => Tri::for_boolean(n.get_double(ast) != 0.0),
            Token::BIGINT => Tri::for_boolean(*n.get_big_int(ast) != BigInt::from(0)),
            Token::NOT => Self::get_boolean_value(ast, n.get_last_child(ast).unwrap()).not(),
            Token::NAME => match n.get_string(ast).to_string_lossy().as_str() {
                "undefined" | "NaN" => Tri::FALSE,
                "Infinity" => Tri::TRUE,
                _ => Tri::UNKNOWN,
            },
            Token::BITNOT | Token::POS | Token::NEG => {
                if let Some(double_val) = Self::get_number_value(ast, n) {
                    let is_falsey =
                        double_val.is_nan() || JSCompDoubles::is_either_zero(double_val);
                    return Tri::for_boolean(!is_falsey);
                }
                if let Some(bigint_val) = Self::get_big_int_value(ast, n) {
                    return Tri::for_boolean(bigint_val != BigInt::from(0));
                }
                Tri::UNKNOWN
            }
            Token::ASSIGN | Token::COMMA => {
                Self::get_boolean_value(ast, n.get_last_child(ast).unwrap())
            }
            Token::AND | Token::ASSIGN_AND => {
                Self::get_boolean_value(ast, n.get_first_child(ast).unwrap())
                    .and(Self::get_boolean_value(ast, n.get_last_child(ast).unwrap()))
            }
            Token::OR | Token::ASSIGN_OR => {
                Self::get_boolean_value(ast, n.get_first_child(ast).unwrap())
                    .or(Self::get_boolean_value(ast, n.get_last_child(ast).unwrap()))
            }
            Token::HOOK => {
                let true_value = Self::get_boolean_value(ast, n.get_second_child(ast).unwrap());
                let false_value = Self::get_boolean_value(ast, n.get_last_child(ast).unwrap());
                if true_value == false_value {
                    true_value
                } else {
                    Tri::UNKNOWN
                }
            }
            Token::COALESCE | Token::ASSIGN_COALESCE => {
                let lhs = Self::get_boolean_value(ast, n.get_first_child(ast).unwrap());
                let rhs = Self::get_boolean_value(ast, n.get_last_child(ast).unwrap());
                if lhs == Tri::TRUE || lhs == rhs {
                    lhs
                } else {
                    Tri::UNKNOWN
                }
            }
            _ => Tri::UNKNOWN,
        }
    }
    // port: NodeUtil#getStringValue
    pub fn get_string_value(ast: &Ast, n: NodeId) -> Option<JsString> {
        match n.get_token(ast) {
            Token::STRINGLIT | Token::STRING_KEY => Some(n.get_string(ast)),
            Token::TEMPLATELIT => {
                let mut string = Vec::new();
                for child in n.children(ast) {
                    let expression = if child.is_template_lit_sub(ast) {
                        child.get_first_child(ast).unwrap()
                    } else {
                        child
                    };
                    string.extend_from_slice(Self::get_string_value(ast, expression)?.as_units());
                }
                Some(JsString::from_units(string))
            }
            Token::TEMPLATELIT_STRING => n.get_cooked_string(ast),
            Token::NAME => {
                let name = n.get_string(ast);
                if name == "undefined" || name == "Infinity" || name == "NaN" {
                    Some(name)
                } else {
                    None
                }
            }
            Token::NEG | Token::NUMBER => Self::get_number_value(ast, n).map(|value| {
                JsString::from_units(
                    d_to_a::number_to_string(value).unwrap_or_else(|cause| panic!("{cause}")),
                )
            }),
            Token::BIGINT => Some(n.get_big_int(ast).to_string().into()),
            Token::FALSE => Some("false".into()),
            Token::TRUE => Some("true".into()),
            Token::NULL => Some("null".into()),
            Token::VOID => Some("undefined".into()),
            Token::NOT => {
                let child = Self::get_boolean_value(ast, n.get_first_child(ast).unwrap());
                if child != Tri::UNKNOWN {
                    Some(
                        if child.to_boolean(true) {
                            "false"
                        } else {
                            "true"
                        }
                        .into(),
                    )
                } else {
                    None
                }
            }
            Token::ARRAYLIT => Self::array_to_string(ast, n),
            Token::OBJECTLIT => Some("[object Object]".into()),
            _ => None,
        }
    }
    // port: NodeUtil#getArrayElementStringValue
    pub fn get_array_element_string_value(ast: &Ast, n: NodeId) -> Option<JsString> {
        if Self::is_null_or_undefined(ast, n) || n.is_empty(ast) {
            Some("".into())
        } else {
            Self::get_string_value(ast, n)
        }
    }
    // port: NodeUtil#arrayToString
    pub fn array_to_string(ast: &Ast, literal: NodeId) -> Option<JsString> {
        let first = literal.get_first_child(ast);
        let mut result = Vec::new();
        for n in literal.children(ast) {
            let child_value = Self::get_array_element_string_value(ast, n)?;
            if Some(n) != first {
                result.push(b',' as u16);
            }
            result.extend_from_slice(child_value.as_units());
        }
        Some(JsString::from_units(result))
    }
    // port: NodeUtil#getNumberValue
    pub fn get_number_value(ast: &Ast, n: NodeId) -> Option<f64> {
        Self::do_get_number_value(ast, n, true)
    }
    // port: NodeUtil#getNumberValueNoConversions
    pub fn get_number_value_no_conversions(ast: &Ast, n: NodeId) -> Option<f64> {
        Self::do_get_number_value(ast, n, false)
    }
    // port: NodeUtil#doGetNumberValue
    pub fn do_get_number_value(ast: &Ast, n: NodeId, number_conversions: bool) -> Option<f64> {
        match n.get_token(ast) {
            Token::NUMBER => Some(n.get_double(ast)),
            Token::BIGINT => None,
            Token::VOID => {
                if number_conversions {
                    Some(f64::NAN)
                } else {
                    None
                }
            }
            Token::NAME => match n.get_string(ast).to_string_lossy().as_str() {
                "undefined" => {
                    if number_conversions {
                        Some(f64::NAN)
                    } else {
                        None
                    }
                }
                "NaN" => Some(f64::NAN),
                "Infinity" => Some(f64::INFINITY),
                _ => None,
            },
            Token::POS => Self::do_get_number_value(ast, n.get_only_child(ast), true),
            Token::NEG => {
                Self::do_get_number_value(ast, n.get_only_child(ast), true).map(|val| -val)
            }
            Token::BITNOT => Self::do_get_number_value(ast, n.get_only_child(ast), true)
                .map(|val| f64::from(!JSCompDoubles::ecmascript_to_int32(val))),
            Token::FALSE | Token::NOT | Token::NULL | Token::TRUE => {
                if !number_conversions {
                    return None;
                }
                match Self::get_boolean_value(ast, n) {
                    Tri::TRUE => Some(1.0),
                    Tri::FALSE => Some(0.0),
                    Tri::UNKNOWN => None,
                }
            }
            Token::TEMPLATELIT | Token::ARRAYLIT | Token::OBJECTLIT => {
                if !number_conversions {
                    return None;
                }
                Self::get_string_number_value(&Self::get_string_value(ast, n)?)
            }
            Token::STRINGLIT => {
                if !number_conversions {
                    return None;
                }
                Self::get_string_number_value(&n.get_string(ast))
            }
            _ => None,
        }
    }
    // port: NodeUtil#getStringNumberValue
    pub fn get_string_number_value(raw_js_string: &JsString) -> Option<f64> {
        if raw_js_string.as_units().contains(&0x000b) {
            return None;
        }
        let s = Self::trim_js_white_space(raw_js_string);
        if s.is_empty() {
            return Some(0.0);
        }
        if s.length() > 2 && s.char_at(0) == b'0' as u16 && matches!(s.char_at(1), 120 | 88) {
            return Some(
                closure_rhino::java_lang::parse_int(s.substring_from(2).as_units(), 16)
                    .map_or(f64::NAN, f64::from),
            );
        }
        if s.length() > 3
            && matches!(s.char_at(0), 45 | 43)
            && s.char_at(1) == b'0' as u16
            && matches!(s.char_at(2), 120 | 88)
        {
            return None;
        }
        if s == "infinity" || s == "-infinity" || s == "+infinity" {
            return None;
        }
        Some(closure_rhino::java_lang::double::parse_double(&s).unwrap_or(f64::NAN))
    }
    // port: NodeUtil#getBigIntValue
    pub fn get_big_int_value(ast: &Ast, n: NodeId) -> Option<BigInt> {
        match n.get_token(ast) {
            Token::NUMBER => {
                let val = n.get_double(ast);
                if JSCompDoubles::is_at_least_integer_precision(val)
                    && JSCompDoubles::is_exact_int64(val)
                {
                    Some(BigInt::from(val as i64))
                } else {
                    None
                }
            }
            Token::BIGINT => Some((*n.get_big_int(ast)).clone()),
            Token::FALSE | Token::NOT | Token::TRUE => match Self::get_boolean_value(ast, n) {
                Tri::TRUE => Some(BigInt::from(1)),
                Tri::FALSE => Some(BigInt::from(0)),
                Tri::UNKNOWN => None,
            },
            Token::TEMPLATELIT | Token::ARRAYLIT | Token::OBJECTLIT => {
                Self::get_string_big_int_value(&Self::get_string_value(ast, n)?)
            }
            Token::STRINGLIT => Self::get_string_big_int_value(&n.get_string(ast)),
            Token::NEG => Self::get_big_int_value(ast, n.get_only_child(ast)).map(|result| -result),
            Token::BITNOT => {
                Self::get_big_int_value(ast, n.get_only_child(ast)).map(|result| !result)
            }
            _ => None,
        }
    }
    // port: NodeUtil#getStringBigIntValue
    pub fn get_string_big_int_value(raw_js_string: &JsString) -> Option<BigInt> {
        if raw_js_string.as_units().contains(&0x000b) {
            return None;
        }
        let s = Self::trim_js_white_space(raw_js_string);
        if s.is_empty() {
            return Some(BigInt::from(0));
        }
        if s.length() > 2 && s.char_at(0) == b'0' as u16 {
            let radix = match s.char_at(1) {
                120 | 88 => 16,
                111 | 79 => 8,
                98 | 66 => 2,
                _ => 0,
            };
            if radix != 0 {
                return closure_rhino::java_lang::big_integer::parse_big_integer(
                    &s.substring_from(2),
                    radix,
                )
                .ok();
            }
        }
        closure_rhino::java_lang::big_integer::parse_big_integer(&s, 10).ok()
    }
    // port: NodeUtil#trimJsWhiteSpace
    pub fn trim_js_white_space(s: &JsString) -> JsString {
        let mut start = 0;
        let mut end = s.length();
        while end > 0
            && TokenUtil::is_str_white_space_char(i32::from(s.char_at(end - 1))) == Tri::TRUE
        {
            end -= 1;
        }
        while start < end
            && TokenUtil::is_str_white_space_char(i32::from(s.char_at(start))) == Tri::TRUE
        {
            start += 1;
        }
        s.substring(start, end)
    }
    // port: NodeUtil#getName
    pub fn get_name(ast: &Ast, n: NodeId) -> Option<JsString> {
        Self::get_name_node(ast, n).and_then(|name_node| name_node.get_qualified_name(ast))
    }
    // port: NodeUtil#getNameNode
    pub fn get_name_node(ast: &Ast, n: NodeId) -> Option<NodeId> {
        check_state!(
            n.is_function(ast) || n.is_class(ast),
            "%s",
            n.to_string(ast)
        );
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::NAME => Some(parent),
            Token::ASSIGN => {
                let first_child = parent.get_first_child(ast).unwrap();
                if first_child.is_qualified_name(ast) {
                    Some(first_child)
                } else {
                    None
                }
            }
            _ => {
                let fun_name_node = n.get_first_child(ast).unwrap();
                if fun_name_node.is_empty(ast) || fun_name_node.get_string_ref(ast).is_empty() {
                    None
                } else {
                    Some(fun_name_node)
                }
            }
        }
    }
    // port: NodeUtil#removeName
    pub fn remove_name(ast: &mut Ast, n: NodeId) {
        check_state!(n.is_function(ast) || n.is_class(ast));
        let original_name = n.get_first_child(ast).unwrap();
        let empty_name = if n.is_function(ast) {
            IR::name(ast, "")
        } else {
            IR::empty(ast)
        };
        empty_name.srcref(ast, original_name);
        original_name.replace_with(ast, empty_name);
    }
    // port: NodeUtil#getNearestFunctionName
    pub fn get_nearest_function_name(ast: &Ast, n: NodeId) -> Option<JsString> {
        if !n.is_function(ast) {
            return None;
        }
        if let Some(name) = Self::get_name(ast, n) {
            return Some(name);
        }
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::MEMBER_FUNCTION_DEF
            | Token::SETTER_DEF
            | Token::GETTER_DEF
            | Token::STRING_KEY => Some(parent.get_string(ast)),
            Token::NUMBER => Self::get_string_value(ast, parent),
            _ => None,
        }
    }
    // port: NodeUtil#getClassMembers
    pub fn get_class_members(ast: &Ast, n: NodeId) -> NodeId {
        check_argument!(n.is_class(ast));
        n.get_last_child(ast).unwrap()
    }
    // port: NodeUtil#getEs6ClassConstructorMemberFunctionDef
    pub fn get_es6_class_constructor_member_function_def(
        ast: &Ast,
        class_node: NodeId,
    ) -> Option<NodeId> {
        check_argument!(class_node.is_class(ast), "%s", class_node.to_string(ast));
        let class_members = check_not_null!(
            class_node.get_last_child(ast),
            "%s",
            class_node.to_string(ast)
        );
        class_members.children(ast).find(|member_function_def| {
            Self::is_es6_constructor_member_function_def(ast, *member_function_def)
        })
    }
    // port: NodeUtil#isImmutableValue
    pub fn is_immutable_value(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::STRINGLIT
            | Token::NUMBER
            | Token::BIGINT
            | Token::NULL
            | Token::TRUE
            | Token::FALSE => true,
            Token::CAST | Token::NOT | Token::VOID | Token::NEG => {
                Self::is_immutable_value(ast, n.get_first_child(ast).unwrap())
            }
            Token::NAME => {
                let name = n.get_string(ast);
                name == "undefined" || name == "Infinity" || name == "NaN"
            }
            Token::TEMPLATELIT => {
                for child in n.children(ast) {
                    if child.is_template_lit_sub(ast)
                        && !Self::is_immutable_value(ast, child.get_first_child(ast).unwrap())
                    {
                        return false;
                    }
                }
                true
            }
            Token::TEMPLATELIT_STRING => panic!("Invalid argument {}", n.to_string(ast)),
            _ => false,
        }
    }
    // port: NodeUtil#isSymmetricOperation
    pub fn is_symmetric_operation(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::EQ | Token::NE | Token::SHEQ | Token::SHNE | Token::MUL
        )
    }
    // port: NodeUtil#isRelationalOperation
    pub fn is_relational_operation(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::GT | Token::GE | Token::LT | Token::LE
        )
    }
    // port: NodeUtil#getInverseOperator
    pub fn get_inverse_operator(r#type: Token) -> Token {
        match r#type {
            Token::GT => Token::LT,
            Token::LT => Token::GT,
            Token::GE => Token::LE,
            Token::LE => Token::GE,
            _ => panic!("Unexpected token: {type}"),
        }
    }
    // port: NodeUtil#isLiteralValue
    pub fn is_literal_value(ast: &Ast, n: NodeId, include_functions: bool) -> bool {
        match n.get_token(ast) {
            Token::CAST => {
                Self::is_literal_value(ast, n.get_first_child(ast).unwrap(), include_functions)
            }
            Token::ARRAYLIT => {
                for child in n.children(ast) {
                    if !child.is_empty(ast)
                        && !Self::is_literal_value(ast, child, include_functions)
                    {
                        return false;
                    }
                }
                true
            }
            Token::REGEXP => {
                for child in n.children(ast) {
                    if !Self::is_literal_value(ast, child, include_functions) {
                        return false;
                    }
                }
                true
            }
            Token::OBJECTLIT => {
                for child in n.children(ast) {
                    match child.get_token(ast) {
                        Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF => {
                            if !include_functions {
                                return false;
                            }
                        }
                        Token::COMPUTED_PROP => {
                            if !Self::is_literal_value(
                                ast,
                                child.get_first_child(ast).unwrap(),
                                include_functions,
                            ) || !Self::is_literal_value(
                                ast,
                                child.get_last_child(ast).unwrap(),
                                include_functions,
                            ) {
                                return false;
                            }
                        }
                        Token::OBJECT_SPREAD | Token::STRING_KEY => {
                            if !Self::is_literal_value(
                                ast,
                                child.get_only_child(ast),
                                include_functions,
                            ) {
                                return false;
                            }
                        }
                        _ => panic!(
                            "Unexpected child of OBJECTLIT: {}",
                            child.to_string_tree(ast)
                        ),
                    }
                }
                true
            }
            Token::FUNCTION => include_functions && !Self::is_function_declaration(ast, n),
            Token::TEMPLATELIT => {
                for child in n.children(ast) {
                    if child.is_template_lit_sub(ast)
                        && !Self::is_literal_value(
                            ast,
                            child.get_first_child(ast).unwrap(),
                            include_functions,
                        )
                    {
                        return false;
                    }
                }
                true
            }
            _ => Self::is_immutable_value(ast, n),
        }
    }
}

impl NodeUtil {
    // port: NodeUtil#findClosureUnawareScriptRoot
    pub fn find_closure_unaware_script_root(ast: &Ast, script: NodeId) -> Option<NodeId> {
        check_argument!(
            script.is_script(ast) && script.get_is_in_closure_unaware_subtree(ast),
            "%s",
            script.to_string(ast)
        );
        Self::find_preorder(ast, script, &|ast, n| n.is_block(ast), &|_, _| true)
    }
    // port: NodeUtil#getInsertionPointAfterAllInnerFunctionDeclarations
    pub fn get_insertion_point_after_all_inner_function_declarations(
        ast: &Ast,
        block: NodeId,
    ) -> Option<NodeId> {
        check_state!(block.is_block(ast));
        let mut current = block.get_first_child(ast);
        while current.is_some_and(|c| Self::is_function_declaration(ast, c)) {
            current = current.unwrap().get_next(ast);
        }
        current
    }
    // port: NodeUtil#isSomeCompileTimeConstStringValue
    pub fn is_some_compile_time_const_string_value(ast: &Ast, node: NodeId) -> bool {
        if node.is_string_lit(ast) || (node.is_template_lit(ast) && node.has_one_child(ast)) {
            return true;
        } else if node.is_add(ast) {
            check_state!(node.has_two_children(ast), "%s", node.to_string(ast));
            return Self::is_some_compile_time_const_string_value(
                ast,
                node.get_first_child(ast).unwrap(),
            ) && Self::is_some_compile_time_const_string_value(
                ast,
                node.get_last_child(ast).unwrap(),
            );
        } else if node.is_hook(ast) {
            return Self::is_some_compile_time_const_string_value(
                ast,
                node.get_second_child(ast).unwrap(),
            ) && Self::is_some_compile_time_const_string_value(
                ast,
                node.get_last_child(ast).unwrap(),
            );
        }
        false
    }
    // port: NodeUtil#isEmptyBlock
    pub fn is_empty_block(ast: &Ast, block: NodeId) -> bool {
        if !block.is_block(ast) {
            return false;
        }
        for n in block.children(ast) {
            if !n.is_empty(ast) {
                return false;
            }
        }
        true
    }

    // port: NodeUtil#isBinaryOperator
    pub fn is_binary_operator(ast: &Ast, n: NodeId) -> bool {
        Self::is_binary_operator_type(n.get_token(ast))
    }

    // port: NodeUtil#isBinaryOperatorType
    pub fn is_binary_operator_type(token: Token) -> bool {
        match token {
            Token::OR
            | Token::AND
            | Token::COALESCE
            | Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::EQ
            | Token::NE
            | Token::SHEQ
            | Token::SHNE
            | Token::LT
            | Token::GT
            | Token::LE
            | Token::GE
            | Token::INSTANCEOF
            | Token::IN
            | Token::LSH
            | Token::RSH
            | Token::URSH
            | Token::ADD
            | Token::SUB
            | Token::MUL
            | Token::DIV
            | Token::MOD
            | Token::EXPONENT => true,
            _ => false,
        }
    }

    // port: NodeUtil#isUnaryOperator
    pub fn is_unary_operator(ast: &Ast, n: NodeId) -> bool {
        Self::is_unary_operator_type(n.get_token(ast))
    }

    // port: NodeUtil#isUnaryOperatorType
    pub fn is_unary_operator_type(token: Token) -> bool {
        match token {
            Token::DELPROP
            | Token::VOID
            | Token::TYPEOF
            | Token::POS
            | Token::NEG
            | Token::BITNOT
            | Token::NOT => true,
            _ => false,
        }
    }

    // port: NodeUtil#isUpdateOperator
    pub fn is_update_operator(ast: &Ast, n: NodeId) -> bool {
        Self::is_update_operator_type(n.get_token(ast))
    }

    // port: NodeUtil#isUpdateOperatorType
    pub fn is_update_operator_type(token: Token) -> bool {
        match token {
            Token::INC | Token::DEC => true,
            _ => false,
        }
    }
    // port: NodeUtil#isSimpleOperator
    pub fn is_simple_operator(ast: &Ast, n: NodeId) -> bool {
        Self::is_simple_operator_type(n.get_token(ast))
    }
    // port: NodeUtil#isSimpleOperatorType
    pub fn is_simple_operator_type(r#type: Token) -> bool {
        matches!(
            r#type,
            Token::ADD
                | Token::BITAND
                | Token::BITNOT
                | Token::BITOR
                | Token::BITXOR
                | Token::COMMA
                | Token::DIV
                | Token::EQ
                | Token::EXPONENT
                | Token::GE
                | Token::GT
                | Token::IN
                | Token::INSTANCEOF
                | Token::LE
                | Token::LSH
                | Token::LT
                | Token::MOD
                | Token::MUL
                | Token::NE
                | Token::NOT
                | Token::RSH
                | Token::SHEQ
                | Token::SHNE
                | Token::SUB
                | Token::TYPEOF
                | Token::VOID
                | Token::POS
                | Token::NEG
                | Token::URSH
        )
    }
    // port: NodeUtil#isNamespaceDecl
    pub fn is_namespace_decl(ast: &Ast, n: NodeId) -> bool {
        let jsdoc = Self::get_best_jsdoc_info(ast, n);
        if jsdoc
            .as_ref()
            .is_some_and(|j| !j.get_type_nodes().is_empty() && !j.has_typedef_type())
        {
            return false;
        }
        let parent = n.get_parent(ast).unwrap();
        let is_marked_const =
            parent.is_const(ast) || jsdoc.as_ref().is_some_and(|j| j.is_constant());
        if !n.is_from_externs(ast) && !is_marked_const {
            return false;
        }
        let (qname_node, initializer) = if Self::is_name_declaration(ast, Some(parent)) {
            (n, n.get_first_child(ast))
        } else if n.is_expr_result(ast) {
            let expr = n.get_first_child(ast).unwrap();
            if !expr.is_assign(ast) || !expr.get_first_child(ast).unwrap().is_get_prop(ast) {
                return false;
            }
            (expr.get_first_child(ast).unwrap(), expr.get_last_child(ast))
        } else if n.is_get_prop(ast) {
            if !parent.is_assign(ast) || !parent.get_parent(ast).unwrap().is_expr_result(ast) {
                return false;
            }
            (n, parent.get_last_child(ast))
        } else {
            return false;
        };
        let Some(initializer) = initializer else {
            return false;
        };
        if initializer.is_object_lit(ast) {
            return true;
        }
        initializer.is_or(ast)
            && qname_node
                .matches_qualified_name_node(ast, initializer.get_first_child(ast).unwrap())
            && initializer.get_last_child(ast).unwrap().is_object_lit(ast)
    }
    // port: NodeUtil#isFromTypeSummary
    pub fn is_from_type_summary(ast: &Ast, n: NodeId) -> bool {
        check_argument!(n.is_script(ast), "%s", n.to_string(ast));
        n.get_jsdoc_info(ast)
            .is_some_and(|info| info.is_type_summary())
    }
    // port: NodeUtil#newExpr
    pub fn new_expr(ast: &mut Ast, child: NodeId) -> NodeId {
        let n = IR::expr_result(ast, child);
        n.srcref(ast, child)
    }
    // port: NodeUtil#iteratesImpureIterable
    pub fn iterates_impure_iterable(ast: &Ast, node: NodeId) -> bool {
        let parent = node.get_parent(ast);
        let iterable = match node.get_token(ast) {
            Token::ITER_SPREAD => node.get_only_child(ast),
            Token::YIELD => {
                if !node.is_yield_all(ast) {
                    return false;
                }
                node.get_only_child(ast)
            }
            Token::FOR_OF | Token::FOR_AWAIT_OF => node.get_second_child(ast).unwrap(),
            Token::ITER_REST => {
                let parent = parent.unwrap();
                return match parent.get_token(ast) {
                    Token::PARAM_LIST => false,
                    Token::ARRAY_PATTERN => true,
                    _ => panic!(
                        "Unexpected parent of ITRE_REST: {}",
                        parent.to_string_tree(ast)
                    ),
                };
            }
            _ => panic!(
                "Expected a kind of node that may trigger iteration: {}",
                node.to_string_tree(ast)
            ),
        };
        !Self::is_pure_iterable(ast, iterable)
    }
    // port: NodeUtil#isPureIterable
    pub fn is_pure_iterable(ast: &Ast, node: NodeId) -> bool {
        matches!(
            node.get_token(ast),
            Token::ARRAYLIT | Token::STRINGLIT | Token::TEMPLATELIT
        )
    }
    // port: NodeUtil#newHasLocalResult
    pub fn new_has_local_result(ast: &Ast, n: NodeId) -> bool {
        check_state!(n.is_new(ast), "%s", n.to_string(ast));
        n.is_only_modifies_this_call(ast)
    }
    // port: NodeUtil#allArgsUnescapedLocal
    pub fn all_args_unescaped_local(ast: &Ast, call_or_new: NodeId) -> bool {
        let mut arg = call_or_new.get_second_child(ast);
        while let Some(a) = arg {
            if !Self::evaluates_to_local_value(ast, a) {
                return false;
            }
            arg = a.get_next(ast);
        }
        true
    }
    pub const KNOWN_CONSTANTS: &'static [&'static str] = &["undefined", "Infinity", "NaN"];
    // port: NodeUtil#canBeSideEffected
    pub fn can_be_side_effected(ast: &Ast, n: NodeId) -> bool {
        Self::can_be_side_effected_internal(
            &mut SideEffectContext::Ast(ast),
            n,
            &Self::KNOWN_CONSTANTS
                .iter()
                .map(|s| JsString::from(*s))
                .collect(),
            None,
        )
    }
    // port: NodeUtil#canBeSideEffected(Node, Set<String>, Scope)
    // A compiler is required when a scope is supplied, because scopes live in its arena.
    pub fn can_be_side_effected_with_scope(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        known_constants: &IndexSet<JsString>,
        scope: Option<ScopeId>,
    ) -> bool {
        Self::can_be_side_effected_internal(
            &mut SideEffectContext::Compiler(compiler),
            n,
            known_constants,
            scope,
        )
    }
    fn can_be_side_effected_internal(
        context: &mut SideEffectContext<'_>,
        n: NodeId,
        known_constants: &IndexSet<JsString>,
        scope: Option<ScopeId>,
    ) -> bool {
        match n.get_token(context.ast()) {
            Token::YIELD
            | Token::CALL
            | Token::OPTCHAIN_CALL
            | Token::NEW
            | Token::TAGGED_TEMPLATELIT
            | Token::AWAIT
            | Token::DYNAMIC_IMPORT => return true,
            Token::NAME => {
                let is_constant = match context {
                    SideEffectContext::Ast(ast) => Self::is_constant_name(ast, n),
                    SideEffectContext::Compiler(compiler) => {
                        Self::is_constant_var(compiler, n, scope)
                    }
                };
                return !is_constant && !known_constants.contains(&n.get_string(context.ast()));
            }
            Token::GETPROP => {
                return !n
                    .get_first_child(context.ast())
                    .unwrap()
                    .matches_name(context.ast(), "Symbol");
            }
            Token::GETELEM | Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_GETELEM => return true,
            Token::FUNCTION => {
                check_state!(
                    !Self::is_function_declaration(context.ast(), n),
                    "%s",
                    n.to_string(context.ast())
                );
                return false;
            }
            _ => {}
        }
        let mut child = n.get_first_child(context.ast());
        while let Some(c) = child {
            if Self::can_be_side_effected_internal(context, c, known_constants, scope) {
                return true;
            }
            child = c.get_next(context.ast());
        }
        false
    }
}
// Rust-only borrowing adapter: scope-free callers only require the node arena; scope lookups
// require the compiler's mutable scope arena for Java's lazy implicit variables.
enum SideEffectContext<'a> {
    Ast(&'a Ast),
    Compiler(&'a mut AbstractCompiler),
}
impl SideEffectContext<'_> {
    fn ast(&self) -> &Ast {
        match self {
            Self::Ast(ast) => ast,
            Self::Compiler(compiler) => compiler,
        }
    }
}

impl NodeUtil {
    // port: NodeUtil#precedence
    pub fn precedence(token: Token) -> i32 {
        match token {
            Token::COMMA => 0,
            Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_OR
            | Token::ASSIGN_AND
            | Token::ASSIGN_COALESCE
            | Token::ASSIGN => 1,
            Token::YIELD => 2,
            Token::HOOK => 3,
            Token::OR => 4,
            Token::AND => 5,
            Token::COALESCE => 6,
            Token::BITOR => 7,
            Token::BITXOR => 8,
            Token::BITAND => 9,
            Token::EQ | Token::NE | Token::SHEQ | Token::SHNE => 10,
            Token::LT | Token::GT | Token::LE | Token::GE | Token::INSTANCEOF | Token::IN => 11,
            Token::LSH | Token::RSH | Token::URSH => 12,
            Token::SUB | Token::ADD => 13,
            Token::MUL | Token::MOD | Token::DIV => 14,
            Token::EXPONENT => 15,
            Token::AWAIT
            | Token::NEW
            | Token::DELPROP
            | Token::TYPEOF
            | Token::VOID
            | Token::NOT
            | Token::BITNOT
            | Token::POS
            | Token::NEG => 16,
            Token::INC | Token::DEC => 17,
            Token::CALL
            | Token::GETELEM
            | Token::GETPROP
            | Token::OPTCHAIN_CALL
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP
            | Token::NEW_TARGET
            | Token::IMPORT_META
            | Token::ARRAYLIT
            | Token::ARRAY_PATTERN
            | Token::DEFAULT_VALUE
            | Token::DESTRUCTURING_LHS
            | Token::EMPTY
            | Token::FALSE
            | Token::FUNCTION
            | Token::CLASS
            | Token::INTERFACE
            | Token::NAME
            | Token::NULL
            | Token::NUMBER
            | Token::BIGINT
            | Token::OBJECTLIT
            | Token::OBJECT_PATTERN
            | Token::REGEXP
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::ITER_SPREAD
            | Token::OBJECT_SPREAD
            | Token::STRINGLIT
            | Token::STRING_KEY
            | Token::MEMBER_VARIABLE_DEF
            | Token::INDEX_SIGNATURE
            | Token::CALL_SIGNATURE
            | Token::THIS
            | Token::SUPER
            | Token::TRUE
            | Token::TAGGED_TEMPLATELIT
            | Token::TEMPLATELIT
            | Token::DYNAMIC_IMPORT
            | Token::UNION_TYPE => 18,
            Token::FUNCTION_TYPE => 19,
            Token::ARRAY_TYPE | Token::PARAMETERIZED_TYPE => 20,
            Token::STRING_TYPE
            | Token::NUMBER_TYPE
            | Token::BOOLEAN_TYPE
            | Token::ANY_TYPE
            | Token::RECORD_TYPE
            | Token::NULLABLE_TYPE
            | Token::NAMED_TYPE
            | Token::UNDEFINED_TYPE
            | Token::VOID_TYPE
            | Token::GENERIC_TYPE => 21,
            Token::CAST => 22,
            _ => {
                check_argument!(token != Token::TEMPLATELIT_STRING);
                panic!("Unknown precedence for {token}");
            }
        }
    }
    // port: NodeUtil#isUndefined
    pub fn is_undefined(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::VOID => true,
            Token::NAME => n.get_string_ref(ast) == "undefined",
            _ => false,
        }
    }
    // port: NodeUtil#isNullOrUndefined
    pub fn is_null_or_undefined(ast: &Ast, n: NodeId) -> bool {
        n.is_null(ast) || Self::is_undefined(ast, n)
    }
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ValueType {
    UNDETERMINED,
    NULL,
    VOID,
    NUMBER,
    BIGINT,
    STRING,
    BOOLEAN,
    OBJECT,
}
impl NodeUtil {
    // port: NodeUtil#getKnownValueType
    pub fn get_known_value_type(ast: &Ast, n: NodeId) -> ValueType {
        use ValueType::*;
        match n.get_token(ast) {
            Token::CAST => Self::get_known_value_type(ast, n.get_first_child(ast).unwrap()),
            Token::ASSIGN | Token::COMMA => {
                Self::get_known_value_type(ast, n.get_last_child(ast).unwrap())
            }
            Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::ASSIGN_OR
            | Token::ASSIGN_AND
            | Token::ASSIGN_COALESCE => Self::and(
                Self::get_known_value_type(ast, n.get_first_child(ast).unwrap()),
                Self::get_known_value_type(ast, n.get_last_child(ast).unwrap()),
            ),
            Token::HOOK => Self::and(
                Self::get_known_value_type(ast, n.get_second_child(ast).unwrap()),
                Self::get_known_value_type(ast, n.get_last_child(ast).unwrap()),
            ),
            Token::ADD => {
                let last = Self::get_known_value_type(ast, n.get_last_child(ast).unwrap());
                if last == STRING {
                    return STRING;
                }
                let first = Self::get_known_value_type(ast, n.get_first_child(ast).unwrap());
                if first == STRING {
                    return STRING;
                }
                if first == OBJECT || last == OBJECT {
                    return UNDETERMINED;
                }
                if !Self::may_be_string_value_type(first) && !Self::may_be_string_value_type(last) {
                    if first == BIGINT || last == BIGINT {
                        BIGINT
                    } else {
                        NUMBER
                    }
                } else {
                    UNDETERMINED
                }
            }
            Token::ASSIGN_ADD => {
                let last = Self::get_known_value_type(ast, n.get_last_child(ast).unwrap());
                if last == STRING { STRING } else { UNDETERMINED }
            }
            Token::NAME => {
                let name = n.get_string(ast);
                if name == "undefined" {
                    VOID
                } else if name == "NaN" || name == "Infinity" {
                    NUMBER
                } else {
                    UNDETERMINED
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
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD => {
                if Self::get_known_value_type(ast, n.get_last_child(ast).unwrap()) == BIGINT {
                    BIGINT
                } else {
                    NUMBER
                }
            }
            Token::BIGINT => BIGINT,
            Token::BITOR
            | Token::BITXOR
            | Token::BITAND
            | Token::LSH
            | Token::RSH
            | Token::SUB
            | Token::MUL
            | Token::MOD
            | Token::DIV
            | Token::EXPONENT => {
                let first = Self::get_known_value_type(ast, n.get_first_child(ast).unwrap());
                let last = Self::get_known_value_type(ast, n.get_last_child(ast).unwrap());
                if first == BIGINT || last == BIGINT {
                    BIGINT
                } else {
                    NUMBER
                }
            }
            Token::BITNOT | Token::NEG => {
                if Self::get_known_value_type(ast, n.get_only_child(ast)) == BIGINT {
                    BIGINT
                } else {
                    NUMBER
                }
            }
            Token::INC | Token::DEC | Token::URSH | Token::POS | Token::NUMBER => NUMBER,
            Token::TRUE
            | Token::FALSE
            | Token::EQ
            | Token::NE
            | Token::SHEQ
            | Token::SHNE
            | Token::LT
            | Token::GT
            | Token::LE
            | Token::GE
            | Token::IN
            | Token::INSTANCEOF
            | Token::NOT
            | Token::DELPROP => BOOLEAN,
            Token::TYPEOF | Token::STRINGLIT | Token::TEMPLATELIT => STRING,
            Token::NULL => NULL,
            Token::VOID => VOID,
            Token::FUNCTION | Token::NEW | Token::ARRAYLIT | Token::OBJECTLIT | Token::REGEXP => {
                OBJECT
            }
            _ => {
                check_argument!(!n.is_template_lit_string(ast));
                UNDETERMINED
            }
        }
    }
    // port: NodeUtil#and
    pub fn and(a: ValueType, b: ValueType) -> ValueType {
        if a == b { a } else { ValueType::UNDETERMINED }
    }
    // port: NodeUtil#isNumericResult
    pub fn is_numeric_result(ast: &Ast, n: NodeId) -> bool {
        Self::get_known_value_type(ast, n) == ValueType::NUMBER
    }
    // port: NodeUtil#isBigIntResult
    pub fn is_big_int_result(ast: &Ast, n: NodeId) -> bool {
        Self::get_known_value_type(ast, n) == ValueType::BIGINT
    }
    // port: NodeUtil#isBooleanResult
    pub fn is_boolean_result(ast: &Ast, n: NodeId) -> bool {
        Self::get_known_value_type(ast, n) == ValueType::BOOLEAN
    }
    // port: NodeUtil#isStringResult
    pub fn is_string_result(ast: &Ast, n: NodeId) -> bool {
        Self::get_known_value_type(ast, n) == ValueType::STRING
    }
    // port: NodeUtil#isObjectResult
    pub fn is_object_result(ast: &Ast, n: NodeId) -> bool {
        Self::get_known_value_type(ast, n) == ValueType::OBJECT
    }
    // port: NodeUtil#mayBeString(Node)
    pub fn may_be_string(ast: &Ast, n: NodeId) -> bool {
        Self::may_be_string_value_type(Self::get_known_value_type(ast, n))
    }
    // port: NodeUtil#mayBeString(Node, boolean)
    // The type registry is a trailing argument because Node stores only type handles (DESIGN §4).
    // It is `None` when the compiler has no registry; then no node carries a JSType.
    pub fn may_be_string_with_type(
        ast: &Ast,
        n: NodeId,
        use_type: bool,
        types: Option<&closure_jstype::JSTypeRegistry>,
    ) -> bool {
        use crate::colors::standard_colors as StandardColors;
        if use_type {
            if let Some(color) = n.get_color(ast) {
                if color == *StandardColors::STRING {
                    return true;
                } else if color == *StandardColors::NUMBER
                    || color == *StandardColors::BIGINT
                    || color == *StandardColors::BOOLEAN
                    || color == *StandardColors::NULL_OR_VOID
                {
                    return false;
                }
            }
            if let Some(r#type) = n.get_jstype(ast) {
                let types = types.expect("a node carries a JSType but no JSTypeRegistry was given");
                if r#type.is_string_value_type(types) {
                    return true;
                } else if r#type.is_number_value_type(types)
                    || r#type.is_big_int_value_type(types)
                    || r#type.is_boolean_value_type(types)
                    || r#type.is_null_type(types)
                    || r#type.is_void_type(types)
                {
                    return false;
                }
            }
        }
        Self::may_be_string_value_type(Self::get_known_value_type(ast, n))
    }
    // port: NodeUtil#mayBeString(ValueType)
    pub fn may_be_string_value_type(r#type: ValueType) -> bool {
        match r#type {
            ValueType::BOOLEAN
            | ValueType::NULL
            | ValueType::NUMBER
            | ValueType::BIGINT
            | ValueType::VOID => false,
            ValueType::OBJECT | ValueType::STRING | ValueType::UNDETERMINED => true,
        }
    }
    // port: NodeUtil#mayBeObject(Node)
    pub fn may_be_object(ast: &Ast, n: NodeId) -> bool {
        Self::may_be_object_value_type(Self::get_known_value_type(ast, n))
    }
    // port: NodeUtil#mayBeObject(ValueType)
    pub fn may_be_object_value_type(r#type: ValueType) -> bool {
        match r#type {
            ValueType::BOOLEAN
            | ValueType::NULL
            | ValueType::NUMBER
            | ValueType::BIGINT
            | ValueType::STRING
            | ValueType::VOID => false,
            ValueType::OBJECT | ValueType::UNDETERMINED => true,
        }
    }
    // port: NodeUtil#isAssociative
    pub fn is_associative(r#type: Token) -> bool {
        matches!(
            r#type,
            Token::AND | Token::OR | Token::COALESCE | Token::BITOR | Token::BITXOR | Token::BITAND
        )
    }
    // port: NodeUtil#isCommutative
    pub fn is_commutative(r#type: Token) -> bool {
        matches!(
            r#type,
            Token::MUL | Token::BITOR | Token::BITXOR | Token::BITAND
        )
    }

    // port: NodeUtil#isAssignmentOp
    pub fn is_assignment_op(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_OR
            | Token::ASSIGN_AND
            | Token::ASSIGN_COALESCE => true,
            _ => false,
        }
    }
    // port: NodeUtil#isLogicalAssignmentOp
    pub fn is_logical_assignment_op(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::ASSIGN_OR | Token::ASSIGN_AND | Token::ASSIGN_COALESCE
        )
    }
    // port: NodeUtil#isCompoundAssignmentOp
    pub fn is_compound_assignment_op(ast: &Ast, n: NodeId) -> bool {
        Self::is_assignment_op(ast, n) && !n.is_assign(ast)
    }
    // port: NodeUtil#getOpFromAssignmentOp
    pub fn get_op_from_assignment_op(ast: &Ast, n: NodeId) -> Token {
        match n.get_token(ast) {
            Token::ASSIGN_BITOR => Token::BITOR,
            Token::ASSIGN_BITXOR => Token::BITXOR,
            Token::ASSIGN_BITAND => Token::BITAND,
            Token::ASSIGN_LSH => Token::LSH,
            Token::ASSIGN_RSH => Token::RSH,
            Token::ASSIGN_URSH => Token::URSH,
            Token::ASSIGN_ADD => Token::ADD,
            Token::ASSIGN_SUB => Token::SUB,
            Token::ASSIGN_MUL => Token::MUL,
            Token::ASSIGN_EXPONENT => Token::EXPONENT,
            Token::ASSIGN_DIV => Token::DIV,
            Token::ASSIGN_MOD => Token::MOD,
            Token::ASSIGN_OR => Token::OR,
            Token::ASSIGN_AND => Token::AND,
            Token::ASSIGN_COALESCE => Token::COALESCE,
            _ => panic!("Not an assignment op:{}", n.to_string(ast)),
        }
    }
    // port: NodeUtil#getEnclosingType
    pub fn get_enclosing_type(ast: &Ast, n: NodeId, r#type: Token) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n1| n1.get_token(ast) == r#type)
    }
    // port: NodeUtil#getEnclosingNonArrowFunction
    pub fn get_enclosing_non_arrow_function(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &Self::is_non_arrow_function)
    }
    // port: NodeUtil#getEnclosingClass
    pub fn get_enclosing_class(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n| n.is_class(ast))
    }
    // port: NodeUtil#getEnclosingModuleIfPresent
    pub fn get_enclosing_module_if_present(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n| n.is_module_body(ast))
    }
    // port: NodeUtil#getEnclosingFunction
    pub fn get_enclosing_function(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n| n.is_function(ast))
    }
    // port: NodeUtil#getEnclosingScript
    pub fn get_enclosing_script(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n| n.is_script(ast))
    }
    // port: NodeUtil#getEnclosingBlock
    pub fn get_enclosing_block(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &|ast, n| n.is_block(ast))
    }
    // port: NodeUtil#getEnclosingBlockScopeRoot
    pub fn get_enclosing_block_scope_root(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &Self::creates_block_scope)
    }
    // port: NodeUtil#getEnclosingScopeRoot
    pub fn get_enclosing_scope_root(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &Self::creates_scope)
    }
    // port: NodeUtil#getEnclosingHoistScopeRoot
    pub fn get_enclosing_hoist_scope_root(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &Self::is_hoist_scope_root)
    }
    // port: NodeUtil#isHoistScopeRoot
    pub fn is_hoist_scope_root(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast) || n.is_module_body(ast) || Self::is_class_static_block(ast, n)
    }
    // port: NodeUtil#isInFunction
    pub fn is_in_function(ast: &Ast, n: NodeId) -> bool {
        Self::get_enclosing_function(ast, n).is_some()
    }
    // port: NodeUtil#getEnclosingStatement
    pub fn get_enclosing_statement(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_enclosing_node(ast, n, &Self::is_statement)
    }
    // port: NodeUtil#getEnclosingNode
    pub fn get_enclosing_node(
        ast: &Ast,
        n: NodeId,
        pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> Option<NodeId> {
        let mut curr = Some(n);
        while curr.is_some_and(|c| !pred(ast, c)) {
            curr = curr.unwrap().get_parent(ast);
        }
        curr
    }
    // port: NodeUtil#getFirstPropMatchingKey
    pub fn get_first_prop_matching_key(
        ast: &Ast,
        n: NodeId,
        key_name: &JsString,
    ) -> Option<NodeId> {
        check_state!(n.is_object_lit(ast) || n.is_class_members(ast));
        for key_node in n.children(ast) {
            if (key_node.is_string_key(ast) || key_node.is_member_function_def(ast))
                && key_node.get_string(ast) == *key_name
            {
                return key_node.get_first_child(ast);
            } else if key_node.is_computed_prop(ast)
                && key_node.get_first_child(ast).unwrap().is_string_lit(ast)
                && key_node.get_first_child(ast).unwrap().get_string(ast) == *key_name
            {
                return key_node.get_last_child(ast);
            }
        }
        None
    }
    // port: NodeUtil#getFirstGetterMatchingKey
    #[allow(clippy::manual_find)] // Retain Java control flow.
    pub fn get_first_getter_matching_key(
        ast: &Ast,
        n: NodeId,
        key_name: &JsString,
    ) -> Option<NodeId> {
        check_state!(
            n.is_class_members(ast) || n.is_object_lit(ast),
            "%s",
            n.to_string(ast)
        );
        for key_node in n.children(ast) {
            if key_node.is_getter_def(ast) && key_node.get_string(ast) == *key_name {
                return Some(key_node);
            }
        }
        None
    }
    // port: NodeUtil#referencesOwnReceiver
    pub fn references_own_receiver(ast: &Ast, r#fn: NodeId) -> bool {
        check_state!(r#fn.is_function(ast));
        if r#fn.is_arrow_function(ast) {
            return false;
        }
        Self::references_enclosing_receiver(ast, Self::get_function_parameters(ast, r#fn))
            || Self::references_enclosing_receiver(ast, Self::get_function_body(ast, r#fn))
    }
    // port: NodeUtil#referencesEnclosingReceiver
    pub fn references_enclosing_receiver(ast: &Ast, n: NodeId) -> bool {
        Self::has(
            ast,
            n,
            &|ast, c| c.is_this(ast) || c.is_super(ast),
            &Self::MATCH_ANYTHING_BUT_NON_ARROW_FUNCTION,
        )
    }
    // port: NodeUtil#referencesSuper
    pub fn references_super(ast: &Ast, n: NodeId) -> bool {
        for curr in n.children(ast) {
            if Self::has(ast, curr, &|ast, n| n.is_super(ast), &|ast, node| {
                !node.is_class(ast)
            }) {
                return true;
            }
        }
        false
    }
}

impl NodeUtil {
    // port: NodeUtil#isNormalOrOptChainGet
    pub fn is_normal_or_opt_chain_get(ast: &Ast, n: NodeId) -> bool {
        Self::is_normal_get(ast, n) || Self::is_opt_chain_get(ast, n)
    }
    // port: NodeUtil#isNormalOrOptChainGetProp
    pub fn is_normal_or_opt_chain_get_prop(ast: &Ast, n: NodeId) -> bool {
        n.is_get_prop(ast) || n.is_opt_chain_get_prop(ast)
    }
    // port: NodeUtil#isNormalOrOptChainCall
    pub fn is_normal_or_opt_chain_call(ast: &Ast, n: NodeId) -> bool {
        n.is_call(ast) || n.is_opt_chain_call(ast)
    }

    // port: NodeUtil#isNormalGet
    pub fn is_normal_get(ast: &Ast, n: NodeId) -> bool {
        n.is_get_prop(ast) || n.is_get_elem(ast)
    }

    // port: NodeUtil#isOptChainGet
    pub fn is_opt_chain_get(ast: &Ast, n: NodeId) -> bool {
        n.is_opt_chain_get_prop(ast) || n.is_opt_chain_get_elem(ast)
    }

    // port: NodeUtil#isOptChainNode
    pub fn is_opt_chain_node(ast: &Ast, n: NodeId) -> bool {
        n.is_opt_chain_get_prop(ast) || n.is_opt_chain_get_elem(ast) || n.is_opt_chain_call(ast)
    }
    // port: NodeUtil#getStartOfOptChainSegment
    pub fn get_start_of_opt_chain_segment(ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_opt_chain_node(ast, n), "%s", n.to_string(ast));
        if n.is_optional_chain_start(ast) {
            n
        } else {
            Self::get_start_of_opt_chain_segment(ast, n.get_first_child(ast).unwrap())
        }
    }
    // port: NodeUtil#getEndOfOptChainSegment
    pub fn get_end_of_opt_chain_segment(ast: &Ast, n: NodeId) -> NodeId {
        check_state!(Self::is_opt_chain_node(ast, n), "%s", n.to_string(ast));
        if Self::is_end_of_opt_chain_segment(ast, n) {
            n
        } else {
            Self::get_end_of_opt_chain_segment(ast, n.get_parent(ast).unwrap())
        }
    }
    // port: NodeUtil#isEndOfFullOptChain
    pub fn is_end_of_full_opt_chain(ast: &Ast, n: NodeId) -> bool {
        if Self::is_opt_chain_node(ast, n) {
            let parent = n.get_parent(ast).unwrap();
            !(Self::is_opt_chain_node(ast, parent) && n.is_first_child_of(ast, Some(parent)))
        } else {
            false
        }
    }
    // port: NodeUtil#isEndOfOptChainSegment
    pub fn is_end_of_opt_chain_segment(ast: &Ast, n: NodeId) -> bool {
        if !Self::is_opt_chain_node(ast, n) {
            return false;
        }
        if let Some(parent) = n.get_parent(ast)
            && n.is_first_child_of(ast, Some(parent))
            && Self::is_opt_chain_node(ast, parent)
        {
            parent.is_optional_chain_start(ast)
        } else {
            true
        }
    }
    // port: NodeUtil#convertToNonOptionalChainSegment
    pub fn convert_to_non_optional_chain_segment(ast: &mut Ast, end_of_opt_chain_segment: NodeId) {
        let start = Self::get_start_of_opt_chain_segment(ast, end_of_opt_chain_segment);
        Self::convert_to_non_optional_chain_segment_down_to(
            ast,
            end_of_opt_chain_segment,
            start.get_first_child(ast).unwrap(),
        );
    }
    // port: NodeUtil#convertToNonOptionalChainSegmentDownTo
    pub fn convert_to_non_optional_chain_segment_down_to(
        ast: &mut Ast,
        end_of_opt_chain_segment: NodeId,
        stop_node: NodeId,
    ) {
        check_argument!(
            Self::is_end_of_opt_chain_segment(ast, end_of_opt_chain_segment),
            "%s",
            end_of_opt_chain_segment.to_string(ast)
        );
        let mut segment_nodes = std::collections::VecDeque::new();
        let mut segment_node = Some(end_of_opt_chain_segment);
        while let Some(n) = segment_node {
            if n == stop_node {
                break;
            }
            if Self::is_opt_chain_node(ast, n) {
                segment_nodes.push_back(n);
            }
            segment_node = n.get_first_child(ast);
        }
        for n in segment_nodes {
            n.set_is_optional_chain_start(ast, false);
            n.set_token(ast, Self::get_non_opt_chain_token(n.get_token(ast)));
        }
    }
    // port: NodeUtil#getNonOptChainToken
    pub fn get_non_opt_chain_token(opt_chain_token: Token) -> Token {
        match opt_chain_token {
            Token::OPTCHAIN_CALL => Token::CALL,
            Token::OPTCHAIN_GETELEM => Token::GETELEM,
            Token::OPTCHAIN_GETPROP => Token::GETPROP,
            _ => panic!("Should be an OPTCHAIN token: {opt_chain_token}"),
        }
    }
    // port: NodeUtil#isBlockScopedDeclaration
    pub fn is_block_scoped_declaration(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::LET | Token::CONST | Token::CATCH => true,
            Token::CLASS => Self::is_class_declaration(ast, n),
            Token::FUNCTION => Self::is_block_scoped_function_declaration(ast, n),
            _ => false,
        }
    }

    // port: NodeUtil#isNameDeclaration
    pub fn is_name_declaration(ast: &Ast, n: Option<NodeId>) -> bool {
        n.is_some_and(|n| n.is_var(ast) || n.is_let(ast) || n.is_const(ast))
    }
    // port: NodeUtil#isDestructuringDeclaration
    pub fn is_destructuring_declaration(ast: &Ast, n: NodeId) -> bool {
        if Self::is_name_declaration(ast, Some(n)) {
            for c in n.children(ast) {
                if c.is_destructuring_lhs(ast) {
                    return true;
                }
            }
        }
        false
    }
    // port: NodeUtil#getAssignedValue
    pub fn get_assigned_value(ast: &Ast, n: NodeId) -> Option<NodeId> {
        check_state!(n.is_name(ast) || n.is_get_prop(ast), "%s", n.to_string(ast));
        let parent = n.get_parent(ast).unwrap();
        if Self::is_name_declaration(ast, Some(parent)) {
            n.get_first_child(ast)
        } else if parent.is_assign(ast) && parent.get_first_child(ast) == Some(n) {
            n.get_next(ast)
        } else {
            None
        }
    }
    // port: NodeUtil#isExprAssign
    pub fn is_expr_assign(ast: &Ast, n: NodeId) -> bool {
        n.is_expr_result(ast) && n.get_first_child(ast).unwrap().is_assign(ast)
    }
    // port: NodeUtil#isExprCall
    pub fn is_expr_call(ast: &Ast, n: NodeId) -> bool {
        n.is_expr_result(ast) && n.get_first_child(ast).unwrap().is_call(ast)
    }
    // port: NodeUtil#isNonArrowFunction
    pub fn is_non_arrow_function(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast) && !n.is_arrow_function(ast)
    }
    // port: NodeUtil#isEnhancedFor
    pub fn is_enhanced_for(ast: &Ast, n: NodeId) -> bool {
        n.is_for_of(ast) || n.is_for_await_of(ast) || n.is_for_in(ast)
    }
    // port: NodeUtil#isAnyFor
    pub fn is_any_for(ast: &Ast, n: NodeId) -> bool {
        n.is_vanilla_for(ast) || n.is_for_in(ast) || n.is_for_of(ast) || n.is_for_await_of(ast)
    }
    // port: NodeUtil#isLoopStructure
    pub fn is_loop_structure(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::DO
                | Token::WHILE
        )
    }
    // port: NodeUtil#getLoopCodeBlock
    pub fn get_loop_code_block(ast: &Ast, n: NodeId) -> Option<NodeId> {
        match n.get_token(ast) {
            Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::WHILE => {
                n.get_last_child(ast)
            }
            Token::DO => n.get_first_child(ast),
            _ => None,
        }
    }
    // port: NodeUtil#isWithinLoop
    pub fn is_within_loop(ast: &Ast, n: NodeId) -> bool {
        for parent in n.ancestors(ast) {
            if Self::is_loop_structure(ast, parent) {
                return true;
            }
            if parent.is_function(ast) {
                break;
            }
        }
        false
    }
    // port: NodeUtil#isControlStructure
    pub fn is_control_structure(ast: &Ast, n: NodeId) -> bool {
        matches!(
            n.get_token(ast),
            Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::DO
                | Token::WHILE
                | Token::WITH
                | Token::IF
                | Token::LABEL
                | Token::TRY
                | Token::CATCH
                | Token::SWITCH
                | Token::CASE
                | Token::DEFAULT_CASE
        )
    }
    // port: NodeUtil#isControlStructureCodeBlock
    pub fn is_control_structure_code_block(ast: &Ast, parent: NodeId, n: NodeId) -> bool {
        match parent.get_token(ast) {
            Token::DO => parent.get_first_child(ast) == Some(n),
            Token::TRY => {
                parent.get_first_child(ast) == Some(n) || parent.get_last_child(ast) == Some(n)
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::WHILE
            | Token::LABEL
            | Token::WITH
            | Token::CATCH => parent.get_last_child(ast) == Some(n),
            Token::IF | Token::SWITCH | Token::CASE => parent.get_first_child(ast) != Some(n),
            Token::DEFAULT_CASE => true,
            _ => {
                check_state!(
                    Self::is_control_structure(ast, parent),
                    "%s",
                    parent.to_string(ast)
                );
                false
            }
        }
    }
    // port: NodeUtil#getConditionExpression
    pub fn get_condition_expression(ast: &Ast, n: NodeId) -> Option<NodeId> {
        match n.get_token(ast) {
            Token::IF | Token::WHILE => n.get_first_child(ast),
            Token::DO => n.get_last_child(ast),
            Token::FOR => n.get_second_child(ast),
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF | Token::CASE => None,
            _ => panic!("{} does not have a condition.", n.to_string(ast)),
        }
    }
    // port: NodeUtil#isStatementBlock
    pub fn is_statement_block(ast: &Ast, n: NodeId) -> bool {
        n.is_root(ast) || n.is_script(ast) || n.is_block(ast) || n.is_module_body(ast)
    }
    // port: NodeUtil#createsBlockScope
    pub fn creates_block_scope(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::BLOCK => {
                let parent = n.get_parent(ast);
                if parent.is_some_and(|p| p.is_block(ast) || p.is_script(ast))
                    && n.is_synthetic_block(ast)
                {
                    return false;
                }
                parent.is_some_and(|p| !Self::is_switch_case(ast, p) && !p.is_catch(ast))
            }
            Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::SWITCH_BODY
            | Token::CLASS => true,
            _ => false,
        }
    }
    // port: NodeUtil#createsScope
    pub fn creates_scope(ast: &Ast, n: NodeId) -> bool {
        Self::creates_block_scope(ast, n)
            || n.is_function(ast)
            || n.is_module_body(ast)
            || n.is_member_field_def(ast)
            || n.is_computed_field_def(ast)
            || (n.is_root(ast) && n.get_parent(ast).is_none())
    }
    pub const DEFINITE_CFG_ROOTS: &'static [Token] = &[
        Token::FUNCTION,
        Token::SCRIPT,
        Token::MODULE_BODY,
        Token::ROOT,
    ];
    // port: NodeUtil#isValidCfgRoot
    pub fn is_valid_cfg_root(ast: &Ast, n: NodeId) -> bool {
        Self::DEFINITE_CFG_ROOTS.contains(&n.get_token(ast)) || Self::is_class_static_block(ast, n)
    }

    // port: NodeUtil#isStatement
    pub fn is_statement(ast: &Ast, n: NodeId) -> bool {
        !n.is_module_body(ast)
            && !n.is_script(ast)
            && !n.is_root(ast)
            && Self::is_statement_parent(ast, n.get_parent(ast).unwrap())
    }

    pub const IS_STATEMENT_PARENT: &[Token] = &[
        Token::SCRIPT,
        Token::MODULE_BODY,
        Token::BLOCK,
        Token::LABEL,
        Token::NAMESPACE_ELEMENTS,
        Token::INTERFACE_MEMBERS,
    ];
    // port: NodeUtil#isStatementParent
    pub fn is_statement_parent(ast: &Ast, parent: NodeId) -> bool {
        Self::IS_STATEMENT_PARENT.contains(&parent.get_token(ast))
    }
    // port: NodeUtil#isDeclarationParent
    pub fn is_declaration_parent(ast: &Ast, parent: NodeId) -> bool {
        match parent.get_token(ast) {
            Token::DECLARE | Token::EXPORT => true,
            _ => Self::is_statement_parent(ast, parent),
        }
    }
    // port: NodeUtil#isSwitchCase
    pub fn is_switch_case(ast: &Ast, n: NodeId) -> bool {
        n.is_case(ast) || n.is_default_case(ast)
    }
    // port: NodeUtil#isReferenceName
    pub fn is_reference_name(ast: &Ast, n: NodeId) -> bool {
        n.is_name(ast) && !n.get_string_ref(ast).is_empty()
    }
    // port: NodeUtil#isNonlocalModuleExportName
    pub fn is_nonlocal_module_export_name(ast: &Ast, n: NodeId) -> bool {
        check_argument!(n.is_name(ast), "%s", n.to_string(ast));
        let parent = n.get_parent(ast).unwrap();
        if parent.is_import_spec(ast) && n.is_first_child_of(ast, Some(parent)) {
            true
        } else if parent.is_export_spec(ast) {
            if n.is_first_child_of(ast, Some(parent)) {
                Self::is_export_from(ast, parent.get_grandparent(ast).unwrap())
            } else {
                true
            }
        } else {
            false
        }
    }
    // port: NodeUtil#isTryFinallyNode
    pub fn is_try_finally_node(ast: &Ast, parent: NodeId, child: NodeId) -> bool {
        parent.is_try(ast)
            && parent.has_x_children(ast, 3)
            && Some(child) == parent.get_last_child(ast)
    }
    // port: NodeUtil#isTryCatchNodeContainer
    pub fn is_try_catch_node_container(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        parent.is_try(ast) && parent.get_second_child(ast) == Some(n)
    }
    // port: NodeUtil#isInSyntheticScript
    pub fn is_in_synthetic_script(ast: &Ast, n: NodeId) -> bool {
        n.get_source_file_name(ast).is_some_and(|source_file_name| {
            source_file_name.starts_with(" [synthetic:")
                || source_file_name.starts_with(AbstractCompiler::RUNTIME_LIB_DIR)
        })
    }
}

impl NodeUtil {
    // port: NodeUtil#deleteNode
    pub fn delete_node(compiler: &mut AbstractCompiler, n: NodeId) {
        let parent = n.get_parent(compiler).unwrap();
        Self::mark_functions_deleted(compiler, n);
        n.detach(compiler);
        compiler.report_change_to_enclosing_scope(parent);
    }
    // port: NodeUtil#deleteFunctionCall
    pub fn delete_function_call(compiler: &mut AbstractCompiler, n: NodeId) {
        check_state!(n.is_call(compiler));
        let mut parent = n.get_parent(compiler).unwrap();
        if parent.is_expr_result(compiler) {
            let grand_parent = parent.get_parent(compiler).unwrap();
            parent.detach(compiler);
            parent = grand_parent;
        } else {
            let undefined = Self::new_undefined_node(compiler, Some(n));
            n.replace_with(compiler, undefined);
        }
        Self::mark_functions_deleted(compiler, n);
        compiler.report_change_to_enclosing_scope(parent);
    }
    // port: NodeUtil#deleteChildren
    pub fn delete_children(compiler: &mut AbstractCompiler, n: NodeId) {
        while n.has_children(compiler) {
            Self::delete_node(compiler, n.get_first_child(compiler).unwrap());
        }
    }
    // port: NodeUtil#removeChild
    pub fn remove_child(ast: &mut Ast, parent: NodeId, node: NodeId) {
        if Self::is_try_finally_node(ast, parent, node) {
            if Self::has_catch_handler(ast, Self::get_catch_block(ast, parent)) {
                node.detach(ast);
            } else {
                node.detach_children(ast);
            }
        } else if node.is_catch(ast) {
            let try_node = node.get_grandparent(ast).unwrap();
            check_state!(Self::has_finally(ast, try_node));
            node.detach(ast);
        } else if Self::is_try_catch_node_container(ast, node) {
            let try_node = node.get_parent(ast).unwrap();
            check_state!(Self::has_finally(ast, try_node));
            node.detach_children(ast);
        } else if node.is_block(ast) {
            node.detach_children(ast);
        } else if Self::is_statement_block(ast, parent)
            || Self::is_switch_case(ast, node)
            || node.is_member_function_def(ast)
        {
            node.detach(ast);
        } else if Self::is_name_declaration(ast, Some(parent)) || parent.is_expr_result(ast) {
            if parent.has_more_than_one_child(ast) {
                node.detach(ast);
            } else {
                node.detach(ast);
                Self::remove_child(ast, parent.get_parent(ast).unwrap(), parent);
            }
        } else if parent.is_label(ast) && Some(node) == parent.get_last_child(ast) {
            node.detach(ast);
            Self::remove_child(ast, parent.get_parent(ast).unwrap(), parent);
        } else if parent.is_vanilla_for(ast) {
            let empty = IR::empty(ast);
            node.replace_with(ast, empty);
        } else if parent.is_object_pattern(ast) {
            node.detach(ast);
        } else if parent.is_array_pattern(ast) {
            if Some(node) == parent.get_last_child(ast) {
                node.detach(ast);
            } else {
                let empty = IR::empty(ast);
                node.replace_with(ast, empty);
            }
        } else if parent.is_destructuring_lhs(ast) {
            node.detach(ast);
            if parent.get_parent(ast).unwrap().has_children(ast) {
                Self::remove_child(ast, parent.get_parent(ast).unwrap(), parent);
            }
        } else if parent.is_rest(ast) {
            parent.detach(ast);
        } else if parent.is_param_list(ast) {
            node.detach(ast);
        } else if parent.is_import(ast) {
            if Some(node) == parent.get_first_child(ast) {
                let empty = IR::empty(ast);
                node.replace_with(ast, empty);
            } else {
                panic!(
                    "Invalid attempt to remove: {} from {}",
                    node.to_string(ast),
                    parent.to_string(ast)
                );
            }
        } else {
            panic!(
                "Invalid attempt to remove node: {} of {}",
                node.to_string(ast),
                parent.to_string(ast)
            );
        }
    }
    // port: NodeUtil#replaceDeclarationChild
    pub fn replace_declaration_child(ast: &mut Ast, decl_child: NodeId, new_statement: NodeId) {
        check_argument!(Self::is_name_declaration(ast, decl_child.get_parent(ast)));
        check_argument!(new_statement.get_parent(ast).is_none());
        let decl = decl_child.get_parent(ast).unwrap();
        if decl.has_one_child(ast) {
            decl.replace_with(ast, new_statement);
        } else if decl_child.get_next(ast).is_none() {
            decl_child.detach(ast);
            new_statement.insert_after(ast, decl);
        } else if decl_child.get_previous(ast).is_none() {
            decl_child.detach(ast);
            new_statement.insert_before(ast, decl);
        } else {
            check_state!(decl.has_more_than_one_child(ast));
            let new_decl = ast.new_node(decl.get_token(ast));
            new_decl.srcref(ast, decl);
            let mut after = decl_child.get_next(ast);
            while let Some(a) = after {
                let next = a.get_next(ast);
                a.detach(ast);
                new_decl.add_child_to_back(ast, a);
                after = next;
            }
            decl_child.detach(ast);
            new_statement.insert_after(ast, decl);
            new_decl.insert_after(ast, new_statement);
        }
    }
    // port: NodeUtil#maybeAddFinally
    pub fn maybe_add_finally(ast: &mut Ast, try_node: NodeId) {
        check_state!(try_node.is_try(ast));
        if !Self::has_finally(ast, try_node) {
            let block = IR::block(ast);
            block.srcref(ast, try_node);
            try_node.add_child_to_back(ast, block);
        }
    }
    // port: NodeUtil#tryMergeBlock
    pub fn try_merge_block(
        ast: &mut Ast,
        block: NodeId,
        ignore_block_scoped_declarations: bool,
    ) -> bool {
        check_state!(block.is_block(ast));
        let parent = block.get_parent(ast).unwrap();
        let can_merge = ignore_block_scoped_declarations || Self::can_merge_block(ast, block);
        if Self::is_statement_block(ast, parent) && can_merge {
            let mut previous = block;
            while block.has_children(ast) {
                let child = block.remove_first_child(ast).unwrap();
                child.insert_after(ast, previous);
                previous = child;
            }
            block.detach(ast);
            true
        } else {
            false
        }
    }
    // port: NodeUtil#canMergeBlock
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    pub fn can_merge_block(ast: &Ast, block: NodeId) -> bool {
        for c in block.children(ast) {
            match c.get_token(ast) {
                Token::LABEL => {
                    if !Self::can_merge_block(ast, c) {
                        return false;
                    }
                }
                Token::CONST | Token::LET | Token::CLASS | Token::FUNCTION => return false,
                _ => {}
            }
        }
        true
    }
    // port: NodeUtil#isCallOrNew
    pub fn is_call_or_new(ast: &Ast, node: NodeId) -> bool {
        node.is_call(ast) || node.is_new(ast) || node.is_opt_chain_call(ast)
    }
    // port: NodeUtil#getFunctionBody
    pub fn get_function_body(ast: &Ast, r#fn: NodeId) -> NodeId {
        check_argument!(r#fn.is_function(ast), "%s", r#fn.to_string(ast));
        r#fn.get_last_child(ast).unwrap()
    }
    // port: NodeUtil#getCallTargetResolvingIndirectCalls
    pub fn get_call_target_resolving_indirect_calls(ast: &Ast, call: NodeId) -> NodeId {
        check_argument!(
            call.is_call(ast) || call.is_new(ast) || call.is_tagged_template_lit(ast),
            "must be call, new or tagged-template-literal expression, got %s",
            call.to_string(ast)
        );
        let target = call.get_first_child(ast).unwrap();
        if target.is_comma(ast) && target.get_second_child(ast).unwrap().is_qualified_name(ast) {
            target.get_second_child(ast).unwrap()
        } else {
            target
        }
    }
    // port: NodeUtil#isDeclaration
    pub fn is_declaration(ast: &Ast, n: NodeId) -> bool {
        Self::is_name_declaration(ast, Some(n))
            || Self::is_function_declaration(ast, n)
            || Self::is_class_declaration(ast, n)
    }
    // port: NodeUtil#isNamedExportsLiteral
    pub fn is_named_exports_literal(ast: &Ast, object_literal: NodeId) -> bool {
        if !object_literal.is_object_lit(ast) || !object_literal.has_children(ast) {
            return false;
        }
        for key in object_literal.children(ast) {
            if !key.is_string_key(ast) || key.is_quoted_string_key(ast) {
                return false;
            }
            if !key.get_first_child(ast).unwrap().is_name(ast) {
                return false;
            }
        }
        true
    }
    // port: NodeUtil#isFunctionDeclaration
    pub fn is_function_declaration(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast)
            && Self::is_declaration_parent(ast, n.get_parent(ast).unwrap())
            && Self::is_named_function(ast, n)
    }
    // port: NodeUtil#isMethodDeclaration
    pub fn is_method_declaration(ast: &Ast, n: NodeId) -> bool {
        if n.is_function(ast) {
            let parent = n.get_parent(ast).unwrap();
            match parent.get_token(ast) {
                Token::GETTER_DEF | Token::SETTER_DEF | Token::MEMBER_FUNCTION_DEF => true,
                Token::COMPUTED_PROP => {
                    parent.get_last_child(ast) == Some(n)
                        && (parent.get_boolean_prop(ast, Prop::COMPUTED_PROP_METHOD)
                            || parent.get_boolean_prop(ast, Prop::COMPUTED_PROP_GETTER)
                            || parent.get_boolean_prop(ast, Prop::COMPUTED_PROP_SETTER))
                }
                _ => false,
            }
        } else {
            false
        }
    }
    // port: NodeUtil#isClassDeclaration
    pub fn is_class_declaration(ast: &Ast, n: NodeId) -> bool {
        n.is_class(ast)
            && Self::is_declaration_parent(ast, n.get_parent(ast).unwrap())
            && Self::is_named_class(ast, n)
    }
    // port: NodeUtil#isHoistedFunctionDeclaration
    pub fn is_hoisted_function_declaration(ast: &Ast, n: NodeId) -> bool {
        if Self::is_function_declaration(ast, n) {
            let parent = n.get_parent(ast).unwrap();
            return parent.is_script(ast)
                || parent.is_module_body(ast)
                || parent.get_parent(ast).unwrap().is_function(ast)
                || parent.is_export(ast);
        }
        false
    }
    // port: NodeUtil#isBlockScopedFunctionDeclaration
    pub fn is_block_scoped_function_declaration(ast: &Ast, n: NodeId) -> bool {
        if !Self::is_function_declaration(ast, n) {
            return false;
        }
        let mut current = n.get_parent(ast);
        while let Some(c) = current {
            match c.get_token(ast) {
                Token::BLOCK => return !c.get_parent(ast).unwrap().is_function(ast),
                Token::FUNCTION
                | Token::SCRIPT
                | Token::DECLARE
                | Token::EXPORT
                | Token::MODULE_BODY => return false,
                _ => {
                    check_state!(c.is_label(ast), "%s", c.to_string(ast));
                    current = c.get_parent(ast);
                }
            }
        }
        false
    }
    // port: NodeUtil#isFunctionBlock
    pub fn is_function_block(ast: &Ast, n: NodeId) -> bool {
        n.is_block(ast) && n.has_parent(ast) && n.get_parent(ast).unwrap().is_function(ast)
    }
    // port: NodeUtil#isClassStaticBlock
    pub fn is_class_static_block(ast: &Ast, n: NodeId) -> bool {
        n.is_block(ast) && n.has_parent(ast) && n.get_parent(ast).unwrap().is_class_members(ast)
    }
    // port: NodeUtil#isFunctionExpression
    pub fn is_function_expression(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast)
            && !Self::is_function_declaration(ast, n)
            && !Self::is_method_declaration(ast, n)
    }
    // port: NodeUtil#isNamedFunctionExpression
    pub fn is_named_function_expression(ast: &Ast, n: NodeId) -> bool {
        Self::is_function_expression(ast, n)
            && !n
                .get_first_child(ast)
                .unwrap()
                .get_string_ref(ast)
                .is_empty()
    }
    // port: NodeUtil#isClassExpression
    pub fn is_class_expression(ast: &Ast, n: NodeId) -> bool {
        n.is_class(ast)
            && (!Self::is_named_class(ast, n)
                || !Self::is_declaration_parent(ast, n.get_parent(ast).unwrap()))
    }
    // port: NodeUtil#isNamedClassExpression
    pub fn is_named_class_expression(ast: &Ast, n: NodeId) -> bool {
        Self::is_class_expression(ast, n) && n.get_first_child(ast).unwrap().is_name(ast)
    }
    // port: NodeUtil#isNamedFunction
    pub fn is_named_function(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast) && Self::is_reference_name(ast, n.get_first_child(ast).unwrap())
    }
    // port: NodeUtil#isNamedClass
    pub fn is_named_class(ast: &Ast, n: NodeId) -> bool {
        n.is_class(ast) && Self::is_reference_name(ast, n.get_first_child(ast).unwrap())
    }
    // port: NodeUtil#isBleedingFunctionName
    pub fn is_bleeding_function_name(ast: &Ast, n: NodeId) -> bool {
        if !n.is_name(ast) || n.get_string_ref(ast).is_empty() {
            return false;
        }
        let parent = n.get_parent(ast).unwrap();
        Self::is_function_expression(ast, parent) && Some(n) == parent.get_first_child(ast)
    }
    // port: NodeUtil#isEmptyFunctionExpression
    pub fn is_empty_function_expression(ast: &Ast, node: NodeId) -> bool {
        Self::is_function_expression(ast, node)
            && Self::is_empty_block(ast, node.get_last_child(ast).unwrap())
    }
    // port: NodeUtil#doesFunctionReferenceOwnArgumentsObject
    pub fn does_function_reference_own_arguments_object(ast: &Ast, r#fn: NodeId) -> bool {
        check_argument!(r#fn.is_function(ast));
        if r#fn.is_arrow_function(ast) {
            return false;
        }
        Self::references_arguments_helper(ast, r#fn.get_second_child(ast).unwrap())
            || Self::references_arguments_helper(ast, r#fn.get_last_child(ast).unwrap())
    }
    // port: NodeUtil#referencesArgumentsHelper
    pub fn references_arguments_helper(ast: &Ast, node: NodeId) -> bool {
        if node.is_name(ast) && node.get_string_ref(ast) == "arguments" {
            return true;
        }
        if Self::is_non_arrow_function(ast, node) {
            return false;
        }
        for c in node.children(ast) {
            if Self::references_arguments_helper(ast, c) {
                return true;
            }
        }
        false
    }
    // port: NodeUtil#isObjectCallMethod
    pub fn is_object_call_method(ast: &Ast, call_node: NodeId, method_name: &JsString) -> bool {
        if call_node.is_call(ast) || call_node.is_opt_chain_call(ast) {
            let callee = call_node.get_first_child(ast).unwrap();
            if Self::is_normal_or_opt_chain_get_prop(ast, callee) {
                return callee.get_string(ast) == *method_name;
            } else if Self::is_normal_or_opt_chain_get(ast, callee)
                && let Some(last) = callee.get_last_child(ast)
                && last.is_string_lit(ast)
            {
                return last.get_string(ast) == *method_name;
            }
        }
        false
    }
    // port: NodeUtil#isFunctionObjectCall
    pub fn is_function_object_call(ast: &Ast, call_node: NodeId) -> bool {
        Self::is_object_call_method(ast, call_node, &"call".into())
    }
    // port: NodeUtil#isFunctionObjectApply
    pub fn is_function_object_apply(ast: &Ast, call_node: NodeId) -> bool {
        Self::is_object_call_method(ast, call_node, &"apply".into())
    }
}

impl NodeUtil {
    // port: NodeUtil#isNameDeclOrSimpleAssignLhs
    pub fn is_name_decl_or_simple_assign_lhs(ast: &Ast, n: NodeId, parent: NodeId) -> bool {
        (parent.is_assign(ast) && parent.get_first_child(ast) == Some(n))
            || Self::is_name_declaration(ast, Some(parent))
    }
    // port: NodeUtil#isLValue
    pub fn is_l_value(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::NAME | Token::GETPROP | Token::GETELEM => {}
            _ => return false,
        }
        let Some(parent) = n.get_parent(ast) else {
            return false;
        };
        match parent.get_token(ast) {
            Token::IMPORT_SPEC => parent.get_last_child(ast) == Some(n),
            Token::VAR
            | Token::LET
            | Token::CONST
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::PARAM_LIST
            | Token::IMPORT
            | Token::INC
            | Token::DEC
            | Token::CATCH => true,
            Token::CLASS
            | Token::FUNCTION
            | Token::DEFAULT_VALUE
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF => parent.get_first_child(ast) == Some(n),
            Token::ARRAY_PATTERN | Token::STRING_KEY | Token::COMPUTED_PROP => {
                Self::is_lhs_by_destructuring(ast, n)
            }
            _ => Self::is_assignment_op(ast, parent) && parent.get_first_child(ast) == Some(n),
        }
    }
    // port: NodeUtil#isDeclarationLValue
    pub fn is_declaration_l_value(ast: &Ast, n: NodeId) -> bool {
        let is_l_value = Self::is_l_value(ast, n);
        if !is_l_value {
            return false;
        }
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::IMPORT_SPEC
            | Token::VAR
            | Token::LET
            | Token::CONST
            | Token::PARAM_LIST
            | Token::IMPORT
            | Token::CATCH
            | Token::CLASS
            | Token::FUNCTION => true,
            Token::STRING_KEY => {
                Self::is_name_declaration(ast, parent.get_parent(ast).unwrap().get_grandparent(ast))
            }
            Token::OBJECT_PATTERN | Token::ARRAY_PATTERN => {
                Self::is_name_declaration(ast, parent.get_grandparent(ast))
            }
            _ => false,
        }
    }
    // port: NodeUtil#isLhsOfAssign
    pub fn is_lhs_of_assign(ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast)
            .is_some_and(|parent| parent.is_assign(ast) && parent.get_first_child(ast) == Some(n))
    }
    // port: NodeUtil#isImportedName
    pub fn is_imported_name(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        parent.is_import(ast)
            || (parent.is_import_spec(ast) && parent.get_last_child(ast) == Some(n))
    }
    // port: NodeUtil#getDeclaringParent
    pub fn get_declaring_parent(ast: &Ast, target_node: NodeId) -> NodeId {
        let root_target = Self::get_root_target(ast, target_node);
        let mut parent = root_target.get_parent(ast).unwrap();
        if parent.is_rest(ast) || parent.is_default_value(ast) {
            parent = parent.get_parent(ast).unwrap();
            check_state!(parent.is_param_list(ast), "%s", parent.to_string(ast));
        } else if parent.is_destructuring_lhs(ast) {
            parent = parent.get_parent(ast).unwrap();
            check_state!(
                Self::is_name_declaration(ast, Some(parent)),
                "%s",
                parent.to_string(ast)
            );
        } else if parent.is_class(ast) || parent.is_function(ast) {
            check_state!(
                Some(target_node) == parent.get_first_child(ast),
                "%s",
                target_node.to_string(ast)
            );
        } else if parent.is_import_spec(ast) {
            check_state!(
                Some(target_node) == parent.get_second_child(ast),
                "%s",
                target_node.to_string(ast)
            );
            parent = parent.get_grandparent(ast).unwrap();
            check_state!(parent.is_import(ast), "%s", parent.to_string(ast));
        } else {
            check_state!(
                parent.is_param_list(ast)
                    || Self::is_name_declaration(ast, Some(parent))
                    || parent.is_import(ast)
                    || parent.is_catch(ast),
                "%s",
                parent.to_string(ast)
            );
        }
        parent
    }
    // port: NodeUtil#getRootTarget
    pub fn get_root_target(ast: &Ast, target_node: NodeId) -> NodeId {
        let mut enclosing_target = target_node;
        while let Some(next_target) = Self::get_enclosing_target(ast, enclosing_target) {
            enclosing_target = next_target;
        }
        enclosing_target
    }
    // port: NodeUtil#getEnclosingTarget
    pub fn get_enclosing_target(ast: &Ast, mut target_node: NodeId) -> Option<NodeId> {
        check_state!(
            target_node.is_valid_assignment_target(ast),
            "%s",
            target_node.to_string(ast)
        );
        let mut parent = check_not_null!(
            target_node.get_parent(ast),
            "%s",
            target_node.to_string(ast)
        );
        let mut target_is_first_child = parent.get_first_child(ast) == Some(target_node);
        if parent.is_default_value(ast) || parent.is_rest(ast) {
            check_state!(target_is_first_child, "%s", parent.to_string(ast));
            target_node = parent;
            parent = check_not_null!(target_node.get_parent(ast));
            target_is_first_child = Some(target_node) == parent.get_first_child(ast);
        }
        match parent.get_token(ast) {
            Token::ARRAY_PATTERN | Token::OBJECT_PATTERN => Some(parent),
            Token::COMPUTED_PROP => {
                check_state!(!target_is_first_child, "%s", parent.to_string(ast));
                let grandparent =
                    check_not_null!(parent.get_parent(ast), "%s", parent.to_string(ast));
                check_state!(
                    grandparent.is_object_pattern(ast),
                    "%s",
                    grandparent.to_string(ast)
                );
                Some(grandparent)
            }
            Token::STRING_KEY => {
                let grandparent =
                    check_not_null!(parent.get_parent(ast), "%s", parent.to_string(ast));
                check_state!(
                    grandparent.is_object_pattern(ast),
                    "%s",
                    grandparent.to_string(ast)
                );
                Some(grandparent)
            }
            Token::PARAM_LIST | Token::LET | Token::CONST | Token::VAR => None,
            Token::FUNCTION
            | Token::CLASS
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::DESTRUCTURING_LHS => {
                check_state!(target_is_first_child, "%s", target_node.to_string(ast));
                None
            }
            Token::IMPORT => None,
            Token::IMPORT_SPEC => {
                check_state!(!target_is_first_child, "%s", parent.to_string(ast));
                None
            }
            Token::CATCH => None,
            _ => {
                check_state!(
                    Self::is_assignment_op(ast, parent) && target_is_first_child,
                    "%s",
                    parent.to_string(ast)
                );
                None
            }
        }
    }
    // port: NodeUtil#isLhsByDestructuring
    pub fn is_lhs_by_destructuring(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::NAME | Token::GETPROP | Token::GETELEM => {
                Self::is_lhs_by_destructuring_helper(ast, n)
            }
            _ => false,
        }
    }
    // port: NodeUtil#isLhsByDestructuringHelper
    pub fn is_lhs_by_destructuring_helper(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        let grandparent = n.get_grandparent(ast);
        match parent.get_token(ast) {
            Token::ARRAY_PATTERN | Token::ITER_REST | Token::OBJECT_REST => true,
            Token::COMPUTED_PROP => {
                !n.is_first_child_of(ast, Some(parent))
                    && grandparent.unwrap().is_object_pattern(ast)
            }
            Token::STRING_KEY => grandparent.unwrap().is_object_pattern(ast),
            Token::DEFAULT_VALUE => {
                n.is_first_child_of(ast, Some(parent))
                    && Self::is_lhs_by_destructuring_helper(ast, parent)
            }
            _ => false,
        }
    }

    // port: NodeUtil#mayBeObjectLitKey
    pub fn may_be_object_lit_key(ast: &Ast, node: NodeId) -> bool {
        match node.get_token(ast) {
            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF => true,
            _ => false,
        }
    }
    // port: NodeUtil#isObjectLitKey
    pub fn is_object_lit_key(ast: &Ast, node: NodeId) -> bool {
        node.get_parent(ast).unwrap().is_object_lit(ast) && Self::may_be_object_lit_key(ast, node)
    }
    // port: NodeUtil#getObjectOrClassLitKeyName
    pub fn get_object_or_class_lit_key_name(ast: &Ast, key: NodeId) -> JsString {
        if let Some(key_node) = Self::get_object_or_class_lit_key_node(ast, key) {
            return key_node.get_string(ast);
        }
        panic!("Unexpected node type: {}", key.to_string(ast))
    }
    // port: NodeUtil#getObjectOrClassLitKeyNode
    pub fn get_object_or_class_lit_key_node(ast: &Ast, key: NodeId) -> Option<NodeId> {
        match key.get_token(ast) {
            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF => return Some(key),
            Token::COMPUTED_PROP | Token::COMPUTED_FIELD_DEF => {
                let first = key.get_first_child(ast).unwrap();
                return if first.is_string_lit(ast) {
                    Some(first)
                } else {
                    None
                };
            }
            _ => {}
        }
        panic!("Unexpected node type: {}", key.to_string(ast));
    }
    // port: NodeUtil#isGetOrSetKey
    pub fn is_get_or_set_key(ast: &Ast, node: NodeId) -> bool {
        match node.get_token(ast) {
            Token::GETTER_DEF | Token::SETTER_DEF => true,
            Token::COMPUTED_PROP => {
                node.get_boolean_prop(ast, Prop::COMPUTED_PROP_GETTER)
                    || node.get_boolean_prop(ast, Prop::COMPUTED_PROP_SETTER)
            }
            _ => false,
        }
    }
}

use crate::{
    ast_analyzer::AstAnalyzer, change_tracker::ChangeTracker, coding_convention::CodingConvention,
    compiler_options::LanguageMode, scope_creator::ScopeCreator, var::VarId,
};
use closure_parsing::{
    parser::feature_set::{Feature, FeatureSet},
    parsing_util::ParsingUtil,
};
use closure_rhino::{node::ObjectProp, token_stream::TokenStream};

impl NodeUtil {
    // port: NodeUtil#opToStr
    pub fn op_to_str(operator: Token) -> Option<&'static str> {
        Some(match operator {
            Token::COALESCE => "??",
            Token::BITOR => "|",
            Token::OR => "||",
            Token::BITXOR => "^",
            Token::AND => "&&",
            Token::BITAND => "&",
            Token::SHEQ => "===",
            Token::EQ => "==",
            Token::NOT => "!",
            Token::NE => "!=",
            Token::SHNE => "!==",
            Token::LSH => "<<",
            Token::IN => "in",
            Token::LE => "<=",
            Token::LT => "<",
            Token::URSH => ">>>",
            Token::RSH => ">>",
            Token::GE => ">=",
            Token::GT => ">",
            Token::MUL => "*",
            Token::DIV => "/",
            Token::MOD => "%",
            Token::EXPONENT => "**",
            Token::BITNOT => "~",
            Token::ADD | Token::POS => "+",
            Token::SUB | Token::NEG => "-",
            Token::ASSIGN => "=",
            Token::ASSIGN_BITOR => "|=",
            Token::ASSIGN_BITXOR => "^=",
            Token::ASSIGN_BITAND => "&=",
            Token::ASSIGN_LSH => "<<=",
            Token::ASSIGN_RSH => ">>=",
            Token::ASSIGN_URSH => ">>>=",
            Token::ASSIGN_ADD => "+=",
            Token::ASSIGN_SUB => "-=",
            Token::ASSIGN_MUL => "*=",
            Token::ASSIGN_EXPONENT => "**=",
            Token::ASSIGN_DIV => "/=",
            Token::ASSIGN_MOD => "%=",
            Token::ASSIGN_OR => "||=",
            Token::ASSIGN_AND => "&&=",
            Token::ASSIGN_COALESCE => "??=",
            Token::VOID => "void",
            Token::TYPEOF => "typeof",
            Token::INSTANCEOF => "instanceof",
            _ => return None,
        })
    }

    // port: NodeUtil#opToStrNoFail
    pub fn op_to_str_no_fail(operator: Token) -> &'static str {
        let res = Self::op_to_str(operator);
        if res.is_none() {
            panic!("Unknown op {operator}");
        }
        res.unwrap()
    }
    // port: NodeUtil#redeclareVarsInsideBranch
    pub fn redeclare_vars_inside_branch(ast: &mut Ast, branch: NodeId) {
        if !Self::can_contain_hoisted_vars_decls(ast, branch) {
            return;
        }
        let vars = Self::get_vars_declared_in_branch(ast, branch);
        if vars.is_empty() {
            return;
        }
        let parent = Self::get_adding_root(ast, branch);
        for name_node in vars {
            let name_string = name_node.get_string(ast);
            let name = IR::name(ast, name_string).srcref(ast, name_node);
            let var = IR::var(ast, name).srcref(ast, name_node);
            let destination = var.get_first_child(ast).unwrap();
            Self::copy_name_annotations(ast, name_node, destination);
            parent.add_child_to_front(ast, var);
        }
    }
    // port: NodeUtil#canContainHoistedVarsDecls
    pub fn can_contain_hoisted_vars_decls(ast: &Ast, n: NodeId) -> bool {
        if Self::is_statement(ast, n) {
            return !matches!(
                n.get_token(ast),
                Token::EXPR_RESULT
                    | Token::RETURN
                    | Token::THROW
                    | Token::BREAK
                    | Token::CONTINUE
                    | Token::EMPTY
                    | Token::DEBUGGER
            );
        }
        true
    }
    // port: NodeUtil#copyNameAnnotations
    pub fn copy_name_annotations(ast: &mut Ast, source: NodeId, destination: NodeId) {
        if source.get_boolean_prop(ast, Prop::IS_CONSTANT_NAME) {
            destination.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
        }
    }
    // port: NodeUtil#getAddingRoot
    fn get_adding_root(ast: &Ast, n: NodeId) -> NodeId {
        let mut adding_root = None;
        let mut ancestor = n.get_parent(ast);
        while let Some(current) = ancestor {
            match current.get_token(ast) {
                Token::SCRIPT | Token::MODULE_BODY => {
                    adding_root = Some(current);
                    break;
                }
                Token::FUNCTION => {
                    adding_root = current.get_last_child(ast);
                    break;
                }
                _ => {
                    ancestor = current.get_parent(ast);
                    continue;
                }
            }
        }
        let adding_root = adding_root.expect("");
        check_state!(
            adding_root.is_block(ast)
                || adding_root.is_module_body(ast)
                || adding_root.is_script(ast)
        );
        check_state!(
            !adding_root.has_children(ast)
                || !adding_root.get_first_child(ast).unwrap().is_script(ast)
        );
        adding_root
    }
    // port: NodeUtil#newDeclaration
    pub fn new_declaration(
        ast: &mut Ast,
        lhs: NodeId,
        rhs: Option<NodeId>,
        declaration_type: Token,
    ) -> NodeId {
        if let Some(rhs) = rhs {
            IR::declaration_with_value(ast, lhs, rhs, declaration_type)
        } else {
            IR::declaration(ast, lhs, declaration_type)
        }
    }
    // port: NodeUtil#newQName(AbstractCompiler,String)
    pub fn new_qname(compiler: &mut AbstractCompiler, name: impl Into<JsString>) -> NodeId {
        let name = name.into();
        let dot = name.index_of(".");
        let mut end_pos = if dot == -1 {
            name.length()
        } else {
            dot as usize
        };
        let node_name = name.substring(0, end_pos);
        let mut qname = if node_name == "this" {
            IR::this_node(compiler)
        } else if node_name == "super" {
            IR::super_node(compiler)
        } else {
            Self::new_name(compiler, node_name)
        };
        qname.set_length(compiler, end_pos as i32);
        let mut start_pos = end_pos + 1;
        while end_pos < name.length() {
            let dot = name.index_of_from(".", start_pos as i32);
            end_pos = if dot == -1 {
                name.length()
            } else {
                dot as usize
            };
            let part = name.substring(start_pos, end_pos);
            qname = IR::getprop(compiler, qname, part.clone());
            if compiler.get_coding_convention().is_constant_key(&part) {
                qname.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            }
            qname.set_length(compiler, part.length() as i32);
            start_pos = end_pos + 1;
        }
        qname
    }
    // port: NodeUtil#newQName(AbstractCompiler,String,Node,String)
    pub fn new_qname_with_basis(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        basis_node: NodeId,
        original_name: impl Into<JsString>,
    ) -> NodeId {
        let node = Self::new_qname(compiler, name);
        node.srcref_tree_if_missing(compiler, basis_node);
        let original_name = original_name.into();
        if node.get_original_name(compiler).as_ref() != Some(&original_name) {
            node.set_original_name(compiler, Some(original_name));
        }
        node
    }
    // port: NodeUtil#getDeclarationFromName
    pub fn get_declaration_from_name(
        ast: &mut Ast,
        name_node: NodeId,
        value: Option<NodeId>,
        type_: Token,
        info: Option<Arc<JSDocInfo>>,
    ) -> NodeId {
        let result;
        if name_node.is_name(ast) {
            result = Self::new_declaration(ast, name_node, value, type_);
            result.set_jsdoc_info(ast, info);
        } else if let Some(value) = value {
            let assign = IR::assign(ast, name_node, value);
            result = IR::expr_result(ast, assign);
            result
                .get_first_child(ast)
                .unwrap()
                .set_jsdoc_info(ast, info);
        } else {
            result = IR::expr_result(ast, name_node);
            result
                .get_first_child(ast)
                .unwrap()
                .set_jsdoc_info(ast, info);
        }
        result
    }
    // port: NodeUtil#newPropertyAccess
    pub fn new_property_access(
        compiler: &mut AbstractCompiler,
        context: NodeId,
        name: impl Into<JsString>,
    ) -> NodeId {
        let name = name.into();
        let prop_node = IR::getprop(compiler, context, name.clone());
        if compiler.get_coding_convention().is_constant_key(&name) {
            prop_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }
        prop_node
    }
    // port: NodeUtil#newQNameDeclaration(AbstractCompiler,String,Node,JSDocInfo)
    pub fn new_qname_declaration_var(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        value: Option<NodeId>,
        info: Option<Arc<JSDocInfo>>,
    ) -> NodeId {
        Self::new_qname_declaration(compiler, name, value, info, Token::VAR)
    }
    // port: NodeUtil#newQNameDeclaration(AbstractCompiler,String,Node,JSDocInfo,Token)
    pub fn new_qname_declaration(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        value: Option<NodeId>,
        info: Option<Arc<JSDocInfo>>,
        type_: Token,
    ) -> NodeId {
        check_state!(
            matches!(type_, Token::VAR | Token::LET | Token::CONST),
            "%s",
            type_
        );
        let name_node = Self::new_qname(compiler, name);
        Self::get_declaration_from_name(compiler, name_node, value, type_, info)
    }
    // port: NodeUtil#getRootOfQualifiedName(Node)
    pub fn get_root_of_qualified_name(ast: &Ast, q_name: NodeId) -> NodeId {
        let mut current = q_name;
        loop {
            if current.is_name(ast) || current.is_this(ast) || current.is_super(ast) {
                return current;
            }
            check_state!(
                current.is_get_prop(ast),
                "Not a getprop node:  (%s)",
                current.to_string(ast)
            );
            current = current.get_first_child(ast).unwrap();
        }
    }
    // port: NodeUtil#getRootOfQualifiedName(String)
    pub fn get_root_of_qualified_name_string(q_name: &JsString) -> JsString {
        let dot = q_name.index_of(".");
        if dot == -1 {
            q_name.clone()
        } else {
            q_name.substring(0, dot as usize)
        }
    }
    // port: NodeUtil#newName(AbstractCompiler,String)
    pub fn new_name(compiler: &mut AbstractCompiler, name: impl Into<JsString>) -> NodeId {
        let name = name.into();
        let name_node = IR::name(compiler, name.clone());
        name_node.set_length(compiler, name.length() as i32);
        if compiler.get_coding_convention().is_constant(&name) {
            name_node.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        }
        name_node
    }
    // port: NodeUtil#newName(AbstractCompiler,String,Node)
    pub fn new_name_with_srcref(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        srcref: NodeId,
    ) -> NodeId {
        Self::new_name(compiler, name).srcref(compiler, srcref)
    }
    // port: NodeUtil#newName(AbstractCompiler,String,Node,String)
    pub fn new_name_with_basis(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        basis_node: NodeId,
        original_name: impl Into<JsString>,
    ) -> NodeId {
        let name_node = Self::new_name_with_srcref(compiler, name, basis_node);
        name_node.set_original_name(compiler, Some(original_name.into()));
        name_node
    }

    // port: NodeUtil#isLatin
    pub fn is_latin(s: &JsString) -> bool {
        let len = s.length();
        for index in 0..len {
            let c = s.char_at(index);
            if c > Self::LARGEST_BASIC_LATIN {
                return false;
            }
        }
        true
    }

    // port: NodeUtil#isValidSimpleName
    pub fn is_valid_simple_name(name: &JsString) -> bool {
        TokenStream::is_js_identifier(name)
            && !TokenStream::is_keyword(name)
            && Self::is_latin(name)
    }
    // port: NodeUtil#isValidQualifiedName(LanguageMode,String)
    pub fn is_valid_qualified_name(mode: LanguageMode, name: &JsString) -> bool {
        Self::is_valid_qualified_name_features(mode.to_feature_set(), name)
    }
    // port: NodeUtil#isValidQualifiedName(FeatureSet,String)
    pub fn is_valid_qualified_name_features(mode: FeatureSet, name: &JsString) -> bool {
        if name.ends_with(".") || name.starts_with(".") {
            return false;
        }
        let mut parts = Vec::new();
        let mut start = 0;
        loop {
            let dot = name.index_of_from(".", start as i32);
            let end = if dot == -1 {
                name.length()
            } else {
                dot as usize
            };
            parts.push(name.substring(start, end));
            if dot == -1 {
                break;
            }
            start = end + 1;
        }
        for part in &parts {
            if !Self::is_valid_property_name(mode, part) {
                return false;
            }
        }
        Self::is_valid_simple_name(&parts[0])
    }
    // port: NodeUtil#isValidPropertyName
    pub fn is_valid_property_name(mode: FeatureSet, name: &JsString) -> bool {
        if Self::is_valid_simple_name(name) {
            true
        } else {
            mode.has(Feature::KEYWORDS_AS_PROPERTIES) && TokenStream::is_keyword(name)
        }
    }
}

// Rust-only context keeps consumers on the caller's arena owner while retaining one Java walk.
pub trait AstContext {
    fn ast(&self) -> &Ast;
    fn ast_mut(&mut self) -> &mut Ast;
}

impl AstContext for Ast {
    fn ast(&self) -> &Ast {
        self
    }
    fn ast_mut(&mut self) -> &mut Ast {
        self
    }
}

impl AstContext for AbstractCompiler {
    fn ast(&self) -> &Ast {
        &self.ast
    }
    fn ast_mut(&mut self) -> &mut Ast {
        &mut self.ast
    }
}

#[derive(Default)]
pub struct VarCollector {
    vars: IndexMap<JsString, NodeId>,
}
impl Visitor for VarCollector {
    // port: NodeUtil.VarCollector#visit
    fn visit(&mut self, ast: &mut Ast, n: NodeId) {
        if n.is_var(ast) {
            NodeUtil::visit_lhs_nodes_in_node(ast, n, &mut |ast, name| {
                self.vars.entry(name.get_string(ast)).or_insert(name);
            });
        }
    }
}

impl NodeUtil {
    // port: NodeUtil#getVarsDeclaredInBranch
    pub fn get_vars_declared_in_branch(ast: &mut Ast, root: NodeId) -> Vec<NodeId> {
        let mut collector = VarCollector::default();
        Self::visit_pre_order_with_predicate(ast, root, &mut collector, &Self::MATCH_NOT_FUNCTION);
        collector.vars.into_values().collect()
    }
    // port: NodeUtil#getLhsNodesHelper
    fn get_lhs_nodes_helper<C: AstContext + ?Sized>(
        context: &mut C,
        n: NodeId,
        consumer: &mut dyn FnMut(&mut C, NodeId),
    ) {
        match n.get_token(context.ast()) {
            Token::IMPORT => {
                let first = n.get_first_child(context.ast()).unwrap();
                Self::get_lhs_nodes_helper(context, first, consumer);
                let second = n.get_second_child(context.ast()).unwrap();
                Self::get_lhs_nodes_helper(context, second, consumer);
            }
            Token::VAR
            | Token::CONST
            | Token::LET
            | Token::OBJECT_PATTERN
            | Token::ARRAY_PATTERN
            | Token::PARAM_LIST
            | Token::IMPORT_SPECS => {
                let mut child = n.get_first_child(context.ast());
                while let Some(current) = child {
                    Self::get_lhs_nodes_helper(context, current, consumer);
                    child = current.get_next(context.ast());
                }
            }
            Token::DESTRUCTURING_LHS
            | Token::DEFAULT_VALUE
            | Token::CATCH
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::CAST => {
                let first = n.get_first_child(context.ast()).unwrap();
                Self::get_lhs_nodes_helper(context, first, consumer)
            }
            Token::IMPORT_SPEC | Token::COMPUTED_PROP | Token::STRING_KEY => {
                let last = n.get_last_child(context.ast()).unwrap();
                Self::get_lhs_nodes_helper(context, last, consumer)
            }
            Token::NAME | Token::IMPORT_STAR | Token::GETPROP | Token::GETELEM => {
                consumer(context, n)
            }
            Token::EMPTY => {}
            Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                let first = n.get_first_child(context.ast()).unwrap();
                Self::get_lhs_nodes_helper(context, first, consumer)
            }
            _ => {
                if Self::is_assignment_op(context.ast(), n) {
                    let first = n.get_first_child(context.ast()).unwrap();
                    Self::get_lhs_nodes_helper(context, first, consumer);
                } else {
                    panic!("Invalid node in lhs: {}", n.to_string(context.ast()));
                }
            }
        }
    }
    // port: NodeUtil#visitLhsNodesInNode
    pub fn visit_lhs_nodes_in_node<C: AstContext + ?Sized>(
        context: &mut C,
        assigning_parent: NodeId,
        consumer: &mut dyn FnMut(&mut C, NodeId),
    ) {
        check_argument!(
            Self::is_name_declaration(context.ast(), Some(assigning_parent))
                || assigning_parent.is_param_list(context.ast())
                || Self::is_assignment_op(context.ast(), assigning_parent)
                || assigning_parent.is_catch(context.ast())
                || assigning_parent.is_destructuring_lhs(context.ast())
                || assigning_parent.is_default_value(context.ast())
                || assigning_parent.is_import(context.ast())
                || Self::is_enhanced_for(context.ast(), assigning_parent),
            "%s",
            assigning_parent.to_string(context.ast())
        );
        Self::get_lhs_nodes_helper(context, assigning_parent, consumer);
    }
    // port: NodeUtil#visitLhsNodesInDestructuringPattern
    pub fn visit_lhs_nodes_in_destructuring_pattern<C: AstContext + ?Sized>(
        context: &mut C,
        destructuring_pattern: NodeId,
        consumer: &mut dyn FnMut(&mut C, NodeId),
    ) {
        check_argument!(
            destructuring_pattern.is_destructuring_pattern(context.ast()),
            "Must be a destructuring pattern node."
        );
        Self::get_lhs_nodes_helper(context, destructuring_pattern, consumer);
    }
    // port: NodeUtil#isObjectDefinePropertiesDefinition
    pub fn is_object_define_properties_definition(ast: &Ast, n: NodeId) -> bool {
        if !n.is_call(ast) || !n.has_x_children(ast, 3) {
            return false;
        }
        let getprop = n.get_first_child(ast).unwrap();
        if !getprop.is_get_prop(ast) {
            return false;
        }
        getprop.get_string_ref(ast) == "defineProperties"
            && Self::is_known_global_object_reference(ast, getprop.get_first_child(ast).unwrap())
    }
}

static GLOBAL_OBJECT: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("$jscomp.global.Object"));
static GLOBAL_OBJECT_MANGLED: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("$jscomp$global.Object"));

impl NodeUtil {
    // port: NodeUtil#isKnownGlobalObjectReference
    fn is_known_global_object_reference(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::NAME => n.get_string_ref(ast) == "Object",
            Token::GETPROP => {
                GLOBAL_OBJECT.matches(ast, n) || GLOBAL_OBJECT_MANGLED.matches(ast, n)
            }
            _ => false,
        }
    }
    // port: NodeUtil#isObjectDefinePropertyDefinition
    pub fn is_object_define_property_definition(ast: &Ast, n: NodeId) -> bool {
        if !n.is_call(ast) || !n.has_x_children(ast, 4) {
            return false;
        }
        let getprop = n.get_first_child(ast).unwrap();
        if !getprop.is_get_prop(ast) {
            return false;
        }
        getprop.get_string_ref(ast) == "defineProperty"
            && Self::is_known_global_object_reference(ast, getprop.get_first_child(ast).unwrap())
    }
    // port: NodeUtil#getObjectDefinedPropertiesKeys
    pub fn get_object_defined_properties_keys(
        ast: &Ast,
        define_properties_call: NodeId,
    ) -> Vec<NodeId> {
        check_argument!(Self::is_object_define_properties_definition(
            ast,
            define_properties_call
        ));
        let mut properties = Vec::new();
        let object_literal = define_properties_call.get_last_child(ast).unwrap();
        for key in object_literal.children(ast) {
            if !key.is_string_key(ast) {
                continue;
            }
            properties.push(key);
        }
        properties
    }
    // port: NodeUtil#isPrototypePropertyDeclaration
    pub fn is_prototype_property_declaration(ast: &Ast, n: NodeId) -> bool {
        Self::is_expr_assign(ast, n)
            && Self::is_prototype_property(ast, n.get_first_first_child(ast).unwrap())
    }
    // port: NodeUtil#isPrototypeProperty
    pub fn is_prototype_property(ast: &Ast, n: NodeId) -> bool {
        if !n.is_get_prop(ast) {
            return false;
        }
        let recv = n.get_first_child(ast).unwrap();
        recv.is_get_prop(ast) && recv.get_string_ref(ast) == "prototype"
    }
    // port: NodeUtil#isPrototypeMethod
    pub fn is_prototype_method(ast: &Ast, n: NodeId) -> bool {
        if !n.is_function(ast) {
            return false;
        }
        let assign_node = n.get_parent(ast).unwrap();
        if !assign_node.is_assign(ast) {
            return false;
        }
        Self::is_prototype_property_declaration(ast, assign_node.get_parent(ast).unwrap())
    }
    // port: NodeUtil#isPrototypeAssignment
    pub fn is_prototype_assignment(ast: &Ast, get_prop: NodeId) -> bool {
        if !get_prop.is_get_prop(ast) {
            return false;
        }
        let parent = get_prop.get_parent(ast).unwrap();
        parent.is_assign(ast)
            && get_prop.is_first_child_of(ast, Some(parent))
            && get_prop.get_string_ref(ast) == "prototype"
    }
    // port: NodeUtil#isPropertyTest
    pub fn is_property_test(compiler: &AbstractCompiler, prop_access: NodeId) -> bool {
        let parent = prop_access.get_parent(compiler).unwrap();
        match parent.get_token(compiler) {
            Token::CALL => {
                parent.get_first_child(compiler) != Some(prop_access)
                    && compiler
                        .get_coding_convention()
                        .is_property_test_function(compiler, parent)
            }
            Token::IF | Token::WHILE | Token::DO | Token::FOR | Token::FOR_IN => {
                Self::get_condition_expression(compiler, parent) == Some(prop_access)
            }
            Token::INSTANCEOF
            | Token::TYPEOF
            | Token::AND
            | Token::OR
            | Token::COALESCE
            | Token::OPTCHAIN_GETPROP => true,
            Token::NE | Token::SHNE => {
                let other = if parent.get_first_child(compiler) == Some(prop_access) {
                    parent.get_second_child(compiler).unwrap()
                } else {
                    parent.get_first_child(compiler).unwrap()
                };
                Self::is_undefined(compiler, other)
                    || (parent.is_ne(compiler) && other.is_null(compiler))
            }
            Token::OPTCHAIN_CALL | Token::OPTCHAIN_GETELEM | Token::HOOK => {
                parent.get_first_child(compiler) == Some(prop_access)
            }
            Token::NOT => {
                let grandparent = parent.get_parent(compiler).unwrap();
                grandparent.is_or(compiler) && grandparent.get_first_child(compiler) == Some(parent)
            }
            Token::CAST => Self::is_property_test(compiler, parent),
            _ => false,
        }
    }
    // port: NodeUtil#isPropertyAbsenceTest
    pub fn is_property_absence_test(ast: &Ast, prop_access: NodeId) -> bool {
        let parent = prop_access.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::EQ | Token::SHEQ => {
                let other = if parent.get_first_child(ast) == Some(prop_access) {
                    parent.get_second_child(ast).unwrap()
                } else {
                    parent.get_first_child(ast).unwrap()
                };
                Self::is_undefined(ast, other) || (parent.is_eq(ast) && other.is_null(ast))
            }
            _ => false,
        }
    }
    // port: NodeUtil#getPrototypeClassName
    pub fn get_prototype_class_name(ast: &Ast, q_name: NodeId) -> Option<NodeId> {
        if !q_name.is_get_prop(ast) {
            return None;
        }
        if q_name.get_string_ref(ast) == "prototype" {
            return q_name.get_first_child(ast);
        }
        let recv = q_name.get_first_child(ast).unwrap();
        if recv.is_get_prop(ast) && recv.get_string_ref(ast) == "prototype" {
            return recv.get_first_child(ast);
        }
        None
    }
    // port: NodeUtil#getPrototypePropertyName
    pub fn get_prototype_property_name(ast: &Ast, q_name: NodeId) -> JsString {
        let q_name_str = q_name.get_qualified_name(ast).unwrap();
        let prototype_idx = q_name_str.last_index_of(".prototype.");
        let member_index = prototype_idx + ".prototype".len() as i32 + 1;
        q_name_str.substring_from(member_index as usize)
    }
    // port: NodeUtil#newUndefinedNode
    pub fn new_undefined_node(ast: &mut Ast, src_reference_node: Option<NodeId>) -> NodeId {
        let number = IR::number(ast, 0.0);
        let node = IR::void_node(ast, number);
        if let Some(src_reference_node) = src_reference_node {
            node.srcref_tree(ast, src_reference_node);
        }
        node
    }
    // port: NodeUtil#newVarNode(String,Node)
    pub fn new_var_node(ast: &mut Ast, name: impl Into<JsString>, value: Option<NodeId>) -> NodeId {
        let lhs = IR::name(ast, name);
        if let Some(value) = value {
            lhs.srcref(ast, value);
        }
        Self::new_var_node_with_lhs(ast, lhs, value)
    }
    // port: NodeUtil#newVarNode(Node,Node)
    pub fn new_var_node_with_lhs(ast: &mut Ast, lhs: NodeId, value: Option<NodeId>) -> NodeId {
        if lhs.is_destructuring_pattern(ast) {
            let value = check_not_null!(value);
            let destructuring_lhs = ast
                .new_node_with_children2(Token::DESTRUCTURING_LHS, lhs, value)
                .srcref(ast, lhs);
            IR::var(ast, destructuring_lhs).srcref(ast, lhs)
        } else {
            check_state!(lhs.is_name(ast) && !lhs.has_children(ast));
            if let Some(value) = value {
                lhs.add_child_to_back(ast, value);
            }
            IR::var(ast, lhs).srcref(ast, lhs)
        }
    }
    // port: NodeUtil#emptyFunction
    pub fn empty_function(ast: &mut Ast) -> NodeId {
        let name = IR::name(ast, "");
        let params = IR::param_list(ast, &[]);
        let body = IR::block(ast);
        IR::function(ast, name, params, body)
    }
}

pub struct MatchNameNode {
    name: JsString,
}
impl MatchNameNode {
    // port: NodeUtil.MatchNameNode#MatchNameNode
    pub fn new(name: impl Into<JsString>) -> Self {
        Self { name: name.into() }
    }
    // port: NodeUtil.MatchNameNode#apply
    pub fn apply(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_name(ast) && n.get_string(ast) == self.name
    }
}

pub struct MatchNodeType {
    type_: Token,
}
impl MatchNodeType {
    // port: NodeUtil.MatchNodeType#MatchNodeType
    pub fn new(type_: Token) -> Self {
        Self { type_ }
    }
    // port: NodeUtil.MatchNodeType#apply
    pub fn apply(&self, ast: &Ast, n: NodeId) -> bool {
        n.get_token(ast) == self.type_
    }
}

pub struct MatchDeclaration;
impl MatchDeclaration {
    // port: NodeUtil.MatchDeclaration#apply
    pub fn apply(&self, ast: &Ast, n: NodeId) -> bool {
        NodeUtil::is_declaration(ast, n)
    }
}

impl NodeUtil {
    pub const MATCH_NOT_FUNCTION: fn(&Ast, NodeId) -> bool = |ast, n| !n.is_function(ast);
    pub const MATCH_ANYTHING_BUT_NON_ARROW_FUNCTION: fn(&Ast, NodeId) -> bool =
        |ast, n| !Self::is_non_arrow_function(ast, n);
    // port: NodeUtil#isShallowStatementTree
    pub fn is_shallow_statement_tree(ast: &Ast, n: Option<NodeId>) -> bool {
        n.is_none_or(|n| {
            Self::is_control_structure(ast, n)
                || Self::is_statement_block(ast, n)
                || n.is_switch_body(ast)
        })
    }
}

pub struct MatchShallowStatement;
impl MatchShallowStatement {
    // port: NodeUtil.MatchShallowStatement#apply
    pub fn apply(&self, ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast);
        n.is_root(ast)
            || n.is_block(ast)
            || (!n.is_function(ast) && NodeUtil::is_shallow_statement_tree(ast, parent))
    }
}

impl NodeUtil {
    // port: NodeUtil#getNodeTypeReferenceCount
    pub fn get_node_type_reference_count(
        ast: &Ast,
        node: NodeId,
        type_: Token,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> i32 {
        let matcher = MatchNodeType::new(type_);
        Self::get_count(
            ast,
            node,
            &|ast, node| matcher.apply(ast, node),
            traverse_children_pred,
        )
    }
    // port: NodeUtil#isNameReferenced(Node,String,Predicate)
    pub fn is_name_referenced_with_predicate(
        ast: &Ast,
        node: NodeId,
        name: impl Into<JsString>,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> bool {
        let matcher = MatchNameNode::new(name);
        Self::has(
            ast,
            node,
            &|ast, node| matcher.apply(ast, node),
            traverse_children_pred,
        )
    }
    // port: NodeUtil#isNameReferenced(Node,String)
    pub fn is_name_referenced(ast: &Ast, node: NodeId, name: impl Into<JsString>) -> bool {
        Self::is_name_referenced_with_predicate(ast, node, name, &|_, _| true)
    }
    // port: NodeUtil#getNameReferenceCount
    pub fn get_name_reference_count(ast: &Ast, node: NodeId, name: impl Into<JsString>) -> i32 {
        let matcher = MatchNameNode::new(name);
        Self::get_count(ast, node, &|ast, node| matcher.apply(ast, node), &|_, _| {
            true
        })
    }

    // port: NodeUtil#has
    pub fn has(
        ast: &Ast,
        node: NodeId,
        pred: &dyn Fn(&Ast, NodeId) -> bool,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> bool {
        if pred(ast, node) {
            return true;
        }
        if !traverse_children_pred(ast, node) {
            return false;
        }
        for c in node.children(ast) {
            if Self::has(ast, c, pred, traverse_children_pred) {
                return true;
            }
        }
        false
    }

    // port: NodeUtil#findPreorder
    pub fn find_preorder(
        ast: &Ast,
        node: NodeId,
        pred: &dyn Fn(&Ast, NodeId) -> bool,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> Option<NodeId> {
        if pred(ast, node) {
            return Some(node);
        }
        if !traverse_children_pred(ast, node) {
            return None;
        }
        for c in node.children(ast) {
            let result = Self::find_preorder(ast, c, pred, traverse_children_pred);
            if result.is_some() {
                return result;
            }
        }
        None
    }
    // port: NodeUtil#getCount
    pub fn get_count(
        ast: &Ast,
        n: NodeId,
        pred: &dyn Fn(&Ast, NodeId) -> bool,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> i32 {
        let mut total: i32 = 0;
        if pred(ast, n) {
            total = total.wrapping_add(1);
        }
        if traverse_children_pred(ast, n) {
            for c in n.children(ast) {
                total = total.wrapping_add(Self::get_count(ast, c, pred, traverse_children_pred));
            }
        }
        total
    }
}

pub trait Visitor {
    // port: NodeUtil.Visitor#visit
    fn visit(&mut self, ast: &mut Ast, node: NodeId);
}

impl<F: FnMut(&mut Ast, NodeId)> Visitor for F {
    fn visit(&mut self, ast: &mut Ast, node: NodeId) {
        self(ast, node);
    }
}

impl NodeUtil {
    // port: NodeUtil#visitPreOrder(Node,Visitor)
    pub fn visit_pre_order(ast: &mut Ast, node: NodeId, visitor: &mut dyn Visitor) {
        Self::visit_pre_order_with_predicate(ast, node, visitor, &|_, _| true);
    }
    // port: NodeUtil#visitPreOrder(Node,Visitor,Predicate)
    pub fn visit_pre_order_with_predicate(
        ast: &mut Ast,
        node: NodeId,
        visitor: &mut dyn Visitor,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) {
        visitor.visit(ast, node);
        if traverse_children_pred(ast, node) {
            let mut c = node.get_first_child(ast);
            while let Some(current) = c {
                Self::visit_pre_order_with_predicate(ast, current, visitor, traverse_children_pred);
                c = current.get_next(ast);
            }
        }
    }
    // port: NodeUtil#visitPostOrder(Node,Visitor)
    pub fn visit_post_order(ast: &mut Ast, node: NodeId, visitor: &mut dyn Visitor) {
        Self::visit_post_order_with_predicate(ast, node, visitor, &|_, _| true);
    }
    // port: NodeUtil#visitPostOrder(Node,Visitor,Predicate)
    pub fn visit_post_order_with_predicate(
        ast: &mut Ast,
        node: NodeId,
        visitor: &mut dyn Visitor,
        traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
    ) {
        if traverse_children_pred(ast, node) {
            let mut c = node.get_first_child(ast);
            while let Some(current) = c {
                let next = current.get_next(ast);
                Self::visit_post_order_with_predicate(
                    ast,
                    current,
                    visitor,
                    traverse_children_pred,
                );
                c = next;
            }
        }
        visitor.visit(ast, node);
    }
    // port: NodeUtil#preOrderIterable(Node,Predicate)
    pub fn pre_order_iterable_with_predicate(
        ast: &Ast,
        root: NodeId,
        traverse_node_predicate: &dyn Fn(&Ast, NodeId) -> bool,
    ) -> Vec<NodeId> {
        PreOrderIterator::new(ast, root, traverse_node_predicate).collect()
    }
    // port: NodeUtil#preOrderIterable(Node)
    pub fn pre_order_iterable(ast: &Ast, root: NodeId) -> Vec<NodeId> {
        Self::pre_order_iterable_with_predicate(ast, root, &|_, _| true)
    }
}

pub struct PreOrderIterator<'a> {
    traverse_node_predicate: &'a dyn Fn(&Ast, NodeId) -> bool,
    current: Option<NodeId>,
    ast: &'a Ast,
}
impl<'a> PreOrderIterator<'a> {
    // port: NodeUtil.PreOrderIterator#PreOrderIterator
    pub fn new(
        ast: &'a Ast,
        root: NodeId,
        traverse_node_predicate: &'a dyn Fn(&Ast, NodeId) -> bool,
    ) -> Self {
        Self {
            traverse_node_predicate,
            current: Some(root),
            ast,
        }
    }
    // port: NodeUtil.PreOrderIterator#computeNext
    pub fn compute_next(&mut self) -> Option<NodeId> {
        let return_value = self.current?;
        self.current = self.calculate_next_node(return_value);
        Some(return_value)
    }
    // port: NodeUtil.PreOrderIterator#calculateNextNode
    fn calculate_next_node(&self, current_node: NodeId) -> Option<NodeId> {
        if (self.traverse_node_predicate)(self.ast, current_node)
            && current_node.has_children(self.ast)
        {
            return current_node.get_first_child(self.ast);
        }
        let mut current_node = Some(current_node);
        while let Some(current) = current_node {
            let next = current.get_next(self.ast);
            if next.is_some() {
                return next;
            }
            current_node = current.get_parent(self.ast);
        }
        None
    }
}
impl Iterator for PreOrderIterator<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        self.compute_next()
    }
}

impl NodeUtil {
    // port: NodeUtil#isExportFrom
    pub fn is_export_from(ast: &Ast, n: NodeId) -> bool {
        check_argument!(n.is_export(ast));
        n.has_two_children(ast)
    }

    // port: NodeUtil#hasFinally
    pub fn has_finally(ast: &Ast, n: NodeId) -> bool {
        check_argument!(n.is_try(ast));
        n.has_x_children(ast, 3)
    }

    // port: NodeUtil#getCatchBlock
    pub fn get_catch_block(ast: &Ast, n: NodeId) -> NodeId {
        check_argument!(n.is_try(ast));
        n.get_second_child(ast).unwrap()
    }
    // port: NodeUtil#hasCatchHandler
    pub fn has_catch_handler(ast: &Ast, n: NodeId) -> bool {
        check_argument!(n.is_block(ast));
        n.has_children(ast) && n.get_first_child(ast).unwrap().is_catch(ast)
    }
    // port: NodeUtil#getFunctionParameters
    pub fn get_function_parameters(ast: &Ast, fn_node: NodeId) -> NodeId {
        check_argument!(fn_node.is_function(ast));
        fn_node.get_second_child(ast).unwrap()
    }
    // port: NodeUtil#hasNonSimpleParameters
    pub fn has_non_simple_parameters(ast: &Ast, fn_node: NodeId) -> bool {
        check_argument!(fn_node.is_function(ast));
        let param_list = Self::get_function_parameters(ast, fn_node);
        for param in param_list.children(ast) {
            if !param.is_name(ast) {
                return true;
            }
        }
        false
    }
    // port: NodeUtil#functionParametersMayHaveSideEffects
    pub fn function_parameters_may_have_side_effects<
        C: crate::ast_analyzer::AstAnalyzerContext + ?Sized,
    >(
        cx: &mut C,
        fn_node: NodeId,
        ast_analyzer: &AstAnalyzer,
    ) -> bool {
        let ast = cx.get_ast();
        check_argument!(fn_node.is_function(ast));
        let param_list = Self::get_function_parameters(ast, fn_node);
        let params: Vec<NodeId> = param_list.children(ast).collect();
        for param in params {
            let ast = cx.get_ast();
            if param.is_name(ast) {
                continue;
            }
            if param.is_rest(ast) {
                if param.get_first_child(ast).unwrap().is_name(ast) {
                    continue;
                }
                return true;
            }
            if param.is_default_value(ast) {
                let target = param.get_first_child(ast).unwrap();
                let initializer = param.get_second_child(ast).unwrap();
                if !target.is_name(ast) {
                    return true;
                }
                if ast_analyzer.may_have_side_effects(cx, initializer) {
                    return true;
                }
                continue;
            }
            return true;
        }
        false
    }
    // port: NodeUtil#isConstantVar
    pub fn is_constant_var(
        compiler: &mut AbstractCompiler,
        node: NodeId,
        scope: Option<ScopeId>,
    ) -> bool {
        if Self::is_constant_name(compiler, node) {
            return true;
        }
        if !node.is_name(compiler) || scope.is_none() {
            return false;
        }
        let name = node.get_string(compiler);
        let var = scope.unwrap().get_var(compiler, name);
        var.is_some_and(|var| var.is_declared_or_inferred_const(compiler) || var.is_const(compiler))
    }
    // port: NodeUtil#isConstantName
    pub fn is_constant_name(ast: &Ast, node: NodeId) -> bool {
        node.get_boolean_prop(ast, Prop::IS_CONSTANT_NAME)
    }
    // port: NodeUtil#isConstantByConvention
    pub fn is_constant_by_convention(
        ast: &Ast,
        convention: &dyn CodingConvention,
        node: NodeId,
    ) -> bool {
        if Self::is_normal_or_opt_chain_get_prop(ast, node)
            || Self::may_be_object_lit_key(ast, node)
        {
            convention.is_constant_key(&node.get_string(ast))
        } else if node.is_name(ast) {
            convention.is_constant(&node.get_string(ast))
        } else {
            false
        }
    }
    // port: NodeUtil#isConstantDeclaration
    pub fn is_constant_declaration(ast: &Ast, info: Option<&JSDocInfo>, node: NodeId) -> bool {
        if Self::is_object_lit_key(ast, node)
            || (node.get_parent(ast).unwrap().is_assign(ast)
                && node.is_first_child_of(ast, node.get_parent(ast)))
            || (node.get_parent(ast).unwrap().is_expr_result(ast) && Self::is_normal_get(ast, node))
            || node.is_member_field_def(ast)
            || node.is_computed_field_def(ast)
            || node.is_computed_prop(ast)
        {
            return info.is_some_and(JSDocInfo::is_constant);
        }
        check_argument!(node.is_name(ast), "%s", node.to_string(ast));
        let declaring_parent = Self::get_declaring_parent(ast, node);
        if declaring_parent.is_const(ast) || info.is_some_and(JSDocInfo::is_constant) {
            return true;
        }
        node.is_inferred_constant_var(ast)
    }
    // port: NodeUtil#functionHasInlineJsdocs
    pub fn function_has_inline_jsdocs(ast: &Ast, function: NodeId) -> bool {
        if !function.is_function(ast) {
            return false;
        }
        if function
            .get_first_child(ast)
            .unwrap()
            .get_jsdoc_info(ast)
            .is_some()
        {
            return true;
        }
        let params = function.get_second_child(ast).unwrap();
        for param in params.children(ast) {
            if param.get_jsdoc_info(ast).is_some() {
                return true;
            }
        }
        false
    }
    // port: NodeUtil#getSourceName
    pub fn get_source_name(ast: &Ast, n: NodeId) -> Option<String> {
        let mut source_name = None;
        let mut n = Some(n);
        while source_name.is_none() && n.is_some() {
            let current = n.unwrap();
            source_name = current.get_source_file_name(ast);
            n = current.get_parent(ast);
        }
        source_name
    }

    // port: NodeUtil#getSourceFile
    pub fn get_source_file(ast: &Ast, mut n: Option<NodeId>) -> Option<Arc<dyn StaticSourceFile>> {
        let mut source_name = None;
        while source_name.is_none() && n.is_some() {
            let node = n.unwrap();
            source_name = node.get_static_source_file(ast);
            n = node.get_parent(ast);
        }
        source_name
    }

    // port: NodeUtil#getInputId
    #[allow(clippy::collapsible_if)] // Retain Java control flow.
    pub fn get_input_id(ast: &Ast, n: NodeId) -> Option<Arc<InputId>> {
        let mut n = Some(n);
        while let Some(current) = n {
            if current.is_script(ast) {
                break;
            }
            n = current.get_parent(ast);
        }
        if let Some(current) = n {
            if current.is_script(ast) {
                return current.get_input_id(ast);
            }
        }
        None
    }
    // port: NodeUtil#newCallNode
    pub fn new_call_node(ast: &mut Ast, call_target: NodeId, parameters: &[NodeId]) -> NodeId {
        let is_free_call = !Self::is_normal_get(ast, call_target);
        let call = IR::call(ast, call_target, &[]);
        call.put_boolean_prop(ast, Prop::FREE_CALL, is_free_call);
        for parameter in parameters {
            call.add_child_to_back(ast, *parameter);
        }
        call
    }
    // port: NodeUtil#evaluatesToLocalValue
    pub fn evaluates_to_local_value(ast: &Ast, value: NodeId) -> bool {
        match value.get_token(ast) {
            Token::ASSIGN => Self::is_immutable_value(ast, value.get_last_child(ast).unwrap()),
            Token::COMMA => Self::evaluates_to_local_value(ast, value.get_last_child(ast).unwrap()),
            Token::AND | Token::OR | Token::COALESCE => {
                Self::evaluates_to_local_value(ast, value.get_first_child(ast).unwrap())
                    && Self::evaluates_to_local_value(ast, value.get_last_child(ast).unwrap())
            }
            Token::HOOK => {
                Self::evaluates_to_local_value(ast, value.get_second_child(ast).unwrap())
                    && Self::evaluates_to_local_value(ast, value.get_last_child(ast).unwrap())
            }
            Token::DYNAMIC_IMPORT => true,
            Token::THIS | Token::SUPER => false,
            Token::NAME => Self::is_immutable_value(ast, value),
            Token::GETELEM | Token::GETPROP | Token::OPTCHAIN_GETELEM | Token::OPTCHAIN_GETPROP => {
                false
            }
            Token::CALL | Token::OPTCHAIN_CALL => Self::is_to_string_method_call(ast, value),
            Token::TAGGED_TEMPLATELIT => false,
            Token::NEW => Self::new_has_local_result(ast, value),
            Token::DELPROP
            | Token::INC
            | Token::DEC
            | Token::CLASS
            | Token::FUNCTION
            | Token::REGEXP
            | Token::EMPTY
            | Token::ARRAYLIT
            | Token::OBJECTLIT
            | Token::TEMPLATELIT => true,
            Token::CAST => Self::evaluates_to_local_value(ast, value.get_first_child(ast).unwrap()),
            Token::ITER_SPREAD
            | Token::OBJECT_SPREAD
            | Token::NEW_TARGET
            | Token::YIELD
            | Token::AWAIT => false,
            _ => {
                if Self::is_logical_assignment_op(ast, value) {
                    return false;
                }
                if Self::is_assignment_op(ast, value)
                    || Self::is_simple_operator(ast, value)
                    || Self::is_immutable_value(ast, value)
                {
                    return true;
                }
                panic!(
                    "Unexpected expression node: {}\n parent:{}",
                    value.to_string(ast),
                    value
                        .get_parent(ast)
                        .map_or_else(|| "null".to_owned(), |n| n.to_string(ast))
                );
            }
        }
    }
    // port: NodeUtil#mayBeUndefined
    pub fn may_be_undefined(ast: &Ast, n: NodeId) -> bool {
        !Self::is_defined_value(ast, n)
    }
    // port: NodeUtil#isDefinedValue
    pub fn is_defined_value(ast: &Ast, value: NodeId) -> bool {
        match value.get_token(ast) {
            Token::ASSIGN | Token::CAST | Token::COMMA => {
                Self::is_defined_value(ast, value.get_last_child(ast).unwrap())
            }
            Token::COALESCE => Self::is_defined_value(ast, value.get_second_child(ast).unwrap()),
            Token::AND | Token::OR => {
                Self::is_defined_value(ast, value.get_first_child(ast).unwrap())
                    && Self::is_defined_value(ast, value.get_last_child(ast).unwrap())
            }
            Token::HOOK => {
                Self::is_defined_value(ast, value.get_second_child(ast).unwrap())
                    && Self::is_defined_value(ast, value.get_last_child(ast).unwrap())
            }
            Token::CALL
            | Token::OPTCHAIN_CALL
            | Token::GETELEM
            | Token::GETPROP
            | Token::OPTCHAIN_GETELEM
            | Token::OPTCHAIN_GETPROP
            | Token::TAGGED_TEMPLATELIT
            | Token::THIS
            | Token::YIELD
            | Token::AWAIT
            | Token::VOID => false,
            Token::DELPROP
            | Token::INC
            | Token::DEC
            | Token::CLASS
            | Token::FUNCTION
            | Token::REGEXP
            | Token::EMPTY
            | Token::ARRAYLIT
            | Token::OBJECTLIT
            | Token::TEMPLATELIT
            | Token::STRINGLIT
            | Token::NUMBER
            | Token::BIGINT
            | Token::NULL
            | Token::TRUE
            | Token::FALSE
            | Token::NEW => true,
            Token::TEMPLATELIT_STRING => value.get_cooked_string(ast).is_some(),
            Token::NAME => {
                let name = value.get_string(ast);
                name == "Infinity" || name == "NaN"
            }
            _ => {
                if Self::is_assignment_op(ast, value) || Self::is_simple_operator(ast, value) {
                    return true;
                }
                panic!(
                    "Unexpected expression node: {}\n parent:{}",
                    value.to_string(ast),
                    value
                        .get_parent(ast)
                        .map_or_else(|| "null".to_owned(), |n| n.to_string(ast))
                );
            }
        }
    }
    // port: NodeUtil#getNthSibling
    fn get_nth_sibling(ast: &Ast, first: Option<NodeId>, mut index: i32) -> Option<NodeId> {
        let mut sibling = first;
        while index != 0 && sibling.is_some() {
            sibling = sibling.unwrap().get_next(ast);
            index = index.wrapping_sub(1);
        }
        sibling
    }
    // port: NodeUtil#getArgumentForFunction
    pub fn get_argument_for_function(ast: &Ast, function: NodeId, index: i32) -> Option<NodeId> {
        check_state!(function.is_function(ast));
        Self::get_nth_sibling(
            ast,
            function.get_second_child(ast).unwrap().get_first_child(ast),
            index,
        )
    }
    // port: NodeUtil#getArgumentForCallOrNew
    pub fn get_argument_for_call_or_new(ast: &Ast, call: NodeId, index: i32) -> Option<NodeId> {
        check_state!(Self::is_call_or_new(ast, call));
        Self::get_nth_sibling(ast, call.get_second_child(ast), index)
    }
    // port: NodeUtil#isInvocationTarget
    pub fn is_invocation_target(ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast).is_some_and(|parent| {
            (Self::is_call_or_new(ast, parent) || parent.is_tagged_template_lit(ast))
                && parent.get_first_child(ast) == Some(n)
        })
    }
    // port: NodeUtil#isInvocation
    pub fn is_invocation(ast: &Ast, n: NodeId) -> bool {
        Self::is_call_or_new(ast, n) || n.is_tagged_template_lit(ast)
    }
    // port: NodeUtil#isCallOrNewArgument
    pub fn is_call_or_new_argument(ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast).is_some_and(|parent| {
            Self::is_call_or_new(ast, parent) && parent.get_first_child(ast) != Some(n)
        })
    }
    // port: NodeUtil#isToStringMethodCall
    fn is_to_string_method_call(ast: &Ast, call: NodeId) -> bool {
        let get_node = call.get_first_child(ast).unwrap();
        let name = if Self::is_normal_or_opt_chain_get_prop(ast, get_node) {
            get_node.get_string(ast)
        } else if Self::is_normal_or_opt_chain_get(ast, get_node) {
            let prop_node = get_node.get_last_child(ast).unwrap();
            if !prop_node.is_string_lit(ast) {
                return false;
            }
            prop_node.get_string(ast)
        } else {
            return false;
        };
        name == "toString"
    }
    // port: NodeUtil#getDeclaredTypeExpression
    pub fn get_declared_type_expression(
        ast: &Ast,
        declaration: NodeId,
    ) -> Option<Arc<JSTypeExpression>> {
        check_argument!(declaration.is_name(ast) || declaration.is_string_key(ast));
        let name_jsdoc = Self::get_best_jsdoc_info(ast, declaration);
        if let Some(name_jsdoc) = name_jsdoc {
            return name_jsdoc.get_type();
        }
        let mut parent = declaration.get_parent(ast).unwrap();
        if parent.is_rest(ast) || parent.is_default_value(ast) {
            parent = parent.get_parent(ast).unwrap();
        }
        if parent.is_param_list(ast) {
            let function_jsdoc = Self::get_best_jsdoc_info(ast, parent.get_parent(ast).unwrap());
            if let Some(function_jsdoc) = function_jsdoc {
                return function_jsdoc.get_parameter_type(declaration.get_string(ast));
            }
        }
        None
    }
    // port: NodeUtil#getBestJSDocInfo
    pub fn get_best_jsdoc_info(ast: &Ast, n: NodeId) -> Option<Arc<JSDocInfo>> {
        Self::get_best_jsdoc_info_node(ast, n).and_then(|jsdoc_node| jsdoc_node.get_jsdoc_info(ast))
    }
    // port: NodeUtil#getBestJsDocInfoNodeStrict
    pub fn get_best_js_doc_info_node_strict(ast: &Ast, n: NodeId) -> Option<NodeId> {
        let is_declared_name = n.is_name(ast) && Self::is_name_declaration(ast, n.get_parent(ast));
        if is_declared_name {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_declared_class = Self::is_class_declaration(ast, n);
        if is_declared_class {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_declared_function = Self::is_function_declaration(ast, n);
        if is_declared_function {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_rhs_class = Self::is_rhs_class(ast, n);
        if is_rhs_class {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_rhs_function = Self::is_rhs_function(ast, n);
        if is_rhs_function {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_rhs_class_name =
            n.is_name(ast) && Self::is_rhs_class(ast, n.get_parent(ast).unwrap());
        if is_rhs_class_name {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_rhs_function_name =
            n.is_name(ast) && Self::is_rhs_function(ast, n.get_parent(ast).unwrap());
        if is_rhs_function_name {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        let is_export = n.is_export(ast);
        if is_export {
            return Self::get_best_js_doc_info_node_internal(ast, n);
        }
        panic!(
            "Not allowed to get JSDocInfo node for node: {}",
            n.to_string(ast)
        );
    }
    // port: NodeUtil#isRhsClass
    fn is_rhs_class(ast: &Ast, n: NodeId) -> bool {
        Self::is_named_class_expression(ast, n) && {
            let parent = n.get_parent(ast).unwrap();
            parent.is_name(ast) || parent.is_assign(ast)
        }
    }
    // port: NodeUtil#isRhsFunction
    fn is_rhs_function(ast: &Ast, n: NodeId) -> bool {
        Self::is_named_function_expression(ast, n) && {
            let parent = n.get_parent(ast).unwrap();
            parent.is_name(ast) || parent.is_assign(ast)
        }
    }
    // port: NodeUtil#getBestJSDocInfoNode
    pub fn get_best_jsdoc_info_node(ast: &Ast, n: NodeId) -> Option<NodeId> {
        Self::get_best_js_doc_info_node_internal(ast, n)
    }
    // port: NodeUtil#getBestJsDocInfoNodeInternal
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn get_best_js_doc_info_node_internal(ast: &Ast, n: NodeId) -> Option<NodeId> {
        if n.is_expr_result(ast) {
            return Self::get_best_jsdoc_info_node(ast, n.get_first_child(ast).unwrap());
        }
        let info = n.get_jsdoc_info(ast);
        if info.is_none() {
            let parent = n.get_parent(ast);
            if parent.is_none() || n.is_expr_result(ast) {
                return None;
            }
            let parent = parent.unwrap();
            if parent.is_name(ast)
                || parent.is_export(ast)
                || parent.is_var(ast)
                || parent.is_let(ast)
                || parent.is_const(ast)
                || parent.is_declare(ast)
            {
                return Self::get_best_jsdoc_info_node(ast, parent);
            } else if parent.is_assign(ast) {
                return Self::get_best_jsdoc_info_node(ast, parent);
            } else if Self::may_be_object_lit_key(ast, parent) || parent.is_computed_prop(ast) {
                return Some(parent);
            } else if (parent.is_function(ast) || parent.is_class(ast))
                && Some(n) == parent.get_first_child(ast)
            {
                return Self::get_best_jsdoc_info_node(ast, parent);
            } else if Self::is_name_declaration(ast, Some(parent)) && parent.has_one_child(ast) {
                return Some(parent);
            } else if (parent.is_hook(ast) && parent.get_first_child(ast) != Some(n))
                || parent.is_or(ast)
                || parent.is_and(ast)
                || (parent.is_comma(ast) && parent.get_first_child(ast) != Some(n))
            {
                return Self::get_best_jsdoc_info_node(ast, parent);
            }
        }
        Some(n)
    }
    // port: NodeUtil#getBestLValue
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    pub fn get_best_l_value(ast: &Ast, n: NodeId) -> Option<NodeId> {
        let parent = n.get_parent(ast);
        if Self::is_function_declaration(ast, n) || Self::is_class_declaration(ast, n) {
            return n.get_first_child(ast);
        } else if n.is_class_members(ast) {
            return Self::get_best_l_value(ast, parent.unwrap());
        }
        let parent = parent.unwrap();
        if parent.is_name(ast) {
            Some(parent)
        } else if parent.is_assign(ast) {
            parent.get_first_child(ast)
        } else if Self::may_be_object_lit_key(ast, parent) || parent.is_computed_prop(ast) {
            Some(parent)
        } else if (parent.is_hook(ast) && parent.get_first_child(ast) != Some(n))
            || parent.is_or(ast)
            || parent.is_and(ast)
            || (parent.is_comma(ast) && parent.get_first_child(ast) != Some(n))
        {
            Self::get_best_l_value(ast, parent)
        } else if parent.is_cast(ast) {
            Self::get_best_l_value(ast, parent)
        } else {
            None
        }
    }
    // port: NodeUtil#getRValueOfLValue
    pub fn get_r_value_of_l_value(ast: &Ast, n: NodeId) -> Option<NodeId> {
        let parent = n.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_EXPONENT
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_OR
            | Token::ASSIGN_AND
            | Token::ASSIGN_COALESCE
            | Token::DESTRUCTURING_LHS => n.get_next(ast),
            Token::VAR | Token::LET | Token::CONST => n.get_last_child(ast),
            Token::OBJECTLIT => {
                check_state!(
                    n.is_string_key(ast)
                        || n.is_computed_prop(ast)
                        || n.is_member_function_def(ast)
                        || n.is_getter_def(ast)
                        || n.is_setter_def(ast),
                    "%s",
                    n.to_string(ast)
                );
                n.get_last_child(ast)
            }
            Token::CLASS_MEMBERS => {
                check_state!(
                    n.is_member_function_def(ast)
                        || n.is_member_field_def(ast)
                        || n.is_computed_field_def(ast)
                        || n.is_getter_def(ast)
                        || n.is_setter_def(ast),
                    "%s",
                    n.to_string(ast)
                );
                n.get_last_child(ast)
            }
            Token::FUNCTION | Token::CLASS => Some(parent),
            _ => None,
        }
    }
    // port: NodeUtil#getBestLValueOwner
    pub fn get_best_l_value_owner(ast: &Ast, l_value: Option<NodeId>) -> Option<NodeId> {
        let l_value = l_value?;
        let parent = l_value.get_parent(ast)?;
        if Self::may_be_object_lit_key(ast, l_value) || l_value.is_computed_prop(ast) {
            Self::get_best_l_value(ast, parent)
        } else if Self::is_normal_get(ast, l_value) {
            l_value.get_first_child(ast)
        } else {
            None
        }
    }
    // port: NodeUtil#getBestLValueName
    pub fn get_best_l_value_name(ast: &Ast, l_value: Option<NodeId>) -> Option<JsString> {
        let l_value = l_value?;
        let parent = l_value.get_parent(ast)?;
        if parent.is_class_members(ast) && !l_value.is_computed_prop(ast) {
            let class_name = Self::get_name(ast, l_value.get_grandparent(ast).unwrap())?;
            let method_name = l_value.get_string(ast);
            let maybe_prototype = if l_value.is_static_member(ast) {
                "."
            } else {
                ".prototype."
            };
            return Some(
                class_name
                    .concat(&maybe_prototype.into())
                    .concat(&method_name),
            );
        }
        if Self::may_be_object_lit_key(ast, l_value) {
            if let Some(owner) = Self::get_best_l_value(ast, parent)
                && let Some(owner_name) = Self::get_best_l_value_name(ast, Some(owner))
            {
                let key = Self::get_object_or_class_lit_key_name(ast, l_value);
                return if TokenStream::is_js_identifier(&key) {
                    Some(owner_name.concat(&".".into()).concat(&key))
                } else {
                    None
                };
            }
            return None;
        }
        l_value.get_qualified_name(ast)
    }
    // port: NodeUtil#getBestLValueRoot
    pub fn get_best_l_value_root(ast: &Ast, l_value: Option<NodeId>) -> Option<NodeId> {
        let l_value = l_value?;
        match l_value.get_token(ast) {
            Token::STRING_KEY => Self::get_best_l_value_root(
                ast,
                Self::get_best_l_value(ast, l_value.get_parent(ast).unwrap()),
            ),
            Token::GETPROP | Token::GETELEM => {
                Self::get_best_l_value_root(ast, l_value.get_first_child(ast))
            }
            Token::THIS | Token::SUPER | Token::NAME => Some(l_value),
            _ => None,
        }
    }
    // port: NodeUtil#isExpressionResultUsed
    pub fn is_expression_result_used(ast: &Ast, expr: NodeId) -> bool {
        let parent = expr.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::BLOCK | Token::EXPR_RESULT => false,
            Token::CAST => Self::is_expression_result_used(ast, parent),
            Token::HOOK | Token::AND | Token::OR | Token::COALESCE => {
                Some(expr) == parent.get_first_child(ast)
                    || Self::is_expression_result_used(ast, parent)
            }
            Token::COMMA => {
                let grandparent = parent.get_parent(ast).unwrap();
                if (grandparent.is_call(ast) || grandparent.is_tagged_template_lit(ast))
                    && Some(parent) == grandparent.get_first_child(ast)
                    && Some(expr) == parent.get_first_child(ast)
                    && parent.has_two_children(ast)
                {
                    let called_fn = parent.get_last_child(ast).unwrap();
                    if called_fn.is_name(ast) && called_fn.matches_name(ast, "eval") {
                        return true;
                    }
                    if Self::is_normal_or_opt_chain_get(ast, called_fn) {
                        return true;
                    }
                }
                Some(expr) != parent.get_first_child(ast)
                    && Self::is_expression_result_used(ast, parent)
            }
            Token::FOR => parent.get_second_child(ast) == Some(expr),
            _ => true,
        }
    }
    // port: NodeUtil#isExecutedExactlyOnce
    #[allow(clippy::collapsible_match)] // Retain Java control flow.
    pub fn is_executed_exactly_once(ast: &Ast, mut n: NodeId) -> bool {
        loop {
            let parent = n.get_parent(ast).unwrap();
            match parent.get_token(ast) {
                Token::IF
                | Token::HOOK
                | Token::AND
                | Token::OR
                | Token::COALESCE
                | Token::ASSIGN_OR
                | Token::ASSIGN_AND
                | Token::ASSIGN_COALESCE => {
                    if parent.get_first_child(ast) != Some(n) {
                        return false;
                    }
                }
                Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                    if Self::is_enhanced_for(ast, parent) {
                        if parent.get_second_child(ast) != Some(n) {
                            return false;
                        }
                    } else if parent.get_first_child(ast) != Some(n) {
                        return false;
                    }
                }
                Token::TRY => {
                    if !Self::has_finally(ast, parent) || parent.get_last_child(ast) != Some(n) {
                        return false;
                    }
                }
                Token::WHILE | Token::DO | Token::CASE | Token::DEFAULT_CASE => return false,
                Token::SCRIPT | Token::FUNCTION => break,
                _ => {}
            }
            if let Some(parent) = n.get_parent(ast) {
                n = parent;
            } else {
                break;
            }
        }
        true
    }
    // port: NodeUtil#booleanNode
    pub fn boolean_node(ast: &mut Ast, value: bool) -> NodeId {
        if value {
            IR::true_node(ast)
        } else {
            IR::false_node(ast)
        }
    }
    // port: NodeUtil#numberNode
    pub fn number_node(ast: &mut Ast, value: f64, srcref: Option<NodeId>) -> NodeId {
        let mut result;
        if value.is_nan() {
            result = IR::name(ast, "NaN");
            result.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
        } else {
            if value.is_infinite() {
                result = IR::name(ast, "Infinity");
                result.put_boolean_prop(ast, Prop::IS_CONSTANT_NAME, true);
            } else {
                result = IR::number(ast, value.abs());
            }
            if JSCompDoubles::is_negative(value) {
                result = IR::neg(ast, result);
            }
        }
        if let Some(srcref) = srcref {
            result.srcref_tree(ast, srcref);
        }
        result
    }
    // port: NodeUtil#isNaN
    pub fn is_nan(ast: &Ast, n: NodeId) -> bool {
        (n.is_name(ast) && n.get_string_ref(ast) == "NaN")
            || (n.get_token(ast) == Token::DIV
                && n.get_first_child(ast).unwrap().is_number(ast)
                && n.get_first_child(ast).unwrap().get_double(ast) == 0.0
                && n.get_last_child(ast).unwrap().is_number(ast)
                && n.get_last_child(ast).unwrap().get_double(ast) == 0.0)
            || NUMBER_NAN.matches(ast, n)
    }
}

// A Java static Node has no compilation arena. QualifiedName preserves the exact Node matcher.
static NUMBER_NAN: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("Number.NaN"));

impl NodeUtil {
    // port: NodeUtil#countAstSizeUpToLimit
    pub fn count_ast_size_up_to_limit(ast: &mut Ast, n: NodeId, limit: i32) -> i32 {
        let wrapped_size = std::cell::Cell::new(0i32);
        Self::visit_pre_order_with_predicate(
            ast,
            n,
            &mut |_: &mut Ast, _| wrapped_size.set(wrapped_size.get().wrapping_add(1)),
            &|_, _| wrapped_size.get() < limit,
        );
        wrapped_size.get()
    }
    // port: NodeUtil#countAstSize
    pub fn count_ast_size(ast: &Ast, n: NodeId) -> i32 {
        let mut count: i32 = 1;
        for c in n.children(ast) {
            count = count.wrapping_add(Self::count_ast_size(ast, c));
        }
        count
    }
    // port: NodeUtil#createConstantJsDoc
    pub fn create_constant_js_doc() -> Arc<JSDocInfo> {
        let mut builder = JSDocInfo::builder();
        builder.record_constancy();
        builder.build().unwrap()
    }
    // port: NodeUtil#isGoogProvideCall
    pub fn is_goog_provide_call(ast: &Ast, n: NodeId) -> bool {
        if Self::is_expr_call(ast, n) {
            return GOOG_PROVIDE.matches(ast, n.get_first_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogModuleCall
    pub fn is_goog_module_call(ast: &Ast, n: NodeId) -> bool {
        if Self::is_expr_call(ast, n) {
            return GOOG_MODULE.matches(ast, n.get_first_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogModuleGetCall
    pub fn is_goog_module_get_call(ast: &Ast, call_node: NodeId) -> bool {
        if !call_node.is_call(ast) {
            return false;
        }
        GOOG_MODULE_GET.matches(ast, call_node.get_first_child(ast).unwrap())
            && call_node.has_two_children(ast)
            && call_node.get_second_child(ast).unwrap().is_string_lit(ast)
    }
    // port: NodeUtil#isGoogRequireCall
    pub fn is_goog_require_call(ast: &Ast, call: NodeId) -> bool {
        if call.is_call(ast) {
            return GOOG_REQUIRE.matches(ast, call.get_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogRequireTypeCall
    pub fn is_goog_require_type_call(ast: &Ast, call: NodeId) -> bool {
        if call.is_call(ast) {
            return GOOG_REQUIRE_TYPE.matches(ast, call.get_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogForwardDeclareCall
    pub fn is_goog_forward_declare_call(ast: &Ast, call: NodeId) -> bool {
        if call.is_call(ast) {
            return GOOG_FORWARD_DECLARE.matches(ast, call.get_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogRequireDynamicCall
    pub fn is_goog_require_dynamic_call(ast: &Ast, call: NodeId) -> bool {
        if call.is_call(ast) {
            return GOOG_REQUIRE_DYNAMIC.matches(ast, call.get_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogWeakUsageCall
    pub fn is_goog_weak_usage_call(ast: &Ast, call: NodeId) -> bool {
        if call.is_call(ast) {
            let target = call.get_first_child(ast).unwrap();
            return GOOG_WEAK_USAGE.matches(ast, target)
                || GOOG_WEAK_USAGE_MANGLED.matches(ast, target);
        }
        false
    }
    // port: NodeUtil#isModuleScopeRoot
    pub fn is_module_scope_root(ast: &Ast, n: NodeId) -> bool {
        n.is_module_body(ast) || Self::is_bundled_goog_module_scope_root(ast, n)
    }
    // port: NodeUtil#isGoogModuleExportsReference
    pub fn is_goog_module_exports_reference(
        compiler: &mut AbstractCompiler,
        scope: ScopeId,
        possible_name: NodeId,
    ) -> bool {
        if !possible_name.is_name(compiler) || possible_name.get_string_ref(compiler) != "exports" {
            return false;
        }
        let name = possible_name.get_string(compiler);
        let var = scope.get_var(compiler, name);
        let scope_root = var.map(|var| var.get_scope_root(compiler));
        scope_root.is_some_and(|scope_root| {
            scope_root.is_module_body(compiler)
                || (scope_root.is_function(compiler)
                    && Self::is_bundled_goog_module_scope_root(
                        compiler,
                        Self::get_function_body(compiler, scope_root),
                    ))
        })
    }
}

static GOOG_LOADMODULE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.loadModule"));

impl NodeUtil {
    // port: NodeUtil#isBundledGoogModuleCall
    pub fn is_bundled_goog_module_call(ast: &Ast, n: NodeId) -> bool {
        if !(n.is_call(ast)
            && n.has_two_children(ast)
            && GOOG_LOADMODULE.matches(ast, n.get_first_child(ast).unwrap()))
        {
            return false;
        }
        n.has_parent(ast)
            && n.get_parent(ast).unwrap().is_expr_result(ast)
            && n.get_grandparent(ast)
                .is_some_and(|grandparent| grandparent.is_script(ast))
    }
    // port: NodeUtil#isBundledGoogModuleScopeRoot
    pub fn is_bundled_goog_module_scope_root(ast: &Ast, n: NodeId) -> bool {
        if !n.is_block(ast)
            || !n.has_children(ast)
            || !Self::is_goog_module_call(ast, n.get_first_child(ast).unwrap())
        {
            return false;
        }
        let function = n.get_parent(ast);
        if function.is_none_or(|function| {
            !function.is_function(ast)
                || !Self::get_function_parameters(ast, function).has_one_child(ast)
                || !Self::get_function_parameters(ast, function)
                    .get_first_child(ast)
                    .unwrap()
                    .matches_name(ast, "exports")
        }) {
            return false;
        }
        let call = function.unwrap().get_parent(ast).unwrap();
        if !call.is_call(ast)
            || !call.has_two_children(ast)
            || !GOOG_LOADMODULE.matches(ast, call.get_first_child(ast).unwrap())
        {
            return false;
        }
        call.get_parent(ast).unwrap().is_expr_result(ast)
            && call.get_grandparent(ast).unwrap().is_script(ast)
    }
    // port: NodeUtil#isGoogModuleDeclareLegacyNamespaceCall
    pub fn is_goog_module_declare_legacy_namespace_call(ast: &Ast, n: NodeId) -> bool {
        if Self::is_expr_call(ast, n) {
            return GOOG_MODULE_DECLARE_LEGACY_NAMESPACE
                .matches(ast, n.get_first_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isGoogSetTestOnlyCall
    pub fn is_goog_set_test_only_call(ast: &Ast, n: NodeId) -> bool {
        if Self::is_expr_call(ast, n) {
            return GOOG_SET_TEST_ONLY.matches(ast, n.get_first_first_child(ast).unwrap());
        }
        false
    }
    // port: NodeUtil#isTopLevel
    pub fn is_top_level(ast: &Ast, n: NodeId) -> bool {
        n.is_script(ast) || n.is_module_body(ast)
    }
    // port: NodeUtil#isGoogModuleFile
    pub fn is_goog_module_file(ast: &Ast, n: NodeId) -> bool {
        n.is_script(ast)
            && n.has_children(ast)
            && n.get_first_child(ast).unwrap().is_module_body(ast)
            && Self::is_goog_module_call(ast, n.get_first_first_child(ast).unwrap())
    }
    // port: NodeUtil#isLegacyGoogModuleFile
    pub fn is_legacy_goog_module_file(ast: &Ast, n: NodeId) -> bool {
        Self::is_goog_module_file(ast, n)
            && Self::is_goog_module_declare_legacy_namespace_call(
                ast,
                n.get_first_child(ast)
                    .unwrap()
                    .get_second_child(ast)
                    .unwrap(),
            )
    }
    // port: NodeUtil#isConstructor
    // The type registry is a trailing argument because Nodes store type handles (DESIGN §4).
    pub fn is_constructor(
        ast: &Ast,
        fn_node: Option<NodeId>,
        types: &closure_jstype::JSTypeRegistry,
    ) -> bool {
        let Some(fn_node) = fn_node else {
            return false;
        };
        if !fn_node.is_function(ast) {
            return false;
        }
        let type_ = fn_node.get_jstype(ast);
        let js_doc_info = Self::get_best_jsdoc_info(ast, fn_node);
        let color = fn_node.get_color(ast);
        type_.is_some_and(|type_| type_.is_constructor(types))
            || js_doc_info.is_some_and(|info| info.is_constructor())
            || color.is_some_and(|color| color.is_constructor())
            || Self::is_es6_constructor(ast, fn_node)
    }
    // port: NodeUtil#isEs6ConstructorMemberFunctionDef
    pub fn is_es6_constructor_member_function_def(ast: &Ast, member_function_def: NodeId) -> bool {
        if !member_function_def.is_member_function_def(ast) {
            return false;
        }
        member_function_def
            .get_parent(ast)
            .unwrap()
            .is_class_members(ast)
            && !member_function_def.is_static_member(ast)
            && member_function_def.get_string_ref(ast) == "constructor"
    }
    // port: NodeUtil#isEs6Constructor
    pub fn is_es6_constructor(ast: &Ast, fn_node: NodeId) -> bool {
        if !fn_node.is_function(ast) {
            return false;
        }
        fn_node.get_parent(ast).is_some_and(|member_function_def| {
            Self::is_es6_constructor_member_function_def(ast, member_function_def)
        })
    }
    // port: NodeUtil#isGetterOrSetter
    pub fn is_getter_or_setter(ast: &Ast, prop_node: NodeId) -> bool {
        if Self::is_get_or_set_key(ast, prop_node) {
            return true;
        }
        if !prop_node.is_string_key(ast)
            || !prop_node.get_first_child(ast).unwrap().is_function(ast)
        {
            return false;
        }
        let key_name = prop_node.get_string(ast);
        key_name == "get" || key_name == "set"
    }
    // port: NodeUtil#isCallTo(Node,String)
    pub fn is_call_to(
        ast: &Ast,
        n: NodeId,
        qualified_name: impl closure_rhino::js_string::JsStrLike,
    ) -> bool {
        n.is_call(ast)
            && n.get_first_child(ast)
                .unwrap()
                .matches_qualified_name(ast, qualified_name)
    }
    // port: NodeUtil#isCallTo(Node,QualifiedName)
    pub fn is_call_to_qualified_name(ast: &Ast, n: NodeId, qualified_name: &QualifiedName) -> bool {
        n.is_call(ast) && qualified_name.matches(ast, n.get_first_child(ast).unwrap())
    }
    // port: NodeUtil#isCallTo(Node,Node)
    pub fn is_call_to_node(ast: &Ast, n: NodeId, target_method: NodeId) -> bool {
        if !n.is_call(ast) {
            return false;
        }
        n.get_first_child(ast)
            .unwrap()
            .matches_qualified_name_node(ast, target_method)
    }
    // port: NodeUtil#collectExternVariableNames
    pub fn collect_extern_variable_names(
        compiler: &mut AbstractCompiler,
        externs: NodeId,
    ) -> IndexSet<JsString> {
        use crate::{
            reference_collector::ReferenceCollector, syntactic_scope_creator::SyntacticScopeCreator,
        };
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut externs_refs = ReferenceCollector::new(
            compiler,
            ReferenceCollector::DO_NOTHING_BEHAVIOR,
            &mut scope_creator,
        );
        externs_refs.process(compiler, externs);
        let mut externs_names = IndexSet::<_>::default();
        for v in externs_refs.get_all_symbols() {
            if !v.is_param(compiler) {
                externs_names.insert(v.get_name(compiler));
            }
        }
        externs_names
    }
    // port: NodeUtil#createSynthesizedExternsSymbol
    pub fn create_synthesized_externs_symbol(
        compiler: &mut AbstractCompiler,
        name_to_add: impl Into<JsString>,
    ) {
        let name = IR::name(compiler, name_to_add);
        name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        let var = IR::var(compiler, name);
        let input = compiler.get_synthesized_externs_input().clone();
        let root = input.get_ast_root(compiler);
        name.set_static_source_file_from(compiler, root);
        var.set_static_source_file_from(compiler, root);
        root.add_child_to_back(compiler, var);
        compiler.report_change_to_enclosing_scope(var);
    }
    // port: NodeUtil#markNewScopesChanged
    pub fn mark_new_scopes_changed(compiler: &mut AbstractCompiler, node: NodeId) {
        compiler
            .change_tracker
            .mark_new_scopes_changed(&mut compiler.ast, node);
    }
    // port: NodeUtil#markFunctionsDeleted
    pub fn mark_functions_deleted(compiler: &mut AbstractCompiler, node: NodeId) {
        if node.is_function(compiler) {
            compiler.report_function_deleted(node);
        }
        let mut child = node.get_first_child(compiler);
        while let Some(current) = child {
            Self::mark_functions_deleted(compiler, current);
            child = current.get_next(compiler);
        }
        let shadowed = node.get_closure_unaware_shadow(compiler);
        if let Some(shadowed) = shadowed {
            Self::mark_functions_deleted(compiler, shadowed);
        }
    }
    // port: NodeUtil#getParentChangeScopeNodes
    pub fn get_parent_change_scope_nodes(ast: &Ast, scope_nodes: &[NodeId]) -> Vec<Option<NodeId>> {
        let mut parent_scope_nodes: IndexSet<Option<NodeId>> =
            scope_nodes.iter().copied().map(Some).collect();
        for scope_node in scope_nodes {
            parent_scope_nodes.insert(ChangeTracker::get_enclosing_change_scope_root(
                ast,
                Some(*scope_node),
            ));
        }
        parent_scope_nodes.into_iter().collect()
    }
    // port: NodeUtil#removeNestedChangeScopeNodes
    pub fn remove_nested_change_scope_nodes(ast: &Ast, scope_nodes: &[NodeId]) -> Vec<NodeId> {
        let mut unique_scope_nodes: IndexSet<NodeId> = scope_nodes.iter().copied().collect();
        for scope_node in scope_nodes {
            let mut ancestor = scope_node.get_parent(ast);
            while let Some(current) = ancestor {
                if ChangeTracker::is_change_scope_root(ast, current)
                    && unique_scope_nodes.contains(&current)
                {
                    unique_scope_nodes.shift_remove(scope_node);
                    break;
                }
                ancestor = current.get_parent(ast);
            }
        }
        unique_scope_nodes.into_iter().collect()
    }
    // port: NodeUtil#getInvocationArgsAsIterable
    pub fn get_invocation_args_as_iterable(ast: &Ast, invocation: NodeId) -> Vec<NodeId> {
        if invocation.is_tagged_template_lit(ast) {
            return TemplateArgsIterable::new(ast, invocation.get_last_child(ast).unwrap())
                .iterator()
                .collect();
        }
        check_state!(
            Self::is_call_or_new(ast, invocation),
            "%s",
            invocation.to_string(ast)
        );
        if invocation.has_one_child(ast) {
            return Vec::new();
        }
        let mut list = Vec::new();
        let mut arg = invocation.get_second_child(ast);
        while let Some(current) = arg {
            list.push(current);
            arg = current.get_next(ast);
        }
        list
    }
    // port: NodeUtil#getInvocationArgsCount
    pub fn get_invocation_args_count(ast: &Ast, invocation: NodeId) -> i32 {
        if invocation.is_tagged_template_lit(ast) {
            TemplateArgsIterable::new(ast, invocation.get_last_child(ast).unwrap())
                .iterator()
                .count() as i32
                + 1
        } else {
            invocation.get_child_count(ast).wrapping_sub(1)
        }
    }
}

pub struct TemplateArgsIterable<'a> {
    template_lit: NodeId,
    ast: &'a Ast,
}
impl<'a> TemplateArgsIterable<'a> {
    // port: NodeUtil.TemplateArgsIterable#TemplateArgsIterable
    pub fn new(ast: &'a Ast, template_lit: NodeId) -> Self {
        check_state!(template_lit.is_template_lit(ast));
        Self { template_lit, ast }
    }
    // port: NodeUtil.TemplateArgsIterable#iterator
    pub fn iterator(&self) -> TemplateArgsIterator<'a> {
        TemplateArgsIterator {
            next_child: self.template_lit.get_first_child(self.ast),
            ast: self.ast,
        }
    }
}
pub struct TemplateArgsIterator<'a> {
    next_child: Option<NodeId>,
    ast: &'a Ast,
}
impl TemplateArgsIterator<'_> {
    // port: NodeUtil.TemplateArgsIterable.iterator#computeNext
    pub fn compute_next(&mut self) -> Option<NodeId> {
        while let Some(next_child) = self.next_child {
            if next_child.is_template_lit_sub(self.ast) {
                break;
            }
            self.next_child = next_child.get_next(self.ast);
        }
        if let Some(next_child) = self.next_child {
            let result = next_child.get_first_child(self.ast);
            self.next_child = next_child.get_next(self.ast);
            result
        } else {
            None
        }
    }
}
impl Iterator for TemplateArgsIterator<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        self.compute_next()
    }
}

impl NodeUtil {
    // port: NodeUtil#getAllVarNamesDeclaredInModule
    pub fn get_all_var_names_declared_in_module(
        compiler: &mut AbstractCompiler,
        module_node: NodeId,
        scope_creator: &mut dyn ScopeCreator,
        global_scope: ScopeId,
    ) -> IndexSet<JsString> {
        use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
        check_state!(
            module_node.is_module_body(compiler),
            "getAllVarsDeclaredInModule expects a module body node"
        );
        check_state!(
            global_scope.is_global(compiler),
            "%s",
            global_scope.to_string(compiler)
        );
        struct Finder {
            name_vars: IndexSet<JsString>,
        }
        impl ScopedCallback for Finder {
            // port: NodeUtil.getAllVarNamesDeclaredInModule#enterScope
            fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
                let current_scope = t.get_scope();
                if current_scope.is_module_scope(t.get_compiler()) {
                    for v in current_scope.get_var_iterable(t.get_compiler()) {
                        self.name_vars.insert(v.get_name(t.get_compiler()));
                    }
                }
            }
            // port: NodeUtil.getAllVarNamesDeclaredInModule#exitScope
            fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
        }
        impl Callback for Finder {
            // port: NodeUtil.getAllVarNamesDeclaredInModule#shouldTraverse
            fn should_traverse(
                &mut self,
                t: &mut NodeTraversal<'_>,
                n: NodeId,
                _parent: Option<NodeId>,
            ) -> bool {
                n.is_module_body(t)
            }
            // port: NodeUtil.getAllVarNamesDeclaredInModule#visit
            fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
            fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
                Some(self)
            }
        }
        let mut finder = Finder {
            name_vars: IndexSet::<_>::default(),
        };
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut finder)
            .set_scope_creator(scope_creator)
            .traverse_with_scope(module_node, global_scope);
        finder.name_vars
    }
    // port: NodeUtil#getAllVarsDeclaredInFunction
    pub fn get_all_vars_declared_in_function(
        compiler: &mut AbstractCompiler,
        scope_creator: &mut dyn ScopeCreator,
        scope: ScopeId,
    ) -> AllVarsDeclaredInFunction {
        use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
        check_state!(
            scope.is_function_scope(compiler),
            "%s",
            scope.to_string(compiler)
        );
        struct Finder {
            name_var_map: IndexMap<JsString, VarId>,
            ordered_vars: Vec<VarId>,
            scope: ScopeId,
        }
        impl ScopedCallback for Finder {
            // port: NodeUtil.getAllVarsDeclaredInFunction#enterScope
            fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
                let current_scope = t.get_scope();
                for v in current_scope.get_var_iterable(t.get_compiler()) {
                    self.name_var_map.insert(v.get_name(t.get_compiler()), v);
                    self.ordered_vars.push(v);
                }
            }
            // port: NodeUtil.getAllVarsDeclaredInFunction#exitScope
            fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {}
        }
        impl Callback for Finder {
            // port: NodeUtil.getAllVarsDeclaredInFunction#shouldTraverse
            fn should_traverse(
                &mut self,
                t: &mut NodeTraversal<'_>,
                n: NodeId,
                _parent: Option<NodeId>,
            ) -> bool {
                !n.is_function(t) || n == self.scope.get_root_node(t.get_compiler())
            }
            // port: NodeUtil.getAllVarsDeclaredInFunction#visit
            fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
            fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
                Some(self)
            }
        }
        let mut finder = Finder {
            name_var_map: IndexMap::<_, _>::default(),
            ordered_vars: Vec::new(),
            scope,
        };
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut finder)
            .set_scope_creator(scope_creator)
            .traverse_at_scope(scope);
        AllVarsDeclaredInFunction::new(finder.name_var_map, finder.ordered_vars)
    }
}

pub struct AllVarsDeclaredInFunction {
    all_vars_in_fn: IndexMap<JsString, VarId>,
    ordered_vars: Vec<VarId>,
}
impl AllVarsDeclaredInFunction {
    // port: NodeUtil.AllVarsDeclaredInFunction#AllVarsDeclaredInFunction
    pub fn new(all_vars_in_fn: IndexMap<JsString, VarId>, ordered_vars: Vec<VarId>) -> Self {
        check_state!(all_vars_in_fn.is_empty() == ordered_vars.is_empty());
        Self {
            all_vars_in_fn,
            ordered_vars,
        }
    }
    // port: NodeUtil.AllVarsDeclaredInFunction#getAllVariables
    pub fn get_all_variables(&self) -> &IndexMap<JsString, VarId> {
        &self.all_vars_in_fn
    }
    // port: NodeUtil.AllVarsDeclaredInFunction#getAllVariablesInOrder
    pub fn get_all_variables_in_order(&self) -> &[VarId] {
        &self.ordered_vars
    }
}

impl NodeUtil {
    // port: NodeUtil#isObjLitProperty
    pub fn is_obj_lit_property(ast: &Ast, node: NodeId) -> bool {
        node.is_string_key(ast)
            || node.is_getter_def(ast)
            || node.is_setter_def(ast)
            || node.is_member_function_def(ast)
            || node.is_computed_prop(ast)
    }
    // port: NodeUtil#isBlocklessArrowFunctionResult
    pub fn is_blockless_arrow_function_result(ast: &Ast, n: NodeId) -> bool {
        n.get_parent(ast).is_some_and(|parent| {
            parent.is_function(ast) && Some(n) == parent.get_last_child(ast) && !n.is_block(ast)
        })
    }
    // port: NodeUtil#getFeatureSetOfScript
    pub fn get_feature_set_of_script(ast: &Ast, script_node: NodeId) -> Option<FeatureSet> {
        check_state!(script_node.is_script(ast), "%s", script_node.to_string(ast));
        script_node
            .get_prop(ast, Prop::FEATURE_SET)
            .map(|prop| match prop {
                ObjectProp::Opaque(prop) => *prop
                    .as_ref()
                    .as_any()
                    .downcast_ref::<FeatureSet>()
                    .expect("ClassCastException"),
                _ => panic!("ClassCastException"),
            })
    }
    // port: NodeUtil#addFeatureToScript
    pub fn add_feature_to_script(
        compiler: &mut AbstractCompiler,
        script_node: NodeId,
        feature: Feature,
    ) {
        check_state!(
            script_node.is_script(compiler),
            "%s",
            script_node.to_string(compiler)
        );
        let current_features = Self::get_feature_set_of_script(compiler, script_node);
        let new_features = current_features
            .unwrap_or(FeatureSet::BARE_MINIMUM)
            .with(feature);
        script_node.put_prop(
            compiler,
            Prop::FEATURE_SET,
            Some(ObjectProp::Opaque(Arc::new(new_features))),
        );
        if feature != Feature::MODULES {
            check_state!(
                compiler.get_allowable_features().contains(feature),
                "Cannot add feature: %s. It is not supported in the output language, and either 1) its corresponding transpilation pass has already run or 2) transpilation of this feature is unsupported entirely",
                feature
            );
        } else {
            compiler.set_allowable_features(compiler.get_allowable_features().with(feature));
        }
    }
    // port: NodeUtil#removeFeatureFromScript
    fn remove_feature_from_script(ast: &mut Ast, script_node: NodeId, feature: Feature) {
        let current_features = Self::get_feature_set_of_script(ast, script_node)
            .map(|features| features.without(feature));
        script_node.put_prop(
            ast,
            Prop::FEATURE_SET,
            current_features.map(|features| ObjectProp::Opaque(Arc::new(features))),
        );
    }
    // port: NodeUtil#removeFeaturesFromScript
    pub fn remove_features_from_script(
        ast: &mut Ast,
        script_node: NodeId,
        feature_set: FeatureSet,
    ) {
        if let Some(current_features) = Self::get_feature_set_of_script(ast, script_node) {
            script_node.put_prop(
                ast,
                Prop::FEATURE_SET,
                Some(ObjectProp::Opaque(Arc::new(
                    current_features.without(feature_set),
                ))),
            );
        }
    }
    // port: NodeUtil#removeFeatureFromAllScripts
    pub fn remove_feature_from_all_scripts(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        feature: Feature,
    ) {
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));
        let mut child_node = root.get_first_child(compiler);
        while let Some(current) = child_node {
            check_state!(current.is_script(compiler));
            Self::remove_feature_from_script(compiler, current, feature);
            child_node = current.get_next(compiler);
        }
        compiler.mark_feature_not_allowed(feature);
    }
    // port: NodeUtil#removeFeaturesFromAllScripts
    pub fn remove_features_from_all_scripts(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        feature_set: FeatureSet,
    ) {
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));
        let mut child_node = root.get_first_child(compiler);
        while let Some(current) = child_node {
            check_state!(current.is_script(compiler));
            Self::remove_features_from_script(compiler, current, feature_set);
            child_node = current.get_next(compiler);
        }
        compiler.mark_feature_set_not_allowed(feature_set);
    }
    // port: NodeUtil#addFeatureToAllScripts
    pub fn add_feature_to_all_scripts(
        compiler: &mut AbstractCompiler,
        root: NodeId,
        feature: Feature,
    ) {
        check_argument!(root.is_root(compiler), "%s", root.to_string(compiler));
        let mut child_node = root.get_first_child(compiler);
        while let Some(current) = child_node {
            check_state!(current.is_script(compiler));
            Self::add_feature_to_script(compiler, current, feature);
            child_node = current.get_next(compiler);
        }
    }
    // port: NodeUtil#addFeaturesToScript
    pub fn add_features_to_script(
        compiler: &mut AbstractCompiler,
        script_node: NodeId,
        features: FeatureSet,
    ) {
        check_state!(
            script_node.is_script(compiler),
            "%s",
            script_node.to_string(compiler)
        );
        for feature in features.get_features() {
            Self::add_feature_to_script(compiler, script_node, feature);
        }
    }
    // port: NodeUtil#getParamOrPatternNames
    pub fn get_param_or_pattern_names(
        ast: &mut Ast,
        n: NodeId,
        cb: &mut dyn FnMut(&mut Ast, NodeId),
    ) {
        ParsingUtil::get_param_or_pattern_names_with_mutable_context(ast, n, cb);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoogRequire {
    pub namespace: JsString,
    pub property: Option<JsString>,
    pub is_strong_require: bool,
}
impl GoogRequire {
    // port: NodeUtil.GoogRequire#GoogRequire
    pub fn new(namespace: JsString, property: Option<JsString>, is_strong_require: bool) -> Self {
        Self {
            namespace,
            property,
            is_strong_require,
        }
    }
    // port: NodeUtil.GoogRequire#fromNamespace
    pub fn from_namespace(namespace: JsString, is_strong_require: bool) -> Self {
        Self::new(namespace, None, is_strong_require)
    }
    // port: NodeUtil.GoogRequire#fromNamespaceAndProperty
    pub fn from_namespace_and_property(
        namespace: JsString,
        property: JsString,
        is_strong_require: bool,
    ) -> Self {
        Self::new(namespace, Some(property), is_strong_require)
    }
    pub fn namespace(&self) -> JsString {
        self.namespace.clone()
    }
    pub fn property(&self) -> Option<JsString> {
        self.property.clone()
    }
    pub fn is_strong_require(&self) -> bool {
        self.is_strong_require
    }
}

impl NodeUtil {
    // port: NodeUtil#getGoogRequireInfo(String,Scope)
    pub fn get_goog_require_info_name(
        compiler: &mut AbstractCompiler,
        name: impl Into<JsString>,
        scope: ScopeId,
    ) -> Option<GoogRequire> {
        let var = scope.get_var(compiler, name)?;
        Self::get_goog_require_info_var(compiler, var)
    }
    // port: NodeUtil#getGoogRequireInfo(Var)
    pub fn get_goog_require_info_var(
        compiler: &AbstractCompiler,
        var: VarId,
    ) -> Option<GoogRequire> {
        if !var.get_scope_root(compiler).is_module_body(compiler)
            || var.get_name_node(compiler).is_none()
        {
            return None;
        }
        Self::get_goog_require_info(compiler, var.get_name_node(compiler).unwrap())
    }
    // port: NodeUtil#getGoogRequireInfo(Node)
    pub fn get_goog_require_info(ast: &Ast, name_node: NodeId) -> Option<GoogRequire> {
        if name_node.is_import_star(ast) {
            return None;
        }
        check_state!(
            name_node.is_name(ast),
            "unexpected node type: %s",
            name_node.to_string(ast)
        );
        if Self::is_name_declaration(ast, name_node.get_parent(ast)) {
            let require_call = name_node.get_first_child(ast)?;
            let is_strong_require = Self::is_goog_require_call(ast, require_call);
            if !(is_strong_require || Self::is_goog_require_type_call(ast, require_call)) {
                return None;
            }
            let namespace = require_call.get_second_child(ast).unwrap().get_string(ast);
            return Some(GoogRequire::from_namespace(namespace, is_strong_require));
        } else if name_node.get_parent(ast).unwrap().is_string_key(ast)
            && name_node
                .get_grandparent(ast)
                .unwrap()
                .is_object_pattern(ast)
        {
            let require_call = name_node.get_grandparent(ast).unwrap().get_next(ast)?;
            let is_strong_require = Self::is_goog_require_call(ast, require_call);
            if !(is_strong_require || Self::is_goog_require_type_call(ast, require_call)) {
                return None;
            }
            let property = name_node.get_parent(ast).unwrap().get_string(ast);
            let namespace = require_call.get_second_child(ast).unwrap().get_string(ast);
            return Some(GoogRequire::from_namespace_and_property(
                namespace,
                property,
                is_strong_require,
            ));
        }
        None
    }
    // port: NodeUtil#estimateNumLines
    pub fn estimate_num_lines(ast: &Ast, script_node: NodeId) -> i32 {
        check_argument!(script_node.is_script(ast));
        let mut current = script_node;
        while current.has_children(ast) {
            current = current.get_last_child(ast).unwrap();
        }
        current.get_lineno(ast).wrapping_add(1)
    }
    // port: NodeUtil#getOriginalName
    pub fn get_original_name(name: &JsString) -> JsString {
        let mut name = strip_unique_name_suffix(name);
        name =
            crate::make_declared_names_unique::ContextualRenameInverter::get_original_name(&name);
        if name.starts_with("module$exports$") {
            let last_dollar = name.last_index_of_char(u16::from(b'$'));
            if last_dollar != -1 {
                name = name.substring_from((last_dollar + 1) as usize);
            }
        } else if name.starts_with("module$contents$") {
            let last_underscore = name.last_index_of_char(u16::from(b'_'));
            if last_underscore != -1 {
                name = name.substring_from((last_underscore + 1) as usize);
            }
        }
        name
    }
}

// Java Pattern's default Dollar anchor also matches before a final line terminator or CRLF.
fn strip_unique_name_suffix(name: &JsString) -> JsString {
    let units = name.as_units();
    let mut end = units.len();
    if units.ends_with(&[0x0d, 0x0a]) {
        end -= 2;
    } else if units
        .last()
        .is_some_and(|c| matches!(c, 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029))
    {
        end -= 1;
    }
    let mut digits_start = end;
    while digits_start > 0 && (u16::from(b'0')..=u16::from(b'9')).contains(&units[digits_start - 1])
    {
        digits_start -= 1;
    }
    if digits_start < end
        && digits_start >= 2
        && units[digits_start - 2..digits_start] == [u16::from(b'$'), u16::from(b'$')]
    {
        name.substring(0, digits_start - 2)
            .concat(&name.substring_from(end))
    } else {
        name.clone()
    }
}
