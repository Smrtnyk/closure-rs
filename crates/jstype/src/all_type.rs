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
//   src/com/google/javascript/rhino/jstype/AllType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
};
use closure_rhino::node::Ast;
pub struct AllTypeData;
pub struct AllType;
impl AllType {
    // port: AllType#AllType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let t = reg.alloc(JSTypeKind::All(AllTypeData), None);
        t.eagerly_resolve_to_self(reg, ast);
        t
    }
}
// port: AllType#getTypeClass
pub(crate) fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::ALL
}
// port: AllType#isAllType
pub(crate) fn is_all_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#matchesStringContext
pub(crate) fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#matchesObjectContext
pub(crate) fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#getDisplayName
pub(crate) fn get_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<String> {
    Some("<Any Type>".to_string())
}
// port: AllType#hasDisplayName
pub(crate) fn has_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#getPossibleToBooleanOutcomes
pub(crate) fn get_possible_to_boolean_outcomes(
    _t: TypeId,
    _reg: &JSTypeRegistry,
) -> crate::boolean_literal_set::BooleanLiteralSet {
    crate::boolean_literal_set::BooleanLiteralSet::BOTH
}
// port: AllType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(t: TypeId, _reg: &JSTypeRegistry) -> i32 {
    t.0 as i32
}
// port: AllType#isNullable
pub(crate) fn is_nullable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#isVoidable
pub(crate) fn is_voidable(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: AllType#visit
pub(crate) fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    let _ = t;
    visitor.case_all_type(reg, ast)
}
// port: AllType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    let _ = t;
    visitor.case_all_type(reg, ast, that)
}
// port: AllType#testForEquality
pub(crate) fn test_for_equality(
    t: TypeId,
    _reg: &mut JSTypeRegistry,
    ast: &Ast,
    that: TypeId,
) -> Option<closure_rhino::jscomp_base::Tri> {
    let _ = (t, ast, that);
    Some(closure_rhino::jscomp_base::Tri::UNKNOWN)
}
// port: AllType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    sb: &mut crate::type_string_builder::TypeStringBuilder,
) {
    let _ = t;
    sb.append("*");
}
// port: AllType#resolveInternal
pub(crate) fn resolve_internal(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    panic!("AssertionError")
}
