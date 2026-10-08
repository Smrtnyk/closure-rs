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
//   src/com/google/javascript/rhino/jstype/NamedType.java.

// Preserve Java branch structure and nullable control flow.
#![allow(clippy::collapsible_if, clippy::unnecessary_unwrap)]
use crate::{
    TypeId,
    js_type::{JSType, JSTypeKind, TypeValidator},
    js_type_class::JSTypeClass,
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    no_resolved_type::NoResolvedType,
    object_type::ObjectType,
    property::PropertyKey,
    proxy_object_type::{ProxyObjectType, ProxyObjectTypeData},
    static_typed_scope::StaticTypedScope,
    type_string_builder::TypeStringBuilder,
    visitor::Visitor,
};
use closure_rhino::{
    check_state,
    error_reporter::ErrorReporter,
    js_string::JsString,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionKind {
    NONE,
    TYPE_NAME,
    TYPEOF,
}

#[derive(Clone)]
pub struct NamedTypeData {
    pub(crate) proxy: ProxyObjectTypeData,
    pub(crate) reference: JsString,
    source_name: String,
    lineno: i32,
    charno: i32,
    resolution_kind: ResolutionKind,
    resolution_scope: Option<Arc<dyn StaticTypedScope>>,
    validator: Option<TypeValidator>,
    property_continuations: Option<Vec<PropertyContinuation>>,
    template_types: Vec<TypeId>,
    restrict_by_null: bool,
}
pub(crate) fn named_data(id: TypeId, reg: &JSTypeRegistry) -> &NamedTypeData {
    match &reg.data(id).kind {
        JSTypeKind::Named(d) => d,
        _ => panic!("ClassCastException"),
    }
}
pub(crate) fn named_data_mut(id: TypeId, reg: &mut JSTypeRegistry) -> &mut NamedTypeData {
    match &mut reg.data_mut(id).kind {
        JSTypeKind::Named(d) => d,
        _ => panic!("ClassCastException"),
    }
}

pub trait NamedType {
    fn get_bang_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn get_referenced_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn to_builder(self, reg: &JSTypeRegistry) -> NamedTypeBuilder;
}
impl NamedType for TypeId {
    // port: NamedType#getBangType
    fn get_bang_type(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        if named_data(self, reg).restrict_by_null {
            self
        } else if self.is_resolved(reg) {
            if self.is_no_resolved_type(reg) || self.is_unknown_type(reg, ast) {
                self
            } else {
                get_referenced_type(self, reg).restrict_by_not_null_or_undefined(reg, ast)
            }
        } else {
            NamedType::to_builder(self, reg)
                .set_restrict_by_null(true)
                .build(reg, ast)
        }
    }
    // port: NamedType#getReferencedType
    fn get_referenced_type(self, reg: &JSTypeRegistry) -> TypeId {
        get_referenced_type(self, reg)
    }
    // port: NamedType#toBuilder
    fn to_builder(self, reg: &JSTypeRegistry) -> NamedTypeBuilder {
        check_state!(
            !self.is_resolved(reg),
            "Only call toBuilder on unresolved NamedTypes"
        );
        let data = named_data(self, reg);
        NamedTypeBuilder::new(reg, data.reference.clone())
            .set_scope(data.resolution_scope.clone())
            .set_resolution_kind(data.resolution_kind)
            .set_error_reporting_location(data.source_name.clone(), data.lineno, data.charno)
            .set_template_types(data.template_types.clone())
            .set_referenced_type(data.proxy.referenced_type)
            .set_restrict_by_null(data.restrict_by_null)
    }
}
// port: NamedType#nominalHashCode
pub fn nominal_hash_code(type_: TypeId, reg: &JSTypeRegistry) -> i32 {
    check_state!(type_.has_reference_name(reg));
    type_
        .get_reference_name(reg)
        .expect("NullPointerException")
        .hash_code()
}
// port: NamedType#getTypeClass
pub(crate) fn get_type_class(_id: TypeId, _reg: &JSTypeRegistry) -> JSTypeClass {
    JSTypeClass::NAMED
}
// port: NamedType#getTemplateTypes
pub(crate) fn get_template_types(id: TypeId, reg: &JSTypeRegistry) -> Option<Vec<TypeId>> {
    Some(named_data(id, reg).template_types.clone())
}
// port: NamedType#defineProperty
pub(crate) fn define_property(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    property_name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    property_node: Option<NodeId>,
) -> bool {
    if !id.is_resolved(reg) {
        named_data_mut(id, reg)
            .property_continuations
            .get_or_insert_with(Vec::new)
            .push(PropertyContinuation::new(
                property_name.clone(),
                type_,
                inferred,
                property_node,
            ));
        true
    } else {
        crate::proxy_object_type::define_property(
            id,
            reg,
            ast,
            property_name,
            type_,
            inferred,
            property_node,
        )
    }
}
// port: NamedType#finishPropertyContinuations
fn finish_property_continuations(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) {
    if let Some(referenced_obj_type) = id.get_referenced_obj_type_internal(reg) {
        if !referenced_obj_type.is_unknown_type(reg, ast) {
            if let Some(continuations) = named_data(id, reg).property_continuations.clone() {
                for c in continuations {
                    c.commit(reg, ast, id);
                }
            }
        }
    }
    named_data_mut(id, reg).property_continuations = None;
}
pub(crate) fn get_referenced_type(id: TypeId, reg: &JSTypeRegistry) -> TypeId {
    id.get_referenced_type_internal(reg)
}
// port: NamedType#getReferenceName
pub(crate) fn get_reference_name(id: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    Some(named_data(id, reg).reference.clone())
}
// port: NamedType#appendTo
pub(crate) fn append_to(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
) {
    let type_ = get_referenced_type(id, reg);
    if !id.is_resolved(reg) || type_.is_no_resolved_type(reg) {
        sb.append(&named_data(id, reg).reference.to_string_lossy());
    } else {
        sb.append_type(reg, ast, type_);
    }
}
// port: NamedType#toMaybeNamedType
pub(crate) fn to_maybe_named_type(id: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(id)
}
// port: NamedType#isNominalType
pub(crate) fn is_nominal_type(id: TypeId, reg: &JSTypeRegistry) -> bool {
    if id.is_resolved(reg) {
        crate::proxy_object_type::is_nominal_type(id, reg)
    } else {
        true
    }
}
// port: NamedType#recursionUnsafeHashCode
pub(crate) fn recursion_unsafe_hash_code(id: TypeId, reg: &JSTypeRegistry) -> i32 {
    if id.is_successfully_resolved(reg) {
        crate::proxy_object_type::recursion_unsafe_hash_code(id, reg)
    } else {
        nominal_hash_code(id, reg)
    }
}
// port: NamedType#resolveInternal
pub(crate) fn resolve_internal(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> TypeId {
    let template_types = named_data(id, reg).template_types.clone();
    let mut resolved_type_args = Vec::new();
    for t in template_types {
        resolved_type_args.push(t.resolve_with_reporter(reg, ast, reporter));
    }
    if named_data(id, reg).resolution_kind == ResolutionKind::NONE {
        return crate::proxy_object_type::resolve_internal(id, reg, ast, reporter);
    }
    check_state!(
        get_referenced_type(id, reg).is_unknown_type(reg, ast),
        "NamedTypes given a referenced type pre-resolution should have ResolutionKind.NONE"
    );
    if named_data(id, reg).resolution_scope.is_none() {
        return id;
    }
    if !resolve_typeof(id, reg, ast, reporter) {
        resolve_via_registry(id, reg, ast, reporter);
    }
    crate::proxy_object_type::resolve_internal(id, reg, ast, reporter);
    if id.detect_inheritance_cycle(reg, ast) {
        handle_type_cycle(id, reg, reporter);
    }
    finish_property_continuations(id, reg, ast);
    let mut result = get_referenced_type(id, reg);
    if id.is_successfully_resolved(reg) {
        named_data_mut(id, reg).resolution_scope = None;
        let Some(result_as_object) = result.to_maybe_object_type(reg) else {
            return result;
        };
        if resolved_type_args.is_empty() || !result_as_object.is_raw_type_of_templatized_type(reg) {
            return result;
        }
        let num_keys = result.get_template_param_count(reg);
        if num_keys < resolved_type_args.len() {
            resolved_type_args.truncate(num_keys);
        }
        result = reg.create_templatized_type(ast, result_as_object, &resolved_type_args);
        let resolved = result.resolve_with_reporter(reg, ast, reporter);
        id.set_referenced_type(reg, resolved);
    }
    result
}
// port: NamedType#resolveViaRegistry
fn resolve_via_registry(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> bool {
    let scope = named_data(id, reg).resolution_scope.clone();
    let reference = named_data(id, reg).reference.clone();
    let Some(type_) = reg.get_type(ast, scope.as_deref(), reference) else {
        handle_unresolved_type(id, reg, ast, reporter);
        return false;
    };
    set_referenced_and_resolved_type(id, reg, ast, type_, reporter);
    true
}
// port: NamedType#resolveTypeof
fn resolve_typeof(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) -> bool {
    if named_data(id, reg).resolution_kind != ResolutionKind::TYPEOF {
        return false;
    }
    let scope_name = named_data(id, reg)
        .reference
        .substring_from("typeof ".len());
    let scope = named_data(id, reg).resolution_scope.clone().unwrap();
    let type_ = scope.lookup_qualified_name(reg, ast, &QualifiedName::of(scope_name.clone()));
    if type_.is_none() || type_.unwrap().is_unknown_type(reg, ast) {
        if reg.is_forward_declared_type(scope_name) {
            let reference = named_data(id, reg).reference.clone();
            let templates = named_data(id, reg).template_types.clone();
            let no_resolved = NoResolvedType::new(reg, ast, reference, Some(templates));
            id.set_referenced_type(reg, no_resolved);
            if let Some(validator) = named_data(id, reg).validator.clone() {
                validator(get_referenced_type(id, reg), reg, ast);
            }
        } else {
            warning(
                id,
                reg,
                reporter,
                "Missing type for `typeof` value. The value must be declared and const.",
            );
            set_referenced_and_resolved_type(
                id,
                reg,
                ast,
                reg.get_native_type(JSTypeNative::UNKNOWN_TYPE),
                reporter,
            );
        }
    } else {
        let mut type_ = type_.unwrap();
        if type_.is_literal_object(reg) {
            type_ = NamedTypeBuilder::new(reg, named_data(id, reg).reference.clone())
                .set_resolution_kind(ResolutionKind::NONE)
                .set_referenced_type(type_)
                .build(reg, ast);
        }
        set_referenced_and_resolved_type(id, reg, ast, type_, reporter);
    }
    true
}
// port: NamedType#setReferencedAndResolvedType
fn set_referenced_and_resolved_type(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    mut type_: TypeId,
    reporter: &mut dyn ErrorReporter,
) {
    if named_data(id, reg).restrict_by_null {
        type_ = type_.restrict_by_not_null_or_undefined(reg, ast);
    }
    if let Some(validator) = named_data(id, reg).validator.clone() {
        validator(type_, reg, ast);
    }
    id.set_referenced_type(reg, type_);
    check_enum_element_cycle(id, reg, reporter);
    check_proto_cycle(id, reg, reporter);
}
// port: NamedType#handleTypeCycle
fn handle_type_cycle(id: TypeId, reg: &mut JSTypeRegistry, reporter: &mut dyn ErrorReporter) {
    id.set_referenced_type(reg, reg.get_native_object_type(JSTypeNative::UNKNOWN_TYPE));
    warning(
        id,
        reg,
        reporter,
        &format!(
            "Cycle detected in inheritance chain of type {}",
            named_data(id, reg).reference
        ),
    );
}
// port: NamedType#checkEnumElementCycle
fn check_enum_element_cycle(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    reporter: &mut dyn ErrorReporter,
) {
    let referenced = get_referenced_type(id, reg);
    if let JSTypeKind::EnumElement(d) = &reg.data(referenced).kind {
        if d.primitive_type == id {
            handle_type_cycle(id, reg, reporter);
        }
    }
}
// port: NamedType#checkProtoCycle
fn check_proto_cycle(id: TypeId, reg: &mut JSTypeRegistry, reporter: &mut dyn ErrorReporter) {
    if get_referenced_type(id, reg) == id {
        handle_type_cycle(id, reg, reporter);
    }
}
// port: NamedType#handleUnresolvedType
fn handle_unresolved_type(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn ErrorReporter,
) {
    let reference = named_data(id, reg).reference.clone();
    if !reg.is_forward_declared_type(reference.clone()) {
        let mut msg = format!("Bad type annotation. Unknown type {reference}");
        let dot = reference.index_of_char(b'.' as u16);
        let root = if dot < 0 {
            reference.clone()
        } else {
            reference.substring(0, dot as usize)
        };
        if local_variable_shadows_global_namespace(id, reg, ast, &root) {
            msg.push_str(&format!("\nIt's possible that a local variable called '{root}' is shadowing the intended global namespace."));
        } else if named_data(id, reg)
            .resolution_scope
            .clone()
            .unwrap()
            .get_slot_creating_implicit_vars(reg, ast, &root)
            .is_some()
        {
            msg.push_str(&format!(
                "\nIt's possible that '{reference}' refers to a value, not a type."
            ));
        }
        warning(id, reg, reporter, &msg);
    } else {
        let templates = named_data(id, reg).template_types.clone();
        let no_resolved = NoResolvedType::new(reg, ast, reference, Some(templates));
        id.set_referenced_type(reg, no_resolved);
        if let Some(validator) = named_data(id, reg).validator.clone() {
            validator(get_referenced_type(id, reg), reg, ast);
        }
    }
}
// port: NamedType#localVariableShadowsGlobalNamespace
fn local_variable_shadows_global_namespace(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    root: &JsString,
) -> bool {
    let scope = named_data(id, reg).resolution_scope.clone().unwrap();
    if let Some(root_var) = scope.get_slot_creating_implicit_vars(reg, ast, root) {
        let var_scope = root_var.get_scope(reg).expect("NullPointerException");
        if let Some(parent) = var_scope.get_parent_scope() {
            return parent
                .get_slot_creating_implicit_vars(reg, ast, root)
                .is_some();
        }
    }
    false
}
// port: NamedType#setValidator
pub(crate) fn set_validator(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    validator: TypeValidator,
) -> bool {
    if id.is_resolved(reg) {
        crate::proxy_object_type::set_validator(id, reg, ast, validator)
    } else {
        named_data_mut(id, reg).validator = Some(validator);
        true
    }
}
// port: NamedType#warning
pub(crate) fn warning(
    id: TypeId,
    reg: &JSTypeRegistry,
    reporter: &mut dyn ErrorReporter,
    message: &str,
) {
    let data = named_data(id, reg);
    reporter.warning(message, &data.source_name, data.lineno, data.charno);
}

#[derive(Clone)]
struct PropertyContinuation {
    property_name: PropertyKey,
    type_: TypeId,
    inferred: bool,
    property_node: Option<NodeId>,
}
impl PropertyContinuation {
    // port: NamedType.PropertyContinuation#PropertyContinuation
    fn new(
        property_name: PropertyKey,
        type_: TypeId,
        inferred: bool,
        property_node: Option<NodeId>,
    ) -> Self {
        Self {
            property_name,
            type_,
            inferred,
            property_node,
        }
    }
    // port: NamedType.PropertyContinuation#commit
    fn commit(&self, reg: &mut JSTypeRegistry, ast: &Ast, target: TypeId) {
        target.define_property(
            reg,
            ast,
            self.property_name.clone(),
            self.type_,
            self.inferred,
            self.property_node,
        );
    }
}
// port: NamedType#isObject
pub(crate) fn is_object(id: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    if let Some(element) = id.to_maybe_enum_element_type(reg) {
        element.is_object(reg, ast)
    } else {
        true
    }
}
// port: NamedType#visit
pub(crate) fn visit<T>(
    id: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl Visitor<T>,
) -> T {
    visitor.case_named_type(reg, ast, id)
}
// port: NamedType#builder
pub fn builder(reg: &JSTypeRegistry, reference: impl Into<JsString>) -> NamedTypeBuilder {
    NamedTypeBuilder::new(reg, reference)
}

#[derive(Clone)]
pub struct NamedTypeBuilder {
    resolution_kind: Option<ResolutionKind>,
    reference_name: JsString,
    scope: Option<Arc<dyn StaticTypedScope>>,
    source_name: String,
    lineno: i32,
    charno: i32,
    referenced_type: TypeId,
    restrict_by_null: bool,
    template_types: Vec<TypeId>,
}
impl NamedTypeBuilder {
    // port: NamedType.Builder#Builder
    pub fn new(reg: &JSTypeRegistry, reference_name: impl Into<JsString>) -> Self {
        Self {
            resolution_kind: None,
            reference_name: reference_name.into(),
            scope: None,
            source_name: String::new(),
            lineno: 0,
            charno: 0,
            referenced_type: reg.get_native_type(JSTypeNative::UNKNOWN_TYPE),
            restrict_by_null: false,
            template_types: Vec::new(),
        }
    }
    // port: NamedType.Builder#setScope
    pub fn set_scope(mut self, scope: Option<Arc<dyn StaticTypedScope>>) -> Self {
        self.scope = scope;
        self
    }
    // port: NamedType.Builder#setResolutionKind
    pub fn set_resolution_kind(mut self, resolution_kind: ResolutionKind) -> Self {
        self.resolution_kind = Some(resolution_kind);
        self
    }
    // port: NamedType.Builder#setErrorReportingLocation
    pub fn set_error_reporting_location(
        mut self,
        source_name: impl Into<String>,
        lineno: i32,
        charno: i32,
    ) -> Self {
        self.source_name = source_name.into();
        self.lineno = lineno;
        self.charno = charno;
        self
    }
    // port: NamedType.Builder#setErrorReportingLocationFrom
    pub fn set_error_reporting_location_from(self, ast: &Ast, source: NodeId) -> Self {
        self.set_error_reporting_location(
            source.get_source_file_name(ast).unwrap_or_default(),
            source.get_lineno(ast),
            source.get_charno(ast),
        )
    }
    // port: NamedType.Builder#setTemplateTypes
    pub fn set_template_types(mut self, template_types: Vec<TypeId>) -> Self {
        self.template_types = template_types;
        self
    }
    // port: NamedType.Builder#setReferencedType
    pub fn set_referenced_type(mut self, referenced_type: TypeId) -> Self {
        self.referenced_type = referenced_type;
        self
    }
    // port: NamedType.Builder#setRestrictByNull
    pub fn set_restrict_by_null(mut self, restrict_by_null: bool) -> Self {
        self.restrict_by_null = restrict_by_null;
        self
    }
    // port: NamedType.Builder#build
    pub fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        create(reg, ast, self)
    }
}
// port: NamedType#NamedType
fn create(reg: &mut JSTypeRegistry, ast: &Ast, builder: NamedTypeBuilder) -> TypeId {
    let resolution_kind = builder.resolution_kind.expect("NullPointerException");
    if resolution_kind == ResolutionKind::TYPEOF {
        check_state!(builder.reference_name.starts_with(&"typeof ".into()));
    }
    let data = NamedTypeData {
        proxy: ProxyObjectTypeData::new(reg, builder.referenced_type),
        reference: builder.reference_name,
        source_name: builder.source_name,
        lineno: builder.lineno,
        charno: builder.charno,
        resolution_kind,
        resolution_scope: builder.scope,
        validator: None,
        property_continuations: None,
        template_types: builder.template_types,
        restrict_by_null: builder.restrict_by_null,
    };
    let id = reg.alloc(JSTypeKind::Named(data), None);
    reg.finish_construction(id, ast);
    id
}
