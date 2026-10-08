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
//   src/com/google/javascript/rhino/jstype/JSType.java,
//   test/com/google/javascript/rhino/jstype/UnitTestingJSType.java.

use crate::function_type::FunctionType;
use crate::object_type::ObjectType;
use crate::{
    JSTypeNative, JSTypeRegistry, TypeId, boolean_literal_set::BooleanLiteralSet,
    js_type_class::JSTypeClass, property::PropertyKey, template_type_map::TemplateTypeMap,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    js_string::JsString,
    jscomp_base::Tri,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    outcome::Outcome,
};
use std::{cell::Cell, sync::Arc};

pub(crate) struct JSTypeData {
    pub(crate) resolve_result: Option<TypeId>,
    pub(crate) template_type_map: Arc<TemplateTypeMap>,
    pub(crate) loosen_typechecking_due_to_forward_referenced_supertype: bool,
    pub(crate) hash_code_in_progress: Cell<bool>,
    pub(crate) in_templated_check_visit: Cell<bool>,
    pub(crate) template_check_result: Cell<Option<bool>>,
    pub(crate) kind: JSTypeKind,
}
impl JSTypeData {
    // port: JSType#JSType
    pub fn new(kind: JSTypeKind, template_type_map: Arc<TemplateTypeMap>) -> Self {
        Self {
            resolve_result: None,
            template_type_map,
            loosen_typechecking_due_to_forward_referenced_supertype: false,
            hash_code_in_progress: Cell::new(false),
            in_templated_check_visit: Cell::new(false),
            template_check_result: Cell::new(None),
            kind,
        }
    }
}
pub(crate) enum JSTypeKind {
    All(crate::all_type::AllTypeData),
    Arrow(crate::arrow_type::ArrowTypeData),
    Boolean(crate::boolean_type::BooleanTypeData),
    BigInt(crate::big_int_type::BigIntTypeData),
    Enum(crate::enum_type::EnumTypeData),
    EnumElement(crate::enum_element_type::EnumElementTypeData),
    Function(crate::function_type::FunctionTypeData),
    InstanceObject(crate::instance_object_type::InstanceObjectTypeData),
    Named(crate::named_type::NamedTypeData),
    No(crate::no_type::NoTypeData),
    NoObject(crate::no_object_type::NoObjectTypeData),
    NoResolved(crate::no_resolved_type::NoResolvedTypeData),
    Null(crate::null_type::NullTypeData),
    Number(crate::number_type::NumberTypeData),
    PrototypeObject(crate::prototype_object_type::PrototypeObjectTypeData),
    ProxyObject(crate::proxy_object_type::ProxyObjectTypeData),
    Record(crate::record_type::RecordTypeData),
    String(crate::string_type::StringTypeData),
    Symbol(crate::symbol_type::SymbolTypeData),
    KnownSymbol(crate::known_symbol_type::KnownSymbolTypeData),
    Template(crate::template_type::TemplateTypeData),
    Templatized(crate::templatized_type::TemplatizedTypeData),
    Union(crate::union_type::UnionTypeData),
    Unknown(crate::unknown_type::UnknownTypeData),
    Void(crate::void_type::VoidTypeData),
    UnitTesting(UnitTestingJSTypeData),
}
pub type TypeValidator = Arc<dyn Fn(TypeId, &mut JSTypeRegistry, &Ast) -> bool + Send + Sync>;
pub type ResolveCallback = Arc<dyn Fn(TypeId, &mut JSTypeRegistry, &Ast) -> TypeId + Send + Sync>;
#[doc(hidden)]
#[derive(Default)]
pub struct UnitTestingJSTypeData {
    pub type_class: Option<JSTypeClass>,
    pub resolve: Option<ResolveCallback>,
    pub hash: Option<i32>,
    pub is_no_resolved_type: Option<bool>,
}
#[doc(hidden)]
pub struct UnitTestingJSType;
impl UnitTestingJSType {
    // port: UnitTestingJSType#UnitTestingJSType
    pub fn new(reg: &mut JSTypeRegistry) -> TypeId {
        reg.alloc(
            JSTypeKind::UnitTesting(UnitTestingJSTypeData::default()),
            None,
        )
    }
    pub fn with_data(reg: &mut JSTypeRegistry, data: UnitTestingJSTypeData) -> TypeId {
        reg.alloc(JSTypeKind::UnitTesting(data), None)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HasPropertyKind {
    ABSENT,
    KNOWN_PRESENT,
    MAYBE_PRESENT,
}
impl HasPropertyKind {
    // port: JSType.HasPropertyKind#of
    pub fn of(has: bool) -> Self {
        if has {
            Self::KNOWN_PRESENT
        } else {
            Self::ABSENT
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nullability {
    EXPLICIT,
    IMPLICIT,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtypingMode {
    NORMAL,
    IGNORE_NULL_UNDEFINED,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchStatus {
    MATCH,
    NOT_MATCH,
    PROCESSING,
}
impl MatchStatus {
    // port: JSType.MatchStatus#MatchStatus
    pub fn new(is_subtype: bool) -> Self {
        Self::value_of(is_subtype)
    }
    // port: JSType.MatchStatus#subtypeValue
    pub fn subtype_value(self) -> bool {
        self != Self::NOT_MATCH
    }
    // port: JSType.MatchStatus#valueOf
    pub fn value_of(matches: bool) -> Self {
        if matches {
            Self::MATCH
        } else {
            Self::NOT_MATCH
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypePair {
    pub type_a: Option<TypeId>,
    pub type_b: Option<TypeId>,
}
impl TypePair {
    // port: JSType.TypePair#TypePair
    pub fn new(type_a: Option<TypeId>, type_b: Option<TypeId>) -> Self {
        Self { type_a, type_b }
    }
}

pub(crate) fn object_data(t: TypeId, reg: &JSTypeRegistry) -> &crate::object_type::ObjectTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Enum(d) => &d.prototype.object,
        JSTypeKind::EnumElement(d) => &d.object,
        JSTypeKind::Function(d) => &d.prototype.object,
        JSTypeKind::InstanceObject(d) => &d.prototype.object,
        JSTypeKind::Named(d) => &d.proxy.object,
        JSTypeKind::No(d) => &d.no_object.function.prototype.object,
        JSTypeKind::NoObject(d) => &d.function.prototype.object,
        JSTypeKind::NoResolved(d) => &d.object,
        JSTypeKind::PrototypeObject(d) => &d.object,
        JSTypeKind::ProxyObject(d) => &d.object,
        JSTypeKind::Record(d) => &d.prototype.object,
        JSTypeKind::Template(d) => &d.proxy.object,
        JSTypeKind::Templatized(d) => &d.proxy.object,
        JSTypeKind::Unknown(d) => &d.object,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn object_data_mut(
    t: TypeId,
    reg: &mut JSTypeRegistry,
) -> &mut crate::object_type::ObjectTypeData {
    match &mut reg.data_mut(t).kind {
        JSTypeKind::Enum(d) => &mut d.prototype.object,
        JSTypeKind::EnumElement(d) => &mut d.object,
        JSTypeKind::Function(d) => &mut d.prototype.object,
        JSTypeKind::InstanceObject(d) => &mut d.prototype.object,
        JSTypeKind::Named(d) => &mut d.proxy.object,
        JSTypeKind::No(d) => &mut d.no_object.function.prototype.object,
        JSTypeKind::NoObject(d) => &mut d.function.prototype.object,
        JSTypeKind::NoResolved(d) => &mut d.object,
        JSTypeKind::PrototypeObject(d) => &mut d.object,
        JSTypeKind::ProxyObject(d) => &mut d.object,
        JSTypeKind::Record(d) => &mut d.prototype.object,
        JSTypeKind::Template(d) => &mut d.proxy.object,
        JSTypeKind::Templatized(d) => &mut d.proxy.object,
        JSTypeKind::Unknown(d) => &mut d.object,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn prototype_data(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> &crate::prototype_object_type::PrototypeObjectTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Enum(d) => &d.prototype,
        JSTypeKind::Function(d) => &d.prototype,
        JSTypeKind::InstanceObject(d) => &d.prototype,
        JSTypeKind::No(d) => &d.no_object.function.prototype,
        JSTypeKind::NoObject(d) => &d.function.prototype,
        JSTypeKind::PrototypeObject(d) => d,
        JSTypeKind::Record(d) => &d.prototype,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn prototype_data_mut(
    t: TypeId,
    reg: &mut JSTypeRegistry,
) -> &mut crate::prototype_object_type::PrototypeObjectTypeData {
    match &mut reg.data_mut(t).kind {
        JSTypeKind::Enum(d) => &mut d.prototype,
        JSTypeKind::Function(d) => &mut d.prototype,
        JSTypeKind::InstanceObject(d) => &mut d.prototype,
        JSTypeKind::No(d) => &mut d.no_object.function.prototype,
        JSTypeKind::NoObject(d) => &mut d.function.prototype,
        JSTypeKind::PrototypeObject(d) => d,
        JSTypeKind::Record(d) => &mut d.prototype,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn function_data(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> &crate::function_type::FunctionTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Function(d) => d,
        JSTypeKind::NoObject(d) => &d.function,
        JSTypeKind::No(d) => &d.no_object.function,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn function_data_mut(
    t: TypeId,
    reg: &mut JSTypeRegistry,
) -> &mut crate::function_type::FunctionTypeData {
    match &mut reg.data_mut(t).kind {
        JSTypeKind::Function(d) => d,
        JSTypeKind::NoObject(d) => &mut d.function,
        JSTypeKind::No(d) => &mut d.no_object.function,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn prototype_data_opt(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> Option<&crate::prototype_object_type::PrototypeObjectTypeData> {
    match &reg.data(t).kind {
        JSTypeKind::Enum(d) => Some(&d.prototype),
        JSTypeKind::Function(d) => Some(&d.prototype),
        JSTypeKind::InstanceObject(d) => Some(&d.prototype),
        JSTypeKind::No(d) => Some(&d.no_object.function.prototype),
        JSTypeKind::NoObject(d) => Some(&d.function.prototype),
        JSTypeKind::PrototypeObject(d) => Some(d),
        JSTypeKind::Record(d) => Some(&d.prototype),
        _ => None,
    }
}
// port: JSType#testForEqualityHelper
pub(crate) fn test_for_equality_helper(
    a_type: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    b_type: TypeId,
) -> Option<Tri> {
    if b_type.is_all_type(reg)
        || b_type.is_unknown_type(reg, ast)
        || b_type.is_no_resolved_type(reg)
        || a_type.is_all_type(reg)
        || a_type.is_unknown_type(reg, ast)
        || a_type.is_no_resolved_type(reg)
    {
        return Some(Tri::UNKNOWN);
    }
    let a_is_empty = a_type.is_empty_type(reg);
    let b_is_empty = b_type.is_empty_type(reg);
    if a_is_empty || b_is_empty {
        return Some(if a_is_empty && b_is_empty {
            Tri::TRUE
        } else {
            Tri::UNKNOWN
        });
    }
    if b_type.is_union_type(reg) {
        return b_type.test_for_equality(reg, ast, a_type);
    }
    if a_type.is_function_type(reg) || b_type.is_function_type(reg) {
        let other_type = if a_type.is_function_type(reg) {
            b_type
        } else {
            a_type
        };
        if other_type.is_symbol(reg, ast) {
            return Some(Tri::FALSE);
        }
        let object = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        let greatest_subtype = other_type.get_greatest_subtype(reg, ast, object);
        return Some(
            if greatest_subtype.is_no_type(reg) || greatest_subtype.is_no_object_type(reg) {
                Tri::FALSE
            } else {
                Tri::UNKNOWN
            },
        );
    }
    if b_type.is_enum_element_type(reg) {
        return b_type.test_for_equality(reg, ast, a_type);
    }
    let symbol = reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
    let symbol_object = reg.get_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE);
    if a_type.is_symbol(reg, ast) {
        return Some(
            if b_type.can_cast_to(reg, ast, symbol) || b_type.can_cast_to(reg, ast, symbol_object) {
                Tri::UNKNOWN
            } else {
                Tri::FALSE
            },
        );
    }
    if b_type.is_symbol(reg, ast) {
        return Some(
            if a_type.can_cast_to(reg, ast, symbol) || a_type.can_cast_to(reg, ast, symbol_object) {
                Tri::UNKNOWN
            } else {
                Tri::FALSE
            },
        );
    }
    None
}

// port: JSType#areSimilar
pub fn are_similar(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    first: Option<TypeId>,
    second: Option<TypeId>,
) -> bool {
    match (first, second) {
        (Some(first), Some(second)) => !first.differs_from(reg, ast, second),
        _ => first == second,
    }
}
// port: JSType#toMaybeFunctionType
pub fn to_maybe_function_type(reg: &JSTypeRegistry, t: Option<TypeId>) -> Option<TypeId> {
    t.and_then(|t| t.to_maybe_function_type(reg))
}
// port: JSType#safeResolve
pub fn safe_resolve(
    t: Option<TypeId>,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> Option<TypeId> {
    t.map(|t| t.resolve_with_reporter(reg, ast, reporter))
}
// port: JSType#getLeastSupertype
pub(crate) fn get_least_supertype(
    this_type: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that_type: TypeId,
) -> TypeId {
    if this_type.equals(reg, ast, that_type) {
        this_type
    } else {
        reg.create_union_type(ast, &[this_type, that_type])
    }
}
// port: JSType#getGreatestSubtype
#[allow(clippy::collapsible_if)] // Preserve the Java nested subtype branches.
pub(crate) fn get_greatest_subtype(
    this_type: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that_type: TypeId,
) -> TypeId {
    if let (Some(this_fn), Some(that_fn)) = (
        this_type.to_maybe_function_type(reg),
        that_type.to_maybe_function_type(reg),
    ) {
        return this_fn.sup_and_inf_helper(reg, ast, that_fn, false);
    }
    if this_type.equals(reg, ast, that_type) {
        return this_type;
    }
    if this_type.is_unknown_type(reg, ast) {
        if !that_type.is_unknown_type(reg, ast) {
            return that_type;
        }
        return reg.get_native_type(
            if this_type.is_checked_unknown_type(reg) || that_type.is_checked_unknown_type(reg) {
                JSTypeNative::CHECKED_UNKNOWN_TYPE
            } else {
                JSTypeNative::UNKNOWN_TYPE
            },
        );
    }
    if that_type.is_unknown_type(reg, ast) {
        return this_type;
    }
    if let Some(union) = this_type.to_maybe_union_type(reg) {
        return crate::union_type::get_greatest_subtype(union, reg, ast, that_type);
    }
    if let Some(union) = that_type.to_maybe_union_type(reg) {
        return crate::union_type::get_greatest_subtype(union, reg, ast, this_type);
    }
    if let Some(templ) = this_type.to_maybe_templatized_type(reg) {
        return crate::templatized_type::get_greatest_subtype_helper(templ, reg, ast, that_type);
    }
    if let Some(templ) = that_type.to_maybe_templatized_type(reg) {
        return crate::templatized_type::get_greatest_subtype_helper(templ, reg, ast, this_type);
    }
    if this_type.is_subtype_of(reg, ast, that_type) {
        return this_type;
    }
    if that_type.is_subtype_of(reg, ast, this_type) {
        return that_type;
    }
    if let Some(record) = this_type.to_maybe_record_type(reg) {
        return crate::record_type::get_greatest_subtype_helper(record, reg, ast, that_type);
    }
    if let Some(record) = that_type.to_maybe_record_type(reg) {
        return crate::record_type::get_greatest_subtype_helper(record, reg, ast, this_type);
    }
    if let Some(element) = this_type.to_maybe_enum_element_type(reg) {
        if let Some(inf) =
            crate::enum_element_type::get_greatest_subtype(element, reg, ast, that_type)
        {
            return inf;
        }
    } else if let Some(element) = that_type.to_maybe_enum_element_type(reg) {
        if let Some(inf) =
            crate::enum_element_type::get_greatest_subtype(element, reg, ast, this_type)
        {
            return inf;
        }
    }
    let both_objects = this_type.is_object(reg, ast) && that_type.is_object(reg, ast);
    reg.get_native_type(if both_objects {
        JSTypeNative::NO_OBJECT_TYPE
    } else {
        JSTypeNative::NO_TYPE
    })
}

pub trait WithSourceRef {
    // port: JSType.WithSourceRef#getSource
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId>;
    // port: JSType.WithSourceRef#getGoogModuleId
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString>;
}

impl WithSourceRef for TypeId {
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId> {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_) => {
                crate::function_type::FunctionType::get_source(self, reg)
            }
            JSTypeKind::Enum(_) => crate::enum_type::EnumType::get_source(self, reg),
            _ => panic!("ClassCastException"),
        }
    }
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString> {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::NoObject(_) | JSTypeKind::No(_) => {
                crate::function_type::FunctionType::get_goog_module_id(self, reg)
            }
            JSTypeKind::Enum(_) => crate::enum_type::EnumType::get_goog_module_id(self, reg),
            _ => panic!("ClassCastException"),
        }
    }
}

// GENERATED DISPATCH
#[allow(clippy::wrong_self_convention)]
pub trait JSType {
    fn get_type_class(self, reg: &JSTypeRegistry) -> JSTypeClass;
    fn get_native_type(self, reg: &JSTypeRegistry, type_id: JSTypeNative) -> TypeId;
    fn get_jsdoc_info(self, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>>;
    fn get_display_name(self, reg: &JSTypeRegistry) -> Option<String>;
    fn has_display_name(self, reg: &JSTypeRegistry) -> bool;
    fn get_property_kind(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
    ) -> HasPropertyKind;
    fn get_property_kind_with_autobox(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
        autobox: bool,
    ) -> HasPropertyKind;
    fn has_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
    ) -> bool;
    fn is_no_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_no_resolved_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_no_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_number_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_number_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_big_int_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_big_int_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_function_prototype_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_string_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_symbol_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_the_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_string_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_symbol_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_known_symbol_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_array_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_readonly_array_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_boolean_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_boolean_value_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_regexp_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_date_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_null_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_void_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_all_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_checked_unknown_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_structural_interface(self, reg: &JSTypeRegistry) -> bool;
    fn is_structural_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_constructor(self, reg: &JSTypeRegistry) -> bool;
    fn is_nominal_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_native_object_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_instance_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_interface(self, reg: &JSTypeRegistry) -> bool;
    fn is_ordinary_function(self, reg: &JSTypeRegistry) -> bool;
    fn is_empty_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_number(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_symbol(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_big_int_or_number(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_only_big_int(self, reg: &JSTypeRegistry) -> bool;
    fn is_unknown_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_some_unknown_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_union_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_function_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_enum_element_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_enum_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_named_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_record_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_templatized_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_template_type(self, reg: &JSTypeRegistry) -> bool;
    fn to_maybe_union_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_function_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_known_symbol_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_enum_element_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_enum_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_named_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_record_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_templatized_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn to_maybe_template_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn assert_function_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn assert_object_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn is_raw_type_of_templatized_type(self, reg: &JSTypeRegistry) -> bool;
    fn is_struct(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_dict(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn contains_reference_ancestor(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        target: TypeId,
    ) -> bool;
    fn is_literal_object(self, reg: &JSTypeRegistry) -> bool;
    fn is_global_this_type(self, reg: &JSTypeRegistry) -> bool;
    fn get_enumerated_type_of_enum_element(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn has_any_template_types(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn has_any_template_types_internal(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn get_template_type_map(self, reg: &JSTypeRegistry) -> Arc<TemplateTypeMap>;
    fn get_type_parameters(self, reg: &JSTypeRegistry) -> Vec<TypeId>;
    fn get_template_param_count(self, reg: &JSTypeRegistry) -> usize;
    fn merge_supertype_template_types(self, reg: &mut JSTypeRegistry, ast: &Ast, other: TypeId);
    fn is_object(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_object_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_nominal_constructor_or_interface(self, reg: &JSTypeRegistry) -> bool;
    fn loosen_typechecking_due_to_forward_referenced_supertype(self, reg: &JSTypeRegistry) -> bool;
    fn maybe_loosen_typechecking_due_to_forward_referenced_supertype(
        self,
        reg: &mut JSTypeRegistry,
        supertype: TypeId,
    );
    fn equals(self, reg: &mut JSTypeRegistry, ast: &Ast, other: impl Into<Option<TypeId>>) -> bool;
    fn differs_from(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool;
    fn hash_code(self, reg: &JSTypeRegistry) -> i32;
    fn recursion_unsafe_hash_code(self, reg: &JSTypeRegistry) -> i32;
    fn matches_number_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn matches_string_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn matches_symbol_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn matches_object_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn can_be_called(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn find_property_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<TypeId>;
    fn find_property_type_without_considering_template_types(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: PropertyKey,
    ) -> Option<TypeId>;
    fn can_cast_to(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool;
    fn autoboxes_to(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn is_boxable_scalar(self, reg: &JSTypeRegistry) -> bool;
    fn to_object_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn autobox(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn dereference(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId>;
    fn can_test_for_equality_with(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool;
    fn test_for_equality(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> Option<Tri>;
    fn can_test_for_shallow_equality_with(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> bool;
    fn is_nullable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_voidable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn is_explicitly_voidable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn collapse_union(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId>;
    fn get_least_supertype(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> TypeId;
    fn get_greatest_subtype(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> TypeId;
    fn get_restricted_type_given_outcome(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        outcome: Outcome,
    ) -> TypeId;
    fn get_possible_to_boolean_outcomes(self, reg: &JSTypeRegistry) -> BooleanLiteralSet;
    fn get_types_under_equality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair;
    fn get_types_under_inequality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair;
    fn get_types_under_shallow_equality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair;
    fn get_types_under_shallow_inequality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair;
    fn is_null_type_or_void_type(self, reg: &JSTypeRegistry) -> bool;
    fn get_union_members(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<Arc<Vec<TypeId>>>;
    fn restrict_by_not_null_or_undefined(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn restrict_by_not_undefined(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn restrict_by_not_null(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn is_subtype_without_structural_typing(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
    ) -> bool;
    fn is_subtype(self, reg: &mut JSTypeRegistry, ast: &Ast, supertype: TypeId) -> bool;
    fn is_subtype_of(self, reg: &mut JSTypeRegistry, ast: &Ast, supertype: TypeId) -> bool;
    fn is_subtype_with_mode(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
        mode: SubtypingMode,
    ) -> bool;
    fn is_subtype_of_with_mode(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
        mode: SubtypingMode,
    ) -> bool;
    fn visit<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl crate::visitor::Visitor<T>,
    ) -> T;
    fn visit_relationship<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
        that: TypeId,
    ) -> T;
    fn resolve(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn resolve_with_reporter(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> TypeId;
    fn resolve_internal(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> TypeId;
    fn eagerly_resolve_to_self(self, reg: &mut JSTypeRegistry, ast: &Ast);
    fn is_resolved(self, reg: &JSTypeRegistry) -> bool;
    fn is_successfully_resolved(self, reg: &JSTypeRegistry) -> bool;
    fn is_unsuccessfully_resolved(self, reg: &JSTypeRegistry) -> bool;
    fn set_validator(self, reg: &mut JSTypeRegistry, ast: &Ast, validator: TypeValidator) -> bool;
    fn to_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> String;
    fn to_annotation_string(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        nullability: Nullability,
    ) -> String;
    fn append_to(self, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder);
    fn match_constraint(self, reg: &mut JSTypeRegistry, ast: &Ast, constraint: TypeId);
    fn to_maybe_object_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
}
impl JSType for TypeId {
    // port: JSType#getTypeClass
    fn get_type_class(self, reg: &JSTypeRegistry) -> JSTypeClass {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::get_type_class(self, reg),
            JSTypeKind::Arrow(_) => crate::arrow_type::get_type_class(self, reg),
            JSTypeKind::Boolean(_) => crate::boolean_type::get_type_class(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::get_type_class(self, reg),
            JSTypeKind::Enum(_) => crate::enum_type::get_type_class(self, reg),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::get_type_class(self, reg),
            JSTypeKind::Function(_) => crate::function_type::get_type_class(self, reg),
            JSTypeKind::InstanceObject(_) => crate::instance_object_type::get_type_class(self, reg),
            JSTypeKind::Named(_) => crate::named_type::get_type_class(self, reg),
            JSTypeKind::No(_) => crate::no_type::get_type_class(self, reg),
            JSTypeKind::NoObject(_) => crate::no_object_type::get_type_class(self, reg),
            JSTypeKind::NoResolved(_) => crate::no_resolved_type::get_type_class(self, reg),
            JSTypeKind::Null(_) => crate::null_type::get_type_class(self, reg),
            JSTypeKind::Number(_) => crate::number_type::get_type_class(self, reg),
            JSTypeKind::PrototypeObject(_) => {
                crate::prototype_object_type::get_type_class(self, reg)
            }
            JSTypeKind::ProxyObject(_) => crate::proxy_object_type::get_type_class(self, reg),
            JSTypeKind::Record(_) => crate::record_type::get_type_class(self, reg),
            JSTypeKind::String(_) => crate::string_type::get_type_class(self, reg),
            JSTypeKind::Symbol(_) => crate::symbol_type::get_type_class(self, reg),
            JSTypeKind::KnownSymbol(_) => crate::known_symbol_type::get_type_class(self, reg),
            JSTypeKind::Template(_) => crate::template_type::get_type_class(self, reg),
            JSTypeKind::Templatized(_) => crate::templatized_type::get_type_class(self, reg),
            JSTypeKind::Union(_) => crate::union_type::get_type_class(self, reg),
            JSTypeKind::Unknown(_) => crate::unknown_type::get_type_class(self, reg),
            JSTypeKind::Void(_) => crate::void_type::get_type_class(self, reg),
            JSTypeKind::UnitTesting(d) => d.type_class.expect("UnsupportedOperationException"),
        }
    }
    // port: JSType#getNativeType
    fn get_native_type(self, reg: &JSTypeRegistry, type_id: JSTypeNative) -> TypeId {
        reg.get_native_type(type_id)
    }
    // port: JSType#getJSDocInfo
    fn get_jsdoc_info(self, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Unknown(_) => crate::object_type::get_jsdoc_info(self, reg),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::get_jsdoc_info(self, reg),
            _ => None,
        }
    }
    // port: JSType#getDisplayName
    fn get_display_name(self, reg: &JSTypeRegistry) -> Option<String> {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::get_display_name(self, reg),
            JSTypeKind::Boolean(_) => crate::boolean_type::get_display_name(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::get_display_name(self, reg),
            JSTypeKind::Enum(_) => crate::enum_type::get_display_name(self, reg),
            JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::object_type::get_display_name(self, reg),
            JSTypeKind::Null(_) => crate::null_type::get_display_name(self, reg),
            JSTypeKind::Number(_) => crate::number_type::get_display_name(self, reg),
            JSTypeKind::String(_) => crate::string_type::get_display_name(self, reg),
            JSTypeKind::Symbol(_) => crate::symbol_type::get_display_name(self, reg),
            JSTypeKind::KnownSymbol(_) => crate::known_symbol_type::get_display_name(self, reg),
            JSTypeKind::Unknown(_) => crate::unknown_type::get_display_name(self, reg),
            JSTypeKind::Void(_) => crate::void_type::get_display_name(self, reg),
            _ => None,
        }
    }
    // port: JSType#hasDisplayName
    fn has_display_name(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::has_display_name(self, reg),
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => crate::value_type::has_display_name(self, reg),
            JSTypeKind::Unknown(_) => crate::unknown_type::has_display_name(self, reg),
            _ => self.get_display_name(reg).is_some_and(|n| !n.is_empty()),
        }
    }
    // port: JSType#getPropertyKind
    fn get_property_kind(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
    ) -> HasPropertyKind {
        self.get_property_kind_with_autobox(reg, ast, pname, true)
    }
    // port: JSType#getPropertyKind
    fn get_property_kind_with_autobox(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
        autobox: bool,
    ) -> HasPropertyKind {
        let pname = pname.into();
        match &reg.data(self).kind {
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => {
                crate::value_type::get_property_kind(self, reg, ast, pname, autobox)
            }
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Unknown(_) => {
                crate::object_type::get_property_kind(self, reg, ast, &pname, autobox)
            }
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::get_property_kind(self, reg, ast, pname, autobox)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::get_property_kind(self, reg, ast, &pname, autobox)
            }
            JSTypeKind::Union(_) => {
                crate::union_type::get_property_kind(self, reg, ast, pname, autobox)
            }
            _ => HasPropertyKind::ABSENT,
        }
    }
    // port: JSType#hasProperty
    fn has_property(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        pname: impl Into<PropertyKey>,
    ) -> bool {
        self.get_property_kind_with_autobox(reg, ast, pname, false) != HasPropertyKind::ABSENT
    }
    // port: JSType#isNoType
    fn is_no_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_no_type(self, reg),
            JSTypeKind::No(_) => crate::no_type::is_no_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isNoResolvedType
    fn is_no_resolved_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_no_resolved_type(self, reg)
            }
            JSTypeKind::NoResolved(_) => crate::no_resolved_type::is_no_resolved_type(self, reg),
            JSTypeKind::UnitTesting(d) => d.is_no_resolved_type.unwrap_or(false),
            _ => false,
        }
    }
    // port: JSType#isNoObjectType
    fn is_no_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_no_object_type(self, reg),
            JSTypeKind::No(_) => crate::no_type::is_no_object_type(self, reg),
            JSTypeKind::NoObject(_) => crate::no_object_type::is_no_object_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isNumberObjectType
    fn is_number_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_number_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isNumberValueType
    fn is_number_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Number(_) => crate::number_type::is_number_value_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isBigIntObjectType
    fn is_big_int_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_big_int_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isBigIntValueType
    fn is_big_int_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::BigInt(_) => crate::big_int_type::is_big_int_value_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isFunctionPrototypeType
    fn is_function_prototype_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_)
            | JSTypeKind::Unknown(_) => crate::object_type::is_function_prototype_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isStringObjectType
    fn is_string_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_string_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isSymbolObjectType
    fn is_symbol_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_symbol_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isTheObjectType
    fn is_the_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_the_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isStringValueType
    fn is_string_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::String(_) => crate::string_type::is_string_value_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isSymbolValueType
    fn is_symbol_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::is_symbol_value_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isKnownSymbolValueType
    fn is_known_symbol_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::KnownSymbol(_) => {
                crate::known_symbol_type::is_known_symbol_value_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isArrayType
    fn is_array_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => crate::instance_object_type::is_array_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isReadonlyArrayType
    fn is_readonly_array_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_readonly_array_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isBooleanObjectType
    fn is_boolean_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_boolean_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isBooleanValueType
    fn is_boolean_value_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Boolean(_) => crate::boolean_type::is_boolean_value_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isRegexpType
    fn is_regexp_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => crate::instance_object_type::is_regexp_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isDateType
    fn is_date_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::InstanceObject(_) => crate::instance_object_type::is_date_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isNullType
    fn is_null_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Null(_) => crate::null_type::is_null_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isVoidType
    fn is_void_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Void(_) => crate::void_type::is_void_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isAllType
    fn is_all_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::is_all_type(self, reg),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_all_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isCheckedUnknownType
    fn is_checked_unknown_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_checked_unknown_type(self, reg)
            }
            JSTypeKind::Unknown(_) => crate::unknown_type::is_checked_unknown_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isStructuralInterface
    fn is_structural_interface(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::is_structural_interface(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isStructuralType
    fn is_structural_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Unknown(_) => crate::object_type::is_structural_type(self, reg),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_structural_type(self, reg),
            JSTypeKind::Record(_) => crate::record_type::is_structural_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isConstructor
    fn is_constructor(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::is_constructor(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_constructor(self, reg),
            _ => false,
        }
    }
    // port: JSType#isNominalType
    fn is_nominal_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::EnumElement(_) => crate::enum_element_type::is_nominal_type(self, reg),
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_nominal_type(self, reg)
            }
            JSTypeKind::Named(_) => crate::named_type::is_nominal_type(self, reg),
            JSTypeKind::ProxyObject(_) | JSTypeKind::Template(_) | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_nominal_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isNativeObjectType
    fn is_native_object_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::is_native_object_type(self, reg)
            }
            JSTypeKind::EnumElement(_) | JSTypeKind::NoResolved(_) | JSTypeKind::Unknown(_) => {
                crate::object_type::is_native_object_type(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_native_object_type(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isInstanceType
    fn is_instance_type(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::is_instance_type(self, reg)
            }
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::is_instance_type(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_instance_type(self, reg),
            _ => false,
        }
    }
    // port: JSType#isInterface
    fn is_interface(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::is_interface(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_interface(self, reg),
            _ => false,
        }
    }
    // port: JSType#isOrdinaryFunction
    fn is_ordinary_function(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::is_ordinary_function(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_ordinary_function(self, reg)
            }
            _ => false,
        }
    }
    // port: JSType#isEmptyType
    fn is_empty_type(self, reg: &JSTypeRegistry) -> bool {
        self.is_no_type(reg)
            || self.is_no_object_type(reg)
            || self.is_no_resolved_type(reg)
            || self == reg.get_native_function_type(JSTypeNative::LEAST_FUNCTION_TYPE)
    }
    // port: JSType#isString
    fn is_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let t1 = reg.get_native_type(JSTypeNative::STRING_TYPE);
        let t2 = reg.get_native_type(JSTypeNative::STRING_OBJECT_TYPE);
        self.is_subtype_of(reg, ast, t1) || self.is_subtype_of(reg, ast, t2)
    }
    // port: JSType#isNumber
    fn is_number(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let t1 = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
        let t2 = reg.get_native_type(JSTypeNative::NUMBER_OBJECT_TYPE);
        self.is_subtype_of(reg, ast, t1) || self.is_subtype_of(reg, ast, t2)
    }
    // port: JSType#isSymbol
    fn is_symbol(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let t1 = reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
        let t2 = reg.get_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE);
        self.is_subtype_of(reg, ast, t1) || self.is_subtype_of(reg, ast, t2)
    }
    // port: JSType#isBigIntOrNumber
    fn is_big_int_or_number(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let t1 = reg.get_native_type(JSTypeNative::BIGINT_NUMBER);
        let t2 = reg.get_native_type(JSTypeNative::BIGINT_NUMBER_OBJECT);
        self.is_subtype_of(reg, ast, t1) || self.is_subtype_of(reg, ast, t2)
    }
    // port: JSType#isOnlyBigInt
    fn is_only_big_int(self, reg: &JSTypeRegistry) -> bool {
        self.is_big_int_value_type(reg) || self.is_big_int_object_type(reg)
    }
    // port: JSType#isUnknownType
    fn is_unknown_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => crate::object_type::is_unknown_type(self, reg, ast),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::is_unknown_type(self, reg, ast)
            }
            JSTypeKind::Union(_) => crate::union_type::is_unknown_type(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::is_unknown_type(self, reg, ast),
            _ => false,
        }
    }
    // port: JSType#isSomeUnknownType
    fn is_some_unknown_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        self.is_unknown_type(reg, ast)
    }
    // port: JSType#isUnionType
    fn is_union_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_union_type(reg).is_some()
    }
    // port: JSType#isFunctionType
    fn is_function_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_function_type(reg).is_some()
    }
    // port: JSType#isEnumElementType
    fn is_enum_element_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_enum_element_type(reg).is_some()
    }
    // port: JSType#isEnumType
    fn is_enum_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_enum_type(reg).is_some()
    }
    // port: JSType#isNamedType
    fn is_named_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_named_type(reg).is_some()
    }
    // port: JSType#isRecordType
    fn is_record_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_record_type(reg).is_some()
    }
    // port: JSType#isTemplatizedType
    fn is_templatized_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_templatized_type(reg).is_some()
    }
    // port: JSType#isTemplateType
    fn is_template_type(self, reg: &JSTypeRegistry) -> bool {
        self.to_maybe_template_type(reg).is_some()
    }
    // port: JSType#toMaybeUnionType
    fn to_maybe_union_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_union_type(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::to_maybe_union_type(self, reg),
            _ => None,
        }
    }
    // port: JSType#toMaybeFunctionType
    fn to_maybe_function_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Function(_) => crate::function_type::to_maybe_function_type(self, reg),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_function_type(self, reg)
            }
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::to_maybe_function_type(self, reg)
            }
            _ => None,
        }
    }
    // port: JSType#toMaybeKnownSymbolType
    fn to_maybe_known_symbol_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_known_symbol_type(self, reg)
            }
            JSTypeKind::KnownSymbol(_) => {
                crate::known_symbol_type::to_maybe_known_symbol_type(self, reg)
            }
            _ => None,
        }
    }
    // port: JSType#toMaybeEnumElementType
    fn to_maybe_enum_element_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::to_maybe_enum_element_type(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_enum_element_type(self, reg)
            }
            _ => None,
        }
    }
    // port: JSType#toMaybeEnumType
    fn to_maybe_enum_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_) => crate::enum_type::to_maybe_enum_type(self, reg),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::to_maybe_enum_type(self, reg),
            _ => None,
        }
    }
    // port: JSType#toMaybeNamedType
    fn to_maybe_named_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_) => crate::named_type::to_maybe_named_type(self, reg),
            _ => None,
        }
    }
    // port: JSType#toMaybeRecordType
    fn to_maybe_record_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_record_type(self, reg)
            }
            JSTypeKind::Record(_) => crate::record_type::to_maybe_record_type(self, reg),
            _ => None,
        }
    }
    // port: JSType#toMaybeTemplatizedType
    fn to_maybe_templatized_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_) | JSTypeKind::ProxyObject(_) | JSTypeKind::Template(_) => {
                crate::proxy_object_type::to_maybe_templatized_type(self, reg)
            }
            JSTypeKind::Templatized(_) => {
                crate::templatized_type::to_maybe_templatized_type(self, reg)
            }
            _ => None,
        }
    }
    // port: JSType#toMaybeTemplateType
    fn to_maybe_template_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_) | JSTypeKind::ProxyObject(_) | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::to_maybe_template_type(self, reg)
            }
            JSTypeKind::Template(_) => crate::template_type::to_maybe_template_type(self, reg),
            _ => None,
        }
    }
    // port: JSType#assertFunctionType
    fn assert_function_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        self.to_maybe_function_type(reg)
            .unwrap_or_else(|| panic!("not a FunctionType: {}", self.to_string(reg, ast)))
    }
    // port: JSType#assertObjectType
    fn assert_object_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        self.to_maybe_object_type(reg)
            .unwrap_or_else(|| panic!("Not an ObjectType: {}", self.to_string(reg, ast)))
    }
    // port: JSType#isRawTypeOfTemplatizedType
    fn is_raw_type_of_templatized_type(self, reg: &JSTypeRegistry) -> bool {
        self.get_template_param_count(reg) > 0 && !self.is_templatized_type(reg)
    }
    // port: JSType#isStruct
    fn is_struct(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_struct(self, reg, ast),
            JSTypeKind::Union(_) => crate::union_type::is_struct(self, reg, ast),
            _ => {
                if self.is_object(reg, ast) {
                    let obj = self.to_object_type(reg).unwrap();
                    if let Some(ctor) = obj.get_constructor(reg) {
                        ctor.makes_structs(reg, ast)
                    } else {
                        obj.get_jsdoc_info(reg).is_some_and(|d| d.makes_structs())
                    }
                } else {
                    false
                }
            }
        }
    }
    // port: JSType#isDict
    fn is_dict(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_dict(self, reg, ast),
            JSTypeKind::Union(_) => crate::union_type::is_dict(self, reg, ast),
            _ => {
                if self.is_object(reg, ast) {
                    let obj = self.to_object_type(reg).unwrap();
                    if let Some(ctor) = obj.get_constructor(reg) {
                        ctor.makes_dicts(reg, ast)
                    } else {
                        obj.get_jsdoc_info(reg).is_some_and(|d| d.makes_dicts())
                    }
                } else {
                    false
                }
            }
        }
    }
    // port: JSType#containsReferenceAncestor
    fn contains_reference_ancestor(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        target: TypeId,
    ) -> bool {
        let mut visitor =
            crate::contains_upper_bound_super_type_visitor::ContainsUpperBoundSuperTypeVisitor::new(
                Some(target),
            );
        self.visit(reg, ast, &mut visitor)
            == crate::contains_upper_bound_super_type_visitor::Result::PRESENT
    }
    // port: JSType#isLiteralObject
    fn is_literal_object(self, reg: &JSTypeRegistry) -> bool {
        reg.prototype_data_opt(self)
            .is_some_and(|d| d.anonymous_type)
    }
    // port: JSType#isGlobalThisType
    fn is_global_this_type(self, reg: &JSTypeRegistry) -> bool {
        self == reg.get_native_type(JSTypeNative::GLOBAL_THIS)
    }
    // port: JSType#getEnumeratedTypeOfEnumElement
    fn get_enumerated_type_of_enum_element(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        self.to_maybe_enum_element_type(reg)
            .map(|t| crate::enum_element_type::get_primitive_type(t, reg))
    }
    // port: JSType#hasAnyTemplateTypes
    fn has_any_template_types(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if let Some(result) = reg.data(self).template_check_result.get() {
            return result;
        }
        if reg.data(self).in_templated_check_visit.get() {
            return false;
        }
        reg.data(self).in_templated_check_visit.set(true);
        let result = self.has_any_template_types_internal(reg, ast);
        reg.data(self).in_templated_check_visit.set(false);
        if self.is_resolved(reg) {
            reg.data(self).template_check_result.set(Some(result));
        }
        result
    }
    // port: JSType#hasAnyTemplateTypesInternal
    fn has_any_template_types_internal(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Arrow(_) => {
                crate::arrow_type::has_any_template_types_internal(self, reg, ast)
            }
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::has_any_template_types_internal(self, reg, ast)
            }
            JSTypeKind::Named(_) | JSTypeKind::ProxyObject(_) => {
                crate::proxy_object_type::has_any_template_types_internal(self, reg, ast)
            }
            JSTypeKind::Template(_) => {
                crate::template_type::has_any_template_types_internal(self, reg)
            }
            JSTypeKind::Templatized(_) => {
                crate::templatized_type::has_any_template_types_internal(self, reg, ast)
            }
            JSTypeKind::Union(_) => {
                crate::union_type::has_any_template_types_internal(self, reg, ast)
            }
            _ => self
                .get_template_type_map(reg)
                .has_any_template_types_internal(reg, ast),
        }
    }
    // port: JSType#getTemplateTypeMap
    fn get_template_type_map(self, reg: &JSTypeRegistry) -> Arc<TemplateTypeMap> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_) | JSTypeKind::ProxyObject(_) | JSTypeKind::Template(_) => {
                crate::proxy_object_type::get_template_type_map(self, reg)
            }
            JSTypeKind::Templatized(_) => crate::templatized_type::get_template_type_map(self, reg),
            _ => reg.data(self).template_type_map.clone(),
        }
    }
    // port: JSType#getTypeParameters
    fn get_type_parameters(self, reg: &JSTypeRegistry) -> Vec<TypeId> {
        let map = self.get_template_type_map(reg);
        let keys = map.get_template_keys();
        keys[keys.len() - self.get_template_param_count(reg)..].to_vec()
    }
    // port: JSType#getTemplateParamCount
    fn get_template_param_count(self, reg: &JSTypeRegistry) -> usize {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::get_template_param_count(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::get_template_param_count(self, reg)
            }
            _ => 0,
        }
    }
    // port: JSType#mergeSupertypeTemplateTypes
    fn merge_supertype_template_types(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut other: TypeId,
    ) {
        self.maybe_loosen_typechecking_due_to_forward_referenced_supertype(reg, other);
        if other.is_raw_type_of_templatized_type(reg) {
            other = reg.create_templatized_type(ast, other, &[]);
        }
        let map = other.get_template_type_map(reg).copy_with_extension_map(
            reg,
            ast,
            &self.get_template_type_map(reg),
        );
        reg.data_mut(self).template_type_map = map;
    }
    // port: JSType#isObject
    fn is_object(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_)
            | JSTypeKind::Unknown(_) => crate::object_type::is_object(self, reg),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::is_object(self, reg, ast),
            JSTypeKind::Named(_) => crate::named_type::is_object(self, reg, ast),
            JSTypeKind::Union(_) => crate::union_type::is_object(self, reg, ast),
            _ => false,
        }
    }
    // port: JSType#isObjectType
    fn is_object_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        self.is_object(reg, ast)
    }
    // port: JSType#isNominalConstructorOrInterface
    #[allow(clippy::collapsible_if)] // Preserve the Java conditional cast.
    fn is_nominal_constructor_or_interface(self, reg: &JSTypeRegistry) -> bool {
        if self.is_constructor(reg) || self.is_interface(reg) {
            if let Some(f) = self.to_maybe_function_type(reg) {
                return FunctionType::get_source(f, reg).is_some() || f.is_native_object_type(reg);
            }
        }
        false
    }
    // port: JSType#loosenTypecheckingDueToForwardReferencedSupertype
    fn loosen_typechecking_due_to_forward_referenced_supertype(self, reg: &JSTypeRegistry) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::loosen_typechecking_due_to_forward_referenced_supertype(
                    self, reg,
                )
            }
            _ => {
                reg.data(self)
                    .loosen_typechecking_due_to_forward_referenced_supertype
            }
        }
    }
    // port: JSType#maybeLoosenTypecheckingDueToForwardReferencedSupertype
    fn maybe_loosen_typechecking_due_to_forward_referenced_supertype(
        self,
        reg: &mut JSTypeRegistry,
        supertype: TypeId,
    ) {
        if (supertype.is_named_type(reg) && !supertype.is_resolved(reg))
            || supertype.loosen_typechecking_due_to_forward_referenced_supertype(reg)
        {
            reg.data_mut(self)
                .loosen_typechecking_due_to_forward_referenced_supertype = true;
        }
    }
    // port: JSType#equals
    fn equals(self, reg: &mut JSTypeRegistry, ast: &Ast, other: impl Into<Option<TypeId>>) -> bool {
        let other = other.into();
        other.is_some_and(|other| {
            self == other
                || crate::equality_checker::EqualityChecker::new()
                    .set_eq_method(crate::equality_checker::EqMethod::IDENTITY)
                    .check(reg, ast, Some(self), Some(other))
        })
    }
    // port: JSType#differsFrom
    fn differs_from(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool {
        !crate::equality_checker::EqualityChecker::new()
            .set_eq_method(crate::equality_checker::EqMethod::DATA_FLOW)
            .check(reg, ast, Some(self), Some(that))
    }
    // port: JSType#hashCode
    fn hash_code(self, reg: &JSTypeRegistry) -> i32 {
        if reg.data(self).hash_code_in_progress.get() {
            return -1;
        }
        reg.data(self).hash_code_in_progress.set(true);
        let hash_code = self.recursion_unsafe_hash_code(reg);
        reg.data(self).hash_code_in_progress.set(false);
        hash_code
    }
    // port: JSType#recursionUnsafeHashCode
    fn recursion_unsafe_hash_code(self, reg: &JSTypeRegistry) -> i32 {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::Arrow(_) => crate::arrow_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => crate::value_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::Enum(_) | JSTypeKind::PrototypeObject(_) | JSTypeKind::Record(_) => {
                crate::prototype_object_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::Function(_) => crate::function_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::Named(_) => crate::named_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::NoResolved(_) => {
                crate::no_resolved_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::ProxyObject(_) => {
                crate::proxy_object_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::Template(_) => crate::template_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::Templatized(_) => {
                crate::templatized_type::recursion_unsafe_hash_code(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::Unknown(_) => crate::unknown_type::recursion_unsafe_hash_code(self, reg),
            JSTypeKind::UnitTesting(d) => d.hash.expect("UnsupportedOperationException"),
        }
    }
    // port: JSType#matchesNumberContext
    fn matches_number_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Boolean(_) => crate::boolean_type::matches_number_context(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::matches_number_context(self, reg),
            JSTypeKind::Enum(_) => crate::enum_type::matches_number_context(self, reg),
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::matches_number_context(self, reg, ast)
            }
            JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::matches_number_context(self, reg, ast)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::matches_number_context(self, reg, ast)
            }
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::matches_number_context(self, reg)
            }
            JSTypeKind::Null(_) => crate::null_type::matches_number_context(self, reg),
            JSTypeKind::Number(_) => crate::number_type::matches_number_context(self, reg),
            JSTypeKind::String(_) => crate::string_type::matches_number_context(self, reg),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::matches_number_context(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::matches_number_context(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::matches_number_context(self, reg),
            JSTypeKind::Void(_) => crate::void_type::matches_number_context(self, reg),
            _ => false,
        }
    }
    // port: JSType#matchesStringContext
    fn matches_string_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::matches_string_context(self, reg),
            JSTypeKind::Boolean(_) => crate::boolean_type::matches_string_context(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::matches_string_context(self, reg),
            JSTypeKind::Enum(_) => crate::enum_type::matches_string_context(self, reg),
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::matches_string_context(self, reg, ast)
            }
            JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::matches_string_context(self, reg, ast)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::matches_string_context(self, reg, ast)
            }
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::matches_string_context(self, reg)
            }
            JSTypeKind::Null(_) => crate::null_type::matches_string_context(self, reg),
            JSTypeKind::Number(_) => crate::number_type::matches_string_context(self, reg),
            JSTypeKind::String(_) => crate::string_type::matches_string_context(self, reg),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::matches_string_context(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::matches_string_context(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::matches_string_context(self, reg),
            JSTypeKind::Void(_) => crate::void_type::matches_string_context(self, reg),
            _ => false,
        }
    }
    // port: JSType#matchesSymbolContext
    fn matches_symbol_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::matches_symbol_context(self, reg)
            }
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::matches_symbol_context(self, reg, ast)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::matches_symbol_context(self, reg, ast)
            }
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::matches_symbol_context(self, reg)
            }
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::matches_symbol_context(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::matches_symbol_context(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::matches_symbol_context(self, reg),
            _ => false,
        }
    }
    // port: JSType#matchesObjectContext
    fn matches_object_context(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::matches_object_context(self, reg),
            JSTypeKind::Boolean(_) => crate::boolean_type::matches_object_context(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::matches_object_context(self, reg),
            JSTypeKind::Enum(_) => crate::enum_type::matches_object_context(self, reg),
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::matches_object_context(self, reg, ast)
            }
            JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::matches_object_context(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::matches_object_context(self, reg, ast)
            }
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::matches_object_context(self, reg)
            }
            JSTypeKind::Null(_) => crate::null_type::matches_object_context(self, reg),
            JSTypeKind::Number(_) => crate::number_type::matches_object_context(self, reg),
            JSTypeKind::String(_) => crate::string_type::matches_object_context(self, reg),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::matches_object_context(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::matches_object_context(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::matches_object_context(self, reg),
            JSTypeKind::Void(_) => crate::void_type::matches_object_context(self, reg),
            _ => false,
        }
    }
    // port: JSType#canBeCalled
    fn can_be_called(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::EnumElement(_) => crate::enum_element_type::can_be_called(self, reg, ast),
            JSTypeKind::Function(_) | JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::function_type::can_be_called(self, reg)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::can_be_called(self, reg, ast),
            JSTypeKind::Union(_) => crate::union_type::can_be_called(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::can_be_called(self, reg),
            _ => false,
        }
    }
    // port: JSType#findPropertyType
    fn find_property_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<TypeId> {
        let name = name.into();
        let property_type =
            self.find_property_type_without_considering_template_types(reg, ast, name)?;
        let map = self.get_template_type_map(reg);
        if map.is_empty() || !property_type.has_any_template_types(reg, ast) {
            return Some(property_type);
        }
        let mut replacer =
            crate::template_type_replacer::TemplateTypeReplacer::for_partial_replacement(map);
        Some(property_type.visit(reg, ast, &mut replacer))
    }
    // port: JSType#findPropertyTypeWithoutConsideringTemplateTypes
    fn find_property_type_without_considering_template_types(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: PropertyKey,
    ) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Unknown(_) => {
                crate::object_type::find_property_type_without_considering_template_types(
                    self, reg, ast, &name,
                )
            }
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::find_property_type_without_considering_template_types(
                    self, reg, ast, name,
                )
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::find_property_type_without_considering_template_types(
                    self, reg, ast, &name,
                )
            }
            JSTypeKind::Union(_) => {
                crate::union_type::find_property_type_without_considering_template_types(
                    self, reg, ast, name,
                )
            }
            _ => self
                .autoboxes_to(reg)
                .and_then(|t| t.to_object_type(reg))
                .and_then(|t| t.find_property_type(reg, ast, name)),
        }
    }
    // port: JSType#canCastTo
    fn can_cast_to(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool {
        self.visit_relationship(
            reg,
            ast,
            &mut crate::can_cast_to_visitor::CanCastToVisitor,
            that,
        )
    }
    // port: JSType#autoboxesTo
    fn autoboxes_to(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Boolean(_) => crate::boolean_type::autoboxes_to(self, reg),
            JSTypeKind::BigInt(_) => crate::big_int_type::autoboxes_to(self, reg),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::autoboxes_to(self, reg),
            JSTypeKind::Number(_) => crate::number_type::autoboxes_to(self, reg),
            JSTypeKind::String(_) => crate::string_type::autoboxes_to(self, reg),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::autoboxes_to(self, reg)
            }
            _ => None,
        }
    }
    // port: JSType#isBoxableScalar
    fn is_boxable_scalar(self, reg: &JSTypeRegistry) -> bool {
        self.autoboxes_to(reg).is_some()
    }
    // port: JSType#toObjectType
    fn to_object_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_)
            | JSTypeKind::Unknown(_) => Some(self),
            _ => None,
        }
    }
    // port: JSType#autobox
    fn autobox(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => crate::union_type::autobox(self, reg, ast),
            _ => {
                let restricted = self.restrict_by_not_null_or_undefined(reg, ast);
                restricted.autoboxes_to(reg).unwrap_or(restricted)
            }
        }
    }
    // port: JSType#dereference
    fn dereference(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        self.autobox(reg, ast).to_object_type(reg)
    }
    // port: JSType#canTestForEqualityWith
    fn can_test_for_equality_with(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool {
        self.test_for_equality(reg, ast, that) == Some(Tri::UNKNOWN)
    }
    // port: JSType#testForEquality
    fn test_for_equality(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> Option<Tri> {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Arrow(_) => crate::arrow_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Boolean(_) => crate::boolean_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::BigInt(_) => crate::big_int_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Enum(_) => crate::enum_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::test_for_equality(self, reg, ast, that)
            }
            JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => crate::object_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::test_for_equality(self, reg, ast, that)
            }
            JSTypeKind::NoResolved(_) => {
                crate::no_resolved_type::test_for_equality(self, reg, ast, that)
            }
            JSTypeKind::Null(_) => crate::null_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Number(_) => crate::number_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::String(_) => crate::string_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::test_for_equality(self, reg, ast, that)
            }
            JSTypeKind::Union(_) => crate::union_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Unknown(_) => crate::unknown_type::test_for_equality(self, reg, ast, that),
            JSTypeKind::Void(_) => crate::void_type::test_for_equality(self, reg, ast, that),
            _ => test_for_equality_helper(self, reg, ast, that),
        }
    }
    // port: JSType#canTestForShallowEqualityWith
    fn can_test_for_shallow_equality_with(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> bool {
        if self.is_no_resolved_type(reg) || that.is_no_resolved_type(reg) {
            return true;
        }
        if self.is_empty_type(reg) || that.is_empty_type(reg) {
            return self.is_subtype_of(reg, ast, that) || that.is_subtype_of(reg, ast, self);
        }
        let inf = self.get_greatest_subtype(reg, ast, that);
        !inf.is_empty_type(reg) || inf == reg.get_native_type(JSTypeNative::LEAST_FUNCTION_TYPE)
    }
    // port: JSType#isNullable
    fn is_nullable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::is_nullable(self, reg),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::is_nullable(self, reg, ast),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_nullable(self, reg, ast),
            JSTypeKind::No(_) => crate::no_type::is_nullable(self, reg),
            JSTypeKind::Null(_) => crate::null_type::is_nullable(self, reg),
            JSTypeKind::Union(_) => crate::union_type::is_nullable(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::is_nullable(self, reg),
            _ => false,
        }
    }
    // port: JSType#isVoidable
    fn is_voidable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::is_voidable(self, reg),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::is_voidable(self, reg, ast),
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => crate::proxy_object_type::is_voidable(self, reg, ast),
            JSTypeKind::No(_) => crate::no_type::is_voidable(self, reg),
            JSTypeKind::Union(_) => crate::union_type::is_voidable(self, reg, ast),
            JSTypeKind::Unknown(_) => crate::unknown_type::is_voidable(self, reg),
            JSTypeKind::Void(_) => crate::void_type::is_voidable(self, reg),
            _ => false,
        }
    }
    // port: JSType#isExplicitlyVoidable
    fn is_explicitly_voidable(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => crate::union_type::is_explicitly_voidable(self, reg, ast),
            JSTypeKind::Void(_) => crate::void_type::is_explicitly_voidable(self, reg),
            _ => false,
        }
    }
    // port: JSType#collapseUnion
    fn collapse_union(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
        match &reg.data(self).kind {
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::collapse_union(self, reg, ast)
            }
            JSTypeKind::Union(_) => crate::union_type::collapse_union(self, reg, ast),
            _ => Some(self),
        }
    }
    // port: JSType#getLeastSupertype
    fn get_least_supertype(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Arrow(_) => crate::arrow_type::get_least_supertype(self, reg, ast, that),
            JSTypeKind::Union(_) => crate::union_type::get_least_supertype(self, reg, ast, that),
            _ => {
                if self == that {
                    return self;
                }
                if let Some(u) = that.to_maybe_union_type(reg) {
                    return u.get_least_supertype(reg, ast, self);
                }
                get_least_supertype(self, reg, ast, that)
            }
        }
    }
    // port: JSType#getGreatestSubtype
    fn get_greatest_subtype(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Arrow(_) => crate::arrow_type::get_greatest_subtype(self, reg, ast, that),
            _ => get_greatest_subtype(self, reg, ast, that),
        }
    }
    // port: JSType#getRestrictedTypeGivenOutcome
    fn get_restricted_type_given_outcome(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        outcome: Outcome,
    ) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => {
                crate::union_type::get_restricted_type_given_outcome(self, reg, ast, outcome)
            }
            _ => {
                if outcome.is_truthy() && self == reg.get_native_type(JSTypeNative::UNKNOWN_TYPE) {
                    return reg.get_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE);
                }
                if outcome.is_nullish().to_boolean(false) {
                    return if self == reg.get_native_type(JSTypeNative::VOID_TYPE)
                        || self == reg.get_native_type(JSTypeNative::NULL_TYPE)
                    {
                        self
                    } else {
                        reg.get_native_type(JSTypeNative::NO_TYPE)
                    };
                }
                if self
                    .get_possible_to_boolean_outcomes(reg)
                    .contains(outcome.is_truthy())
                {
                    self
                } else {
                    reg.get_native_type(JSTypeNative::NO_TYPE)
                }
            }
        }
    }
    // port: JSType#getPossibleToBooleanOutcomes
    fn get_possible_to_boolean_outcomes(self, reg: &JSTypeRegistry) -> BooleanLiteralSet {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::Arrow(_) => crate::arrow_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::Boolean(_) => {
                crate::boolean_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::BigInt(_) => {
                crate::big_int_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::Enum(_)
            | JSTypeKind::EnumElement(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::Named(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Record(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::object_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::No(_) => crate::no_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::NoResolved(_) => {
                crate::no_resolved_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::Null(_) => crate::null_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::Number(_) => {
                crate::number_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::String(_) => {
                crate::string_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::Union(_) => crate::union_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::Unknown(_) => {
                crate::unknown_type::get_possible_to_boolean_outcomes(self, reg)
            }
            JSTypeKind::Void(_) => crate::void_type::get_possible_to_boolean_outcomes(self, reg),
            JSTypeKind::UnitTesting(_) => {
                panic!("UnsupportedOperationException")
            }
        }
    }
    // port: JSType#getTypesUnderEquality
    fn get_types_under_equality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => {
                crate::union_type::get_types_under_equality(self, reg, ast, that)
            }
            _ => {
                if let Some(u) = that.to_maybe_union_type(reg) {
                    let p = u.get_types_under_equality(reg, ast, self);
                    return TypePair::new(p.type_b, p.type_a);
                }
                match self
                    .test_for_equality(reg, ast, that)
                    .expect("NullPointerException")
                {
                    Tri::FALSE => TypePair::new(None, None),
                    Tri::TRUE | Tri::UNKNOWN => TypePair::new(Some(self), Some(that)),
                }
            }
        }
    }
    // port: JSType#getTypesUnderInequality
    fn get_types_under_inequality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => {
                crate::union_type::get_types_under_inequality(self, reg, ast, that)
            }
            _ => {
                if let Some(u) = that.to_maybe_union_type(reg) {
                    let p = u.get_types_under_inequality(reg, ast, self);
                    return TypePair::new(p.type_b, p.type_a);
                }
                if self.is_null_type_or_void_type(reg) && that.is_null_type_or_void_type(reg) {
                    let no = reg.get_native_type(JSTypeNative::NO_TYPE);
                    return TypePair::new(Some(no), Some(no));
                }
                TypePair::new(Some(self), Some(that))
            }
        }
    }
    // port: JSType#getTypesUnderShallowEquality
    fn get_types_under_shallow_equality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair {
        let common = self.get_greatest_subtype(reg, ast, that);
        TypePair::new(Some(common), Some(common))
    }
    // port: JSType#getTypesUnderShallowInequality
    fn get_types_under_shallow_inequality(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
    ) -> TypePair {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => {
                crate::union_type::get_types_under_shallow_inequality(self, reg, ast, that)
            }
            _ => {
                if let Some(u) = that.to_maybe_union_type(reg) {
                    let p = u.get_types_under_shallow_inequality(reg, ast, self);
                    return TypePair::new(p.type_b, p.type_a);
                }
                if self == that && self.is_null_type_or_void_type(reg) {
                    return TypePair::new(None, None);
                }
                TypePair::new(Some(self), Some(that))
            }
        }
    }
    // port: JSType#isNullTypeOrVoidType
    fn is_null_type_or_void_type(self, reg: &JSTypeRegistry) -> bool {
        self.is_null_type(reg) || self.is_void_type(reg)
    }
    // port: JSType#getUnionMembers
    fn get_union_members(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<Arc<Vec<TypeId>>> {
        self.to_maybe_union_type(reg)
            .map(|t| crate::union_type::get_alternates(t, reg, ast))
    }
    // port: JSType#restrictByNotNullOrUndefined
    fn restrict_by_not_null_or_undefined(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Null(_) => crate::null_type::restrict_by_not_null_or_undefined(self, reg),
            JSTypeKind::Union(_) => {
                crate::union_type::restrict_by_not_null_or_undefined(self, reg, ast)
            }
            JSTypeKind::Void(_) => crate::void_type::restrict_by_not_null_or_undefined(self, reg),
            _ => self,
        }
    }
    // port: JSType#restrictByNotUndefined
    fn restrict_by_not_undefined(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Union(_) => crate::union_type::restrict_by_not_undefined(self, reg, ast),
            JSTypeKind::Void(_) => crate::void_type::restrict_by_not_undefined(self, reg),
            _ => self,
        }
    }
    // port: JSType#restrictByNotNull
    fn restrict_by_not_null(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::Null(_) => crate::null_type::restrict_by_not_null(self, reg),
            JSTypeKind::Union(_) => crate::union_type::restrict_by_not_null(self, reg, ast),
            _ => self,
        }
    }
    // port: JSType#isSubtypeWithoutStructuralTyping
    fn is_subtype_without_structural_typing(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
    ) -> bool {
        crate::subtype_checker::SubtypeChecker::new()
            .set_subtype(self)
            .set_supertype(supertype)
            .set_using_structural_subtyping(false)
            .set_subtyping_mode(SubtypingMode::NORMAL)
            .check(reg, ast)
    }
    // port: JSType#isSubtype
    fn is_subtype(self, reg: &mut JSTypeRegistry, ast: &Ast, supertype: TypeId) -> bool {
        self.is_subtype_of(reg, ast, supertype)
    }
    // port: JSType#isSubtypeOf
    fn is_subtype_of(self, reg: &mut JSTypeRegistry, ast: &Ast, supertype: TypeId) -> bool {
        self.is_subtype_of_with_mode(reg, ast, supertype, SubtypingMode::NORMAL)
    }
    // port: JSType#isSubtype
    fn is_subtype_with_mode(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
        mode: SubtypingMode,
    ) -> bool {
        self.is_subtype_of_with_mode(reg, ast, supertype, mode)
    }
    // port: JSType#isSubtypeOf
    fn is_subtype_of_with_mode(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        supertype: TypeId,
        mode: SubtypingMode,
    ) -> bool {
        crate::subtype_checker::SubtypeChecker::new()
            .set_subtype(self)
            .set_supertype(supertype)
            .set_using_structural_subtyping(true)
            .set_subtyping_mode(mode)
            .check(reg, ast)
    }
    // port: JSType#visit
    fn visit<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl crate::visitor::Visitor<T>,
    ) -> T {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::visit(self, reg, ast, visitor),
            JSTypeKind::Arrow(_) => crate::arrow_type::visit(self, reg, ast, visitor),
            JSTypeKind::Boolean(_) => crate::boolean_type::visit(self, reg, ast, visitor),
            JSTypeKind::BigInt(_) => crate::big_int_type::visit(self, reg, ast, visitor),
            JSTypeKind::Enum(_) => crate::enum_type::visit(self, reg, ast, visitor),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::visit(self, reg, ast, visitor),
            JSTypeKind::Function(_) => crate::function_type::visit(self, reg, ast, visitor),
            JSTypeKind::InstanceObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => crate::object_type::visit(self, reg, ast, visitor),
            JSTypeKind::Named(_) => crate::named_type::visit(self, reg, ast, visitor),
            JSTypeKind::No(_) => crate::no_type::visit(self, reg, ast, visitor),
            JSTypeKind::NoObject(_) => crate::no_object_type::visit(self, reg, ast, visitor),
            JSTypeKind::Null(_) => crate::null_type::visit(self, reg, ast, visitor),
            JSTypeKind::Number(_) => crate::number_type::visit(self, reg, ast, visitor),
            JSTypeKind::ProxyObject(_) => crate::proxy_object_type::visit(self, reg, ast, visitor),
            JSTypeKind::String(_) => crate::string_type::visit(self, reg, ast, visitor),
            JSTypeKind::Symbol(_) | JSTypeKind::KnownSymbol(_) => {
                crate::symbol_type::visit(self, reg, ast, visitor)
            }
            JSTypeKind::Template(_) => crate::template_type::visit(self, reg, ast, visitor),
            JSTypeKind::Templatized(_) => crate::templatized_type::visit(self, reg, ast, visitor),
            JSTypeKind::Union(_) => crate::union_type::visit(self, reg, ast, visitor),
            JSTypeKind::Unknown(_) => crate::unknown_type::visit(self, reg, ast, visitor),
            JSTypeKind::Void(_) => crate::void_type::visit(self, reg, ast, visitor),
            JSTypeKind::UnitTesting(_) => {
                panic!("UnsupportedOperationException")
            }
        }
    }
    // port: JSType#visit
    fn visit_relationship<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
        that: TypeId,
    ) -> T {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => {
                crate::all_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Arrow(_) => {
                crate::arrow_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => {
                crate::value_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Enum(_) => {
                crate::enum_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Function(_) => {
                crate::function_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::InstanceObject(_)
            | JSTypeKind::NoResolved(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::object_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Named(_) | JSTypeKind::ProxyObject(_) => {
                crate::proxy_object_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::No(_) => crate::no_type::visit_relationship(self, reg, ast, visitor, that),
            JSTypeKind::NoObject(_) => {
                crate::no_object_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Template(_) => {
                crate::template_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Templatized(_) => {
                crate::templatized_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Union(_) => {
                crate::union_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::Unknown(_) => {
                crate::unknown_type::visit_relationship(self, reg, ast, visitor, that)
            }
            JSTypeKind::UnitTesting(_) => {
                panic!("UnsupportedOperationException")
            }
        }
    }
    // port: JSType#resolve
    fn resolve(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let mut reporter = reg.get_error_reporter();
        self.resolve_with_reporter(reg, ast, &mut reporter)
    }
    // port: JSType#resolve
    fn resolve_with_reporter(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> TypeId {
        reg.get_resolver().assert_legal_to_resolve_types();
        if !self.is_resolved(reg) {
            reg.data_mut(self).resolve_result = Some(self);
            let result = self.resolve_internal(reg, ast, reporter);
            reg.data_mut(self).resolve_result = Some(result);
            closure_rhino::check_state!(self.is_resolved(reg));
        }
        reg.data(self).resolve_result.unwrap()
    }
    // port: JSType#resolveInternal
    fn resolve_internal(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
    ) -> TypeId {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::Arrow(_) => crate::arrow_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => crate::value_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::Enum(_) => crate::enum_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::EnumElement(_) => {
                crate::enum_element_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::Function(_) => {
                crate::function_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::Named(_) => crate::named_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::No(_) | JSTypeKind::NoObject(_) => {
                crate::no_object_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::NoResolved(_) => {
                crate::no_resolved_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::PrototypeObject(_) | JSTypeKind::Record(_) => {
                crate::prototype_object_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::ProxyObject(_) | JSTypeKind::Template(_) => {
                crate::proxy_object_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::Templatized(_) => {
                crate::templatized_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::Union(_) => crate::union_type::resolve_internal(self, reg, ast, reporter),
            JSTypeKind::Unknown(_) => {
                crate::unknown_type::resolve_internal(self, reg, ast, reporter)
            }
            JSTypeKind::UnitTesting(d) => {
                let callback = d.resolve.clone();
                if let Some(callback) = callback {
                    callback(self, reg, ast)
                } else {
                    self
                }
            }
        }
    }
    // port: JSType#eagerlyResolveToSelf
    fn eagerly_resolve_to_self(self, reg: &mut JSTypeRegistry, ast: &Ast) {
        closure_rhino::check_state!(!self.is_resolved(reg), "{}", self.to_string(reg, ast));
        reg.data_mut(self).resolve_result = Some(self);
        reg.finish_construction(self, ast);
    }
    // port: JSType#isResolved
    fn is_resolved(self, reg: &JSTypeRegistry) -> bool {
        reg.data(self).resolve_result.is_some()
    }
    // port: JSType#isSuccessfullyResolved
    fn is_successfully_resolved(self, reg: &JSTypeRegistry) -> bool {
        self.is_resolved(reg) && !self.is_no_resolved_type(reg)
    }
    // port: JSType#isUnsuccessfullyResolved
    fn is_unsuccessfully_resolved(self, reg: &JSTypeRegistry) -> bool {
        self.is_resolved(reg) && self.is_no_resolved_type(reg)
    }
    // port: JSType#setValidator
    fn set_validator(self, reg: &mut JSTypeRegistry, ast: &Ast, validator: TypeValidator) -> bool {
        match &reg.data(self).kind {
            JSTypeKind::Named(_) => crate::named_type::set_validator(self, reg, ast, validator),
            JSTypeKind::ProxyObject(_) | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::set_validator(self, reg, ast, validator)
            }
            JSTypeKind::Template(_) => {
                crate::template_type::set_validator(self, reg, ast, validator)
            }
            _ => validator(self, reg, ast),
        }
    }
    // port: JSType#toString
    fn to_string(self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        if matches!(reg.data(self).kind, JSTypeKind::Arrow(_)) {
            crate::arrow_type::to_string(self, reg, ast)
        } else {
            TypeStringBuilder::new(false)
                .append_type(reg, ast, self)
                .build()
        }
    }
    // port: JSType#toAnnotationString
    fn to_annotation_string(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        nullability: Nullability,
    ) -> String {
        let mut builder = TypeStringBuilder::new(true);
        if nullability == Nullability::EXPLICIT {
            builder.append_non_null(reg, ast, self);
        } else {
            builder.append_type(reg, ast, self);
        }
        builder.build()
    }
    // port: JSType#appendTo
    fn append_to(self, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder) {
        match &reg.data(self).kind {
            JSTypeKind::All(_) => crate::all_type::append_to(self, reg, ast, sb),
            JSTypeKind::Arrow(_) => crate::arrow_type::append_to(self, reg, ast, sb),
            JSTypeKind::Boolean(_)
            | JSTypeKind::BigInt(_)
            | JSTypeKind::Null(_)
            | JSTypeKind::Number(_)
            | JSTypeKind::String(_)
            | JSTypeKind::Symbol(_)
            | JSTypeKind::KnownSymbol(_)
            | JSTypeKind::Void(_) => crate::value_type::append_to(self, reg, ast, sb),
            JSTypeKind::Enum(_) => crate::enum_type::append_to(self, reg, ast, sb),
            JSTypeKind::EnumElement(_) => crate::enum_element_type::append_to(self, reg, ast, sb),
            JSTypeKind::Function(_) => crate::function_type::append_to(self, reg, ast, sb),
            JSTypeKind::InstanceObject(_) => {
                crate::instance_object_type::append_to(self, reg, ast, sb)
            }
            JSTypeKind::Named(_) => crate::named_type::append_to(self, reg, ast, sb),
            JSTypeKind::No(_) => crate::no_type::append_to(self, reg, ast, sb),
            JSTypeKind::NoObject(_) => crate::no_object_type::append_to(self, reg, ast, sb),
            JSTypeKind::NoResolved(_) => crate::no_resolved_type::append_to(self, reg, ast, sb),
            JSTypeKind::PrototypeObject(_) | JSTypeKind::Record(_) => {
                crate::prototype_object_type::append_to(self, reg, ast, sb)
            }
            JSTypeKind::ProxyObject(_) => crate::proxy_object_type::append_to(self, reg, ast, sb),
            JSTypeKind::Template(_) => crate::template_type::append_to(self, reg, ast, sb),
            JSTypeKind::Templatized(_) => crate::templatized_type::append_to(self, reg, ast, sb),
            JSTypeKind::Union(_) => crate::union_type::append_to(self, reg, ast, sb),
            JSTypeKind::Unknown(_) => crate::unknown_type::append_to(self, reg, ast, sb),
            JSTypeKind::UnitTesting(_) => {
                panic!("UnsupportedOperationException")
            }
        }
    }
    // port: JSType#matchConstraint
    fn match_constraint(self, reg: &mut JSTypeRegistry, ast: &Ast, constraint: TypeId) {
        match &reg.data(self).kind {
            JSTypeKind::Enum(_)
            | JSTypeKind::Function(_)
            | JSTypeKind::InstanceObject(_)
            | JSTypeKind::No(_)
            | JSTypeKind::NoObject(_)
            | JSTypeKind::PrototypeObject(_)
            | JSTypeKind::Record(_) => {
                crate::prototype_object_type::match_constraint(self, reg, ast, constraint)
            }
            JSTypeKind::Named(_)
            | JSTypeKind::ProxyObject(_)
            | JSTypeKind::Template(_)
            | JSTypeKind::Templatized(_) => {
                crate::proxy_object_type::match_constraint(self, reg, ast, constraint)
            }
            JSTypeKind::Union(_) => crate::union_type::match_constraint(self, reg, ast, constraint),
            _ => {}
        }
    }
    // port: JSType#toMaybeObjectType
    fn to_maybe_object_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        self.to_object_type(reg)
    }
}
