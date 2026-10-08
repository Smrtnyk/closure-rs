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
//   src/com/google/javascript/rhino/jstype/UnknownType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
};
use closure_rhino::node::Ast;
pub struct UnknownTypeData {
    pub(crate) object: crate::object_type::ObjectTypeData,
    pub(crate) is_checked: bool,
}
pub struct UnknownType;
impl UnknownType {
    // port: UnknownType#UnknownType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast, is_checked: bool) -> TypeId {
        let t = reg.alloc(
            JSTypeKind::Unknown(UnknownTypeData {
                object: crate::object_type::ObjectTypeData::new(),
                is_checked,
            }),
            None,
        );
        t.eagerly_resolve_to_self(reg, ast);
        t
    }
}
// port: UnknownType#getTypeClass
pub(crate) fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::UNKNOWN
}
// port: UnknownType#isUnknownType
pub(crate) fn is_unknown_type(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast) -> bool {
    true
}
// port: UnknownType#isCheckedUnknownType
pub(crate) fn is_checked_unknown_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    match &reg.data(t).kind {
        JSTypeKind::Unknown(d) => d.is_checked,
        _ => panic!("ClassCastException"),
    }
}
// port: UnknownType#canBeCalled
pub(crate) fn can_be_called(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#matchesNumberContext
pub(crate) fn matches_number_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#matchesObjectContext
pub(crate) fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#matchesStringContext
pub(crate) fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#matchesSymbolContext
pub(crate) fn matches_symbol_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#isNullable
pub(crate) fn is_nullable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#isVoidable
pub(crate) fn is_voidable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#getReferenceName
pub(crate) fn get_reference_name(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> Option<closure_rhino::js_string::JsString> {
    Some(closure_rhino::js_string::JsString::from(
        if is_checked_unknown_type(t, reg) {
            "??"
        } else {
            "?"
        },
    ))
}
// port: UnknownType#getDisplayName
pub fn get_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<String> {
    Some("Unknown".to_string())
}
// port: UnknownType#hasDisplayName
pub(crate) fn has_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: UnknownType#getPossibleToBooleanOutcomes
pub(crate) fn get_possible_to_boolean_outcomes(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::boolean_literal_set::BooleanLiteralSet {
    crate::boolean_literal_set::BooleanLiteralSet::BOTH
}
// port: UnknownType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(t: TypeId, _reg: &JSTypeRegistry) -> i32 {
    t.0 as i32
}
// port: UnknownType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    let _ = t;
    visitor.case_unknown_type(reg, ast)
}
// port: UnknownType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    let _ = t;
    visitor.case_unknown_type(reg, ast, t, that)
}
// port: UnknownType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    _reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<closure_rhino::jscomp_base::Tri> {
    let _ = (t, ast, that);
    Some(closure_rhino::jscomp_base::Tri::UNKNOWN)
}
// port: UnknownType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    _ast: &Ast,
    sb: &mut crate::type_string_builder::TypeStringBuilder,
) {
    let _ = t;
    sb.append(&get_reference_name(t, reg).unwrap().to_string_lossy());
}
// port: UnknownType#resolveInternal
pub(crate) fn resolve_internal(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    panic!("AssertionError")
}
// port: UnknownType#defineProperty
pub fn define_property(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _name: crate::property::PropertyKey,
    _typ: TypeId,
    _inferred: bool,
    _node: Option<closure_rhino::node::NodeId>,
) -> bool {
    true
}
// port: UnknownType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
    None
}
// port: UnknownType#getConstructor
pub fn get_constructor(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
