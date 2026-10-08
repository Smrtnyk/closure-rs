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
//   src/com/google/javascript/rhino/jstype/StringType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
};
use closure_rhino::node::Ast;
pub struct StringTypeData {
    pub value: crate::value_type::ValueTypeData,
}
pub struct StringType;
impl StringType {
    // port: StringType#StringType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let t = reg.alloc(
            JSTypeKind::String(StringTypeData {
                value: crate::value_type::ValueTypeData,
            }),
            None,
        );
        crate::value_type::initialize(t, reg, ast);
        t
    }
}
// port: StringType#getTypeClass
pub(crate) fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::STRING
}
// port: StringType#isStringValueType
pub(crate) fn is_string_value_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: StringType#matchesNumberContext
pub(crate) fn matches_number_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: StringType#matchesStringContext
pub(crate) fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: StringType#matchesObjectContext
pub(crate) fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: StringType#getDisplayName
pub(crate) fn get_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<String> {
    Some("string".to_string())
}
// port: StringType#getPossibleToBooleanOutcomes
pub(crate) fn get_possible_to_boolean_outcomes(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::boolean_literal_set::BooleanLiteralSet {
    crate::boolean_literal_set::BooleanLiteralSet::BOTH
}
// port: StringType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    let _ = t;
    visitor.case_string_type(reg, ast)
}
// port: StringType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<closure_rhino::jscomp_base::Tri> {
    let _ = (t, ast, that);
    if let Some(result) = crate::js_type::test_for_equality_helper(t, reg, ast, that) {
        return Some(result);
    }
    if that.is_unknown_type(reg, ast)
        || [
            crate::JSTypeNative::OBJECT_TYPE,
            crate::JSTypeNative::NUMBER_TYPE,
            crate::JSTypeNative::STRING_TYPE,
            crate::JSTypeNative::BOOLEAN_TYPE,
            crate::JSTypeNative::BIGINT_TYPE,
        ]
        .iter()
        .any(|&n| {
            let native = reg.get_native_type(n);
            that.is_subtype_of(reg, ast, native)
        })
    {
        Some(closure_rhino::jscomp_base::Tri::UNKNOWN)
    } else {
        Some(closure_rhino::jscomp_base::Tri::FALSE)
    }
}
// port: StringType#autoboxesTo
pub(crate) fn autoboxes_to(_t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(reg.get_native_type(crate::JSTypeNative::STRING_OBJECT_TYPE))
}
