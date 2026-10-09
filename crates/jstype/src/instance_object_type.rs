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
//   src/com/google/javascript/rhino/jstype/InstanceObjectType.java.

use crate::{
    TypeId,
    function_type::FunctionType,
    js_type::{JSType, JSTypeKind},
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    property::PropertyKey,
    prototype_object_type::{PrototypeObjectTypeBuilder, PrototypeObjectTypeData},
    template_type_map::TemplateTypeMap,
    type_string_builder::TypeStringBuilder,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct InstanceObjectTypeData {
    pub(crate) prototype: PrototypeObjectTypeData,
    pub(crate) constructor: TypeId,
}
#[derive(Clone, Debug)]
pub struct InstanceObjectTypeBuilder {
    prototype: PrototypeObjectTypeBuilder,
    constructor: Option<TypeId>,
}
pub type Builder = InstanceObjectTypeBuilder;
impl Default for InstanceObjectTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl InstanceObjectTypeBuilder {
    // port: InstanceObjectType.Builder#Builder
    pub fn new() -> Self {
        Self {
            prototype: PrototypeObjectTypeBuilder::new(),
            constructor: None,
        }
    }
    // port: InstanceObjectType.Builder#setConstructor
    pub fn set_constructor(mut self, x: TypeId) -> Self {
        self.constructor = Some(x);
        self
    }
    pub fn set_template_type_map(mut self, x: Arc<TemplateTypeMap>) -> Self {
        self.prototype.template_type_map = Some(x);
        self
    }
    pub fn set_template_param_count(mut self, x: usize) -> Self {
        self.prototype.template_param_count = x;
        self
    }
    // port: InstanceObjectType.Builder#build
    pub fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        new(reg, ast, self)
    }
    // port: InstanceObjectType#builderForCtor
    pub fn builder_for_ctor(ctor: TypeId, reg: &JSTypeRegistry) -> Self {
        let d = reg.prototype_data(ctor);
        Self {
            prototype: PrototypeObjectTypeBuilder::new()
                .set_name_option(ctor.get_reference_name(reg))
                .set_implicit_prototype(None)
                .set_native(ctor.is_native_object_type(reg))
                .set_anonymous(d.anonymous_type)
                .set_template_type_map(ctor.get_template_type_map(reg))
                .set_template_param_count(ctor.get_template_param_count(reg)),
            constructor: Some(ctor),
        }
    }
}
// port: InstanceObjectType#InstanceObjectType
fn new(reg: &mut JSTypeRegistry, ast: &Ast, builder: InstanceObjectTypeBuilder) -> TypeId {
    let prototype = PrototypeObjectTypeData::from_builder(reg, &builder.prototype);
    let constructor = builder.constructor.expect("NullPointerException");
    let t = reg.alloc(
        JSTypeKind::InstanceObject(InstanceObjectTypeData {
            prototype,
            constructor,
        }),
        builder.prototype.template_type_map,
    );
    crate::prototype_object_type::initialize_after_allocation(t, reg);
    reg.finish_construction(t, ast);
    t
}
// port: InstanceObjectType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::INSTANCE_OBJECT
}
// port: InstanceObjectType#getReferenceName
pub fn get_reference_name(t: TypeId, reg: &JSTypeRegistry) -> Option<JsString> {
    t.get_constructor(reg).unwrap().get_reference_name(reg)
}
// port: InstanceObjectType#getImplicitPrototype
pub fn get_implicit_prototype(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<TypeId> {
    Some(t.get_constructor(reg).unwrap().get_prototype(reg, ast))
}
// port: InstanceObjectType#getConstructor
pub fn get_constructor(t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    match &reg.data(t).kind {
        JSTypeKind::InstanceObject(d) => Some(d.constructor),
        _ => panic!("ClassCastException"),
    }
}
#[allow(clippy::collapsible_if)] // Keep Java's nullable prototype lookup.
// port: InstanceObjectType#defineProperty
pub fn define_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    node: Option<NodeId>,
) -> bool {
    if let Some(proto) = t.get_implicit_prototype(reg, ast) {
        if proto.has_own_declared_property(reg, ast, name.clone()) {
            return false;
        }
    }
    crate::prototype_object_type::define_property(t, reg, ast, name, type_, inferred, node)
}
// port: InstanceObjectType#appendTo
pub fn append_to(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder) {
    let constructor = t.get_constructor(reg).unwrap();
    if !constructor.has_reference_name(reg) {
        crate::prototype_object_type::append_to(t, reg, ast, sb);
        return;
    } else if sb.is_for_annotations() {
        sb.append(
            &constructor
                .get_normalized_reference_name(reg)
                .unwrap()
                .to_string_lossy(),
        );
        return;
    }
    let name = constructor.get_reference_name(reg).unwrap();
    if name.is_empty() {
        let n = constructor.get_source(reg);
        let source_name = n
            .and_then(|n| n.get_source_file_name(ast))
            .unwrap_or_else(|| {
                if n.is_some() {
                    "null".into()
                } else {
                    "unknown".into()
                }
            });
        let lineno = n.map_or(0, |n| n.get_lineno(ast));
        sb.append("<anonymous@")
            .append(&source_name)
            .append(":")
            .append(&lineno.to_string())
            .append(">");
    }
    sb.append(&name.to_string_lossy());
}
fn is_native_instance(t: TypeId, reg: &JSTypeRegistry, name: &str) -> bool {
    t.get_constructor(reg).unwrap().is_native_object_type(reg)
        && t.get_reference_name(reg).is_some_and(|n| n == name)
}
// port: InstanceObjectType#isTheObjectType
pub fn is_the_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Object")
}
// port: InstanceObjectType#isInstanceType
pub fn is_instance_type(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: InstanceObjectType#isArrayType
pub fn is_array_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Array")
}
// port: InstanceObjectType#isReadonlyArrayType
pub fn is_readonly_array_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "ReadonlyArray")
}
// port: InstanceObjectType#isBigIntObjectType
pub fn is_big_int_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "BigInt")
}
// port: InstanceObjectType#isStringObjectType
pub fn is_string_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "String")
}
// port: InstanceObjectType#isSymbolObjectType
pub fn is_symbol_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Symbol")
}
// port: InstanceObjectType#isBooleanObjectType
pub fn is_boolean_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Boolean")
}
// port: InstanceObjectType#isNumberObjectType
pub fn is_number_object_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Number")
}
// port: InstanceObjectType#isDateType
pub fn is_date_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "Date")
}
// port: InstanceObjectType#isRegexpType
pub fn is_regexp_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    is_native_instance(t, reg, "RegExp")
}
// port: InstanceObjectType#isNominalType
pub fn is_nominal_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.has_reference_name(reg)
}
// port: InstanceObjectType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    if t.has_reference_name(reg) {
        crate::named_type::nominal_hash_code(t, reg)
    } else {
        crate::prototype_object_type::recursion_unsafe_hash_code(t, reg)
    }
}
// port: InstanceObjectType#getCtorImplementedInterfaces
pub fn get_ctor_implemented_interfaces(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Vec<TypeId> {
    t.get_constructor(reg)
        .unwrap()
        .get_implemented_interfaces(reg, ast)
}
// port: InstanceObjectType#getCtorExtendedInterfaces
pub fn get_ctor_extended_interfaces(t: TypeId, reg: &JSTypeRegistry) -> Vec<TypeId> {
    t.get_constructor(reg).unwrap().get_extended_interfaces(reg)
}
// port: InstanceObjectType#resolveInternal
pub fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    let resolved = crate::prototype_object_type::resolve_internal(t, reg, ast, reporter);
    t.get_constructor(reg)
        .unwrap()
        .resolve_with_reporter(reg, ast, reporter);
    resolved
}
