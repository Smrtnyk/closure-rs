/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Michael Zhou
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/TypeDeclarationsIR.java.

//! Construction helpers for type declaration ASTs.
use crate::{
    check_argument,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    token::Token,
};
use indexmap::IndexMap;
#[derive(Debug)]
pub struct TypeDeclarationsIR;
impl TypeDeclarationsIR {
    // port: TypeDeclarationsIR#stringType
    pub fn string_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::STRING_TYPE)
    }
    // port: TypeDeclarationsIR#numberType
    pub fn number_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::NUMBER_TYPE)
    }
    // port: TypeDeclarationsIR#booleanType
    pub fn boolean_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::BOOLEAN_TYPE)
    }
    // port: TypeDeclarationsIR#anyType
    pub fn any_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::ANY_TYPE)
    }
    // port: TypeDeclarationsIR#voidType
    pub fn void_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::VOID_TYPE)
    }
    // port: TypeDeclarationsIR#undefinedType
    pub fn undefined_type(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::UNDEFINED_TYPE)
    }
    // port: TypeDeclarationsIR#namedType(String)
    pub fn named_type(ast: &mut Ast, type_name: impl Into<JsString>) -> NodeId {
        let type_name = type_name.into();
        let mut segments = vec![];
        let mut start = 0;
        for i in 0..type_name.length() {
            if type_name.char_at(i) == b'.' as u16 {
                segments.push(type_name.substring(start, i));
                start = i + 1;
            }
        }
        segments.push(type_name.substring_from(start));
        Self::named_type_with_segments(ast, segments)
    }
    // port: TypeDeclarationsIR#namedType(Iterable)
    pub fn named_type_with_segments(
        ast: &mut Ast,
        segments: impl IntoIterator<Item = impl Into<JsString>>,
    ) -> NodeId {
        let mut segments_it = segments.into_iter();
        let mut node = IR::name(ast, segments_it.next().expect("NoSuchElementException"));
        for segment in segments_it {
            node = IR::getprop(ast, node, segment);
        }
        ast.new_node_with_child(Token::NAMED_TYPE, node)
    }
    // port: TypeDeclarationsIR#recordType
    pub fn record_type(ast: &mut Ast, properties: &IndexMap<JsString, Option<NodeId>>) -> NodeId {
        let node = ast.new_node(Token::RECORD_TYPE);
        for (key, value) in properties {
            let string_key = IR::string_key(ast, key.clone());
            node.add_child_to_back(ast, string_key);
            if let Some(value) = value {
                string_key.add_child_to_front(ast, *value);
            }
        }
        node
    }
    // port: TypeDeclarationsIR#maybeAddType
    fn maybe_add_type(ast: &mut Ast, node: NodeId, r#type: Option<NodeId>) -> NodeId {
        if r#type.is_some() {
            node.set_declared_type_expression(ast, r#type);
        }
        node
    }
    // port: TypeDeclarationsIR#functionType
    pub fn function_type(
        ast: &mut Ast,
        return_type: NodeId,
        required_params: &IndexMap<JsString, Option<NodeId>>,
        optional_params: &IndexMap<JsString, Option<NodeId>>,
        rest_name: Option<JsString>,
        rest_type: Option<NodeId>,
    ) -> NodeId {
        let node = ast.new_node_with_child(Token::FUNCTION_TYPE, return_type);
        for (key, value) in required_params {
            let name = IR::name(ast, key.clone());
            let name = Self::maybe_add_type(ast, name, *value);
            node.add_child_to_back(ast, name);
        }
        for (key, value) in optional_params {
            let name = IR::name(ast, key.clone());
            name.put_boolean_prop(ast, Prop::OPT_ES6_TYPED, true);
            let name = Self::maybe_add_type(ast, name, *value);
            node.add_child_to_back(ast, name);
        }
        if let Some(rest_name) = rest_name {
            let name = IR::name(ast, rest_name);
            let rest = ast.new_node_with_child(Token::ITER_REST, name);
            let rest = Self::maybe_add_type(ast, rest, rest_type);
            node.add_child_to_back(ast, rest);
        }
        node
    }
    // port: TypeDeclarationsIR#parameterizedType
    pub fn parameterized_type(
        ast: &mut Ast,
        base_type: NodeId,
        type_parameters: &[NodeId],
    ) -> NodeId {
        if type_parameters.is_empty() {
            return base_type;
        }
        let node = ast.new_node_with_child(Token::PARAMETERIZED_TYPE, base_type);
        for &type_parameter in type_parameters {
            node.add_child_to_back(ast, type_parameter);
        }
        node
    }
    // port: TypeDeclarationsIR#arrayType
    pub fn array_type(ast: &mut Ast, element_type: NodeId) -> NodeId {
        ast.new_node_with_child(Token::ARRAY_TYPE, element_type)
    }
    // port: TypeDeclarationsIR#unionType(Iterable)
    pub fn union_type_iterable(ast: &mut Ast, options: impl IntoIterator<Item = NodeId>) -> NodeId {
        let mut options = options.into_iter().peekable();
        check_argument!(
            options.peek().is_some(),
            "union must have at least one option"
        );
        let node = ast.new_node(Token::UNION_TYPE);
        for option in options {
            node.add_child_to_back(ast, option);
        }
        node
    }
    // port: TypeDeclarationsIR#unionType(Node...)
    pub fn union_type(ast: &mut Ast, options: &[NodeId]) -> NodeId {
        Self::union_type_iterable(ast, options.iter().copied())
    }
    // port: TypeDeclarationsIR#optionalParameter
    pub fn optional_parameter(ast: &mut Ast, parameter_type: NodeId) -> NodeId {
        ast.new_node_with_child(Token::OPTIONAL_PARAMETER, parameter_type)
    }
}
