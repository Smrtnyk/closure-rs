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
 *   Bob Jervis
 *   Google Inc.
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
//   src/com/google/javascript/rhino/jstype/RecordTypeBuilder.java.

use crate::{JSTypeNative, JSTypeRegistry, TypeId, js_type::JSType};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::collections::BTreeMap;

pub struct RecordTypeBuilder {
    is_empty: bool,
    is_declared: bool,
    properties: IndexMap<JsString, RecordProperty>,
}
impl Default for RecordTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl RecordTypeBuilder {
    // port: RecordTypeBuilder#RecordTypeBuilder
    pub fn new() -> Self {
        Self {
            is_empty: true,
            is_declared: true,
            properties: IndexMap::<_, _>::default(),
        }
    }
    // port: RecordTypeBuilder#setSynthesized
    pub fn set_synthesized(&mut self, synthesized: bool) {
        self.is_declared = !synthesized;
    }
    // port: RecordTypeBuilder#addProperty
    pub fn add_property(
        &mut self,
        name: impl Into<JsString>,
        type_: TypeId,
        property_node: Option<NodeId>,
    ) -> &mut Self {
        self.is_empty = false;
        self.properties
            .insert(name.into(), RecordProperty::new(type_, property_node));
        self
    }
    // port: RecordTypeBuilder#build
    pub fn build(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        if self.is_empty {
            reg.get_native_object_type(JSTypeNative::OBJECT_TYPE)
        } else {
            let properties: BTreeMap<_, _> = self.properties.clone().into_iter().collect();
            crate::record_type::new(reg, ast, properties, self.is_declared)
        }
    }
}
#[derive(Clone, Copy)]
pub struct RecordProperty {
    type_: TypeId,
    property_node: Option<NodeId>,
}
impl RecordProperty {
    // port: RecordTypeBuilder.RecordProperty#RecordProperty
    pub fn new(type_: TypeId, property_node: Option<NodeId>) -> Self {
        Self {
            type_,
            property_node,
        }
    }
    // port: RecordTypeBuilder.RecordProperty#getType
    pub fn get_type(&self) -> TypeId {
        self.type_
    }
    // port: RecordTypeBuilder.RecordProperty#getPropertyNode
    pub fn get_property_node(&self) -> Option<NodeId> {
        self.property_node
    }
    // port: RecordTypeBuilder.RecordProperty#toString
    pub fn to_string(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        format!(
            "RecordProperty{{type: {}, node: {}}}",
            self.type_.to_string(reg, ast),
            self.property_node
                .map_or_else(|| "null".to_string(), |n| n.to_string(ast))
        )
    }
}
