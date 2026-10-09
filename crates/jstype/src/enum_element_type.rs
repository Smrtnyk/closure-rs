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
//   src/com/google/javascript/rhino/jstype/EnumElementType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{HasPropertyKind, JSType, JSTypeKind},
    object_type::{ObjectType, ObjectTypeData},
    property::PropertyKey,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    jscomp_base::Tri,
    node::{Ast, NodeId},
};

pub(crate) struct EnumElementTypeData {
    pub(crate) object: ObjectTypeData,
    pub(crate) primitive_type: TypeId,
    pub(crate) primitive_object_type: Option<TypeId>,
    pub(crate) name: Option<JsString>,
    pub(crate) enum_type: TypeId,
}
// port: EnumElementType#EnumElementType
pub(crate) fn new(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    element_type: TypeId,
    name: Option<JsString>,
    enum_type: TypeId,
) -> TypeId {
    let primitive_object_type = element_type.to_object_type(reg);
    let t = reg.alloc(
        JSTypeKind::EnumElement(EnumElementTypeData {
            object: ObjectTypeData::new(),
            primitive_type: element_type,
            primitive_object_type,
            name,
            enum_type,
        }),
        None,
    );
    reg.finish_construction(t, ast);
    t
}
pub trait EnumElementType {
    fn get_enum_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn get_primitive_type(self, reg: &JSTypeRegistry) -> TypeId;
}
impl EnumElementType for TypeId {
    // port: EnumElementType#getEnumType
    fn get_enum_type(self, reg: &JSTypeRegistry) -> TypeId {
        data(self, reg).enum_type
    }
    // port: EnumElementType#getPrimitiveType
    fn get_primitive_type(self, reg: &JSTypeRegistry) -> TypeId {
        get_primitive_type(self, reg)
    }
}
pub(crate) fn data(t: TypeId, reg: &JSTypeRegistry) -> &EnumElementTypeData {
    match &reg.data(t).kind {
        JSTypeKind::EnumElement(d) => d,
        _ => panic!("ClassCastException"),
    }
}
// port: EnumElementType#getTypeClass
pub(crate) fn get_type_class(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::ENUM_ELEMENT
}
// port: EnumElementType#getPropertyKind
pub(crate) fn get_property_kind(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: PropertyKey,
    autobox: bool,
) -> HasPropertyKind {
    get_primitive_type(t, reg).get_property_kind_with_autobox(reg, ast, name, autobox)
}
// port: EnumElementType#getPropertyMap
pub(crate) fn get_property_map(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> &crate::property_map::PropertyMap {
    match data(t, reg).primitive_object_type {
        Some(primitive) => primitive.get_property_map(reg),
        None => crate::property_map::PropertyMap::immutable_empty_map(),
    }
}
// port: EnumElementType#toMaybeEnumElementType
pub(crate) fn to_maybe_enum_element_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
// port: EnumElementType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<Tri> {
    get_primitive_type(t, reg).test_for_equality(reg, ast, that)
}
// port: EnumElementType#isNominalType
pub(crate) fn is_nominal_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.has_reference_name(reg)
}
// port: EnumElementType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    if !t.has_reference_name(reg) {
        2
    } else {
        crate::named_type::nominal_hash_code(t, reg)
    }
}
// port: EnumElementType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    let primitive = get_primitive_type(t, reg);
    if sb.is_for_annotations() {
        sb.append_type(reg, ast, primitive);
    } else {
        sb.append(
            &get_reference_name(t, reg)
                .unwrap_or_else(|| "null".into())
                .to_string_lossy(),
        )
        .append("<")
        .append_type(reg, ast, primitive)
        .append(">");
    }
}
// port: EnumElementType#getReferenceName
pub(crate) fn get_reference_name(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    data(t, reg).name.clone()
}
// port: EnumElementType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_enum_element_type(reg, ast, t)
}
// port: EnumElementType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_enum_element_type(reg, ast, t, that)
}
// port: EnumElementType#defineProperty
pub fn define_property(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _name: &PropertyKey,
    _type: TypeId,
    _inferred: bool,
    _node: Option<NodeId>,
) -> bool {
    true
}
// port: EnumElementType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: EnumElementType#findPropertyTypeWithoutConsideringTemplateTypes
pub(crate) fn find_property_type_without_considering_template_types(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: PropertyKey,
) -> Option<TypeId> {
    get_primitive_type(t, reg).find_property_type(reg, ast, name)
}
// port: EnumElementType#getConstructor
pub fn get_constructor(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    data(t, reg)
        .primitive_object_type
        .and_then(|t| t.get_constructor(reg))
}
// port: EnumElementType#autoboxesTo
pub(crate) fn autoboxes_to(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    get_primitive_type(t, reg).autoboxes_to(reg)
}
// port: EnumElementType#getPrimitiveType
pub(crate) fn get_primitive_type(t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    data(t, reg).primitive_type
}
// port: EnumElementType#getGreatestSubtype
pub fn get_greatest_subtype(
    element: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<TypeId> {
    let data = data(element, reg);
    let primitive = data.primitive_type;
    let name = data.name.clone();
    let enum_type = data.enum_type;
    let meet_primitive = primitive.get_greatest_subtype(reg, ast, that);
    if meet_primitive.is_empty_type(reg) {
        None
    } else {
        Some(new(reg, ast, meet_primitive, name, enum_type))
    }
}
// port: EnumElementType#resolveInternal
pub(crate) fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> TypeId {
    let primitive = get_primitive_type(t, reg).resolve_with_reporter(reg, ast, reporter);
    let primitive_object = primitive.to_object_type(reg);
    if let JSTypeKind::EnumElement(d) = &mut reg.data_mut(t).kind {
        d.primitive_type = primitive;
        d.primitive_object_type = primitive_object;
    }
    t
}
// port: EnumElementType#matchesNumberContext
pub(crate) fn matches_number_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).matches_number_context(reg, ast)
}
// port: EnumElementType#matchesStringContext
pub(crate) fn matches_string_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).matches_string_context(reg, ast)
}
// port: EnumElementType#matchesObjectContext
pub(crate) fn matches_object_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).matches_object_context(reg, ast)
}
// port: EnumElementType#matchesSymbolContext
pub(crate) fn matches_symbol_context(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).matches_symbol_context(reg, ast)
}
// port: EnumElementType#canBeCalled
pub(crate) fn can_be_called(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).can_be_called(reg, ast)
}
// port: EnumElementType#isObject
pub(crate) fn is_object(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).is_object(reg, ast)
}
// port: EnumElementType#isNullable
pub(crate) fn is_nullable(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).is_nullable(reg, ast)
}
// port: EnumElementType#isVoidable
pub(crate) fn is_voidable(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    get_primitive_type(t, reg).is_voidable(reg, ast)
}
