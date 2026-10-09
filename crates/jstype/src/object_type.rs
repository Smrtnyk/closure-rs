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
//   src/com/google/javascript/rhino/jstype/ObjectType.java.

use crate::{
    TypeId,
    boolean_literal_set::BooleanLiteralSet,
    function_type::FunctionType,
    js_type::{HasPropertyKind, JSType, JSTypeKind},
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    property::{OwnedProperty, Property, PropertyId, PropertyKey},
    property_map::{AllKeys, PropertyMap},
};
use closure_rhino::{
    check_argument,
    js_string::JsString,
    jscomp_base::tri::Tri,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug)]
pub struct ObjectTypeData {
    pub(crate) visited: bool,
    pub(crate) doc_info: Option<Arc<JSDocInfo>>,
    pub(crate) unknown: bool,
}
impl Default for ObjectTypeData {
    fn default() -> Self {
        Self::new()
    }
}
impl ObjectTypeData {
    // port: ObjectType#ObjectType
    pub fn new() -> Self {
        Self {
            visited: false,
            doc_info: None,
            unknown: true,
        }
    }
    // port: ObjectType#ObjectType
    pub fn with_template_type_map() -> Self {
        Self::new()
    }
}

pub trait ObjectType {
    fn get_property_map(self, reg: &JSTypeRegistry) -> &PropertyMap;
    fn get_slot(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<PropertyId>;
    fn get_own_slot(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<PropertyId>;
    fn get_type_of_this(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn get_template_types(self, reg: &JSTypeRegistry) -> Option<Vec<TypeId>>;
    fn set_jsdoc_info(self, reg: &mut JSTypeRegistry, info: Option<Arc<JSDocInfo>>);
    fn detect_implicit_prototype_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn detect_inheritance_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn get_reference_name(self, reg: &JSTypeRegistry) -> Option<JsString>;
    fn has_reference_name(self, reg: &JSTypeRegistry) -> bool;
    fn get_normalized_reference_name(self, reg: &JSTypeRegistry) -> Option<JsString>;
    fn get_raw_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn get_constructor(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn get_super_class_constructor(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId>;
    fn get_closest_defining_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<JsString>,
    ) -> Option<TypeId>;
    fn find_closest_definition(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<OwnedProperty>;
    fn get_implicit_prototype(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId>;
    fn get_implicit_prototype_chain(self) -> ImplicitPrototypeChain;
    fn define_declared_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool;
    fn define_synthesized_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<JsString>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool;
    fn define_inferred_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool;
    fn define_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
        type_: TypeId,
        inferred: bool,
        node: Option<NodeId>,
    ) -> bool;
    fn get_property_node(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<NodeId>;
    fn get_property_def_site(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<NodeId>;
    fn get_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<Arc<JSDocInfo>>;
    fn get_own_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<Arc<JSDocInfo>>;
    fn get_own_property_def_site(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> Option<NodeId>;
    fn set_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
        info: Option<Arc<JSDocInfo>>,
    );
    fn set_property_node(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
        node: Option<NodeId>,
    );
    fn get_property_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> TypeId;
    fn get_own_property_kind(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> HasPropertyKind;
    fn has_own_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property_name: impl Into<PropertyKey>,
    ) -> bool;
    fn get_own_property_known_symbols(self, reg: &JSTypeRegistry) -> Vec<TypeId>;
    fn get_own_property_names(
        self,
        reg: &JSTypeRegistry,
    ) -> closure_rhino::fx_hash::IndexSet<JsString>;
    fn get_own_property_keys(self, reg: &JSTypeRegistry) -> Vec<PropertyKey>;
    fn is_property_type_inferred(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool;
    fn is_property_type_declared(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool;
    fn has_own_declared_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool;
    fn is_property_in_externs(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
    ) -> bool;
    fn get_properties_count(self, reg: &mut JSTypeRegistry, ast: &Ast) -> usize;
    fn get_property_names(self, reg: &mut JSTypeRegistry, ast: &Ast) -> BTreeSet<JsString>;
    fn get_all_keys(self, reg: &mut JSTypeRegistry, ast: &Ast) -> AllKeys;
    fn is_implicit_prototype_of(self, reg: &mut JSTypeRegistry, ast: &Ast, other: TypeId) -> bool;
    fn has_cached_values(self, reg: &JSTypeRegistry) -> bool;
    fn clear_cached_values(self, reg: &mut JSTypeRegistry);
    fn get_owner_function(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn set_owner_function(self, reg: &mut JSTypeRegistry, type_: Option<TypeId>);
    fn get_ctor_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId>;
    fn get_ctor_extended_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId>;
    fn get_property_type_map(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> closure_rhino::fx_hash::IndexMap<JsString, TypeId>;
    fn get_enumerated_type_of_enum_object(self, reg: &JSTypeRegistry) -> Option<TypeId>;
}

pub struct ImplicitPrototypeChain {
    next: Option<TypeId>,
}
impl ImplicitPrototypeChain {
    // port: ObjectType#computeNext
    pub fn next(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.next = self.next.and_then(|n| n.get_implicit_prototype(reg, ast));
        self.next
    }
}

impl ObjectType for TypeId {
    // port: ObjectType#getPropertyMap
    fn get_property_map(self, reg: &JSTypeRegistry) -> &PropertyMap {
        get_property_map(self, reg)
    }
    // port: ObjectType#getSlot
    fn get_slot(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<PropertyId> {
        let name = name.into();
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            crate::function_type::get_slot(self, reg, ast, &name)
        } else {
            get_slot(self, reg, ast, &name)
        }
    }
    // port: ObjectType#getOwnSlot
    fn get_own_slot(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<PropertyId> {
        let name = name.into();
        let map = self.get_property_map(reg);
        if let PropertyKey::String(n) = &name {
            // getOwnProperty of a string key only reads the map: no copy (D-025).
            return map.properties.get(n).copied();
        }
        let map = map.clone();
        map.get_own_property(reg, ast, &name)
    }
    // port: ObjectType#getTypeOfThis
    fn get_type_of_this(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        if is_proxy(self, reg) {
            return crate::proxy_object_type::get_type_of_this(self, reg);
        }
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            Some(crate::function_type::get_type_of_this(self, reg))
        } else {
            None
        }
    }
    // port: ObjectType#getTemplateTypes
    fn get_template_types(self, reg: &JSTypeRegistry) -> Option<Vec<TypeId>> {
        match &reg.data(self).kind {
            JSTypeKind::NoResolved(d) => d.template_types.clone(),
            JSTypeKind::Templatized(_) => crate::templatized_type::get_template_types(self, reg),
            JSTypeKind::Named(_) => crate::named_type::get_template_types(self, reg),
            JSTypeKind::ProxyObject(_) | JSTypeKind::Template(_) => {
                crate::proxy_object_type::get_template_types(self, reg)
            }
            _ => Some(Vec::new()),
        }
    }
    // port: ObjectType#setJSDocInfo
    fn set_jsdoc_info(self, reg: &mut JSTypeRegistry, info: Option<Arc<JSDocInfo>>) {
        if is_proxy(self, reg) {
            crate::proxy_object_type::set_jsdoc_info(self, reg, info);
        } else {
            reg.object_data_mut(self).doc_info = info;
        }
    }
    // port: ObjectType#detectImplicitPrototypeCycle
    fn detect_implicit_prototype_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        reg.object_data_mut(self).visited = true;
        let mut p = self.get_implicit_prototype(reg, ast);
        while let Some(current) = p {
            if reg.object_data(current).visited {
                return true;
            }
            reg.object_data_mut(current).visited = true;
            p = current.get_implicit_prototype(reg, ast);
        }
        p = Some(self);
        while let Some(current) = p {
            reg.object_data_mut(current).visited = false;
            p = current.get_implicit_prototype(reg, ast);
        }
        false
    }
    // port: ObjectType#detectInheritanceCycle
    fn detect_inheritance_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if self.detect_implicit_prototype_cycle(reg, ast) {
            return true;
        }
        for t in self.get_ctor_implemented_interfaces(reg, ast) {
            if t.equals(reg, ast, self) {
                return true;
            }
        }
        self.get_constructor(reg)
            .is_some_and(|f| f.check_extends_loop(reg, ast).is_some())
    }
    // port: ObjectType#getReferenceName
    fn get_reference_name(self, reg: &JSTypeRegistry) -> Option<JsString> {
        get_reference_name(self, reg)
    }
    // port: ObjectType#hasReferenceName
    fn has_reference_name(self, reg: &JSTypeRegistry) -> bool {
        self.get_reference_name(reg).is_some()
    }
    // port: ObjectType#getNormalizedReferenceName
    fn get_normalized_reference_name(self, reg: &JSTypeRegistry) -> Option<JsString> {
        let name = self.get_reference_name(reg)?;
        if let Some(start) = name.as_units().iter().position(|&c| c == b'(' as u16) {
            let end = name
                .as_units()
                .iter()
                .rposition(|&c| c == b')' as u16)
                .map(|n| n as i32)
                .unwrap_or(-1);
            let prefix = name.substring(0, start);
            return Some(if end + (1 % name.length() as i32) == 0 {
                prefix
            } else {
                prefix.concat(&name.substring_from((end + 1) as usize))
            });
        }
        Some(name)
    }
    // port: ObjectType#getRawType
    fn get_raw_type(self, reg: &JSTypeRegistry) -> TypeId {
        match self.to_maybe_templatized_type(reg) {
            None => self,
            Some(t) => crate::templatized_type::get_referenced_type(t, reg),
        }
    }
    // port: ObjectType#getConstructor
    fn get_constructor(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        get_constructor(self, reg)
    }
    // port: ObjectType#getSuperClassConstructor
    fn get_super_class_constructor(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            return crate::function_type::get_super_class_constructor(self, reg, ast);
        }
        let iproto = self.get_implicit_prototype(reg, ast)?;
        iproto
            .get_implicit_prototype(reg, ast)
            .and_then(|p| p.get_constructor(reg))
    }
    // port: ObjectType#getClosestDefiningType
    fn get_closest_defining_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
    ) -> Option<TypeId> {
        self.find_closest_definition(reg, ast, PropertyKey::String(name.into()))
            .and_then(OwnedProperty::get_owner)
    }
    // port: ObjectType#findClosestDefinition
    fn find_closest_definition(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<OwnedProperty> {
        PropertyMap::find_closest_of_type(self, reg, ast, &name.into())
    }
    // port: ObjectType#getImplicitPrototype
    fn get_implicit_prototype(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        get_implicit_prototype(self, reg, ast)
    }
    // port: ObjectType#getImplicitPrototypeChain
    fn get_implicit_prototype_chain(self) -> ImplicitPrototypeChain {
        ImplicitPrototypeChain { next: Some(self) }
    }
    // port: ObjectType#defineDeclaredProperty
    fn define_declared_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool {
        let name = name.into();
        let result = self.define_property(reg, ast, name.clone(), type_, false, node);
        if let PropertyKey::String(name) = name {
            reg.register_property_on_type(ast, name, self);
        }
        result
    }
    // port: ObjectType#defineSynthesizedProperty
    fn define_synthesized_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool {
        self.define_property(
            reg,
            ast,
            PropertyKey::String(name.into()),
            type_,
            false,
            node,
        )
    }
    // port: ObjectType#defineInferredProperty
    fn define_inferred_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        mut type_: TypeId,
        node: Option<NodeId>,
    ) -> bool {
        let name = name.into();
        let PropertyKey::String(property_name) = name else {
            panic!("UnsupportedOperationException");
        };
        check_argument!(
            true,
            "Symbol-based property keys are not supported for inferred properties."
        );
        if self.has_property(reg, ast, &property_name) {
            if self.is_property_type_declared(reg, ast, &property_name) {
                return true;
            }
            let original_type = self.get_property_type(reg, ast, &property_name);
            type_ = original_type.get_least_supertype(reg, ast, type_);
        }
        let result = self.define_property(reg, ast, &property_name, type_, true, node);
        reg.register_property_on_type(ast, property_name, self);
        result
    }
    // port: ObjectType#defineProperty
    fn define_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        type_: TypeId,
        inferred: bool,
        node: Option<NodeId>,
    ) -> bool {
        define_property(self, reg, ast, &name.into(), type_, inferred, node)
    }
    // port: ObjectType#getPropertyNode
    fn get_property_node(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<NodeId> {
        self.get_slot(reg, ast, name).and_then(|p| p.get_node(reg))
    }
    // port: ObjectType#getPropertyDefSite
    fn get_property_def_site(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<NodeId> {
        self.get_property_node(reg, ast, name)
    }
    // port: ObjectType#getPropertyJSDocInfo
    fn get_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<Arc<JSDocInfo>> {
        self.get_slot(reg, ast, name)
            .and_then(|p| p.get_jsdoc_info(reg))
    }
    // port: ObjectType#getOwnPropertyJSDocInfo
    fn get_own_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<Arc<JSDocInfo>> {
        self.get_own_slot(reg, ast, name)
            .and_then(|p| p.get_jsdoc_info(reg))
    }
    // port: ObjectType#getOwnPropertyDefSite
    fn get_own_property_def_site(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<NodeId> {
        self.get_own_slot(reg, ast, name)
            .and_then(|p| p.get_node(reg))
    }
    // port: ObjectType#setPropertyJSDocInfo
    fn set_property_jsdoc_info(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        info: Option<Arc<JSDocInfo>>,
    ) {
        if is_proxy(self, reg) {
            crate::proxy_object_type::set_property_jsdoc_info(self, reg, ast, &name.into(), info);
            return;
        }
        if matches!(
            reg.data(self).kind,
            JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            return;
        }
        if reg.prototype_data_opt(self).is_some() {
            crate::prototype_object_type::set_property_jsdoc_info(
                self,
                reg,
                ast,
                &name.into(),
                info,
            );
        }
    }
    // port: ObjectType#setPropertyNode
    fn set_property_node(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        node: Option<NodeId>,
    ) {
        if reg.prototype_data_opt(self).is_some() {
            crate::prototype_object_type::set_property_node(self, reg, ast, &name.into(), node);
        }
    }
    // port: ObjectType#getPropertyType
    fn get_property_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> TypeId {
        let name = name.into();
        // Java virtual dispatch: only a TemplatizedType itself runs its override; a
        // proxy wrapping one (toMaybeTemplatizedType != null) uses ProxyObjectType's.
        if matches!(reg.data(self).kind, JSTypeKind::Templatized(_)) {
            return crate::templatized_type::get_property_type(self, reg, ast, &name)
                .expect("NullPointerException");
        }
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            crate::function_type::get_property_type(self, reg, ast, &name)
        } else {
            get_property_type(self, reg, ast, &name)
        }
    }
    // port: ObjectType#getOwnPropertyKind
    fn get_own_property_kind(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> HasPropertyKind {
        if self.get_own_slot(reg, ast, name).is_some() {
            HasPropertyKind::KNOWN_PRESENT
        } else {
            HasPropertyKind::ABSENT
        }
    }
    // port: ObjectType#hasOwnProperty
    fn has_own_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool {
        self.get_own_property_kind(reg, ast, name) != HasPropertyKind::ABSENT
    }
    // port: ObjectType#getOwnPropertyKnownSymbols
    fn get_own_property_known_symbols(self, reg: &JSTypeRegistry) -> Vec<TypeId> {
        self.get_property_map(reg).get_own_known_symbols()
    }
    // port: ObjectType#getOwnPropertyNames
    fn get_own_property_names(
        self,
        reg: &JSTypeRegistry,
    ) -> closure_rhino::fx_hash::IndexSet<JsString> {
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            crate::function_type::get_own_property_names(self, reg)
        } else {
            self.get_property_map(reg)
                .get_own_property_names()
                .into_iter()
                .collect()
        }
    }
    // port: ObjectType#getOwnPropertyKeys
    fn get_own_property_keys(self, reg: &JSTypeRegistry) -> Vec<PropertyKey> {
        self.get_own_property_names(reg)
            .into_iter()
            .map(PropertyKey::String)
            .chain(
                self.get_own_property_known_symbols(reg)
                    .into_iter()
                    .map(PropertyKey::Symbol),
            )
            .collect()
    }
    // port: ObjectType#isPropertyTypeInferred
    fn is_property_type_inferred(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool {
        self.get_slot(reg, ast, name)
            .is_some_and(|p| p.is_type_inferred(reg))
    }
    // port: ObjectType#isPropertyTypeDeclared
    fn is_property_type_declared(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool {
        self.get_slot(reg, ast, name)
            .is_some_and(|p| !p.is_type_inferred(reg))
    }
    // port: ObjectType#hasOwnDeclaredProperty
    fn has_own_declared_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> bool {
        let name = name.into();
        self.has_own_property(reg, ast, name.clone())
            && self.is_property_type_declared(reg, ast, name)
    }
    // port: ObjectType#isPropertyInExterns
    fn is_property_in_externs(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
    ) -> bool {
        self.get_slot(reg, ast, PropertyKey::String(name.into()))
            .is_some_and(|p| p.is_from_externs(reg, ast))
    }
    // port: ObjectType#getPropertiesCount
    fn get_properties_count(self, reg: &mut JSTypeRegistry, ast: &Ast) -> usize {
        let map = self.get_property_map(reg).clone();
        map.get_properties_count(reg, ast)
    }
    // port: ObjectType#getPropertyNames
    fn get_property_names(self, reg: &mut JSTypeRegistry, ast: &Ast) -> BTreeSet<JsString> {
        (*self.get_all_keys(reg, ast).string_keys).clone()
    }
    // port: ObjectType#getAllKeys
    fn get_all_keys(self, reg: &mut JSTypeRegistry, ast: &Ast) -> AllKeys {
        let map = self.get_property_map(reg).clone();
        map.get_all_keys(reg, ast)
    }
    // port: ObjectType#isImplicitPrototypeOf
    fn is_implicit_prototype_of(self, reg: &mut JSTypeRegistry, ast: &Ast, other: TypeId) -> bool {
        let unwrapped_this = deeply_unwrap(self, reg);
        let mut other = deeply_unwrap(other, reg);
        while let Some(current) = other {
            if unwrapped_this == Some(current) {
                return true;
            }
            other = deeply_unwrap(current.get_implicit_prototype(reg, ast), reg);
        }
        false
    }
    // port: ObjectType#hasCachedValues
    fn has_cached_values(self, reg: &JSTypeRegistry) -> bool {
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            crate::function_type::has_cached_values(self, reg)
        } else {
            !reg.object_data(self).unknown
        }
    }
    // port: ObjectType#clearCachedValues
    fn clear_cached_values(self, reg: &mut JSTypeRegistry) {
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            crate::function_type::clear_cached_values(self, reg);
        } else {
            reg.object_data_mut(self).unknown = true;
        }
    }
    // port: ObjectType#getOwnerFunction
    fn get_owner_function(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        if is_proxy(self, reg) {
            crate::proxy_object_type::get_owner_function(self, reg)
        } else {
            reg.prototype_data_opt(self).and_then(|d| d.owner_function)
        }
    }
    // port: ObjectType#setOwnerFunction
    fn set_owner_function(self, reg: &mut JSTypeRegistry, type_: Option<TypeId>) {
        if reg.prototype_data_opt(self).is_some() {
            crate::prototype_object_type::set_owner_function(self, reg, type_);
        }
    }
    // port: ObjectType#getCtorImplementedInterfaces
    fn get_ctor_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        // Java virtual dispatch: only a TemplatizedType itself runs its override; a
        // proxy wrapping one (toMaybeTemplatizedType != null) uses ProxyObjectType's.
        if matches!(reg.data(self).kind, JSTypeKind::Templatized(_)) {
            return crate::templatized_type::get_ctor_implemented_interfaces(self, reg, ast);
        }
        if is_proxy(self, reg) {
            return crate::proxy_object_type::get_ctor_implemented_interfaces(self, reg, ast);
        }
        if let JSTypeKind::InstanceObject(d) = &reg.data(self).kind {
            let ctor = d.constructor;
            return ctor.get_implemented_interfaces(reg, ast);
        }
        if self.is_function_prototype_type(reg) {
            self.get_owner_function(reg)
                .unwrap()
                .get_implemented_interfaces(reg, ast)
        } else {
            Vec::new()
        }
    }
    // port: ObjectType#getCtorExtendedInterfaces
    fn get_ctor_extended_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        // Java virtual dispatch: only a TemplatizedType itself runs its override; a
        // proxy wrapping one (toMaybeTemplatizedType != null) uses ProxyObjectType's.
        if matches!(reg.data(self).kind, JSTypeKind::Templatized(_)) {
            return crate::templatized_type::get_ctor_extended_interfaces(self, reg, ast);
        }
        if is_proxy(self, reg) {
            return crate::proxy_object_type::get_ctor_extended_interfaces(self, reg, ast);
        }
        if let JSTypeKind::InstanceObject(d) = &reg.data(self).kind {
            return d.constructor.get_extended_interfaces(reg);
        }
        if self.is_function_prototype_type(reg) {
            self.get_owner_function(reg)
                .unwrap()
                .get_extended_interfaces(reg)
        } else {
            Vec::new()
        }
    }
    // port: ObjectType#getPropertyTypeMap
    fn get_property_type_map(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> closure_rhino::fx_hash::IndexMap<JsString, TypeId> {
        if matches!(
            reg.data(self).kind,
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_)
        ) {
            return crate::function_type::get_property_type_map(self, reg, ast)
                .into_iter()
                .collect();
        }
        self.get_property_names(reg, ast)
            .into_iter()
            .map(|name| {
                let type_ = self.get_property_type(reg, ast, &name);
                (name, type_)
            })
            .collect()
    }
    // port: ObjectType#getEnumeratedTypeOfEnumObject
    fn get_enumerated_type_of_enum_object(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        if self.is_enum_type(reg) {
            Some(crate::enum_type::get_elements_type(self, reg))
        } else {
            None
        }
    }
}

// port: ObjectType#getPropertyMap
pub fn get_property_map(t: TypeId, reg: &JSTypeRegistry) -> &PropertyMap {
    if matches!(reg.data(t).kind, JSTypeKind::EnumElement(_)) {
        return crate::enum_element_type::get_property_map(t, reg);
    }
    if is_proxy(t, reg) {
        return crate::proxy_object_type::get_property_map(t, reg);
    }
    if let Some(d) = reg.prototype_data_opt(t) {
        &d.properties
    } else {
        PropertyMap::immutable_empty_map()
    }
}
// port: ObjectType#getSlot
pub fn get_slot(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
) -> Option<PropertyId> {
    PropertyMap::find_closest_of_type(t, reg, ast, name).map(OwnedProperty::get_value)
}
// port: ObjectType#getJSDocInfo
pub fn get_jsdoc_info(t: TypeId, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
    reg.object_data(t).doc_info.clone()
}
// port: ObjectType#getDisplayName
pub fn get_display_name(t: TypeId, reg: &JSTypeRegistry) -> Option<String> {
    t.get_normalized_reference_name(reg)
        .map(|name| name.to_string_lossy())
}
// port: ObjectType#createDelegateSuffix
pub fn create_delegate_suffix(suffix: &str) -> String {
    format!("({suffix})")
}
// port: ObjectType#testForEquality
pub fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<Tri> {
    if let Some(result) = crate::js_type::test_for_equality_helper(t, reg, ast, that) {
        return Some(result);
    }
    if that.is_unknown_type(reg, ast) {
        return Some(Tri::UNKNOWN);
    }
    for native in [
        JSTypeNative::OBJECT_TYPE,
        JSTypeNative::NUMBER_TYPE,
        JSTypeNative::STRING_TYPE,
        JSTypeNative::BOOLEAN_TYPE,
        JSTypeNative::SYMBOL_TYPE,
        JSTypeNative::BIGINT_TYPE,
    ] {
        let type_ = reg.get_native_type(native);
        if that.is_subtype_of(reg, ast, type_) {
            return Some(Tri::UNKNOWN);
        }
    }
    Some(Tri::FALSE)
}
// port: ObjectType#findPropertyTypeWithoutConsideringTemplateTypes
pub fn find_property_type_without_considering_template_types(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
) -> Option<TypeId> {
    t.has_property(reg, ast, name.clone())
        .then(|| t.get_property_type(reg, ast, name.clone()))
}
// port: ObjectType#getPropertyType
pub fn get_property_type(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
) -> TypeId {
    let slot = t.get_slot(reg, ast, name.clone());
    get_property_type_from_slot(t, reg, slot)
}
// port: ObjectType#getPropertyTypeFromSlot
fn get_property_type_from_slot(
    t: TypeId,
    reg: &JSTypeRegistry,
    slot: Option<PropertyId>,
) -> TypeId {
    if let Some(slot) = slot {
        return slot.get_type(reg);
    }
    reg.get_native_type(
        if t.is_no_resolved_type(reg) || t.is_checked_unknown_type(reg) {
            JSTypeNative::CHECKED_UNKNOWN_TYPE
        } else if t.is_empty_type(reg) {
            JSTypeNative::NO_TYPE
        } else {
            JSTypeNative::UNKNOWN_TYPE
        },
    )
}
// port: ObjectType#getPropertyKind
pub fn get_property_kind(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    _autobox: bool,
) -> HasPropertyKind {
    HasPropertyKind::of(
        t.is_empty_type(reg)
            || t.is_unknown_type(reg, ast)
            || t.get_slot(reg, ast, name.clone()).is_some(),
    )
}
// port: ObjectType#isStructuralType
pub fn is_structural_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.get_constructor(reg)
        .is_some_and(|c| c.is_structural_interface(reg))
}
// port: ObjectType#deeplyUnwrap
pub fn deeply_unwrap(original: impl Into<Option<TypeId>>, reg: &JSTypeRegistry) -> Option<TypeId> {
    let mut current = original.into();
    while let Some(type_) = current {
        if !is_proxy(type_, reg) {
            break;
        }
        if type_.is_templatized_type(reg) {
            current = Some(crate::templatized_type::get_referenced_type(
                type_.to_maybe_templatized_type(reg).expect("templatized"),
                reg,
            ));
        } else if type_.is_named_type(reg) && !type_.is_successfully_resolved(reg) {
            break;
        } else {
            current = crate::proxy_object_type::proxy_data(type_, reg).referenced_obj_type;
        }
    }
    current
}
// port: ObjectType#getPossibleToBooleanOutcomes
pub fn get_possible_to_boolean_outcomes(_t: TypeId, _reg: &JSTypeRegistry) -> BooleanLiteralSet {
    BooleanLiteralSet::TRUE
}
// port: ObjectType#isUnknownType
pub fn is_unknown_type(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    if reg.object_data(t).unknown {
        let implicit_proto = t.get_implicit_prototype(reg, ast);
        if implicit_proto.is_none_or(|p| p.is_native_object_type(reg)) {
            reg.object_data_mut(t).unknown = false;
            for interface_type in t.get_ctor_extended_interfaces(reg, ast) {
                if interface_type.is_unknown_type(reg, ast) {
                    reg.object_data_mut(t).unknown = true;
                    break;
                }
            }
        } else {
            let unknown = implicit_proto.unwrap().is_unknown_type(reg, ast);
            reg.object_data_mut(t).unknown = unknown;
        }
    }
    reg.object_data(t).unknown
}
// port: ObjectType#isObject
pub fn is_object(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: ObjectType#visit
pub fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_object_type(reg, ast, t)
}
// port: ObjectType#visit
pub fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_object_type(reg, ast, t, that)
}
// port: ObjectType#isNativeObjectType
pub fn is_native_object_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    false
}
// port: ObjectType#cast
pub fn cast(reg: &JSTypeRegistry, type_: Option<TypeId>) -> Option<TypeId> {
    type_.and_then(|t| t.to_object_type(reg))
}
// port: ObjectType#isFunctionPrototypeType
pub fn is_function_prototype_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.get_owner_function(reg).is_some()
}

pub fn get_reference_name(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    match &reg.data(t).kind {
        JSTypeKind::InstanceObject(d) => d.constructor.get_reference_name(reg),
        JSTypeKind::NoObject(_) | JSTypeKind::No(_) => None,
        JSTypeKind::NoResolved(d) => Some(d.reference_name.clone()),
        JSTypeKind::Named(_) => crate::named_type::get_reference_name(t, reg),
        JSTypeKind::Template(_) => crate::template_type::get_reference_name(t, reg),
        JSTypeKind::Templatized(_) => crate::proxy_object_type::get_reference_name(t, reg),
        JSTypeKind::ProxyObject(_) => crate::proxy_object_type::get_reference_name(t, reg),
        JSTypeKind::Unknown(_) => crate::unknown_type::get_reference_name(t, reg),
        JSTypeKind::EnumElement(_) => crate::enum_element_type::get_reference_name(t, reg),
        _ => crate::prototype_object_type::get_reference_name(t, reg),
    }
}
pub fn get_constructor(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    match &reg.data(t).kind {
        JSTypeKind::Function(_) => Some(reg.get_native_type(JSTypeNative::FUNCTION_FUNCTION_TYPE)),
        JSTypeKind::InstanceObject(d) => Some(d.constructor),
        JSTypeKind::EnumElement(_) => crate::enum_element_type::get_constructor(t, reg),
        JSTypeKind::Named(_)
        | JSTypeKind::Template(_)
        | JSTypeKind::Templatized(_)
        | JSTypeKind::ProxyObject(_) => crate::proxy_object_type::get_constructor(t, reg),
        _ => None,
    }
}
pub fn get_implicit_prototype(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
    match &reg.data(t).kind {
        JSTypeKind::InstanceObject(d) => {
            let ctor = d.constructor;
            Some(ctor.get_prototype(reg, ast))
        }
        JSTypeKind::NoObject(_)
        | JSTypeKind::No(_)
        | JSTypeKind::NoResolved(_)
        | JSTypeKind::Unknown(_)
        | JSTypeKind::EnumElement(_) => None,
        JSTypeKind::Enum(_) | JSTypeKind::Record(_) => {
            Some(reg.get_native_type(JSTypeNative::OBJECT_TYPE))
        }
        JSTypeKind::Named(_)
        | JSTypeKind::Template(_)
        | JSTypeKind::Templatized(_)
        | JSTypeKind::ProxyObject(_) => {
            crate::proxy_object_type::get_implicit_prototype(t, reg, ast)
        }
        _ => reg.prototype_data(t).implicit_prototype_fallback,
    }
}
pub fn define_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    node: Option<NodeId>,
) -> bool {
    match &reg.data(t).kind {
        JSTypeKind::Function(_) => {
            crate::function_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
        JSTypeKind::InstanceObject(_) => {
            crate::instance_object_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
        JSTypeKind::NoObject(_)
        | JSTypeKind::No(_)
        | JSTypeKind::NoResolved(_)
        | JSTypeKind::Unknown(_)
        | JSTypeKind::EnumElement(_) => true,
        JSTypeKind::Named(_) => {
            crate::named_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
        JSTypeKind::Template(_) | JSTypeKind::Templatized(_) | JSTypeKind::ProxyObject(_) => {
            crate::proxy_object_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
        JSTypeKind::Record(_) => {
            crate::record_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
        _ => {
            crate::prototype_object_type::define_property(t, reg, ast, name, type_, inferred, node)
        }
    }
}

fn is_proxy(t: TypeId, reg: &JSTypeRegistry) -> bool {
    matches!(
        reg.data(t).kind,
        JSTypeKind::ProxyObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_)
    )
}
