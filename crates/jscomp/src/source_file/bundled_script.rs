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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/GatherExternProperties.java.

//! Read-only pass inputs recorded for a parsed bundled script. GatherExternProperties
//! uses the same field selectors during the host build and ordinary target traversals.
use closure_rhino::{
    check_state,
    fast_hash::IndexSet,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct BundledScript {
    pub(crate) declarations: &'static [u8],
    pub(crate) properties: &'static [&'static str],
    pub(crate) type_fields: &'static [&'static str],
    pub(crate) check_function_annotations: bool,
    pub(crate) may_have_calls: bool,
    pub(crate) may_have_module_syntax: bool,
    pub(crate) global_property_roots: Option<&'static [&'static str]>,
}

// port: GatherExternProperties#visit
pub(crate) fn property(ast: &Ast, n: NodeId, parent: Option<NodeId>) -> Option<&JsString> {
    match n.get_token(ast) {
        Token::GETPROP
        | Token::MEMBER_FUNCTION_DEF
        | Token::MEMBER_FIELD_DEF
        | Token::GETTER_DEF
        | Token::SETTER_DEF => Some(n.get_string_ref(ast)),
        Token::STRING_KEY if parent.unwrap().is_object_lit(ast) => Some(n.get_string_ref(ast)),
        _ => None,
    }
}

// port: GatherExternProperties#gatherPropertiesFromJsTypeExpressionNode
pub(crate) fn gather_type_fields(
    names: &mut IndexSet<String>,
    ast: &Ast,
    js_type_expression_node: NodeId,
) {
    match js_type_expression_node.get_token(ast) {
        Token::LB => gather_record_fields(names, ast, js_type_expression_node),
        _ => {
            let mut child = js_type_expression_node.get_first_child(ast);
            while let Some(c) = child {
                gather_type_fields(names, ast, c);
                child = c.get_next(ast);
            }
        }
    }
}

// port: GatherExternProperties#gatherPropertiesFromJsDocRecordType
fn gather_record_fields(names: &mut IndexSet<String>, ast: &Ast, js_doc_record_node: NodeId) {
    check_state!(
        js_doc_record_node.get_token(ast) == Token::LB,
        &js_doc_record_node.to_string(ast)
    );
    let mut field_node = js_doc_record_node.get_first_child(ast);
    while let Some(field) = field_node {
        let field_name_node: NodeId;
        let field_type_node: Option<NodeId>;
        if field.get_token(ast) == Token::COLON {
            field_name_node = field.get_first_child(ast).unwrap();
            field_type_node = field_name_node.get_next(ast);
        } else {
            field_name_node = field;
            field_type_node = None;
        }
        check_state!(
            field_name_node.is_string_key(ast),
            &field_name_node.to_string(ast)
        );
        let mut field_name = field_name_node.get_string(ast);
        // TODO(bradfordcsmith): The JSDoc parser should do this.
        if field_name.starts_with("'") || field_name.starts_with("\"") {
            field_name = field_name.substring(1, field_name.length() - 1);
        }
        names.insert(field_name.to_string_lossy());
        if let Some(field_type_node) = field_type_node {
            gather_type_fields(names, ast, field_type_node);
        }
        field_node = field.get_next(ast);
    }
}
