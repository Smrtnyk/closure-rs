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
 *   Norris Boyd
 *   Roger Lawrence
 *   Mike McCabe
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
//   src/com/google/javascript/rhino/QualifiedName.java.

use crate::{
    js_string::JsString,
    node::{Ast, NodeId},
    rhino_string_pool::RhinoStringPool,
    token::Token,
};
use std::sync::Arc;
#[derive(Debug, Clone)]
// Names are the Java nested classes.
#[allow(clippy::enum_variant_names)]
enum QnameKind {
    StringListQname {
        terms: Arc<[JsString]>,
        size: usize,
    },
    GetpropQname {
        owner: QualifiedName,
        prop: JsString,
    },
    NodeQname {
        node: NodeId,
    },
}
#[derive(Debug, Clone)]
pub struct QualifiedName {
    kind: Arc<QnameKind>,
}
impl PartialEq for QualifiedName {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.kind, &other.kind)
    }
}
impl Eq for QualifiedName {}
impl QualifiedName {
    // port: QualifiedName#QualifiedName
    fn new(kind: QnameKind) -> Self {
        Self {
            kind: Arc::new(kind),
        }
    }
    // port: StringListQname#StringListQname
    fn from_terms(terms: Arc<[JsString]>, size: usize) -> Self {
        Self::new(QnameKind::StringListQname { terms, size })
    }
    // port: QualifiedName#of
    pub fn of(string: impl Into<JsString>) -> Self {
        let string = string.into();
        let mut last_index = 0;
        let mut builder = Vec::new();
        loop {
            let index = string.index_of_from(&".".into(), last_index as i32);
            let term = string.substring(
                last_index,
                if index < 0 {
                    string.length()
                } else {
                    index as usize
                },
            );
            builder.push(RhinoStringPool::add_or_get(term));
            last_index = (index + 1) as usize;
            if index < 0 {
                break;
            }
        }
        let size = builder.len();
        Self::from_terms(Arc::from(builder), size)
    }
    // port: NodeQname#NodeQname
    pub fn from_node(n: NodeId) -> Self {
        Self::new(QnameKind::NodeQname { node: n })
    }
    // port: QualifiedName#getOwner
    // port: StringListQname#getOwner
    // port: GetpropQname#getOwner
    // port: NodeQname#getOwner
    pub fn get_owner(&self, ast: &Ast) -> Option<Self> {
        match self.kind.as_ref() {
            QnameKind::StringListQname { terms, size } => {
                if *size > 1 {
                    Some(Self::from_terms(terms.clone(), size - 1))
                } else {
                    None
                }
            }
            QnameKind::GetpropQname { owner, .. } => Some(owner.clone()),
            QnameKind::NodeQname { node } => {
                if node.is_get_prop(ast) {
                    Some(Self::from_node(node.get_first_child(ast).unwrap()))
                } else {
                    None
                }
            }
        }
    }
    // port: QualifiedName#getComponent
    // port: StringListQname#getComponent
    // port: GetpropQname#getComponent
    // port: NodeQname#getComponent
    pub fn get_component(&self, ast: &Ast) -> JsString {
        match self.kind.as_ref() {
            QnameKind::StringListQname { terms, size } => terms[size - 1].clone(),
            QnameKind::GetpropQname { prop, .. } => prop.clone(),
            QnameKind::NodeQname { node } => match node.get_token(ast) {
                Token::THIS => RhinoStringPool::add_or_get("this"),
                Token::SUPER => RhinoStringPool::add_or_get("super"),
                Token::NAME | Token::GETPROP | Token::MEMBER_FUNCTION_DEF => node.get_string(ast),
                _ => panic!("Not a qualified name: {}", node.to_string(ast)),
            },
        }
    }
    // port: QualifiedName#isSimple
    // port: StringListQname#isSimple
    // port: GetpropQname#isSimple
    // port: NodeQname#isSimple
    pub fn is_simple(&self, ast: &Ast) -> bool {
        match self.kind.as_ref() {
            QnameKind::StringListQname { size, .. } => *size == 1,
            QnameKind::GetpropQname { .. } => false,
            QnameKind::NodeQname { node } => !node.is_get_prop(ast),
        }
    }
    // port: QualifiedName#appendTo
    // port: StringListQname#appendTo
    // port: GetpropQname#appendTo
    // port: NodeQname#appendTo
    fn append_to(&self, ast: &Ast, sb: &mut Vec<u16>, separator: u16) {
        match self.kind.as_ref() {
            QnameKind::StringListQname { terms, size } => {
                for (i, term) in terms[..*size].iter().enumerate() {
                    if i > 0 {
                        sb.push(separator);
                    }
                    sb.extend_from_slice(term.as_units());
                }
            }
            QnameKind::GetpropQname { owner, prop } => {
                owner.append_to(ast, sb, separator);
                sb.push(separator);
                sb.extend_from_slice(prop.as_units());
            }
            QnameKind::NodeQname { .. } => {
                let mut components = self.components(ast).into_iter();
                sb.extend_from_slice(components.next().unwrap().as_units());
                for c in components {
                    sb.push(separator);
                    sb.extend_from_slice(c.as_units());
                }
            }
        }
    }
    // port: QualifiedName#matches
    // port: StringListQname#matches
    // port: GetpropQname#matches
    // port: NodeQname#matches
    pub fn matches(&self, ast: &Ast, mut n: NodeId) -> bool {
        match self.kind.as_ref() {
            QnameKind::StringListQname { terms, size } => {
                let mut pos = size - 1;
                while pos > 0 && n.is_get_prop(ast) {
                    if !RhinoStringPool::unchecked_equals(&n.get_string(ast), &terms[pos]) {
                        return false;
                    }
                    pos -= 1;
                    n = n.get_first_child(ast).unwrap();
                }
                if pos > 0 {
                    return false;
                }
                let term = &terms[0];
                match n.get_token(ast) {
                    Token::NAME | Token::MEMBER_FUNCTION_DEF => {
                        RhinoStringPool::unchecked_equals(term, &n.get_string(ast))
                    }
                    Token::THIS => RhinoStringPool::unchecked_equals(
                        term,
                        &RhinoStringPool::add_or_get("this"),
                    ),
                    Token::SUPER => RhinoStringPool::unchecked_equals(
                        term,
                        &RhinoStringPool::add_or_get("super"),
                    ),
                    _ => false,
                }
            }
            QnameKind::GetpropQname { owner, prop } => {
                n.is_get_prop(ast)
                    && RhinoStringPool::unchecked_equals(&n.get_string(ast), prop)
                    && owner.matches(ast, n.get_first_child(ast).unwrap())
            }
            QnameKind::NodeQname { node } => n.matches_qualified_name_node(ast, *node),
        }
    }
    // port: QualifiedName#getRoot
    pub fn get_root(&self, ast: &Ast) -> JsString {
        let mut name = self.clone();
        while !name.is_simple(ast) {
            name = name.get_owner(ast).unwrap();
        }
        name.get_component(ast)
    }
    // port: QualifiedName#components
    // port: StringListQname#components
    pub fn components(&self, ast: &Ast) -> Vec<JsString> {
        if let QnameKind::StringListQname { terms, size } = self.kind.as_ref() {
            terms[..*size].to_vec()
        } else {
            let mut components = Vec::new();
            self.build_components(ast, &mut components);
            components
        }
    }
    // port: QualifiedName#buildComponents
    fn build_components(&self, ast: &Ast, builder: &mut Vec<JsString>) {
        if let Some(owner) = self.get_owner(ast) {
            owner.build_components(ast, builder);
        }
        builder.push(self.get_component(ast));
    }
    // port: QualifiedName#join()
    // port: NodeQname#join
    pub fn join(&self, ast: &Ast) -> JsString {
        if let QnameKind::NodeQname { node } = self.kind.as_ref() {
            node.get_qualified_name(ast).expect("NullPointerException")
        } else {
            self.join_with_separator(ast, b'.' as u16)
        }
    }
    // port: QualifiedName#join(Character)
    pub fn join_with_separator(&self, ast: &Ast, separator: u16) -> JsString {
        let mut sb = Vec::new();
        self.append_to(ast, &mut sb, separator);
        JsString::from_units(sb)
    }
    // port: QualifiedName#getComponentCount
    // port: StringListQname#getComponentCount
    // port: GetpropQname#getComponentCount
    // port: NodeQname#getComponentCount
    pub fn get_component_count(&self, ast: &Ast) -> i32 {
        match self.kind.as_ref() {
            QnameKind::StringListQname { size, .. } => *size as i32,
            QnameKind::GetpropQname { owner, .. } => owner.get_component_count(ast).wrapping_add(1),
            QnameKind::NodeQname { node } => {
                let mut count = 1;
                let mut current = *node;
                while current.is_get_prop(ast) {
                    count += 1;
                    current = current.get_first_child(ast).unwrap();
                }
                count
            }
        }
    }
    // port: QualifiedName#getprop
    // port: GetpropQname#GetpropQname
    pub fn getprop(&self, property_name: impl Into<JsString>) -> Self {
        Self::new(QnameKind::GetpropQname {
            owner: self.clone(),
            prop: RhinoStringPool::add_or_get(property_name),
        })
    }
}
