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
//   src/com/google/javascript/rhino/jstype/VoidType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
};
use closure_rhino::node::Ast;
pub struct VoidTypeData {
    pub value: crate::value_type::ValueTypeData,
}
pub struct VoidType;
impl VoidType {
    // port: VoidType#VoidType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let t = reg.alloc(
            JSTypeKind::Void(VoidTypeData {
                value: crate::value_type::ValueTypeData,
            }),
            None,
        );
        crate::value_type::initialize(t, reg, ast);
        t
    }
}
// port: VoidType#getTypeClass
pub(crate) fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::VOID
}
// port: VoidType#matchesNumberContext
pub(crate) fn matches_number_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    false
}
// port: VoidType#matchesObjectContext
pub(crate) fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    false
}
// port: VoidType#matchesStringContext
pub(crate) fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: VoidType#isVoidType
pub(crate) fn is_void_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: VoidType#isVoidable
pub(crate) fn is_voidable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: VoidType#isExplicitlyVoidable
pub(crate) fn is_explicitly_voidable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: VoidType#getDisplayName
pub(crate) fn get_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<String> {
    Some("undefined".to_string())
}
// port: VoidType#getPossibleToBooleanOutcomes
pub(crate) fn get_possible_to_boolean_outcomes(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::boolean_literal_set::BooleanLiteralSet {
    crate::boolean_literal_set::BooleanLiteralSet::FALSE
}
// port: VoidType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    let _ = t;
    visitor.case_void_type(reg, ast)
}
// port: VoidType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<closure_rhino::jscomp_base::Tri> {
    let _ = (t, ast, that);
    if crate::js_type::test_for_equality_helper(t, reg, ast, that)
        == Some(closure_rhino::jscomp_base::Tri::UNKNOWN)
    {
        return Some(closure_rhino::jscomp_base::Tri::UNKNOWN);
    }
    let null = reg.get_native_type(crate::JSTypeNative::NULL_TYPE);
    if that.is_subtype_of(reg, ast, t) || that.is_subtype_of(reg, ast, null) {
        Some(closure_rhino::jscomp_base::Tri::TRUE)
    } else {
        Some(closure_rhino::jscomp_base::Tri::FALSE)
    }
}
// port: VoidType#restrictByNotNullOrUndefined
pub(crate) fn restrict_by_not_null_or_undefined(_t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    reg.get_native_type(crate::JSTypeNative::NO_TYPE)
}
// port: VoidType#restrictByNotUndefined
pub(crate) fn restrict_by_not_undefined(_t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    reg.get_native_type(crate::JSTypeNative::NO_TYPE)
}
