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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/jstype/PrototypeObjectType.java.

use crate::{
    TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    object_type::{ObjectType, ObjectTypeData},
    property::{Property, PropertyData, PropertyKey},
    property_map::PropertyMap,
    template_type_map::TemplateTypeMap,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PrototypeObjectTypeData {
    pub(crate) object: ObjectTypeData,
    pub(crate) class_name: Option<JsString>,
    pub(crate) template_param_count: usize,
    pub(crate) properties: PropertyMap,
    pub(crate) native_type: bool,
    pub(crate) anonymous_type: bool,
    pub(crate) implicit_prototype_fallback: Option<TypeId>,
    pub(crate) owner_function: Option<TypeId>,
    pub(crate) pretty_print: bool,
}
impl PrototypeObjectTypeData {
    pub fn from_builder(reg: &JSTypeRegistry, builder: &PrototypeObjectTypeBuilder) -> Self {
        Self {
            object: ObjectTypeData::new(),
            class_name: builder.class_name.clone(),
            template_param_count: builder.template_param_count,
            properties: PropertyMap::new(),
            native_type: builder.native_type,
            anonymous_type: builder.anonymous_type,
            implicit_prototype_fallback: if builder.native_type
                || builder.implicit_prototype.is_some()
            {
                builder.implicit_prototype
            } else {
                Some(reg.get_native_type(JSTypeNative::OBJECT_TYPE))
            },
            owner_function: None,
            pretty_print: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PrototypeObjectTypeBuilder {
    pub(crate) class_name: Option<JsString>,
    pub(crate) implicit_prototype: Option<TypeId>,
    pub(crate) native_type: bool,
    pub(crate) anonymous_type: bool,
    pub(crate) template_type_map: Option<Arc<TemplateTypeMap>>,
    pub(crate) template_param_count: usize,
}
pub type Builder = PrototypeObjectTypeBuilder;
impl PrototypeObjectTypeBuilder {
    // port: PrototypeObjectType.Builder#Builder
    pub fn new() -> Self {
        Self::default()
    }
    // port: PrototypeObjectType.Builder#setName
    pub fn set_name(mut self, x: impl Into<JsString>) -> Self {
        self.class_name = Some(x.into());
        self
    }
    pub fn set_name_option(mut self, x: Option<JsString>) -> Self {
        self.class_name = x;
        self
    }
    // port: PrototypeObjectType.Builder#setImplicitPrototype
    pub fn set_implicit_prototype(mut self, x: Option<TypeId>) -> Self {
        self.implicit_prototype = x;
        self
    }
    // port: PrototypeObjectType.Builder#setNative
    pub fn set_native(mut self, x: bool) -> Self {
        self.native_type = x;
        self
    }
    // port: PrototypeObjectType.Builder#setAnonymous
    pub fn set_anonymous(mut self, x: bool) -> Self {
        self.anonymous_type = x;
        self
    }
    // port: PrototypeObjectType.Builder#setTemplateTypeMap
    pub fn set_template_type_map(mut self, x: Arc<TemplateTypeMap>) -> Self {
        self.template_type_map = Some(x);
        self
    }
    // port: PrototypeObjectType.Builder#setTemplateParamCount
    pub fn set_template_param_count(mut self, x: usize) -> Self {
        self.template_param_count = x;
        self
    }
    // port: PrototypeObjectType.Builder#castThis
    pub fn cast_this(self) -> Self {
        self
    }
    // port: PrototypeObjectType.Builder#build
    pub fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        new(reg, ast, self)
    }
}
// port: PrototypeObjectType#builder
pub fn builder() -> PrototypeObjectTypeBuilder {
    PrototypeObjectTypeBuilder::new()
}
// port: PrototypeObjectType#PrototypeObjectType
pub fn new(reg: &mut JSTypeRegistry, ast: &Ast, builder: PrototypeObjectTypeBuilder) -> TypeId {
    let data = PrototypeObjectTypeData::from_builder(reg, &builder);
    let t = reg.alloc(JSTypeKind::PrototypeObject(data), builder.template_type_map);
    initialize_after_allocation(t, reg);
    reg.finish_construction(t, ast);
    t
}

// The common constructor portion also runs for each PrototypeObjectType subclass.
// port: PrototypeObjectType#PrototypeObjectType
pub(crate) fn initialize_after_allocation(t: TypeId, reg: &mut JSTypeRegistry) {
    reg.prototype_data_mut(t).properties.set_parent_source(t);
    let implicit_prototype = reg.prototype_data(t).implicit_prototype_fallback;
    t.set_implicit_prototype(reg, implicit_prototype);
    check_state!(
        !reg.prototype_data(t).anonymous_type || reg.prototype_data(t).class_name.is_none()
    );
    check_state!(t.get_template_type_map(reg).size() >= reg.prototype_data(t).template_param_count);
}

pub trait PrototypeObjectType {
    fn set_implicit_prototype(self, reg: &mut JSTypeRegistry, implicit_prototype: Option<TypeId>);
    fn is_anonymous(self, reg: &JSTypeRegistry) -> bool;
    fn set_pretty_print(self, reg: &mut JSTypeRegistry, pretty_print: bool);
    fn is_pretty_print(self, reg: &JSTypeRegistry) -> bool;
    fn match_record_type_constraint(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        constraint_obj: TypeId,
    );
}
impl PrototypeObjectType for TypeId {
    // port: PrototypeObjectType#setImplicitPrototype
    fn set_implicit_prototype(self, reg: &mut JSTypeRegistry, implicit_prototype: Option<TypeId>) {
        check_state!(!self.has_cached_values(reg));
        reg.prototype_data_mut(self).implicit_prototype_fallback = implicit_prototype;
        if let Some(proto) = implicit_prototype {
            self.maybe_loosen_typechecking_due_to_forward_referenced_supertype(reg, proto);
        }
    }
    // port: PrototypeObjectType#isAnonymous
    fn is_anonymous(self, reg: &JSTypeRegistry) -> bool {
        reg.prototype_data(self).anonymous_type
    }
    // port: PrototypeObjectType#setPrettyPrint
    fn set_pretty_print(self, reg: &mut JSTypeRegistry, pretty_print: bool) {
        reg.prototype_data_mut(self).pretty_print = pretty_print;
    }
    // port: PrototypeObjectType#isPrettyPrint
    fn is_pretty_print(self, reg: &JSTypeRegistry) -> bool {
        reg.prototype_data(self).pretty_print
    }
    // port: PrototypeObjectType#matchRecordTypeConstraint
    fn match_record_type_constraint(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        constraint_obj: TypeId,
    ) {
        for prop in constraint_obj.get_own_property_names(reg) {
            let prop_type = constraint_obj.get_property_type(reg, ast, &prop);
            if !self.is_property_type_declared(reg, ast, &prop) {
                let mut type_to_infer = prop_type;
                if !self.has_property(reg, ast, &prop) {
                    type_to_infer = reg
                        .get_native_type(JSTypeNative::VOID_TYPE)
                        .get_least_supertype(reg, ast, prop_type);
                }
                self.define_inferred_property(reg, ast, prop, type_to_infer, None);
            }
        }
    }
}

// port: PrototypeObjectType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::PROTOTYPE_OBJECT
}
// port: PrototypeObjectType#getPropertyMap
pub fn get_property_map(t: TypeId, reg: &JSTypeRegistry) -> &PropertyMap {
    &reg.prototype_data(t).properties
}
// port: PrototypeObjectType#defineProperty
pub fn define_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    node: Option<NodeId>,
) -> bool {
    if t.has_own_declared_property(reg, ast, name.clone()) {
        return false;
    }
    let new_prop = reg.alloc_property(PropertyData::new(name.clone(), type_, inferred, node));
    let mut properties = std::mem::take(&mut reg.prototype_data_mut(t).properties);
    properties.put_property(reg, ast, name.clone(), new_prop);
    reg.prototype_data_mut(t).properties = properties;
    true
}
// port: PrototypeObjectType#setPropertyJSDocInfo
pub fn set_property_jsdoc_info(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    info: Option<Arc<JSDocInfo>>,
) {
    if let Some(info) = info {
        if t.get_own_slot(reg, ast, name.clone()).is_none() {
            let type_ = t.get_property_type(reg, ast, name.clone());
            t.define_inferred_property(reg, ast, name.clone(), type_, None);
        }
        if let Some(prop) = t.get_own_slot(reg, ast, name.clone()) {
            prop.set_jsdoc_info(reg, Some(info));
        }
    }
}
// port: PrototypeObjectType#setPropertyNode
pub fn set_property_node(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    node: Option<NodeId>,
) {
    if let Some(prop) = t.get_own_slot(reg, ast, name.clone()) {
        prop.set_node(reg, node);
    }
}
// port: PrototypeObjectType#matchesNumberContext
pub fn matches_number_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    t.is_number_object_type(reg)
        || t.is_date_type(reg)
        || t.is_boolean_object_type(reg)
        || t.is_string_object_type(reg)
        || has_overridden_native_property(t, reg, ast, "valueOf")
}
// port: PrototypeObjectType#matchesStringContext
pub fn matches_string_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    t.is_the_object_type(reg)
        || t.is_string_object_type(reg)
        || t.is_date_type(reg)
        || t.is_regexp_type(reg)
        || t.is_array_type(reg)
        || t.is_number_object_type(reg)
        || t.is_big_int_object_type(reg)
        || t.is_boolean_object_type(reg)
        || has_overridden_native_property(t, reg, ast, "toString")
}
// port: PrototypeObjectType#matchesSymbolContext
pub fn matches_symbol_context(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.is_symbol_object_type(reg)
}
// port: PrototypeObjectType#hasOverriddenNativeProperty
fn has_overridden_native_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &str,
) -> bool {
    if t.is_native_object_type(reg) {
        return false;
    }
    let property_type = t.get_property_type(reg, ast, name);
    let native_type = reg.get_native_type(if t.is_function_type(reg) {
        JSTypeNative::FUNCTION_PROTOTYPE
    } else {
        JSTypeNative::OBJECT_PROTOTYPE
    });
    let native_property_type = native_type.get_property_type(reg, ast, name);
    property_type != native_property_type
}
// port: PrototypeObjectType#matchesObjectContext
pub fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}

struct PropertyForPrettyPrinting {
    key: PropertyKey,
    type_: TypeId,
}
impl PropertyForPrettyPrinting {
    // port: PrototypeObjectType#PropertyForPrettyPrinting
    fn new(key: PropertyKey, type_: TypeId) -> Self {
        Self { key, type_ }
    }
    // port: PrototypeObjectType.PropertyForPrettyPrinting#appendTo
    fn append_to(&self, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder) {
        let name = self.key.human_readable_name(reg).to_string_lossy();
        match self.key {
            PropertyKey::String(_) => {
                sb.append(&name);
            }
            PropertyKey::Symbol(_) => {
                sb.append("[").append(&name).append("]");
            }
        }
        sb.append(": ").append_non_null(reg, ast, self.type_);
    }
}

// port: PrototypeObjectType#appendTo
pub fn append_to(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder) {
    if t.has_reference_name(reg) {
        let name = if sb.is_for_annotations() {
            t.get_normalized_reference_name(reg)
        } else {
            t.get_reference_name(reg)
        }
        .unwrap();
        sb.append(&name.to_string_lossy());
        return;
    }
    if !t.is_pretty_print(reg) {
        sb.append(if sb.is_for_annotations() {
            "?"
        } else {
            "{...}"
        });
        return;
    }
    t.set_pretty_print(reg, false);
    let mut properties: Vec<PropertyForPrettyPrinting> = Vec::new();
    let mut current = Some(t);
    while let Some(c) = current {
        if c.is_native_object_type(reg) || properties.len() > 10 {
            break;
        }
        for key in c.get_own_property_keys(reg) {
            let type_ = c.get_property_type(reg, ast, key.clone());
            let prop = PropertyForPrettyPrinting::new(key, type_);
            if !properties
                .iter()
                .any(|old| compare_properties(old, &prop, reg, ast).is_eq())
            {
                properties.push(prop);
            }
        }
        current = c.get_implicit_prototype(reg, ast);
    }
    properties.sort_by(|a, b| compare_properties(a, b, reg, ast));
    let multiline = !sb.is_for_annotations() && properties.len() > 1;
    sb.append("{").indent(|sb| {
        if multiline {
            sb.break_line_and_indent();
        }
        for (index, property) in properties.iter().enumerate() {
            let i = index + 1;
            if !sb.is_for_annotations() && i > 10 {
                sb.append("...");
                break;
            }
            property.append_to(reg, ast, sb);
            if i < properties.len() {
                sb.append(",");
                if multiline {
                    sb.break_line_and_indent();
                } else {
                    sb.append(" ");
                }
            }
        }
    });
    if multiline {
        sb.break_line_and_indent();
    }
    sb.append("}");
    t.set_pretty_print(reg, true);
}
fn compare_properties(
    a: &PropertyForPrettyPrinting,
    b: &PropertyForPrettyPrinting,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> std::cmp::Ordering {
    let names = a
        .key
        .human_readable_name(reg)
        .cmp(&b.key.human_readable_name(reg));
    if !names.is_eq() {
        return names;
    }
    match (&a.key, &b.key) {
        (PropertyKey::String(a), PropertyKey::String(b)) => a.cmp(b),
        (PropertyKey::String(_), PropertyKey::Symbol(_)) => std::cmp::Ordering::Less,
        (PropertyKey::Symbol(_), PropertyKey::String(_)) => std::cmp::Ordering::Greater,
        _ => {
            let a = JsString::from(a.type_.to_string(reg, ast));
            let b = JsString::from(b.type_.to_string(reg, ast));
            a.cmp(&b)
        }
    }
}
// port: PrototypeObjectType#getConstructor
pub fn get_constructor(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: PrototypeObjectType#getImplicitPrototype
pub fn get_implicit_prototype(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    reg.prototype_data(t).implicit_prototype_fallback
}
// port: PrototypeObjectType#getTemplateParamCount
pub fn get_template_param_count(t: TypeId, reg: &JSTypeRegistry) -> usize {
    reg.prototype_data(t).template_param_count
}
// port: PrototypeObjectType#getReferenceName
pub fn get_reference_name(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    let d = reg.prototype_data(t);
    if let Some(class_name) = &d.class_name {
        Some(class_name.clone())
    } else {
        d.owner_function.map(|owner| {
            owner
                .get_reference_name(reg)
                .unwrap_or_else(|| "null".into())
                .concat(&".prototype".into())
        })
    }
}
// port: PrototypeObjectType#isNativeObjectType
pub fn is_native_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    reg.prototype_data(t).native_type
}
// port: PrototypeObjectType#setOwnerFunction
pub fn set_owner_function(t: TypeId, reg: &mut JSTypeRegistry, type_: Option<TypeId>) {
    check_state!(reg.prototype_data(t).owner_function.is_none() || type_.is_none());
    reg.prototype_data_mut(t).owner_function = type_;
}
// port: PrototypeObjectType#getOwnerFunction
pub fn get_owner_function(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    reg.prototype_data(t).owner_function
}
// port: PrototypeObjectType#getCtorImplementedInterfaces
pub fn get_ctor_implemented_interfaces(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    t.get_ctor_implemented_interfaces(reg, ast)
}
// port: PrototypeObjectType#getCtorExtendedInterfaces
pub fn get_ctor_extended_interfaces(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
    t.get_ctor_extended_interfaces(reg, ast)
}
// port: PrototypeObjectType#resolveInternal
pub fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    if let Some(implicit_prototype) = t.get_implicit_prototype(reg, ast) {
        let resolved = implicit_prototype.resolve_with_reporter(reg, ast, reporter);
        reg.prototype_data_mut(t).implicit_prototype_fallback = resolved.to_object_type(reg);
    }
    for prop in reg.prototype_data(t).properties.values() {
        let type_ = prop.get_type(reg).resolve_with_reporter(reg, ast, reporter);
        prop.set_type(reg, type_);
    }
    t
}
// port: PrototypeObjectType#matchConstraint
pub fn match_constraint(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast, constraint: TypeId) {
    if t.has_reference_name(reg) {
        return;
    }
    if constraint.is_record_type(reg) {
        t.match_record_type_constraint(reg, ast, constraint);
    } else if constraint.is_union_type(reg) {
        for alt in crate::union_type::get_alternates(constraint, reg, ast)
            .iter()
            .copied()
        {
            if alt.is_record_type(reg) {
                t.match_record_type_constraint(reg, ast, alt);
            }
        }
    }
}
// port: PrototypeObjectType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    if t.is_structural_type(reg) {
        let d = reg.prototype_data(t);
        31_i32
            .wrapping_add(d.class_name.as_ref().map_or(0, JsString::hash_code))
            .wrapping_mul(31)
            .wrapping_add(d.properties.hash_code(reg))
    } else {
        t.0 as i32
    }
}
