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
//   src/com/google/javascript/rhino/jstype/TemplatizedType.java.

use crate::TypeId;
use crate::js_type::{JSType, JSTypeKind};
use crate::js_type_class::JSTypeClass;
use crate::js_type_native::JSTypeNative;
use crate::js_type_registry::JSTypeRegistry;
use crate::object_type::ObjectType;
use crate::property::PropertyKey;
use crate::proxy_object_type::{ProxyObjectType, ProxyObjectTypeData};
use crate::relationship_visitor::RelationshipVisitor;
use crate::template_type_map::TemplateTypeMap;
use crate::template_type_replacer::TemplateTypeReplacer;
use crate::type_string_builder::TypeStringBuilder;
use crate::visitor::Visitor;
use closure_rhino::node::Ast;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct TemplatizedTypeData {
    pub(crate) proxy: ProxyObjectTypeData,
    pub(crate) template_types: Vec<TypeId>,
    pub(crate) is_specialized_only_with_unknown: bool,
    pub(crate) replacer: TemplateTypeReplacer,
}

// port: TemplatizedType#TemplatizedType
pub(crate) fn create(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    object_type: TypeId,
    template_types: &[TypeId],
) -> TypeId {
    let map = object_type
        .get_template_type_map(reg)
        .copy_filled_with_values(reg, ast, template_types);
    let proxy = ProxyObjectTypeData::new(reg, object_type);
    let mut builder = Vec::new();
    let mut maybe_is_specialized_only_with_unknown = true;
    for newly_filled_template_key in object_type.get_type_parameters(reg) {
        let resolved_type = map.get_resolved_template_type(reg, ast, newly_filled_template_key);
        builder.push(resolved_type);
        if maybe_is_specialized_only_with_unknown {
            maybe_is_specialized_only_with_unknown = reg
                .get_native_type(JSTypeNative::UNKNOWN_TYPE)
                .equals(reg, ast, resolved_type);
        }
    }
    let replacer = TemplateTypeReplacer::for_partial_replacement(map.clone());
    let data = TemplatizedTypeData {
        proxy,
        template_types: builder,
        is_specialized_only_with_unknown: maybe_is_specialized_only_with_unknown,
        replacer,
    };
    let id = reg.alloc(JSTypeKind::Templatized(data), Some(map));
    reg.finish_construction(id, ast);
    id
}

pub trait TemplatizedType {
    // port: TemplatizedType#wrapsSameRawType
    fn wraps_same_raw_type(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool;
    // port: TemplatizedType#getGreatestSubtypeHelper
    fn get_greatest_subtype_helper(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        raw_that: TypeId,
    ) -> TypeId;
    // port: TemplatizedType#getReferencedType
    fn get_referenced_type(self, reg: &JSTypeRegistry) -> TypeId;
}
impl TemplatizedType for TypeId {
    // port: TemplatizedType#wrapsSameRawType
    fn wraps_same_raw_type(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool {
        that.is_templatized_type(reg)
            && self.get_referenced_type_internal(reg).equals(
                reg,
                ast,
                that.get_referenced_type_internal(reg),
            )
    }
    // port: TemplatizedType#getGreatestSubtypeHelper
    fn get_greatest_subtype_helper(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        raw_that: TypeId,
    ) -> TypeId {
        if !self.wraps_same_raw_type(reg, ast, raw_that) {
            if self.is_subtype_of(reg, ast, raw_that) {
                return self;
            }
            if raw_that.is_subtype_of(reg, ast, self) {
                return raw_that;
            }
            if self.is_object(reg, ast) && raw_that.is_object(reg, ast) {
                return reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
            }
            return reg.get_native_type(JSTypeNative::NO_TYPE);
        }
        if self.equals(reg, ast, raw_that) {
            self
        } else {
            self.get_referenced_obj_type_internal(reg).expect("")
        }
    }
    // port: TemplatizedType#getReferencedType
    fn get_referenced_type(self, reg: &JSTypeRegistry) -> TypeId {
        self.get_referenced_obj_type_internal(reg).expect("")
    }
}

// port: TemplatizedType#getGreatestSubtypeHelper
pub(crate) fn get_greatest_subtype_helper(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    raw_that: TypeId,
) -> TypeId {
    id.get_greatest_subtype_helper(reg, ast, raw_that)
}
// port: TemplatizedType#getReferencedType
pub(crate) fn get_referenced_type(id: TypeId, reg: &JSTypeRegistry) -> TypeId {
    id.get_referenced_type(reg)
}

pub(crate) fn data(id: TypeId, reg: &JSTypeRegistry) -> &TemplatizedTypeData {
    match &reg.data(id).kind {
        JSTypeKind::Templatized(data) => data,
        _ => panic!("ClassCastException"),
    }
}
// port: TemplatizedType#getTypeClass
pub(crate) fn get_type_class(_id: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::TEMPLATIZED
}
// port: TemplatizedType#getCtorImplementedInterfaces
pub(crate) fn get_ctor_implemented_interfaces(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    let reference = id.get_referenced_obj_type_internal(reg).expect("");
    let before = reference.get_ctor_implemented_interfaces(reg, ast);
    let mut replacer = data(id, reg).replacer.clone();
    let mut resolved = Vec::<TypeId>::new();
    for obj in before {
        let after = obj
            .visit(reg, ast, &mut replacer)
            .to_maybe_object_type(reg)
            .expect("");
        let hash = after.hash_code(reg);
        if !resolved
            .iter()
            .any(|previous| previous.hash_code(reg) == hash && previous.equals(reg, ast, after))
        {
            resolved.push(after);
        }
    }
    if let JSTypeKind::Templatized(data) = &mut reg.data_mut(id).kind {
        data.replacer = replacer;
    }
    resolved
}
// port: TemplatizedType#getCtorExtendedInterfaces
pub(crate) fn get_ctor_extended_interfaces(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    let reference = id.get_referenced_obj_type_internal(reg).expect("");
    let before = reference.get_ctor_extended_interfaces(reg, ast);
    let mut replacer = data(id, reg).replacer.clone();
    let mut resolved = Vec::<TypeId>::new();
    for obj in before {
        let after = obj
            .visit(reg, ast, &mut replacer)
            .to_maybe_object_type(reg)
            .expect("");
        let hash = after.hash_code(reg);
        if !resolved
            .iter()
            .any(|previous| previous.hash_code(reg) == hash && previous.equals(reg, ast, after))
        {
            resolved.push(after);
        }
    }
    if let JSTypeKind::Templatized(data) = &mut reg.data_mut(id).kind {
        data.replacer = replacer;
    }
    resolved
}
// port: TemplatizedType#appendTo
pub(crate) fn append_to(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    crate::proxy_object_type::append_to(id, reg, ast, sb);
    let template_types = data(id, reg).template_types.clone();
    if !template_types.is_empty() {
        sb.append("<");
        sb.append_all_types(reg, ast, &template_types, ",");
        sb.append(">");
    }
}
// port: TemplatizedType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(id: TypeId, reg: &JSTypeRegistry) -> i32 {
    let base_hash = crate::proxy_object_type::recursion_unsafe_hash_code(id, reg);
    if data(id, reg).is_specialized_only_with_unknown {
        return base_hash;
    }
    let list_hash = data(id, reg)
        .template_types
        .iter()
        .fold(1_i32, |hash, type_| {
            hash.wrapping_mul(31).wrapping_add(type_.hash_code(reg))
        });
    31_i32
        .wrapping_add(list_hash)
        .wrapping_mul(31)
        .wrapping_add(base_hash)
}
// port: TemplatizedType#visit
pub(crate) fn visit<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl Visitor<T>,
) -> T {
    visitor.case_templatized_type(reg, ast, id)
}
// port: TemplatizedType#visit
pub(crate) fn visit_relationship<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_templatized_type(reg, ast, id, that)
}
// port: TemplatizedType#toMaybeTemplatizedType
pub(crate) fn to_maybe_templatized_type(id: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(id)
}
// port: TemplatizedType#getTemplateTypes
pub(crate) fn get_template_types(id: TypeId, reg: &JSTypeRegistry) -> Option<Vec<TypeId>> {
    Some(data(id, reg).template_types.clone())
}
// port: TemplatizedType#getPropertyType
pub(crate) fn get_property_type(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
) -> Option<TypeId> {
    let result = Some(crate::object_type::get_property_type(
        id,
        reg,
        ast,
        property_name,
    ));
    let mut replacer = data(id, reg).replacer.clone();
    let result = result.map(|result| result.visit(reg, ast, &mut replacer));
    if let JSTypeKind::Templatized(data) = &mut reg.data_mut(id).kind {
        data.replacer = replacer;
    }
    result
}
// port: TemplatizedType#getTemplateTypeMap
pub(crate) fn get_template_type_map(id: TypeId, reg: &JSTypeRegistry) -> Arc<TemplateTypeMap> {
    reg.data(id).template_type_map.clone()
}
// port: TemplatizedType#hasAnyTemplateTypesInternal
pub(crate) fn has_any_template_types_internal(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> bool {
    let map = get_template_type_map(id, reg);
    map.has_any_template_types_internal(reg, ast)
}
// port: TemplatizedType#resolveInternal
pub(crate) fn resolve_internal(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    let base_type_before = id.get_referenced_type(reg);
    crate::proxy_object_type::resolve_internal(id, reg, ast, reporter);
    let mut rebuild = base_type_before != id.get_referenced_type(reg);
    let mut builder = Vec::new();
    for type_ in get_template_types(id, reg).unwrap() {
        let resolved = type_.resolve_with_reporter(reg, ast, reporter);
        rebuild |= resolved != type_;
        builder.push(resolved);
    }
    if rebuild {
        create(reg, ast, id.get_referenced_type(reg), &builder)
    } else {
        id
    }
}
