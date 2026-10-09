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
//   src/com/google/javascript/rhino/jstype/TemplateType.java.

use crate::TypeId;
use crate::contains_upper_bound_super_type_visitor::{ContainsUpperBoundSuperTypeVisitor, Result};
use crate::js_type::{JSType, JSTypeKind, TypeValidator};
use crate::js_type_class::JSTypeClass;
use crate::js_type_native::JSTypeNative;
use crate::js_type_registry::JSTypeRegistry;
use crate::proxy_object_type::{ProxyObjectType, ProxyObjectTypeData};
use crate::relationship_visitor::RelationshipVisitor;
use crate::type_string_builder::TypeStringBuilder;
use crate::visitor::Visitor;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};

#[derive(Clone, Debug)]
pub(crate) struct TemplateTypeData {
    pub(crate) proxy: ProxyObjectTypeData,
    pub(crate) name: JsString,
    pub(crate) bound: TypeId,
    pub(crate) type_transformation: Option<NodeId>,
}

// port: TemplateType#TemplateType
pub(crate) fn create(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: impl Into<JsString>,
    bound: Option<TypeId>,
    type_transformation: Option<NodeId>,
) -> TypeId {
    let bound = bound.unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE));
    let data = TemplateTypeData {
        proxy: ProxyObjectTypeData::new(reg, bound),
        name: name.into(),
        bound,
        type_transformation,
    };
    let id = reg.alloc(JSTypeKind::Template(data), None);
    reg.finish_construction(id, ast);
    id
}

pub trait TemplateType {
    // port: TemplateType#isTypeTransformation
    fn is_type_transformation(self, reg: &JSTypeRegistry) -> bool;
    // port: TemplateType#getTypeTransformation
    fn get_type_transformation(self, reg: &JSTypeRegistry) -> Option<NodeId>;
    // port: TemplateType#getBound
    fn get_bound(self, reg: &JSTypeRegistry) -> TypeId;
    // port: TemplateType#setBound
    fn set_bound(self, reg: &mut JSTypeRegistry, bound: TypeId);
    // port: TemplateType#containsCycle
    fn contains_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
}

impl TemplateType for TypeId {
    // port: TemplateType#isTypeTransformation
    fn is_type_transformation(self, reg: &JSTypeRegistry) -> bool {
        data(self, reg).type_transformation.is_some()
    }
    // port: TemplateType#getTypeTransformation
    fn get_type_transformation(self, reg: &JSTypeRegistry) -> Option<NodeId> {
        data(self, reg).type_transformation
    }
    // port: TemplateType#getBound
    fn get_bound(self, reg: &JSTypeRegistry) -> TypeId {
        data(self, reg).bound
    }
    // port: TemplateType#setBound
    fn set_bound(self, reg: &mut JSTypeRegistry, bound: TypeId) {
        match &mut reg.data_mut(self).kind {
            JSTypeKind::Template(data) => data.bound = bound,
            _ => panic!("ClassCastException"),
        }
        self.set_referenced_type(reg, bound);
    }
    // port: TemplateType#containsCycle
    fn contains_cycle(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        let mut visitor = ContainsUpperBoundSuperTypeVisitor::new(None);
        self.visit(reg, ast, &mut visitor) == Result::CYCLE
    }
}

pub(crate) fn data(id: TypeId, reg: &JSTypeRegistry) -> &TemplateTypeData {
    match &reg.data(id).kind {
        JSTypeKind::Template(data) => data,
        _ => panic!("ClassCastException"),
    }
}
// port: TemplateType#getTypeClass
pub(crate) fn get_type_class(_id: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::TEMPLATE
}
// port: TemplateType#getReferenceName
pub(crate) fn get_reference_name(id: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    Some(data(id, reg).name.clone())
}
// port: TemplateType#appendTo
pub(crate) fn append_to(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    let name = data(id, reg).name.to_string_lossy();
    let bound = data(id, reg).bound;
    sb.append(&name);
    if bound != reg.get_native_type(JSTypeNative::UNKNOWN_TYPE) {
        sb.append(" extends ");
        sb.append_type(reg, ast, bound);
    }
}
// port: TemplateType#toMaybeTemplateType
pub(crate) fn to_maybe_template_type(id: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(id)
}
// port: TemplateType#hasAnyTemplateTypesInternal
pub(crate) fn has_any_template_types_internal(_id: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: TemplateType#visit
pub(crate) fn visit<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl Visitor<T>,
) -> T {
    visitor.case_template_type(reg, ast, id)
}
// port: TemplateType#visit
pub(crate) fn visit_relationship<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_template_type(reg, ast, id, that)
}
// port: TemplateType#setValidator
pub(crate) fn set_validator(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    validator: TypeValidator,
) -> bool {
    validator(id, reg, ast)
}
// port: TemplateType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(id: TypeId, _reg: &JSTypeRegistry) -> i32 {
    id.0 as i32
}
