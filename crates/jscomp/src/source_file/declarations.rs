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

//! Statement records shared by the host build and the compiler. Each describes a global
//! declaration with at most one local scope containing names (an empty function's parameters).

use closure_rhino::{
    node::{Ast, NodeId},
    token::Token,
};

pub(crate) const OTHER: u8 = 0;
pub(crate) const PROPERTY: u8 = 1;
pub(crate) const FUNCTION: u8 = 2;
pub(crate) const VARIABLE: u8 = 3;
pub(crate) const ASSIGNMENT: u8 = 4;
const KIND_MASK: u8 = 7;
pub(crate) const HAS_FUNCTION: u8 = 8;
pub(crate) const CHECK_NON_NULLABLE: u8 = 16;
/// An explicitly typed field on the prototype of a simple global name.
pub(crate) const PROTOTYPE_FIELD: u8 = 32;

// The high two bits encode common closed property annotations in the embedded externs.
pub(crate) const NATIVE_NUMBER: u8 = 0x40;
pub(crate) const NATIVE_STRING: u8 = 0x80;
pub(crate) const NATIVE_BOOLEAN: u8 = 0xc0;
pub(crate) const NATIVE_MASK: u8 = 0xc0;

pub(crate) fn closed_annotation(ast: &Ast, root: NodeId) -> u8 {
    if !root.is_string_lit(ast) || root.has_children(ast) {
        return 0;
    }
    let Some(name) = root.string_payload(ast) else {
        return 0;
    };
    if name == "number" {
        NATIVE_NUMBER
    } else if name == "string" {
        NATIVE_STRING
    } else if name == "boolean" {
        NATIVE_BOOLEAN
    } else {
        0
    }
}

pub(crate) fn kind(record: u8) -> u8 {
    record & KIND_MASK
}

/// Bind the recorded initializer to this statement of the verified syntax prefix.
pub(crate) fn initializer(ast: &Ast, statement: NodeId, record: u8) -> Option<NodeId> {
    match kind(record) {
        FUNCTION => Some(statement),
        VARIABLE => statement.get_first_child(ast)?.get_first_child(ast),
        ASSIGNMENT => statement.get_first_child(ast)?.get_second_child(ast),
        _ => None,
    }
}

pub(crate) enum DocSite {
    None,
    On(NodeId),
    BestForName(NodeId),
}

pub(crate) struct Declaration {
    pub(crate) function: Option<NodeId>,
    pub(crate) doc: DocSite,
    pub(crate) possible_alias: bool,
}

/// A chain of single-child GETPROPs over a leaf NAME. A rewritten child could contain
/// another declaration or a scope even when its ancestor still has the recorded token.
fn bare_name(ast: &Ast, mut n: NodeId) -> bool {
    while n.is_get_prop(ast) && n.has_one_child(ast) {
        n = n.get_first_child(ast).unwrap();
    }
    n.is_name(ast) && !n.has_children(ast) && !n.get_string_ref(ast).is_empty()
}

pub(crate) fn bare_property(ast: &Ast, statement: NodeId) -> Option<NodeId> {
    if !statement.is_expr_result(ast) || !statement.has_one_child(ast) {
        return None;
    }
    let property = statement.get_first_child(ast)?;
    (property.is_get_prop(ast) && bare_name(ast, property)).then_some(property)
}

fn stub_parameters(ast: &Ast, function: NodeId) -> Option<NodeId> {
    if !function.is_function(ast) {
        return None;
    }
    let body = function.get_last_child(ast)?;
    if !body.is_block(ast) || body.has_children(ast) {
        return None;
    }
    let name = function.get_first_child(ast)?;
    let params = name.get_next(ast)?;
    (name.is_name(ast)
        && !name.has_children(ast)
        && params.is_param_list(ast)
        && params.get_next(ast) == Some(body))
    .then_some(params)
}

pub(crate) fn is_plain_function_stub(ast: &Ast, function: NodeId) -> bool {
    stub_parameters(ast, function).is_some_and(|params| {
        params
            .children(ast)
            .all(|param| param.is_name(ast) && !param.has_children(ast))
    })
}

fn is_function_stub(ast: &Ast, function: NodeId, declaration: bool) -> bool {
    stub_parameters(ast, function).is_some_and(|params| {
        function
            .get_first_child(ast)
            .unwrap()
            .get_string_ref(ast)
            .is_empty()
            != declaration
            && params.children(ast).all(|param| {
                param.is_name(ast)
                    && !param.has_children(ast)
                    && !param.get_string_ref(ast).is_empty()
            })
    })
}

impl Declaration {
    /// Recheck the build record against the current statement. Return all sites at which
    /// its two initial-scope visitors can have observable effects.
    pub(crate) fn read(ast: &Ast, statement: NodeId, record: u8) -> Option<Self> {
        let (doc, initializer) = match kind(record) {
            PROPERTY => (DocSite::On(bare_property(ast, statement)?), None),
            FUNCTION if is_function_stub(ast, statement, true) => {
                return Some(Self {
                    function: Some(statement),
                    doc: DocSite::None,
                    possible_alias: false,
                });
            }
            VARIABLE
                if matches!(
                    statement.get_token(ast),
                    Token::VAR | Token::LET | Token::CONST
                ) && statement.has_one_child(ast) =>
            {
                let name = statement.get_first_child(ast)?;
                if !name.is_name(ast) || name.get_string_ref(ast).is_empty() {
                    return None;
                }
                let initializer = name.get_first_child(ast);
                if initializer.is_some_and(|n| n.get_next(ast).is_some()) {
                    return None;
                }
                (DocSite::BestForName(name), initializer)
            }
            ASSIGNMENT if statement.is_expr_result(ast) && statement.has_one_child(ast) => {
                let assign = statement.get_first_child(ast)?;
                if !assign.is_assign(ast)
                    || !assign.has_two_children(ast)
                    || !bare_name(ast, assign.get_first_child(ast)?)
                {
                    return None;
                }
                (DocSite::On(assign), assign.get_second_child(ast))
            }
            _ => return None,
        };
        let mut function = None;
        let mut possible_alias = false;
        if let Some(value) = initializer {
            match value.get_token(ast) {
                Token::FUNCTION if is_function_stub(ast, value, false) => function = Some(value),
                Token::NAME | Token::GETPROP if bare_name(ast, value) => possible_alias = true,
                Token::NULL
                | Token::TRUE
                | Token::FALSE
                | Token::NUMBER
                | Token::STRINGLIT
                | Token::OBJECTLIT
                | Token::ARRAYLIT
                    if !value.has_children(ast) => {}
                _ => return None,
            }
        }
        Some(Self {
            function,
            doc,
            possible_alias,
        })
    }
}
