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
//   src/com/google/javascript/rhino/jstype/ValueType.java.

use crate::{
    JSTypeRegistry, TypeId,
    js_type::{HasPropertyKind, JSType},
    property::PropertyKey,
};
use closure_rhino::node::Ast;

pub struct ValueTypeData;

// port: ValueType#ValueType
pub(crate) fn initialize(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) {
    t.eagerly_resolve_to_self(reg, ast);
}
// port: ValueType#appendTo
pub(crate) fn append_to(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    _ast: &Ast,
    sb: &mut crate::type_string_builder::TypeStringBuilder,
) {
    sb.append(&t.get_display_name(reg).unwrap());
}
// port: ValueType#getDisplayName
pub fn get_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<String> {
    panic!("AbstractMethodError")
}
// port: ValueType#hasDisplayName
pub(crate) fn has_display_name(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: ValueType#visit
pub(crate) fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_value_type(reg, ast, t, that)
}
// port: ValueType#getPropertyKind
pub(crate) fn get_property_kind(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: PropertyKey,
    autobox: bool,
) -> HasPropertyKind {
    if autobox && t.is_boxable_scalar(reg) {
        t.autoboxes_to(reg)
            .unwrap()
            .get_property_kind_with_autobox(reg, ast, name, autobox)
    } else {
        HasPropertyKind::ABSENT
    }
}
// port: ValueType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(t: TypeId, _reg: &JSTypeRegistry) -> i32 {
    t.0 as i32
}
// port: ValueType#resolveInternal
pub(crate) fn resolve_internal(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    panic!("AssertionError")
}
