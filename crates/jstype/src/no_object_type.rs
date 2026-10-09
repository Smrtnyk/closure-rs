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
//   src/com/google/javascript/rhino/jstype/NoObjectType.java.

use crate::{
    TypeId,
    function_type::{FunctionTypeBuilder, FunctionTypeData, Kind},
    js_type::JSTypeKind,
    js_type_class::JSTypeClass,
    js_type_registry::JSTypeRegistry,
    property::PropertyKey,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct NoObjectTypeData {
    pub(crate) function: FunctionTypeData,
}
pub struct NoObjectType;
impl NoObjectType {
    // port: NoObjectType#NoObjectType
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        crate::function_type::new_with_kind(
            reg,
            ast,
            FunctionTypeBuilder::new()
                .with_kind(Kind::NONE)
                .with_returns_own_instance_type()
                .for_native_type(),
            |function| JSTypeKind::NoObject(NoObjectTypeData { function }),
            true,
        )
    }
}
// port: NoObjectType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::NO_OBJECT
}
// port: NoObjectType#toMaybeFunctionType
pub fn to_maybe_function_type(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: NoObjectType#isNoObjectType
pub fn is_no_object_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoObjectType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
    None
}
// port: NoObjectType#getReferenceName
pub fn get_reference_name(_t: TypeId, _reg: &JSTypeRegistry) -> Option<JsString> {
    None
}
// port: NoObjectType#matchesNumberContext
pub fn matches_number_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoObjectType#matchesObjectContext
pub fn matches_object_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoObjectType#matchesStringContext
pub fn matches_string_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoObjectType#matchesSymbolContext
pub fn matches_symbol_context(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoObjectType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, _reg: &JSTypeRegistry) -> i32 {
    t.0 as i32
}
// port: NoObjectType#defineProperty
pub fn define_property(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _name: &PropertyKey,
    _type_: TypeId,
    _inferred: bool,
    _node: Option<NodeId>,
) -> bool {
    true
}
// port: NoObjectType#setPropertyJSDocInfo
pub fn set_property_jsdoc_info(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _name: &PropertyKey,
    _info: Option<Arc<JSDocInfo>>,
) {
}
// port: NoObjectType#visit
pub fn visit<T>(
    _t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_no_object_type(reg, ast)
}
// port: NoObjectType#visit
pub fn visit_relationship<T>(
    _t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_no_object_type(reg, ast, that)
}
// port: NoObjectType#appendTo
pub fn append_to(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast, sb: &mut TypeStringBuilder) {
    sb.append(if sb.is_for_annotations() {
        "?"
    } else {
        "NoObject"
    });
}
// port: NoObjectType#getConstructor
pub fn get_constructor(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: NoObjectType#resolveInternal
pub fn resolve_internal(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    panic!("AssertionError")
}
