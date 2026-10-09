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
 *   Nick Santos
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
//   src/com/google/javascript/rhino/jstype/Property.java.

use std::sync::Arc;

use crate::{
    TypeId, function_type::FunctionType, js_type::JSType, js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
};
use closure_rhino::{
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    static_source_file::StaticSourceFile,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PropertyId(pub u32);
impl PropertyId {
    // port: Property#Property
    pub fn new(
        reg: &mut JSTypeRegistry,
        name: impl Into<PropertyKey>,
        type_: TypeId,
        inferred: bool,
        node: Option<NodeId>,
    ) -> Self {
        reg.alloc_property(PropertyData::new(name, type_, inferred, node))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyKind {
    STRING,
    SYMBOL,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyKey {
    String(JsString),
    Symbol(TypeId),
}
pub type Key = PropertyKey;

impl From<&str> for PropertyKey {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}
impl From<&PropertyKey> for PropertyKey {
    fn from(value: &PropertyKey) -> Self {
        value.clone()
    }
}
impl From<JsString> for PropertyKey {
    fn from(value: JsString) -> Self {
        Self::String(value)
    }
}
impl From<&JsString> for PropertyKey {
    fn from(value: &JsString) -> Self {
        Self::String(value.clone())
    }
}
impl From<TypeId> for PropertyKey {
    fn from(value: TypeId) -> Self {
        Self::Symbol(value)
    }
}

impl PropertyKey {
    // port: Property.Key#kind
    pub fn kind(&self) -> KeyKind {
        match self {
            Self::String(_) => KeyKind::STRING,
            Self::Symbol(_) => KeyKind::SYMBOL,
        }
    }
    // port: Property.Key#string
    pub fn string(&self) -> &JsString {
        match self {
            Self::String(s) => s,
            Self::Symbol(_) => panic!("UnsupportedOperationException"),
        }
    }
    // port: Property.Key#symbol
    pub fn symbol(&self) -> TypeId {
        match self {
            Self::Symbol(s) => *s,
            Self::String(_) => panic!("UnsupportedOperationException"),
        }
    }
    // port: Property.Key#humanReadableName
    pub fn human_readable_name(&self, reg: &JSTypeRegistry) -> JsString {
        match self {
            Self::String(s) => s.clone(),
            Self::Symbol(s) => s.get_display_name(reg).unwrap().into(),
        }
    }
    // port: Property.Key#matches
    pub fn matches(&self, string_key: impl closure_rhino::js_string::JsStrLike) -> bool {
        matches!(self, Self::String(s) if string_key.with_units(|k| k == s.as_units()))
    }
}

#[derive(Clone, Debug)]
pub struct StringKey(pub JsString);
impl StringKey {
    // port: Property.StringKey#StringKey
    pub fn new(string: impl Into<JsString>) -> Self {
        Self(string.into())
    }
    // port: Property.StringKey#symbol
    pub fn symbol(&self) -> TypeId {
        panic!("UnsupportedOperationException")
    }
    // port: Property.StringKey#kind
    pub fn kind(&self) -> KeyKind {
        KeyKind::STRING
    }
    // port: Property.StringKey#humanReadableName
    pub fn human_readable_name(&self) -> JsString {
        self.0.clone()
    }
}
impl From<StringKey> for PropertyKey {
    fn from(key: StringKey) -> Self {
        Self::String(key.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SymbolKey(pub TypeId);
impl SymbolKey {
    // port: Property.SymbolKey#SymbolKey
    pub fn new(symbol: TypeId) -> Self {
        Self(symbol)
    }
    // port: Property.SymbolKey#string
    pub fn string(&self) -> JsString {
        panic!("UnsupportedOperationException")
    }
    // port: Property.SymbolKey#kind
    pub fn kind(&self) -> KeyKind {
        KeyKind::SYMBOL
    }
    // port: Property.SymbolKey#humanReadableName
    pub fn human_readable_name(&self, reg: &JSTypeRegistry) -> JsString {
        self.0.get_display_name(reg).unwrap().into()
    }
}
impl From<SymbolKey> for PropertyKey {
    fn from(key: SymbolKey) -> Self {
        Self::Symbol(key.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedProperty {
    pub(crate) owner: Option<TypeId>,
    pub(crate) value: PropertyId,
}
impl OwnedProperty {
    // port: Property.OwnedProperty#OwnedProperty
    pub fn new(owner: impl Into<Option<TypeId>>, value: PropertyId) -> Self {
        Self {
            owner: owner.into(),
            value,
        }
    }
    // port: Property.OwnedProperty#getOwner
    pub fn get_owner(self) -> Option<TypeId> {
        self.owner
    }
    // port: Property.OwnedProperty#getValue
    pub fn get_value(self) -> PropertyId {
        self.value
    }
    // port: Property.OwnedProperty#getOwnerInstanceType
    pub fn get_owner_instance_type(self, reg: &JSTypeRegistry) -> TypeId {
        let owner = self.owner.expect("NullPointerException");
        if owner.is_function_prototype_type(reg) {
            owner
                .get_owner_function(reg)
                .unwrap()
                .get_instance_type(reg)
                .unwrap()
        } else {
            owner
        }
    }
    // port: Property.OwnedProperty#isOwnedByInterface
    pub fn is_owned_by_interface(self, reg: &JSTypeRegistry) -> bool {
        let owner = self.owner.expect("NullPointerException");
        if owner.is_function_prototype_type(reg) {
            owner.get_owner_function(reg).unwrap().is_interface(reg)
        } else {
            owner.is_interface(reg)
        }
    }
}

#[derive(Clone, Debug)]
pub struct PropertyData {
    pub(crate) name: PropertyKey,
    pub(crate) type_: TypeId,
    pub(crate) inferred: bool,
    pub(crate) property_node: Option<NodeId>,
    pub(crate) doc_info: Option<Arc<JSDocInfo>>,
}
impl PropertyData {
    // port: Property#Property
    pub fn new(
        name: impl Into<PropertyKey>,
        type_: TypeId,
        inferred: bool,
        property_node: Option<NodeId>,
    ) -> Self {
        Self {
            name: name.into(),
            type_,
            inferred,
            property_node,
            doc_info: None,
        }
    }
}

pub trait Property {
    fn get_name(self, reg: &JSTypeRegistry) -> JsString;
    fn get_node(self, reg: &JSTypeRegistry) -> Option<NodeId>;
    fn get_source_file(self, reg: &JSTypeRegistry, ast: &Ast) -> Option<Arc<dyn StaticSourceFile>>;
    fn get_symbol(self) -> PropertyId;
    fn get_declaration(self, reg: &JSTypeRegistry) -> Option<PropertyId>;
    fn get_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn is_type_inferred(self, reg: &JSTypeRegistry) -> bool;
    fn is_from_externs(self, reg: &JSTypeRegistry, ast: &Ast) -> bool;
    fn set_type(self, reg: &mut JSTypeRegistry, type_: TypeId);
    fn get_jsdoc_info(self, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>>;
    fn set_jsdoc_info(self, reg: &mut JSTypeRegistry, info: Option<Arc<JSDocInfo>>);
    fn set_node(self, reg: &mut JSTypeRegistry, node: Option<NodeId>);
    fn to_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> String;
    fn get_scope(self) -> Arc<dyn crate::static_typed_scope::StaticTypedScope>;
    fn hash_code(self, reg: &JSTypeRegistry) -> i32;
}
impl Property for PropertyId {
    // port: Property#getName
    fn get_name(self, reg: &JSTypeRegistry) -> JsString {
        reg.property(self).name.human_readable_name(reg)
    }
    // port: Property#getNode
    fn get_node(self, reg: &JSTypeRegistry) -> Option<NodeId> {
        reg.property(self).property_node
    }
    // port: Property#getSourceFile
    fn get_source_file(self, reg: &JSTypeRegistry, ast: &Ast) -> Option<Arc<dyn StaticSourceFile>> {
        self.get_node(reg)
            .and_then(|n| n.get_static_source_file(ast))
    }
    // port: Property#getSymbol
    fn get_symbol(self) -> PropertyId {
        self
    }
    // port: Property#getDeclaration
    fn get_declaration(self, reg: &JSTypeRegistry) -> Option<PropertyId> {
        self.get_node(reg).map(|_| self)
    }
    // port: Property#getType
    fn get_type(self, reg: &JSTypeRegistry) -> TypeId {
        reg.property(self).type_
    }
    // port: Property#isTypeInferred
    fn is_type_inferred(self, reg: &JSTypeRegistry) -> bool {
        reg.property(self).inferred
    }
    // port: Property#isFromExterns
    fn is_from_externs(self, reg: &JSTypeRegistry, ast: &Ast) -> bool {
        self.get_node(reg).is_some_and(|n| n.is_from_externs(ast))
    }
    // port: Property#setType
    fn set_type(self, reg: &mut JSTypeRegistry, type_: TypeId) {
        reg.property_mut(self).type_ = type_;
    }
    // port: Property#getJSDocInfo
    fn get_jsdoc_info(self, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        reg.property(self).doc_info.clone()
    }
    // port: Property#setJSDocInfo
    fn set_jsdoc_info(self, reg: &mut JSTypeRegistry, info: Option<Arc<JSDocInfo>>) {
        reg.property_mut(self).doc_info = info;
    }
    // port: Property#setNode
    fn set_node(self, reg: &mut JSTypeRegistry, node: Option<NodeId>) {
        reg.property_mut(self).property_node = node;
    }
    // port: Property#toString
    fn to_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        let name = match reg.property(self).name.clone() {
            PropertyKey::String(name) => name.to_string_lossy(),
            PropertyKey::Symbol(name) => name.to_string(reg, ast),
        };
        format!(
            "Property {{  name: {}, type:{}, inferred: {}}}",
            name,
            self.get_type(reg).to_string(reg, ast),
            self.is_type_inferred(reg)
        )
    }
    // port: Property#getScope
    fn get_scope(self) -> Arc<dyn crate::static_typed_scope::StaticTypedScope> {
        panic!("UnsupportedOperationException")
    }
    // port: Property#hashCode
    fn hash_code(self, reg: &JSTypeRegistry) -> i32 {
        let p = reg.property(self);
        let name_hash = match &p.name {
            PropertyKey::String(n) => n.hash_code(),
            PropertyKey::Symbol(s) => s.hash_code(reg),
        };
        31_i32
            .wrapping_add(name_hash)
            .wrapping_mul(31)
            .wrapping_add(p.type_.hash_code(reg))
    }
}

impl crate::static_typed_slot::StaticTypedSlot for PropertyId {
    // port: Property#getName
    fn get_name(&self, reg: &JSTypeRegistry) -> JsString {
        Property::get_name(*self, reg)
    }
    // port: Property#getType
    fn get_type(&self, reg: &JSTypeRegistry) -> Option<TypeId> {
        Some(Property::get_type(*self, reg))
    }
    // port: Property#isTypeInferred
    fn is_type_inferred(&self, reg: &JSTypeRegistry) -> bool {
        Property::is_type_inferred(*self, reg)
    }
    // port: Property#getDeclaration
    fn get_declaration(
        &self,
        reg: &JSTypeRegistry,
    ) -> Option<&dyn crate::static_typed_ref::StaticTypedRef> {
        Property::get_node(*self, reg).map(|_| self as &dyn crate::static_typed_ref::StaticTypedRef)
    }
    // port: Property#getJSDocInfo
    fn get_jsdoc_info(&self, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        Property::get_jsdoc_info(*self, reg)
    }
    // port: Property#getScope
    fn get_scope(
        &self,
        _reg: &JSTypeRegistry,
    ) -> Option<&dyn crate::static_typed_scope::StaticTypedScope> {
        panic!("UnsupportedOperationException")
    }
}
impl crate::static_typed_ref::StaticTypedRef for PropertyId {
    // port: Property#getSymbol
    fn get_symbol(&self, _reg: &JSTypeRegistry) -> &dyn crate::static_typed_slot::StaticTypedSlot {
        self
    }
    // port: Property#getNode
    fn get_node(&self, reg: &JSTypeRegistry) -> Option<NodeId> {
        Property::get_node(*self, reg)
    }
    // port: Property#getSourceFile
    fn get_source_file(
        &self,
        reg: &JSTypeRegistry,
        ast: &Ast,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        Property::get_source_file(*self, reg, ast)
    }
}
