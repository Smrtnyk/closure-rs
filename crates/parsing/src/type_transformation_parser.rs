/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/TypeTransformationParser.java.

//! Port of com.google.javascript.jscomp.parsing.TypeTransformationParser.
use crate::{
    config::{Config, LanguageMode, StrictMode},
    js_doc_info_parser::JsDocInfoParser,
    parser_runner::ParserRunner,
};
use closure_rhino::{
    check_argument,
    error_reporter::ErrorReporter,
    js_string::JsString,
    msg::Msg,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};
use std::sync::Arc;
const VAR_ARGS: i32 = i32::MAX;
const TTL_NODE_LENGTH: i32 = 9;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
    TYPE_CONSTRUCTOR,
    OPERATION,
    STRING_PREDICATE,
    TYPE_PREDICATE,
    TYPEVAR_PREDICATE,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keywords {
    ALL,
    COND,
    EQ,
    ISCTOR,
    ISDEFINED,
    ISRECORD,
    ISTEMPLATIZED,
    ISUNKNOWN,
    INSTANCEOF,
    MAPUNION,
    MAPRECORD,
    NONE,
    PRINTTYPE,
    PROPTYPE,
    RAWTYPEOF,
    SUB,
    STREQ,
    RECORD,
    TEMPLATETYPEOF,
    TYPE,
    TYPEEXPR,
    TYPEOFVAR,
    UNION,
    UNKNOWN,
}
impl Keywords {
    pub const VALUES: [Self; 24] = [
        Self::ALL,
        Self::COND,
        Self::EQ,
        Self::ISCTOR,
        Self::ISDEFINED,
        Self::ISRECORD,
        Self::ISTEMPLATIZED,
        Self::ISUNKNOWN,
        Self::INSTANCEOF,
        Self::MAPUNION,
        Self::MAPRECORD,
        Self::NONE,
        Self::PRINTTYPE,
        Self::PROPTYPE,
        Self::RAWTYPEOF,
        Self::SUB,
        Self::STREQ,
        Self::RECORD,
        Self::TEMPLATETYPEOF,
        Self::TYPE,
        Self::TYPEEXPR,
        Self::TYPEOFVAR,
        Self::UNION,
        Self::UNKNOWN,
    ];
    // port: TypeTransformationParser.Keywords#Keywords
    pub fn properties(self) -> (&'static str, i32, i32, OperationKind) {
        use OperationKind::*;
        match self {
            Self::ALL => ("all", 0, 0, TYPE_CONSTRUCTOR),
            Self::COND => ("cond", 3, 3, OPERATION),
            Self::EQ => ("eq", 2, 2, TYPE_PREDICATE),
            Self::ISCTOR => ("isCtor", 1, 1, TYPE_PREDICATE),
            Self::ISDEFINED => ("isDefined", 1, 1, TYPEVAR_PREDICATE),
            Self::ISRECORD => ("isRecord", 1, 1, TYPE_PREDICATE),
            Self::ISTEMPLATIZED => ("isTemplatized", 1, 1, TYPE_PREDICATE),
            Self::ISUNKNOWN => ("isUnknown", 1, 1, TYPE_PREDICATE),
            Self::INSTANCEOF => ("instanceOf", 1, 1, OPERATION),
            Self::MAPUNION => ("mapunion", 2, 2, OPERATION),
            Self::MAPRECORD => ("maprecord", 2, 2, OPERATION),
            Self::NONE => ("none", 0, 0, TYPE_CONSTRUCTOR),
            Self::PRINTTYPE => ("printType", 2, 2, OPERATION),
            Self::PROPTYPE => ("propType", 2, 2, OPERATION),
            Self::RAWTYPEOF => ("rawTypeOf", 1, 1, TYPE_CONSTRUCTOR),
            Self::SUB => ("sub", 2, 2, TYPE_PREDICATE),
            Self::STREQ => ("streq", 2, 2, STRING_PREDICATE),
            Self::RECORD => ("record", 1, VAR_ARGS, TYPE_CONSTRUCTOR),
            Self::TEMPLATETYPEOF => ("templateTypeOf", 2, 2, TYPE_CONSTRUCTOR),
            Self::TYPE => ("type", 2, VAR_ARGS, TYPE_CONSTRUCTOR),
            Self::TYPEEXPR => ("typeExpr", 1, 1, TYPE_CONSTRUCTOR),
            Self::TYPEOFVAR => ("typeOfVar", 1, 1, OPERATION),
            Self::UNION => ("union", 2, VAR_ARGS, TYPE_CONSTRUCTOR),
            Self::UNKNOWN => ("unknown", 0, 0, TYPE_CONSTRUCTOR),
        }
    }
}
pub struct TypeTransformationParser<'a> {
    type_transformation_string: JsString,
    type_transformation_ast: Option<NodeId>,
    source_file: Option<Arc<dyn StaticSourceFile>>,
    error_reporter: &'a mut dyn ErrorReporter,
    template_lineno: i32,
    template_charno: i32,
}
impl<'a> TypeTransformationParser<'a> {
    // port: TypeTransformationParser#TypeTransformationParser
    pub fn new(
        type_transformation_string: JsString,
        source_file: Option<Arc<dyn StaticSourceFile>>,
        error_reporter: &'a mut dyn ErrorReporter,
        template_lineno: i32,
        template_charno: i32,
    ) -> Self {
        Self {
            type_transformation_string,
            type_transformation_ast: None,
            source_file,
            error_reporter,
            template_lineno,
            template_charno,
        }
    }
    // port: TypeTransformationParser#getTypeTransformationAst
    pub fn get_type_transformation_ast(&self) -> Option<NodeId> {
        self.type_transformation_ast
    }
    // port: TypeTransformationParser#addNewWarning
    fn add_new_warning(&mut self, message_id: Msg, message_arg: &str) {
        self.error_reporter.warning(
            &format!(
                "Bad type annotation. {}",
                message_id.format_with(&[message_arg.to_owned()])
            ),
            self.source_file.as_ref().map_or("", |s| s.get_name()),
            self.template_lineno,
            self.template_charno,
        );
    }
    // port: TypeTransformationParser#nameToKeyword
    fn name_to_keyword(&self, s: &JsString) -> Keywords {
        // All callers first validate the name against the ASCII Keywords table.
        let upper = s.to_string_lossy().to_ascii_uppercase();
        Keywords::VALUES
            .into_iter()
            .find(|k| format!("{k:?}") == upper)
            .unwrap()
    }
    // port: TypeTransformationParser#isValidKeyword
    fn is_valid_keyword(&self, name: &JsString) -> bool {
        Keywords::VALUES
            .into_iter()
            .any(|k| name == k.properties().0)
    }
    // port: TypeTransformationParser#isOperationKind
    fn is_operation_kind(&self, name: &JsString, kind: OperationKind) -> bool {
        self.is_valid_keyword(name) && self.name_to_keyword(name).properties().3 == kind
    }
    // port: TypeTransformationParser#isValidStringPredicate
    fn is_valid_string_predicate(&self, name: &JsString) -> bool {
        self.is_operation_kind(name, OperationKind::STRING_PREDICATE)
    }
    // port: TypeTransformationParser#isValidTypePredicate
    fn is_valid_type_predicate(&self, name: &JsString) -> bool {
        self.is_operation_kind(name, OperationKind::TYPE_PREDICATE)
    }
    // port: TypeTransformationParser#isValidTypevarPredicate
    fn is_valid_typevar_predicate(&self, name: &JsString) -> bool {
        self.is_operation_kind(name, OperationKind::TYPEVAR_PREDICATE)
    }
    // port: TypeTransformationParser#isBooleanOperation
    fn is_boolean_operation(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_and(ast) || n.is_or(ast) || n.is_not(ast)
    }
    // port: TypeTransformationParser#isValidPredicate
    fn is_valid_predicate(&self, name: &JsString) -> bool {
        self.is_valid_string_predicate(name)
            || self.is_valid_type_predicate(name)
            || self.is_valid_typevar_predicate(name)
    }
    // port: TypeTransformationParser#getFunctionParamCount
    fn get_function_param_count(&self, ast: &Ast, n: NodeId) -> i32 {
        check_argument!(
            n.is_function(ast),
            "Expected a function node, found %s",
            n.to_string(ast)
        );
        n.get_second_child(ast).unwrap().get_child_count(ast)
    }
    // port: TypeTransformationParser#getFunctionBody
    fn get_function_body(&self, ast: &Ast, n: NodeId) -> NodeId {
        check_argument!(
            n.is_function(ast),
            "Expected a function node, found %s",
            n.to_string(ast)
        );
        n.get_child_at_index(ast, 2).unwrap()
    }
    // port: TypeTransformationParser#getCallName
    fn get_call_name(&self, ast: &Ast, n: NodeId) -> JsString {
        check_argument!(
            n.is_call(ast),
            "Expected a call node, found %s",
            n.to_string(ast)
        );
        n.get_first_child(ast).unwrap().get_string(ast)
    }
    // port: TypeTransformationParser#getCallArgument
    fn get_call_argument(&self, ast: &Ast, n: NodeId, i: i32) -> NodeId {
        check_argument!(
            n.is_call(ast),
            "Expected a call node, found %s",
            n.to_string(ast)
        );
        n.get_child_at_index(ast, i.wrapping_add(1)).unwrap()
    }
    // port: TypeTransformationParser#getCallParamCount
    fn get_call_param_count(&self, ast: &Ast, n: NodeId) -> i32 {
        check_argument!(
            n.is_call(ast),
            "Expected a call node, found %s",
            n.to_string(ast)
        );
        n.get_child_count(ast).wrapping_sub(1)
    }
    // port: TypeTransformationParser#isTypeVar
    fn is_type_var(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_name(ast)
    }
    // port: TypeTransformationParser#isTypeName
    fn is_type_name(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_string_lit(ast)
    }
    // port: TypeTransformationParser#isOperation
    fn is_operation(&self, ast: &Ast, n: NodeId) -> bool {
        n.is_call(ast)
    }
    // port: TypeTransformationParser#isValidExpression
    fn is_valid_expression(&self, ast: &Ast, e: NodeId) -> bool {
        self.is_type_var(ast, e) || self.is_type_name(ast, e) || self.is_operation(ast, e)
    }
    // port: TypeTransformationParser#warnInvalid
    fn warn_invalid(&mut self, msg: &str) {
        self.add_new_warning(Msg::JSDOC_TYPETRANSFORMATION_INVALID, msg);
    }
    // port: TypeTransformationParser#warnInvalidExpression
    fn warn_invalid_expression(&mut self, msg: &str) {
        self.add_new_warning(Msg::JSDOC_TYPETRANSFORMATION_INVALID_EXPRESSION, msg);
    }
    // port: TypeTransformationParser#warnMissingParam
    fn warn_missing_param(&mut self, msg: &str) {
        self.add_new_warning(Msg::JSDOC_TYPETRANSFORMATION_MISSING_PARAM, msg);
    }
    // port: TypeTransformationParser#warnExtraParam
    fn warn_extra_param(&mut self, msg: &str) {
        self.add_new_warning(Msg::JSDOC_TYPETRANSFORMATION_EXTRA_PARAM, msg);
    }
    // port: TypeTransformationParser#warnInvalidInside
    fn warn_invalid_inside(&mut self, msg: &str) {
        self.add_new_warning(Msg::JSDOC_TYPETRANSFORMATION_INVALID_INSIDE, msg);
    }
    // port: TypeTransformationParser#checkParameterCount
    fn check_parameter_count(&mut self, ast: &Ast, expr: NodeId, keyword: Keywords) -> bool {
        let count = self.get_call_param_count(ast, expr);
        let (name, min, max, _) = keyword.properties();
        if count < min {
            self.warn_missing_param(name);
            return false;
        }
        if count > max {
            self.warn_extra_param(name);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#parseTypeTransformation
    pub fn parse_type_transformation(&mut self, ast: &mut Ast) -> bool {
        let config = Config::builder()
            .set_language_mode(LanguageMode::ES_NEXT)
            .set_strict_mode(StrictMode::SLOPPY)
            .build();
        let result = ParserRunner::parse(
            ast,
            self.source_file.clone().unwrap(),
            self.type_transformation_string.clone(),
            &config,
            self.error_reporter,
        );
        let root = result.ast;
        if root.is_none_or(|n| {
            !n.is_script(ast)
                || !n.has_children(ast)
                || !n.get_first_child(ast).unwrap().is_expr_result(ast)
        }) {
            self.warn_invalid_expression("type transformation");
            return false;
        }
        let expr = root.unwrap().get_first_first_child(ast).unwrap();
        if !self.valid_type_transformation_expression(ast, expr) {
            return false;
        }
        self.fix_ttl_node_line_no_char_no_and_length(ast, expr);
        self.type_transformation_ast = Some(expr);
        true
    }
    // port: TypeTransformationParser#fixTTLNodeLineNoCharNoAndLength
    fn fix_ttl_node_line_no_char_no_and_length(&self, ast: &mut Ast, expr: NodeId) {
        expr.set_lineno_charno(ast, self.template_lineno, self.template_charno);
        expr.set_length(ast, TTL_NODE_LENGTH);
        let mut child = expr.get_first_child(ast);
        while let Some(c) = child {
            self.fix_ttl_node_line_no_char_no_and_length(ast, c);
            child = c.get_next(ast);
        }
    }
    // port: TypeTransformationParser#validTemplateTypeExpression
    fn valid_template_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::TYPE) {
            return false;
        }
        let count = self.get_call_param_count(ast, expr);
        let first = self.get_call_argument(ast, expr, 0);
        if !self.is_type_var(ast, first) && !self.is_type_name(ast, first) {
            self.warn_invalid("type name or type variable");
            self.warn_invalid_inside("template type operation");
            return false;
        }
        for i in 1..count {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.valid_type_transformation_expression(ast, arg) {
                self.warn_invalid_inside("template type operation");
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#validUnionTypeExpression
    fn valid_union_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::UNION) {
            return false;
        }
        for i in 0..self.get_call_param_count(ast, expr) {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.valid_type_transformation_expression(ast, arg) {
                self.warn_invalid_inside("union type");
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#validNoneTypeExpression
    fn valid_none_type_expression(&mut self, ast: &Ast, expr: NodeId) -> bool {
        self.check_parameter_count(ast, expr, Keywords::NONE)
    }
    // port: TypeTransformationParser#validAllTypeExpression
    fn valid_all_type_expression(&mut self, ast: &Ast, expr: NodeId) -> bool {
        self.check_parameter_count(ast, expr, Keywords::ALL)
    }
    // port: TypeTransformationParser#validUnknownTypeExpression
    fn valid_unknown_type_expression(&mut self, ast: &Ast, expr: NodeId) -> bool {
        self.check_parameter_count(ast, expr, Keywords::UNKNOWN)
    }
    // port: TypeTransformationParser#validRawTypeOfTypeExpression
    fn valid_raw_type_of_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::RAWTYPEOF) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::RAWTYPEOF.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validTemplateTypeOfExpression
    fn valid_template_type_of_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::TEMPLATETYPEOF) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::TEMPLATETYPEOF.properties().0);
            return false;
        }
        let index = self.get_call_argument(ast, expr, 1);
        if !index.is_number(ast) {
            self.warn_invalid("index");
            self.warn_invalid_inside(Keywords::TEMPLATETYPEOF.properties().0);
            return false;
        }
        let index = index.get_double(ast);
        if index.is_nan()
            || !closure_rhino::jscomp_base::js_comp_doubles::JSCompDoubles::is_exact_int32(index)
        {
            self.warn_invalid("index");
            self.warn_invalid_inside(Keywords::TEMPLATETYPEOF.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validRecordParam
    fn valid_record_param(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if expr.is_object_lit(ast) {
            let mut prop = expr.get_first_child(ast);
            while let Some(p) = prop {
                if p.is_shorthand_property(ast) {
                    self.warn_invalid("property, missing type");
                    return false;
                } else if !self
                    .valid_type_transformation_expression(ast, p.get_first_child(ast).unwrap())
                {
                    return false;
                }
                prop = p.get_next(ast);
            }
        } else if !self.valid_type_transformation_expression(ast, expr) {
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validRecordTypeExpression
    fn valid_record_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::RECORD) {
            return false;
        }
        for i in 0..self.get_call_param_count(ast, expr) {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.valid_record_param(ast, arg) {
                self.warn_invalid_inside(Keywords::RECORD.properties().0);
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#validNativeTypeExpr
    fn valid_native_type_expr(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::TYPEEXPR) {
            return false;
        }
        let string = self.get_call_argument(ast, expr, 0);
        if !string.is_string_lit(ast) {
            self.warn_invalid_expression("native type");
            self.warn_invalid_inside(Keywords::TYPEEXPR.properties().0);
            return false;
        }
        let type_expr = JsDocInfoParser::parse_type_string(ast, string.get_string(ast));
        string.detach(ast);
        expr.add_child_to_back(ast, type_expr.unwrap());
        true
    }
    // port: TypeTransformationParser#validTypeExpression
    fn valid_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        let name = self.get_call_name(ast, expr);
        match self.name_to_keyword(&name) {
            Keywords::TYPE => self.valid_template_type_expression(ast, expr),
            Keywords::UNION => self.valid_union_type_expression(ast, expr),
            Keywords::NONE => self.valid_none_type_expression(ast, expr),
            Keywords::ALL => self.valid_all_type_expression(ast, expr),
            Keywords::UNKNOWN => self.valid_unknown_type_expression(ast, expr),
            Keywords::RAWTYPEOF => self.valid_raw_type_of_type_expression(ast, expr),
            Keywords::TEMPLATETYPEOF => self.valid_template_type_of_expression(ast, expr),
            Keywords::RECORD => self.valid_record_type_expression(ast, expr),
            Keywords::TYPEEXPR => self.valid_native_type_expr(ast, expr),
            _ => panic!("Invalid type expression"),
        }
    }
    // port: TypeTransformationParser#validTypePredicate
    fn valid_type_predicate(&mut self, ast: &mut Ast, expr: NodeId, count: i32) -> bool {
        for i in 0..count {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.valid_type_transformation_expression(ast, arg) {
                self.warn_invalid_inside("boolean");
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#isValidStringParam
    fn is_valid_string_param(&mut self, ast: &Ast, expr: NodeId) -> bool {
        if !expr.is_name(ast) && !expr.is_string_lit(ast) {
            self.warn_invalid("string");
            return false;
        }
        if expr.get_string_ref(ast).is_empty() {
            self.warn_invalid("string parameter");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validStringPredicate
    fn valid_string_predicate(&mut self, ast: &Ast, expr: NodeId, count: i32) -> bool {
        for i in 0..count {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.is_valid_string_param(ast, arg) {
                self.warn_invalid_inside("boolean");
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#validTypevarParam
    fn valid_typevar_param(&mut self, ast: &Ast, expr: NodeId) -> bool {
        if !self.is_type_var(ast, expr) {
            self.warn_invalid("name");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validTypevarPredicate
    fn valid_typevar_predicate(&mut self, ast: &Ast, expr: NodeId, count: i32) -> bool {
        for i in 0..count {
            let arg = self.get_call_argument(ast, expr, i);
            if !self.valid_typevar_param(ast, arg) {
                self.warn_invalid_inside("boolean");
                return false;
            }
        }
        true
    }
    // port: TypeTransformationParser#validBooleanOperation
    fn valid_boolean_operation(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        let valid = if expr.is_not(ast) {
            self.valid_boolean_expression(ast, expr.get_first_child(ast).unwrap())
        } else {
            self.valid_boolean_expression(ast, expr.get_first_child(ast).unwrap())
                && self.valid_boolean_expression(ast, expr.get_second_child(ast).unwrap())
        };
        if !valid {
            self.warn_invalid_inside("boolean");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validBooleanExpression
    fn valid_boolean_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if self.is_boolean_operation(ast, expr) {
            return self.valid_boolean_operation(ast, expr);
        }
        if !self.is_operation(ast, expr) {
            self.warn_invalid_expression("boolean");
            return false;
        }
        if !self.is_valid_predicate(&self.get_call_name(ast, expr)) {
            self.warn_invalid("boolean predicate");
            return false;
        }
        let keyword = self.name_to_keyword(&self.get_call_name(ast, expr));
        if !self.check_parameter_count(ast, expr, keyword) {
            return false;
        }
        let count = self.get_call_param_count(ast, expr);
        match keyword.properties().3 {
            OperationKind::TYPE_PREDICATE => self.valid_type_predicate(ast, expr, count),
            OperationKind::STRING_PREDICATE => self.valid_string_predicate(ast, expr, count),
            OperationKind::TYPEVAR_PREDICATE => self.valid_typevar_predicate(ast, expr, count),
            _ => panic!("Invalid boolean expression"),
        }
    }
    // port: TypeTransformationParser#validConditionalExpression
    fn valid_conditional_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::COND) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_boolean_expression(ast, arg) {
            self.warn_invalid_inside("conditional");
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 1);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside("conditional");
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 2);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside("conditional");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validMapunionExpression
    fn valid_mapunion_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::MAPUNION) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::MAPUNION.properties().0);
            return false;
        }
        let map_fn = self.get_call_argument(ast, expr, 1);
        if !map_fn.is_function(ast) {
            self.warn_invalid("map function");
            self.warn_invalid_inside(Keywords::MAPUNION.properties().0);
            return false;
        }
        let count = self.get_function_param_count(ast, map_fn);
        if count < 1 {
            self.warn_missing_param("map function");
            self.warn_invalid_inside(Keywords::MAPUNION.properties().0);
            return false;
        }
        if count > 1 {
            self.warn_extra_param("map function");
            self.warn_invalid_inside(Keywords::MAPUNION.properties().0);
            return false;
        }
        let body = self.get_function_body(ast, map_fn);
        if !self.valid_type_transformation_expression(ast, body) {
            self.warn_invalid_inside("map function body");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validMaprecordExpression
    fn valid_maprecord_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::MAPRECORD) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::MAPRECORD.properties().0);
            return false;
        }
        let map_fn = self.get_call_argument(ast, expr, 1);
        if !map_fn.is_function(ast) {
            self.warn_invalid("map function");
            self.warn_invalid_inside(Keywords::MAPRECORD.properties().0);
            return false;
        }
        let count = self.get_function_param_count(ast, map_fn);
        if count < 2 {
            self.warn_missing_param("map function");
            self.warn_invalid_inside(Keywords::MAPRECORD.properties().0);
            return false;
        }
        if count > 2 {
            self.warn_extra_param("map function");
            self.warn_invalid_inside(Keywords::MAPRECORD.properties().0);
            return false;
        }
        let body = self.get_function_body(ast, map_fn);
        if !self.valid_type_transformation_expression(ast, body) {
            self.warn_invalid_inside("map function body");
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validTypeOfVarExpression
    fn valid_type_of_var_expression(&mut self, ast: &Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::TYPEOFVAR) {
            return false;
        }
        if !self.get_call_argument(ast, expr, 0).is_string_lit(ast) {
            self.warn_invalid("name");
            self.warn_invalid_inside(Keywords::TYPEOFVAR.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validInstanceOfExpression
    fn valid_instance_of_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::INSTANCEOF) {
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 0);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::INSTANCEOF.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validPrintTypeExpression
    fn valid_print_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::PRINTTYPE) {
            return false;
        }
        if !self.get_call_argument(ast, expr, 0).is_string_lit(ast) {
            self.warn_invalid("message");
            self.warn_invalid_inside(Keywords::PRINTTYPE.properties().0);
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 1);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::PRINTTYPE.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validPropTypeExpression
    fn valid_prop_type_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.check_parameter_count(ast, expr, Keywords::PROPTYPE) {
            return false;
        }
        if !self.get_call_argument(ast, expr, 0).is_string_lit(ast) {
            self.warn_invalid("property name");
            self.warn_invalid_inside(Keywords::PROPTYPE.properties().0);
            return false;
        }
        let arg = self.get_call_argument(ast, expr, 1);
        if !self.valid_type_transformation_expression(ast, arg) {
            self.warn_invalid_inside(Keywords::PROPTYPE.properties().0);
            return false;
        }
        true
    }
    // port: TypeTransformationParser#validOperationExpression
    fn valid_operation_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        let name = self.get_call_name(ast, expr);
        match self.name_to_keyword(&name) {
            Keywords::COND => self.valid_conditional_expression(ast, expr),
            Keywords::MAPUNION => self.valid_mapunion_expression(ast, expr),
            Keywords::MAPRECORD => self.valid_maprecord_expression(ast, expr),
            Keywords::TYPEOFVAR => self.valid_type_of_var_expression(ast, expr),
            Keywords::INSTANCEOF => self.valid_instance_of_expression(ast, expr),
            Keywords::PRINTTYPE => self.valid_print_type_expression(ast, expr),
            Keywords::PROPTYPE => self.valid_prop_type_expression(ast, expr),
            _ => panic!("Invalid type transformation operation"),
        }
    }
    // port: TypeTransformationParser#validTypeTransformationExpression
    fn valid_type_transformation_expression(&mut self, ast: &mut Ast, expr: NodeId) -> bool {
        if !self.is_valid_expression(ast, expr) {
            self.warn_invalid_expression("type transformation");
            return false;
        }
        if self.is_type_var(ast, expr) || self.is_type_name(ast, expr) {
            return true;
        }
        let name = self.get_call_name(ast, expr);
        if !self.is_valid_keyword(&name) {
            self.warn_invalid_expression("type transformation");
            return false;
        }
        match self.name_to_keyword(&name).properties().3 {
            OperationKind::TYPE_CONSTRUCTOR => self.valid_type_expression(ast, expr),
            OperationKind::OPERATION => self.valid_operation_expression(ast, expr),
            _ => panic!("Invalid type transformation expression"),
        }
    }
}
