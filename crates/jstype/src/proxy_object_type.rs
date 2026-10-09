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
//   src/com/google/javascript/rhino/jstype/ProxyObjectType.java.

use crate::{
    TypeId,
    js_type::{HasPropertyKind, JSType, JSTypeKind, TypeValidator},
    js_type_class::JSTypeClass,
    js_type_registry::JSTypeRegistry,
    object_type::{ObjectType, ObjectTypeData},
    property::PropertyKey,
    property_map::PropertyMap,
    relationship_visitor::RelationshipVisitor,
    template_type_map::TemplateTypeMap,
    type_string_builder::TypeStringBuilder,
    visitor::Visitor,
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    jscomp_base::tri::Tri,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ProxyObjectTypeData {
    pub(crate) object: ObjectTypeData,
    pub(crate) referenced_type: TypeId,
    pub(crate) referenced_obj_type: Option<TypeId>,
}
impl ProxyObjectTypeData {
    pub(crate) fn new(reg: &JSTypeRegistry, referenced_type: TypeId) -> Self {
        Self {
            object: ObjectTypeData::new(),
            referenced_type,
            referenced_obj_type: referenced_type.to_maybe_object_type(reg),
        }
    }
}
pub(crate) fn proxy_data(id: TypeId, reg: &JSTypeRegistry) -> &ProxyObjectTypeData {
    match &reg.data(id).kind {
        JSTypeKind::ProxyObject(d) => d,
        JSTypeKind::Named(d) => &d.proxy,
        JSTypeKind::Template(d) => &d.proxy,
        JSTypeKind::Templatized(d) => &d.proxy,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn proxy_data_mut(id: TypeId, reg: &mut JSTypeRegistry) -> &mut ProxyObjectTypeData {
    match &mut reg.data_mut(id).kind {
        JSTypeKind::ProxyObject(d) => d,
        JSTypeKind::Named(d) => &mut d.proxy,
        JSTypeKind::Template(d) => &mut d.proxy,
        JSTypeKind::Templatized(d) => &mut d.proxy,
        _ => panic!("ClassCastException"),
    }
}

// port: ProxyObjectType#ProxyObjectType
pub fn create(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    referenced_type: TypeId,
    template_type_map: Option<Arc<TemplateTypeMap>>,
) -> TypeId {
    let data = ProxyObjectTypeData::new(reg, referenced_type);
    let id = reg.alloc(JSTypeKind::ProxyObject(data), template_type_map);
    reg.finish_construction(id, ast);
    id
}
pub trait ProxyObjectType {
    fn get_referenced_type_internal(self, reg: &JSTypeRegistry) -> TypeId;
    fn get_referenced_obj_type_internal(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn set_referenced_type(self, reg: &mut JSTypeRegistry, referenced_type: TypeId);
    fn visit_reference_type<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl Visitor<T>,
    ) -> T;
}
impl ProxyObjectType for TypeId {
    // port: ProxyObjectType#getReferencedTypeInternal
    fn get_referenced_type_internal(self, reg: &JSTypeRegistry) -> TypeId {
        proxy_data(self, reg).referenced_type
    }
    // port: ProxyObjectType#getReferencedObjTypeInternal
    fn get_referenced_obj_type_internal(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        proxy_data(self, reg).referenced_obj_type
    }
    // port: ProxyObjectType#setReferencedType
    fn set_referenced_type(self, reg: &mut JSTypeRegistry, referenced_type: TypeId) {
        let obj_type = referenced_type.to_maybe_object_type(reg);
        let data = proxy_data_mut(self, reg);
        data.referenced_type = referenced_type;
        data.referenced_obj_type = obj_type;
    }
    // port: ProxyObjectType#visitReferenceType
    fn visit_reference_type<T>(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        visitor: &mut impl Visitor<T>,
    ) -> T {
        self.get_referenced_type_internal(reg)
            .visit(reg, ast, visitor)
    }
}
// port: ProxyObjectType#getTypeClass
pub(crate) fn get_type_class(_id: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::PROXY_OBJECT
}
// port: ProxyObjectType#getPropertyKind
pub(crate) fn get_property_kind(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
    autobox: bool,
) -> HasPropertyKind {
    id.get_referenced_type_internal(reg)
        .get_property_kind_with_autobox(reg, ast, property_name.clone(), autobox)
}
// port: ProxyObjectType#getPropertyMap
pub(crate) fn get_property_map(id: TypeId, reg: &JSTypeRegistry) -> &PropertyMap {
    match id.get_referenced_obj_type_internal(reg) {
        Some(t) => t.get_property_map(reg),
        None => PropertyMap::immutable_empty_map(),
    }
}
// port: ProxyObjectType#loosenTypecheckingDueToForwardReferencedSupertype
pub(crate) fn loosen_typechecking_due_to_forward_referenced_supertype(
    id: TypeId,
    reg: &JSTypeRegistry,
) -> bool {
    id.get_referenced_type_internal(reg)
        .loosen_typechecking_due_to_forward_referenced_supertype(reg)
}
// port: ProxyObjectType#setValidator
pub(crate) fn set_validator(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    validator: TypeValidator,
) -> bool {
    id.get_referenced_type_internal(reg)
        .set_validator(reg, ast, validator)
}
// port: ProxyObjectType#getReferenceName
pub(crate) fn get_reference_name(id: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    match id.get_referenced_obj_type_internal(reg) {
        Some(t) => t.get_reference_name(reg),
        None => Some(JsString::default()),
    }
}
// port: ProxyObjectType#getOwnerFunction
pub(crate) fn get_owner_function(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .and_then(|t| t.get_owner_function(reg))
}
// port: ProxyObjectType#getCtorImplementedInterfaces
pub(crate) fn get_ctor_implemented_interfaces(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .map_or_else(Vec::new, |t| t.get_ctor_implemented_interfaces(reg, ast))
}
// port: ProxyObjectType#getCtorExtendedInterfaces
pub(crate) fn get_ctor_extended_interfaces(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .map_or_else(Vec::new, |t| t.get_ctor_extended_interfaces(reg, ast))
}
// port: ProxyObjectType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(id: TypeId, reg: &JSTypeRegistry) -> i32 {
    id.get_referenced_type_internal(reg).hash_code(reg)
}
// port: ProxyObjectType#appendTo
pub(crate) fn append_to(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    id.get_referenced_type_internal(reg).append_to(reg, ast, sb);
}
// port: ProxyObjectType#getImplicitPrototype
pub(crate) fn get_implicit_prototype(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Option<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .and_then(|t| t.get_implicit_prototype(reg, ast))
}
// port: ProxyObjectType#defineProperty
pub(crate) fn define_property(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    property_node: Option<NodeId>,
) -> bool {
    id.get_referenced_obj_type_internal(reg).is_none_or(|t| {
        t.define_property(
            reg,
            ast,
            property_name.clone(),
            type_,
            inferred,
            property_node,
        )
    })
}
// port: ProxyObjectType#findPropertyTypeWithoutConsideringTemplateTypes
pub(crate) fn find_property_type_without_considering_template_types(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .find_property_type(reg, ast, property_name.clone())
}
// port: ProxyObjectType#getJSDocInfo
pub(crate) fn get_jsdoc_info(id: TypeId, reg: &JSTypeRegistry) -> Option<Arc<JSDocInfo>> {
    id.get_referenced_type_internal(reg).get_jsdoc_info(reg)
}
// port: ProxyObjectType#setJSDocInfo
pub(crate) fn set_jsdoc_info(id: TypeId, reg: &mut JSTypeRegistry, info: Option<Arc<JSDocInfo>>) {
    if let Some(t) = id.get_referenced_obj_type_internal(reg) {
        t.set_jsdoc_info(reg, info);
    }
}
// port: ProxyObjectType#setPropertyJSDocInfo
pub(crate) fn set_property_jsdoc_info(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
    info: Option<Arc<JSDocInfo>>,
) {
    if let Some(t) = id.get_referenced_obj_type_internal(reg) {
        t.set_property_jsdoc_info(reg, ast, property_name.clone(), info);
    }
}
// port: ProxyObjectType#getConstructor
pub(crate) fn get_constructor(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .and_then(|t| t.get_constructor(reg))
}
// port: ProxyObjectType#getTemplateTypes
pub(crate) fn get_template_types(id: TypeId, reg: &JSTypeRegistry) -> Option<Vec<TypeId>> {
    id.get_referenced_obj_type_internal(reg)
        .and_then(|t| t.get_template_types(reg))
}
// port: ProxyObjectType#getTemplateParamCount
pub(crate) fn get_template_param_count(id: TypeId, reg: &JSTypeRegistry) -> usize {
    id.get_referenced_type_internal(reg)
        .get_template_param_count(reg)
}
// port: ProxyObjectType#visit
pub(crate) fn visit<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl Visitor<T>,
) -> T {
    visitor.case_proxy_object_type(reg, ast, id)
}
// port: ProxyObjectType#visit(RelationshipVisitor,JSType)
pub(crate) fn visit_relationship<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    id.get_referenced_type_internal(reg)
        .visit_relationship(reg, ast, visitor, that)
}
// port: ProxyObjectType#resolveInternal
pub(crate) fn resolve_internal(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> TypeId {
    let type_ = id
        .get_referenced_type_internal(reg)
        .resolve_with_reporter(reg, ast, reporter);
    id.set_referenced_type(reg, type_);
    id
}
// port: ProxyObjectType#getTypeOfThis
pub(crate) fn get_type_of_this(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_obj_type_internal(reg)
        .and_then(|t| t.get_type_of_this(reg))
}
// port: ProxyObjectType#collapseUnion
pub(crate) fn collapse_union(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
    let type_ = id.get_referenced_type_internal(reg);
    if type_.is_union_type(reg) {
        type_.collapse_union(reg, ast)
    } else {
        Some(id)
    }
}
// port: ProxyObjectType#matchConstraint
pub(crate) fn match_constraint(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    constraint: TypeId,
) {
    id.get_referenced_type_internal(reg)
        .match_constraint(reg, ast, constraint);
}
// port: ProxyObjectType#hasAnyTemplateTypesInternal
pub(crate) fn has_any_template_types_internal(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> bool {
    id.get_referenced_type_internal(reg)
        .has_any_template_types(reg, ast)
}
// port: ProxyObjectType#getTemplateTypeMap
pub(crate) fn get_template_type_map(id: TypeId, reg: &JSTypeRegistry) -> Arc<TemplateTypeMap> {
    id.get_referenced_type_internal(reg)
        .get_template_type_map(reg)
}
// port: ProxyObjectType#testForEquality
pub(crate) fn test_for_equality(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<Tri> {
    id.get_referenced_type_internal(reg)
        .test_for_equality(reg, ast, that)
}
// port: ProxyObjectType#isNativeObjectType
pub(crate) fn is_native_object_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_obj_type_internal(reg)
        .is_some_and(|t| t.is_native_object_type(reg))
}
// port: ProxyObjectType#isStructuralType
pub(crate) fn is_structural_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_structural_type(reg)
}
// port: ProxyObjectType#isNoType
pub(crate) fn is_no_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_no_type(reg)
}
// port: ProxyObjectType#isNoObjectType
pub(crate) fn is_no_object_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_no_object_type(reg)
}
// port: ProxyObjectType#isNoResolvedType
pub(crate) fn is_no_resolved_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg)
        .is_no_resolved_type(reg)
}
// port: ProxyObjectType#isConstructor
pub(crate) fn is_constructor(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_constructor(reg)
}
// port: ProxyObjectType#isNominalType
pub(crate) fn is_nominal_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_nominal_type(reg)
}
// port: ProxyObjectType#isInstanceType
pub(crate) fn is_instance_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_instance_type(reg)
}
// port: ProxyObjectType#isInterface
pub(crate) fn is_interface(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_interface(reg)
}
// port: ProxyObjectType#isOrdinaryFunction
pub(crate) fn is_ordinary_function(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg)
        .is_ordinary_function(reg)
}
// port: ProxyObjectType#isAllType
pub(crate) fn is_all_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg).is_all_type(reg)
}
// port: ProxyObjectType#matchesNumberContext
pub(crate) fn matches_number_context(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg)
        .matches_number_context(reg, ast)
}
// port: ProxyObjectType#matchesStringContext
pub(crate) fn matches_string_context(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg)
        .matches_string_context(reg, ast)
}
// port: ProxyObjectType#matchesSymbolContext
pub(crate) fn matches_symbol_context(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg)
        .matches_symbol_context(reg, ast)
}
// port: ProxyObjectType#matchesObjectContext
pub(crate) fn matches_object_context(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg)
        .matches_object_context(reg, ast)
}
// port: ProxyObjectType#canBeCalled
pub(crate) fn can_be_called(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg).can_be_called(reg, ast)
}
// port: ProxyObjectType#isUnknownType
pub(crate) fn is_unknown_type(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg)
        .is_unknown_type(reg, ast)
}
// port: ProxyObjectType#isCheckedUnknownType
pub(crate) fn is_checked_unknown_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    id.get_referenced_type_internal(reg)
        .is_checked_unknown_type(reg)
}
// port: ProxyObjectType#isNullable
pub(crate) fn is_nullable(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg).is_nullable(reg, ast)
}
// port: ProxyObjectType#isVoidable
pub(crate) fn is_voidable(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg).is_voidable(reg, ast)
}
// port: ProxyObjectType#isStruct
pub(crate) fn is_struct(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg).is_struct(reg, ast)
}
// port: ProxyObjectType#isDict
pub(crate) fn is_dict(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    id.get_referenced_type_internal(reg).is_dict(reg, ast)
}
// port: ProxyObjectType#toMaybeEnumType
pub(crate) fn to_maybe_enum_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg).to_maybe_enum_type(reg)
}
// port: ProxyObjectType#toMaybeRecordType
pub(crate) fn to_maybe_record_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_record_type(reg)
}
// port: ProxyObjectType#toMaybeUnionType
pub(crate) fn to_maybe_union_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_union_type(reg)
}
// port: ProxyObjectType#toMaybeFunctionType
pub(crate) fn to_maybe_function_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_function_type(reg)
}
// port: ProxyObjectType#toMaybeEnumElementType
pub(crate) fn to_maybe_enum_element_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_enum_element_type(reg)
}
// port: ProxyObjectType#toMaybeKnownSymbolType
pub(crate) fn to_maybe_known_symbol_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_known_symbol_type(reg)
}
// port: ProxyObjectType#toMaybeTemplatizedType
pub(crate) fn to_maybe_templatized_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_templatized_type(reg)
}
// port: ProxyObjectType#toMaybeTemplateType
pub(crate) fn to_maybe_template_type(id: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    id.get_referenced_type_internal(reg)
        .to_maybe_template_type(reg)
}
