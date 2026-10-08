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
//   src/com/google/javascript/rhino/jstype/ArrowType.java.

use crate::{
    TypeId,
    boolean_literal_set::BooleanLiteralSet,
    function_type::Parameter,
    js_type::{JSType, JSTypeKind},
    js_type_class::JSTypeClass,
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    type_string_builder::{TypeStringBuilder, TypeStringItem},
};
use closure_rhino::node::Ast;

#[derive(Clone, Debug)]
pub struct ArrowTypeData {
    pub(crate) parameter_list: Vec<Parameter>,
    pub(crate) return_type: TypeId,
    pub(crate) return_type_inferred: bool,
}
pub struct ArrowType;
impl ArrowType {
    // port: ArrowType#ArrowType
    pub fn new(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        parameters: Option<Vec<Parameter>>,
        return_type: Option<TypeId>,
        return_type_inferred: bool,
    ) -> TypeId {
        let unknown = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let parameter_list =
            parameters.unwrap_or_else(|| vec![Parameter::create(unknown, false, true)]);
        let return_type = return_type.unwrap_or(unknown);
        let t = reg.alloc(
            JSTypeKind::Arrow(ArrowTypeData {
                parameter_list,
                return_type,
                return_type_inferred,
            }),
            None,
        );
        reg.finish_construction(t, ast);
        t
    }
}
pub fn data(t: TypeId, reg: &JSTypeRegistry) -> &ArrowTypeData {
    match &reg.data(t).kind {
        JSTypeKind::Arrow(d) => d,
        _ => panic!("ClassCastException"),
    }
}
pub fn data_mut(t: TypeId, reg: &mut JSTypeRegistry) -> &mut ArrowTypeData {
    match &mut reg.data_mut(t).kind {
        JSTypeKind::Arrow(d) => d,
        _ => panic!("ClassCastException"),
    }
}
// port: ArrowType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::ARROW
}
// port: ArrowType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    let d = data(t, reg);
    let mut hash_code = d.return_type.hash_code(reg);
    for param in &d.parameter_list {
        hash_code = hash_code
            .wrapping_mul(31)
            .wrapping_add(param.get_jstype().hash_code(reg));
    }
    hash_code
}
// port: ArrowType#getReturnType
pub fn get_return_type(t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    data(t, reg).return_type
}
// port: ArrowType#getParameterList
pub fn get_parameter_list(t: TypeId, reg: &JSTypeRegistry) -> Vec<Parameter> {
    data(t, reg).parameter_list.clone()
}
// port: ArrowType#getLeastSupertype
pub fn get_least_supertype(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _that: TypeId,
) -> TypeId {
    panic!("UnsupportedOperationException")
}
// port: ArrowType#getGreatestSubtype
pub fn get_greatest_subtype(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _that: TypeId,
) -> TypeId {
    panic!("UnsupportedOperationException")
}
// port: ArrowType#testForEquality
pub fn test_for_equality(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _that: TypeId,
) -> Option<closure_rhino::jscomp_base::Tri> {
    panic!("UnsupportedOperationException")
}
// port: ArrowType#visit
pub fn visit<T>(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    panic!("UnsupportedOperationException")
}
// port: ArrowType#visit
pub fn visit_relationship<T>(
    _t: TypeId,
    _reg: &mut JSTypeRegistry,
    _ast: &Ast,
    _visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    _that: TypeId,
) -> T {
    panic!("UnsupportedOperationException")
}
// port: ArrowType#getPossibleToBooleanOutcomes
pub fn get_possible_to_boolean_outcomes(_t: TypeId, _reg: &JSTypeRegistry) -> BooleanLiteralSet {
    BooleanLiteralSet::TRUE
}
// port: ArrowType#resolveInternal
pub fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    let return_type = data(t, reg)
        .return_type
        .resolve_with_reporter(reg, ast, reporter);
    data_mut(t, reg).return_type = return_type;
    for param in data(t, reg).parameter_list.clone() {
        param.get_jstype().resolve_with_reporter(reg, ast, reporter);
    }
    t
}
// port: ArrowType#hasUnknownParamsOrReturn
pub fn has_unknown_params_or_return(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for param in data(t, reg).parameter_list.clone() {
        if param.get_jstype().is_unknown_type(reg, ast) {
            return true;
        }
    }
    data(t, reg).return_type.is_unknown_type(reg, ast)
}
// port: ArrowType#appendTo
pub fn append_to(_t: TypeId, _reg: &mut JSTypeRegistry, _ast: &Ast, sb: &mut TypeStringBuilder) {
    sb.append("[ArrowType]");
}
// port: ArrowType#hasAnyTemplateTypesInternal
pub fn has_any_template_types_internal(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    data(t, reg).return_type.has_any_template_types(reg, ast)
        || has_templated_parameter_type(t, reg, ast)
}
// port: ArrowType#hasTemplatedParameterType
fn has_templated_parameter_type(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    for param in data(t, reg).parameter_list.clone() {
        if param.get_jstype().has_any_template_types(reg, ast) {
            return true;
        }
    }
    false
}
// port: ArrowType#toString
pub fn to_string(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
    let mut sb = TypeStringBuilder::new(false);
    sb.append("(");
    let params = data(t, reg).parameter_list.clone();
    sb.append_all_objects(reg, ast, params.iter().map(TypeStringItem::Parameter), ",");
    sb.append(") -> ");
    let return_type = data(t, reg).return_type;
    sb.append_type(reg, ast, return_type);
    sb.build()
}
