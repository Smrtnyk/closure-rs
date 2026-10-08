/*
 * Copyright 2026 The closure-rs Authors.
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

#![allow(clippy::collapsible_if)]
#[path = "printer_jsdoc.rs"]
mod printer_jsdoc;
use closure_jscomp::source_file::SourceFile;
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::{
    js_string::JsString,
    jscomp_parsing_parser::util::source_position::SourcePosition,
    node::{Ast, NodeId, ObjectProp, Prop},
    non_jsdoc_comment::NonJSDocComment,
    token::Token,
};
use serde_json::Value;
use std::sync::Arc;

pub fn js_string(value: &Value) -> JsString {
    if let Some(value) = value.as_str() {
        value.into()
    } else {
        JsString::from_units(
            value["utf16"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_u64().unwrap() as u16)
                .collect::<Vec<_>>(),
        )
    }
}
fn token(name: &str) -> Token {
    match name {
        "RETURN" => Token::RETURN,
        "BITOR" => Token::BITOR,
        "BITXOR" => Token::BITXOR,
        "BITAND" => Token::BITAND,
        "EQ" => Token::EQ,
        "NE" => Token::NE,
        "LT" => Token::LT,
        "LE" => Token::LE,
        "GT" => Token::GT,
        "GE" => Token::GE,
        "LSH" => Token::LSH,
        "RSH" => Token::RSH,
        "URSH" => Token::URSH,
        "ADD" => Token::ADD,
        "SUB" => Token::SUB,
        "MUL" => Token::MUL,
        "DIV" => Token::DIV,
        "MOD" => Token::MOD,
        "EXPONENT" => Token::EXPONENT,
        "NOT" => Token::NOT,
        "BITNOT" => Token::BITNOT,
        "POS" => Token::POS,
        "NEG" => Token::NEG,
        "NEW" => Token::NEW,
        "DELPROP" => Token::DELPROP,
        "TYPEOF" => Token::TYPEOF,
        "GETPROP" => Token::GETPROP,
        "GETELEM" => Token::GETELEM,
        "CALL" => Token::CALL,
        "OPTCHAIN_GETPROP" => Token::OPTCHAIN_GETPROP,
        "OPTCHAIN_GETELEM" => Token::OPTCHAIN_GETELEM,
        "OPTCHAIN_CALL" => Token::OPTCHAIN_CALL,
        "NAME" => Token::NAME,
        "NUMBER" => Token::NUMBER,
        "BIGINT" => Token::BIGINT,
        "STRINGLIT" => Token::STRINGLIT,
        "NULL" => Token::NULL,
        "THIS" => Token::THIS,
        "FALSE" => Token::FALSE,
        "TRUE" => Token::TRUE,
        "SHEQ" => Token::SHEQ,
        "SHNE" => Token::SHNE,
        "REGEXP" => Token::REGEXP,
        "THROW" => Token::THROW,
        "IN" => Token::IN,
        "INSTANCEOF" => Token::INSTANCEOF,
        "ARRAYLIT" => Token::ARRAYLIT,
        "OBJECTLIT" => Token::OBJECTLIT,
        "TRY" => Token::TRY,
        "PARAM_LIST" => Token::PARAM_LIST,
        "COMMA" => Token::COMMA,
        "ASSIGN" => Token::ASSIGN,
        "ASSIGN_BITOR" => Token::ASSIGN_BITOR,
        "ASSIGN_BITXOR" => Token::ASSIGN_BITXOR,
        "ASSIGN_BITAND" => Token::ASSIGN_BITAND,
        "ASSIGN_LSH" => Token::ASSIGN_LSH,
        "ASSIGN_RSH" => Token::ASSIGN_RSH,
        "ASSIGN_URSH" => Token::ASSIGN_URSH,
        "ASSIGN_ADD" => Token::ASSIGN_ADD,
        "ASSIGN_SUB" => Token::ASSIGN_SUB,
        "ASSIGN_MUL" => Token::ASSIGN_MUL,
        "ASSIGN_DIV" => Token::ASSIGN_DIV,
        "ASSIGN_MOD" => Token::ASSIGN_MOD,
        "ASSIGN_EXPONENT" => Token::ASSIGN_EXPONENT,
        "ASSIGN_OR" => Token::ASSIGN_OR,
        "ASSIGN_AND" => Token::ASSIGN_AND,
        "ASSIGN_COALESCE" => Token::ASSIGN_COALESCE,
        "HOOK" => Token::HOOK,
        "OR" => Token::OR,
        "AND" => Token::AND,
        "COALESCE" => Token::COALESCE,
        "INC" => Token::INC,
        "DEC" => Token::DEC,
        "FUNCTION" => Token::FUNCTION,
        "IF" => Token::IF,
        "SWITCH" => Token::SWITCH,
        "CASE" => Token::CASE,
        "DEFAULT_CASE" => Token::DEFAULT_CASE,
        "WHILE" => Token::WHILE,
        "DO" => Token::DO,
        "FOR" => Token::FOR,
        "FOR_IN" => Token::FOR_IN,
        "BREAK" => Token::BREAK,
        "CONTINUE" => Token::CONTINUE,
        "VAR" => Token::VAR,
        "WITH" => Token::WITH,
        "CATCH" => Token::CATCH,
        "VOID" => Token::VOID,
        "EMPTY" => Token::EMPTY,
        "ROOT" => Token::ROOT,
        "BLOCK" => Token::BLOCK,
        "SWITCH_BODY" => Token::SWITCH_BODY,
        "LABEL" => Token::LABEL,
        "EXPR_RESULT" => Token::EXPR_RESULT,
        "SCRIPT" => Token::SCRIPT,
        "GETTER_DEF" => Token::GETTER_DEF,
        "SETTER_DEF" => Token::SETTER_DEF,
        "CONST" => Token::CONST,
        "DEBUGGER" => Token::DEBUGGER,
        "LABEL_NAME" => Token::LABEL_NAME,
        "STRING_KEY" => Token::STRING_KEY,
        "CAST" => Token::CAST,
        "ARRAY_PATTERN" => Token::ARRAY_PATTERN,
        "OBJECT_PATTERN" => Token::OBJECT_PATTERN,
        "DESTRUCTURING_LHS" => Token::DESTRUCTURING_LHS,
        "CLASS" => Token::CLASS,
        "CLASS_MEMBERS" => Token::CLASS_MEMBERS,
        "MEMBER_FUNCTION_DEF" => Token::MEMBER_FUNCTION_DEF,
        "MEMBER_FIELD_DEF" => Token::MEMBER_FIELD_DEF,
        "COMPUTED_FIELD_DEF" => Token::COMPUTED_FIELD_DEF,
        "SUPER" => Token::SUPER,
        "LET" => Token::LET,
        "FOR_OF" => Token::FOR_OF,
        "FOR_AWAIT_OF" => Token::FOR_AWAIT_OF,
        "YIELD" => Token::YIELD,
        "AWAIT" => Token::AWAIT,
        "IMPORT" => Token::IMPORT,
        "IMPORT_SPECS" => Token::IMPORT_SPECS,
        "IMPORT_SPEC" => Token::IMPORT_SPEC,
        "IMPORT_STAR" => Token::IMPORT_STAR,
        "EXPORT" => Token::EXPORT,
        "EXPORT_SPECS" => Token::EXPORT_SPECS,
        "EXPORT_SPEC" => Token::EXPORT_SPEC,
        "MODULE_BODY" => Token::MODULE_BODY,
        "DYNAMIC_IMPORT" => Token::DYNAMIC_IMPORT,
        "ITER_REST" => Token::ITER_REST,
        "OBJECT_REST" => Token::OBJECT_REST,
        "ITER_SPREAD" => Token::ITER_SPREAD,
        "OBJECT_SPREAD" => Token::OBJECT_SPREAD,
        "COMPUTED_PROP" => Token::COMPUTED_PROP,
        "TAGGED_TEMPLATELIT" => Token::TAGGED_TEMPLATELIT,
        "TEMPLATELIT" => Token::TEMPLATELIT,
        "TEMPLATELIT_SUB" => Token::TEMPLATELIT_SUB,
        "TEMPLATELIT_STRING" => Token::TEMPLATELIT_STRING,
        "DEFAULT_VALUE" => Token::DEFAULT_VALUE,
        "NEW_TARGET" => Token::NEW_TARGET,
        "IMPORT_META" => Token::IMPORT_META,
        "STRING_TYPE" => Token::STRING_TYPE,
        "BOOLEAN_TYPE" => Token::BOOLEAN_TYPE,
        "NUMBER_TYPE" => Token::NUMBER_TYPE,
        "FUNCTION_TYPE" => Token::FUNCTION_TYPE,
        "PARAMETERIZED_TYPE" => Token::PARAMETERIZED_TYPE,
        "UNION_TYPE" => Token::UNION_TYPE,
        "ANY_TYPE" => Token::ANY_TYPE,
        "NULLABLE_TYPE" => Token::NULLABLE_TYPE,
        "VOID_TYPE" => Token::VOID_TYPE,
        "REST_PARAMETER_TYPE" => Token::REST_PARAMETER_TYPE,
        "NAMED_TYPE" => Token::NAMED_TYPE,
        "OPTIONAL_PARAMETER" => Token::OPTIONAL_PARAMETER,
        "RECORD_TYPE" => Token::RECORD_TYPE,
        "UNDEFINED_TYPE" => Token::UNDEFINED_TYPE,
        "ARRAY_TYPE" => Token::ARRAY_TYPE,
        "GENERIC_TYPE" => Token::GENERIC_TYPE,
        "GENERIC_TYPE_LIST" => Token::GENERIC_TYPE_LIST,
        "ANNOTATION" => Token::ANNOTATION,
        "PIPE" => Token::PIPE,
        "STAR" => Token::STAR,
        "EOC" => Token::EOC,
        "QMARK" => Token::QMARK,
        "BANG" => Token::BANG,
        "EQUALS" => Token::EQUALS,
        "LB" => Token::LB,
        "LC" => Token::LC,
        "COLON" => Token::COLON,
        "INTERFACE" => Token::INTERFACE,
        "INTERFACE_EXTENDS" => Token::INTERFACE_EXTENDS,
        "INTERFACE_MEMBERS" => Token::INTERFACE_MEMBERS,
        "ENUM" => Token::ENUM,
        "ENUM_MEMBERS" => Token::ENUM_MEMBERS,
        "IMPLEMENTS" => Token::IMPLEMENTS,
        "TYPE_ALIAS" => Token::TYPE_ALIAS,
        "DECLARE" => Token::DECLARE,
        "MEMBER_VARIABLE_DEF" => Token::MEMBER_VARIABLE_DEF,
        "INDEX_SIGNATURE" => Token::INDEX_SIGNATURE,
        "CALL_SIGNATURE" => Token::CALL_SIGNATURE,
        "NAMESPACE" => Token::NAMESPACE,
        "NAMESPACE_ELEMENTS" => Token::NAMESPACE_ELEMENTS,
        "PLACEHOLDER1" => Token::PLACEHOLDER1,
        "PLACEHOLDER2" => Token::PLACEHOLDER2,
        "PLACEHOLDER3" => Token::PLACEHOLDER3,

        _ => panic!("Unknown dump token {name}"),
    }
}
pub fn load_node(
    ast: &mut Ast,
    value: &Value,
    source: &Arc<SourceFile>,
    number_expectations: &mut Vec<(NodeId, Option<JsString>)>,
) -> NodeId {
    let ty = token(value["token"].as_str().unwrap());
    let node = match value["node_class"].as_str().unwrap() {
        "Node" => ast.new_node(ty),
        "StringNode" => ast.new_string_with_token(ty, js_string(&value["string"])),
        "NumberNode" => {
            let bits = u64::from_str_radix(
                value["double_bits"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let node = ast.new_number(f64::from_bits(bits));
            number_expectations.push((
                node,
                if value["double_closure"].is_null() {
                    None
                } else {
                    Some(js_string(&value["double_closure"]))
                },
            ));
            node
        }
        "BigIntNode" => ast.new_big_int(
            value["bigint"]
                .as_str()
                .unwrap()
                .parse::<num_bigint::BigInt>()
                .unwrap(),
        ),
        "TemplateLiteralSubstringNode" => ast.new_template_lit_string(
            if value["cooked"].is_null() {
                None
            } else {
                Some(js_string(&value["cooked"]))
            },
            js_string(&value["raw"]),
        ),
        other => panic!("Unknown node subclass {other}"),
    };
    node.set_lineno_charno(
        ast,
        value["lineno"].as_i64().unwrap() as i32,
        value["charno"].as_i64().unwrap() as i32,
    );
    node.set_length(ast, value["length"].as_i64().unwrap() as i32);
    if let Some(original_name) = value.get("original_name") {
        if !original_name.is_null() {
            node.set_original_name(ast, Some(js_string(original_name)));
        }
    }
    for (name, prop) in value["props"].as_object().unwrap() {
        let key = *Prop::VALUES
            .iter()
            .find(|&&key| format!("{key:?}") == *name)
            .unwrap_or_else(|| panic!("Unknown dump property {name}"));
        if let Some(value) = prop.as_i64() {
            node.put_int_prop(ast, key, value as i32);
        } else if key == Prop::SOURCE_FILE {
            let name = prop["name"].as_str().unwrap();
            if name == source.get_name() {
                node.set_static_source_file(ast, Some(source.clone()));
            } else {
                let kind = match prop["kind"].as_str().unwrap() {
                    "STRONG" => closure_rhino::static_source_file::SourceKind::STRONG,
                    "WEAK" => closure_rhino::static_source_file::SourceKind::WEAK,
                    "EXTERN" => closure_rhino::static_source_file::SourceKind::EXTERN,
                    "NON_CODE" => closure_rhino::static_source_file::SourceKind::NON_CODE,
                    value => panic!("Unknown source kind {value}"),
                };
                node.set_static_source_file(
                    ast,
                    Some(Arc::new(
                        closure_rhino::simple_source_file::SimpleSourceFile::new(name, kind),
                    )),
                );
            }
        } else if key == Prop::NON_JSDOC_COMMENT || key == Prop::TRAILING_NON_JSDOC_COMMENT {
            let position = |v: &Value| {
                SourcePosition::new(
                    None,
                    v["offset"].as_i64().unwrap() as i32,
                    v["line"].as_i64().unwrap() as i32,
                    v["column"].as_i64().unwrap() as i32,
                )
            };
            let mut comment = NonJSDocComment::new(
                position(&prop["start"]),
                position(&prop["end"]),
                Some(js_string(&prop["contents"])),
            );
            comment.set_ends_as_line_comment(prop["ends_as_line_comment"].as_bool().unwrap());
            comment.set_is_inline(prop["is_inline"].as_bool().unwrap());
            node.put_prop(
                ast,
                key,
                Some(ObjectProp::NonJSDocComment(Arc::new(comment))),
            );
        } else if key == Prop::JSDOC_INFO {
            let info = printer_jsdoc::load(ast, prop, source, number_expectations);
            node.set_jsdoc_info(ast, Some(info));
        } else if matches!(
            key,
            Prop::DECLARED_TYPE_EXPR
                | Prop::GENERIC_TYPE
                | Prop::IMPLEMENTS
                | Prop::CLOSURE_UNAWARE_SHADOW
        ) {
            let value = load_node(ast, prop, source, number_expectations);
            node.put_prop(ast, key, Some(ObjectProp::Node(value)));
        } else if !matches!(key, Prop::INPUT_ID | Prop::FEATURE_SET) {
            panic!("Unrepresented printer property {name}: {prop}");
        }
    }
    for child in value["children"].as_array().unwrap() {
        let child = load_node(ast, child, source, number_expectations);
        node.add_child_to_back(ast, child);
    }
    node
}
