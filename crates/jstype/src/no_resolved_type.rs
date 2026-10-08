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
//   src/com/google/javascript/rhino/jstype/NoResolvedType.java.

use crate::{
    TypeId,
    boolean_literal_set::BooleanLiteralSet,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectTypeData,
    property::PropertyKey,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    js_string::JsString,
    jscomp_base::Tri,
    node::{Ast, NodeId},
};

#[derive(Clone, Debug)]
pub struct NoResolvedTypeData {
    pub(crate) object: ObjectTypeData,
    pub(crate) reference_name: JsString,
    pub(crate) template_types: Option<Vec<TypeId>>,
}
pub struct NoResolvedType;
impl NoResolvedType {
    // port: NoResolvedType#NoResolvedType
    pub fn new(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reference_name: impl Into<JsString>,
        template_types: Option<Vec<TypeId>>,
    ) -> TypeId {
        let t = reg.alloc(
            JSTypeKind::NoResolved(NoResolvedTypeData {
                object: ObjectTypeData::new(),
                reference_name: reference_name.into(),
                template_types,
            }),
            None,
        );
        t.eagerly_resolve_to_self(reg, ast);
        t
    }
}
// port: NoResolvedType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::NO_RESOLVED
}
// port: NoResolvedType#getReferenceName
pub fn get_reference_name(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    match &reg.data(t).kind {
        JSTypeKind::NoResolved(d) => Some(d.reference_name.clone()),
        _ => panic!("ClassCastException"),
    }
}
// port: NoResolvedType#getTemplateTypes
pub fn get_template_types(t: TypeId, reg: &JSTypeRegistry) -> Option<Vec<TypeId>> {
    match &reg.data(t).kind {
        JSTypeKind::NoResolved(d) => d.template_types.clone(),
        _ => panic!("ClassCastException"),
    }
}
// port: NoResolvedType#isNoResolvedType
pub fn is_no_resolved_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: NoResolvedType#getPossibleToBooleanOutcomes
pub fn get_possible_to_boolean_outcomes(_t: TypeId, _reg: &JSTypeRegistry) -> BooleanLiteralSet {
    BooleanLiteralSet::BOTH
}
// port: NoResolvedType#testForEquality
pub fn test_for_equality(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _that: TypeId,
) -> Option<Tri> {
    Some(Tri::UNKNOWN)
}
// port: NoResolvedType#resolveInternal
pub fn resolve_internal(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    panic!("AssertionError")
}
// port: NoResolvedType#getConstructor
pub fn get_constructor(_t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    None
}
// port: NoResolvedType#getImplicitPrototype
pub fn get_implicit_prototype(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast) -> Option<TypeId> {
    None
}
// port: NoResolvedType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, _reg: &JSTypeRegistry) -> i32 {
    t.0 as i32
}
// port: NoResolvedType#defineProperty
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
// port: NoResolvedType#appendTo
pub fn append_to(t: TypeId, reg: &mut JSTypeRegistry, _ast: &Ast, sb: &mut TypeStringBuilder) {
    if sb.is_for_annotations() {
        sb.append("?");
    } else {
        sb.append(&format!(
            "NoResolvedType<{}>",
            get_reference_name(t, reg).unwrap()
        ));
    }
}
