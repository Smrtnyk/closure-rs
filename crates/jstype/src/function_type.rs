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
//   src/com/google/javascript/rhino/jstype/FunctionType.java.

use crate::{
    TypeId,
    arrow_type::{self, ArrowType},
    instance_object_type::InstanceObjectTypeBuilder,
    js_type::{JSType, JSTypeKind},
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    property::{Property, PropertyData, PropertyId, PropertyKey},
    prototype_object_type::{
        PrototypeObjectType, PrototypeObjectTypeBuilder, PrototypeObjectTypeData,
    },
    template_type_map::TemplateTypeMap,
    type_string_builder::TypeStringBuilder,
    union_type::UnionType,
};
use closure_rhino::{
    check_argument, check_state,
    closure_primitive::ClosurePrimitive,
    js_string::JsString,
    node::{Ast, NodeId},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    ORDINARY,
    CONSTRUCTOR,
    INTERFACE,
    NONE,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropAccess {
    ANY,
    ANY_EXPLICIT,
    STRUCT,
    DICT,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstructorAmbiguity {
    UNKNOWN,
    IS_AMBIGUOUS_CONSTRUCTOR,
    IS_UNAMBIGUOUS_CONSTRUCTOR,
}

#[derive(Clone, Debug)]
pub struct FunctionTypeData {
    pub(crate) prototype: PrototypeObjectTypeData,
    pub(crate) constructor_ambiguity: ConstructorAmbiguity,
    pub(crate) call: Option<TypeId>,
    pub(crate) prototype_slot: Option<PropertyId>,
    pub(crate) kind: Kind,
    pub(crate) prop_access: Option<PropAccess>,
    pub(crate) type_of_this: Option<TypeId>,
    pub(crate) source: Option<NodeId>,
    pub(crate) goog_module_id: Option<JsString>,
    pub(crate) is_structural_interface: bool,
    pub(crate) is_abstract: bool,
    pub(crate) implemented_interfaces: Vec<TypeId>,
    pub(crate) extended_interfaces: Vec<TypeId>,
    pub(crate) closure_primitive: Option<ClosurePrimitive>,
    pub(crate) canonical_representation: Option<TypeId>,
}

#[derive(Clone, Copy, Debug)]
pub struct Parameter {
    type_: TypeId,
    is_optional: bool,
    is_variadic: bool,
}
impl Parameter {
    // port: FunctionType.Parameter#create
    pub fn create(type_: TypeId, is_optional: bool, is_variadic: bool) -> Self {
        Self {
            type_,
            is_optional,
            is_variadic,
        }
    }
    // port: FunctionType.Parameter#getJSType
    pub fn get_jstype(&self) -> TypeId {
        self.type_
    }
    // port: FunctionType.Parameter#isOptional
    pub fn is_optional(&self) -> bool {
        self.is_optional
    }
    // port: FunctionType.Parameter#isVariadic
    pub fn is_variadic(&self) -> bool {
        self.is_variadic
    }
    // port: AutoValue_FunctionType_Parameter#equals
    pub fn equals(&self, reg: &mut JSTypeRegistry, ast: &Ast, other: Option<&Parameter>) -> bool {
        let Some(that) = other else {
            return false;
        };
        if std::ptr::eq(self, that) {
            return true;
        }
        self.type_.equals(reg, ast, that.get_jstype())
            && self.is_optional == that.is_optional()
            && self.is_variadic == that.is_variadic()
    }
    // port: AutoValue_FunctionType_Parameter#hashCode
    pub fn hash_code(&self, reg: &JSTypeRegistry) -> i32 {
        let mut h = 1i32;
        h = h.wrapping_mul(1_000_003);
        h ^= self.type_.hash_code(reg);
        h = h.wrapping_mul(1_000_003);
        h ^= if self.is_optional { 1231 } else { 1237 };
        h = h.wrapping_mul(1_000_003);
        h ^= if self.is_variadic { 1231 } else { 1237 };
        h
    }
    // port: AutoValue_FunctionType_Parameter#toString
    pub fn to_string(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        format!(
            "Parameter{{JSType={}, optional={}, variadic={}}}",
            self.type_.to_string(reg, ast),
            self.is_optional,
            self.is_variadic
        )
    }
}

#[derive(Clone, Debug)]
pub struct FunctionTypeBuilder {
    pub(crate) prototype: PrototypeObjectTypeBuilder,
    source_node: Option<NodeId>,
    goog_module_id: Option<JsString>,
    parameters: Option<Vec<Parameter>>,
    return_type: Option<TypeId>,
    type_of_this: Option<TypeId>,
    set_prototype_based_on: Option<TypeId>,
    constructor_only_keys: Vec<TypeId>,
    kind: Kind,
    is_abstract: bool,
    is_known_ambiguous: bool,
    return_type_is_inferred: bool,
    returns_own_instance_type: bool,
    primitive_id: Option<ClosurePrimitive>,
    canonical_representation: Option<TypeId>,
}
pub type Builder = FunctionTypeBuilder;
impl Default for FunctionTypeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
impl FunctionTypeBuilder {
    // port: FunctionType.Builder#Builder
    pub fn new() -> Self {
        Self {
            prototype: PrototypeObjectTypeBuilder::new(),
            source_node: None,
            goog_module_id: None,
            parameters: None,
            return_type: None,
            type_of_this: None,
            set_prototype_based_on: None,
            constructor_only_keys: Vec::new(),
            kind: Kind::ORDINARY,
            is_abstract: false,
            is_known_ambiguous: false,
            return_type_is_inferred: false,
            returns_own_instance_type: false,
            primitive_id: None,
            canonical_representation: None,
        }
    }
    // port: FunctionType.Builder#withName
    pub fn with_name(mut self, name: impl Into<JsString>) -> Self {
        self.prototype.class_name = Some(name.into());
        self
    }
    pub fn with_name_option(mut self, name: Option<JsString>) -> Self {
        self.prototype.class_name = name;
        self
    }
    pub fn set_name(self, name: impl Into<JsString>) -> Self {
        self.with_name(name)
    }
    pub fn set_name_option(self, name: Option<JsString>) -> Self {
        self.with_name_option(name)
    }
    // port: FunctionType.Builder#withSourceNode
    pub fn with_source_node(mut self, source_node: Option<NodeId>) -> Self {
        self.source_node = source_node;
        self
    }
    // port: FunctionType.Builder#setGoogModuleId
    pub fn set_goog_module_id(mut self, x: Option<JsString>) -> Self {
        self.goog_module_id = x;
        self
    }
    // port: FunctionType.Builder#withParameters
    pub fn with_parameters(mut self, parameters: impl Into<Option<Vec<Parameter>>>) -> Self {
        self.parameters = parameters.into();
        self
    }
    // port: FunctionType.Builder#withParameters
    pub fn with_no_parameters(mut self) -> Self {
        self.parameters = Some(Vec::new());
        self
    }
    // port: FunctionType.Builder#withReturnType
    pub fn with_return_type(mut self, return_type: impl Into<Option<TypeId>>) -> Self {
        self.return_type = return_type.into();
        self
    }
    // port: FunctionType.Builder#withReturnType
    pub fn with_return_type_inferred(
        mut self,
        return_type: impl Into<Option<TypeId>>,
        inferred: bool,
    ) -> Self {
        self.return_type = return_type.into();
        self.return_type_is_inferred = inferred;
        self
    }
    // port: FunctionType.Builder#withReturnsOwnInstanceType
    pub fn with_returns_own_instance_type(mut self) -> Self {
        self.returns_own_instance_type = true;
        self
    }
    // port: FunctionType.Builder#withInferredReturnType
    pub fn with_inferred_return_type(mut self, return_type: TypeId) -> Self {
        self.return_type = Some(return_type);
        self.return_type_is_inferred = true;
        self
    }
    // port: FunctionType.Builder#withTypeOfThis
    pub fn with_type_of_this(mut self, type_of_this: impl Into<Option<TypeId>>) -> Self {
        self.type_of_this = type_of_this.into();
        self
    }
    // port: FunctionType.Builder#withTemplateKeys
    pub fn with_template_keys(
        mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        template_keys: impl Into<Option<Vec<TypeId>>>,
    ) -> Self {
        let template_keys = template_keys.into().unwrap_or_default();
        let map = reg.get_empty_template_type_map();
        self.prototype.template_type_map =
            Some(map.copy_with_extension(reg, ast, &template_keys, &[]));
        self.prototype.template_param_count = template_keys.len();
        self
    }
    // port: FunctionType.Builder#withConstructorTemplateKeys
    pub fn with_constructor_template_keys(
        mut self,
        constructor_only_keys: impl IntoIterator<Item = TypeId>,
    ) -> Self {
        self.constructor_only_keys.clear();
        for k in constructor_only_keys {
            if !self.constructor_only_keys.contains(&k) {
                self.constructor_only_keys.push(k);
            }
        }
        self
    }
    // port: FunctionType.Builder#withKind
    pub fn with_kind(mut self, kind: Kind) -> Self {
        self.kind = kind;
        self
    }
    // port: FunctionType.Builder#forConstructor
    pub fn for_constructor(mut self) -> Self {
        self.kind = Kind::CONSTRUCTOR;
        self
    }
    // port: FunctionType.Builder#forInterface
    pub fn for_interface(mut self) -> Self {
        self.kind = Kind::INTERFACE;
        self.parameters = Some(Vec::new());
        self
    }
    // port: FunctionType.Builder#forNativeType
    pub fn for_native_type(mut self) -> Self {
        self.prototype.native_type = true;
        self
    }
    pub fn set_native(mut self, x: bool) -> Self {
        self.prototype.native_type = x;
        self
    }
    pub fn set_anonymous(mut self, x: bool) -> Self {
        self.prototype.anonymous_type = x;
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
    pub fn set_implicit_prototype(mut self, x: Option<TypeId>) -> Self {
        self.prototype.implicit_prototype = x;
        self
    }
    // port: FunctionType.Builder#withIsAbstract
    pub fn with_is_abstract(mut self, is_abstract: bool) -> Self {
        self.is_abstract = is_abstract;
        self
    }
    // port: FunctionType.Builder#setIsKnownAmbiguous
    pub fn set_is_known_ambiguous(mut self, x: bool) -> Self {
        self.is_known_ambiguous = x;
        self
    }
    // port: FunctionType.Builder#withPrototypeBasedOn
    pub fn with_prototype_based_on(mut self, x: TypeId) -> Self {
        self.set_prototype_based_on = Some(x);
        self
    }
    // port: FunctionType.Builder#withClosurePrimitiveId
    pub fn with_closure_primitive_id(mut self, id: Option<ClosurePrimitive>) -> Self {
        self.primitive_id = id;
        self
    }
    // port: FunctionType.Builder#withCanonicalRepresentation
    pub fn with_canonical_representation(mut self, representation: Option<TypeId>) -> Self {
        self.canonical_representation = representation;
        self
    }
    // port: FunctionType.Builder#build
    pub fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        match self.kind {
            Kind::CONSTRUCTOR | Kind::INTERFACE => {}
            Kind::NONE => check_state!(self.returns_own_instance_type),
            Kind::ORDINARY => check_state!(!self.returns_own_instance_type),
        }
        if self.returns_own_instance_type {
            check_state!(self.return_type.is_none());
            check_state!(self.type_of_this.is_none());
        }
        if matches!(self.kind, Kind::NONE | Kind::ORDINARY) {
            check_state!(self.constructor_only_keys.is_empty());
        }
        new(reg, ast, self)
    }
    // port: FunctionType.Builder#buildAndResolve
    pub fn build_and_resolve(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        self.build(reg, ast)
            .to_maybe_function_type(reg)
            .expect("NullPointerException")
    }
}
// port: FunctionType#builder
pub fn builder() -> FunctionTypeBuilder {
    FunctionTypeBuilder::new()
}

// port: FunctionType#FunctionType
pub fn new(reg: &mut JSTypeRegistry, ast: &Ast, builder: FunctionTypeBuilder) -> TypeId {
    new_with_kind(reg, ast, builder, JSTypeKind::Function, false)
}

pub(crate) fn new_with_kind(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    mut builder: FunctionTypeBuilder,
    wrap: impl FnOnce(FunctionTypeData) -> JSTypeKind,
    eagerly_resolved: bool,
) -> TypeId {
    if builder.prototype.implicit_prototype.is_none() {
        builder.prototype.implicit_prototype =
            Some(reg.get_native_type(JSTypeNative::FUNCTION_PROTOTYPE));
    }
    let prototype = PrototypeObjectTypeData::from_builder(reg, &builder.prototype);
    let t = reg.alloc(
        wrap(FunctionTypeData {
            prototype,
            constructor_ambiguity: if builder.is_known_ambiguous {
                ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
            } else {
                ConstructorAmbiguity::UNKNOWN
            },
            call: None,
            prototype_slot: None,
            kind: builder.kind,
            prop_access: if builder.kind == Kind::CONSTRUCTOR {
                Some(PropAccess::ANY)
            } else {
                None
            },
            type_of_this: None,
            source: builder.source_node,
            goog_module_id: builder.goog_module_id,
            is_structural_interface: false,
            is_abstract: builder.is_abstract,
            implemented_interfaces: Vec::new(),
            extended_interfaces: Vec::new(),
            closure_primitive: builder.primitive_id,
            canonical_representation: builder.canonical_representation,
        }),
        builder.prototype.template_type_map,
    );
    crate::prototype_object_type::initialize_after_allocation(t, reg);
    t.set_pretty_print(reg, true);
    check_argument!(
        builder
            .source_node
            .is_none_or(|n| n.is_function(ast) || n.is_class(ast))
    );
    let type_of_this = if let Some(type_of_this) = builder.type_of_this {
        type_of_this
    } else {
        match builder.kind {
            Kind::CONSTRUCTOR | Kind::INTERFACE => {
                let mut type_of_this_builder = InstanceObjectTypeBuilder::builder_for_ctor(t, reg);
                if !builder.constructor_only_keys.is_empty() {
                    let map = t.get_template_type_map(reg);
                    let new_map = map.copy_without_keys(reg, ast, &builder.constructor_only_keys);
                    type_of_this_builder = type_of_this_builder
                        .set_template_type_map(new_map)
                        .set_template_param_count(
                            t.get_template_param_count(reg) - builder.constructor_only_keys.len(),
                        );
                }
                type_of_this_builder.build(reg, ast)
            }
            Kind::ORDINARY => reg.get_native_type(JSTypeNative::UNKNOWN_TYPE),
            Kind::NONE => t,
        }
    };
    reg.function_data_mut(t).type_of_this = Some(type_of_this);
    let call = ArrowType::new(
        reg,
        ast,
        builder.parameters,
        if builder.returns_own_instance_type {
            Some(type_of_this)
        } else {
            builder.return_type
        },
        builder.return_type_is_inferred,
    );
    reg.function_data_mut(t).call = Some(call);
    check_argument!(
        builder.canonical_representation.is_none() || builder.kind == Kind::CONSTRUCTOR,
        "Only constructors should have canonical representations"
    );
    if let Some(base) = builder.set_prototype_based_on {
        t.set_prototype_based_on(reg, ast, base);
    }
    if eagerly_resolved {
        t.eagerly_resolve_to_self(reg, ast);
    } else {
        reg.finish_construction(t, ast);
    }
    t
}

pub trait FunctionType {
    fn get_kind(self, reg: &JSTypeRegistry) -> Kind;
    fn makes_structs(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn makes_dicts(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn set_struct(self, reg: &mut JSTypeRegistry);
    fn set_dict(self, reg: &mut JSTypeRegistry);
    fn set_explicit_unrestricted(self, reg: &mut JSTypeRegistry);
    fn has_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn get_parameters(self, reg: &JSTypeRegistry) -> Vec<Parameter>;
    fn get_min_arity(self, reg: &JSTypeRegistry) -> usize;
    fn get_max_arity(self, reg: &JSTypeRegistry) -> usize;
    fn get_return_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn is_return_type_inferred(self, reg: &JSTypeRegistry) -> bool;
    fn get_internal_arrow_type(self, reg: &JSTypeRegistry) -> TypeId;
    fn get_prototype_property(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn get_prototype(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn set_prototype_based_on(self, reg: &mut JSTypeRegistry, ast: &Ast, base_type: TypeId);
    fn set_prototype(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prototype: Option<TypeId>,
        property_node: Option<NodeId>,
    ) -> bool;
    fn get_all_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId>;
    fn get_ancestor_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId>;
    fn get_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId>;
    fn get_own_implemented_interfaces(self, reg: &JSTypeRegistry) -> Vec<TypeId>;
    fn set_implemented_interfaces(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        implemented_interfaces: Vec<TypeId>,
    );
    fn get_extended_interfaces(self, reg: &JSTypeRegistry) -> Vec<TypeId>;
    fn get_extended_interfaces_count(self, reg: &JSTypeRegistry) -> usize;
    fn set_extended_interfaces(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        extended_interfaces: Vec<TypeId>,
    );
    fn get_bind_return_type(self, reg: &mut JSTypeRegistry, ast: &Ast, args_to_bind: i32)
    -> TypeId;
    fn sup_and_inf_helper(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
        least_super: bool,
    ) -> TypeId;
    fn has_equal_call_type(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool;
    fn get_instance_type(self, reg: &JSTypeRegistry) -> Option<TypeId>;
    fn has_instance_type(self, reg: &JSTypeRegistry) -> bool;
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId>;
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString>;
    fn set_source(self, reg: &mut JSTypeRegistry, source: Option<NodeId>);
    fn has_properties(self, reg: &JSTypeRegistry) -> bool;
    fn set_implicit_match(self, reg: &mut JSTypeRegistry, flag: bool);
    fn is_abstract(self, reg: &JSTypeRegistry) -> bool;
    fn check_extends_loop(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<Vec<TypeId>>;
    fn accepts_arguments(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        argument_types: &[TypeId],
    ) -> bool;
    fn forget_parameter_and_return_types(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId;
    fn get_constructor_only_template_parameters(self, reg: &JSTypeRegistry) -> Vec<TypeId>;
    fn is_ambiguous_constructor(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool;
    fn get_closure_primitive(self, reg: &JSTypeRegistry) -> Option<ClosurePrimitive>;
    fn to_builder(self, reg: &JSTypeRegistry) -> FunctionTypeBuilder;
    fn get_canonical_representation(self, reg: &JSTypeRegistry) -> Option<TypeId>;
}
impl FunctionType for TypeId {
    // port: FunctionType#getKind
    fn get_kind(self, reg: &JSTypeRegistry) -> Kind {
        reg.function_data(self).kind
    }
    #[allow(clippy::collapsible_match)] // Keep Java's switch and conditional control flow.
    // port: FunctionType#makesStructs
    fn makes_structs(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if !self.has_instance_type(reg) {
            return false;
        }
        match reg.function_data(self).prop_access {
            Some(PropAccess::STRUCT) => true,
            Some(PropAccess::ANY) => {
                if self
                    .get_super_class_constructor(reg, ast)
                    .is_some_and(|s| s.makes_structs(reg, ast))
                {
                    self.set_struct(reg);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    #[allow(clippy::collapsible_match)] // Keep Java's switch and conditional control flow.
    // port: FunctionType#makesDicts
    fn makes_dicts(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if !self.is_constructor(reg) {
            return false;
        }
        match reg.function_data(self).prop_access {
            Some(PropAccess::DICT) => true,
            Some(PropAccess::ANY) => {
                if self
                    .get_super_class_constructor(reg, ast)
                    .is_some_and(|s| s.makes_dicts(reg, ast))
                {
                    self.set_dict(reg);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
    // port: FunctionType#setStruct
    fn set_struct(self, reg: &mut JSTypeRegistry) {
        reg.function_data_mut(self).prop_access = Some(PropAccess::STRUCT);
    }
    // port: FunctionType#setDict
    fn set_dict(self, reg: &mut JSTypeRegistry) {
        reg.function_data_mut(self).prop_access = Some(PropAccess::DICT);
    }
    // port: FunctionType#setExplicitUnrestricted
    fn set_explicit_unrestricted(self, reg: &mut JSTypeRegistry) {
        reg.function_data_mut(self).prop_access = Some(PropAccess::ANY_EXPLICIT);
    }
    #[allow(clippy::collapsible_if)] // Keep Java's constructor branch and nullable lookup.
    // port: FunctionType#hasImplementedInterfaces
    fn has_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if !reg.function_data(self).implemented_interfaces.is_empty() {
            return true;
        }
        if self.is_constructor(reg) {
            if let Some(super_ctor) = self.get_super_class_constructor(reg, ast) {
                return super_ctor.has_implemented_interfaces(reg, ast);
            }
        }
        false
    }
    // port: FunctionType#getParameters
    fn get_parameters(self, reg: &JSTypeRegistry) -> Vec<Parameter> {
        arrow_type::get_parameter_list(self.get_internal_arrow_type(reg), reg)
    }
    // port: FunctionType#getMinArity
    fn get_min_arity(self, reg: &JSTypeRegistry) -> usize {
        let mut min = 0;
        for (index, parameter) in self.get_parameters(reg).iter().enumerate() {
            if !parameter.is_optional() && !parameter.is_variadic() {
                min = index + 1;
            }
        }
        min
    }
    // port: FunctionType#getMaxArity
    fn get_max_arity(self, reg: &JSTypeRegistry) -> usize {
        let params = self.get_parameters(reg);
        if params.last().is_some_and(Parameter::is_variadic) {
            i32::MAX as usize
        } else {
            params.len()
        }
    }
    // port: FunctionType#getReturnType
    fn get_return_type(self, reg: &JSTypeRegistry) -> TypeId {
        arrow_type::get_return_type(self.get_internal_arrow_type(reg), reg)
    }
    // port: FunctionType#isReturnTypeInferred
    fn is_return_type_inferred(self, reg: &JSTypeRegistry) -> bool {
        arrow_type::data(self.get_internal_arrow_type(reg), reg).return_type_inferred
    }
    // port: FunctionType#getInternalArrowType
    fn get_internal_arrow_type(self, reg: &JSTypeRegistry) -> TypeId {
        reg.function_data(self).call.expect("NullPointerException")
    }
    // port: FunctionType#getPrototypeProperty
    fn get_prototype_property(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        self.get_prototype(reg, ast)
    }
    // port: FunctionType#getPrototype
    fn get_prototype(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        if reg.function_data(self).prototype_slot.is_none() {
            if let Some(ref_name) = self.get_reference_name(reg) {
                let object_type = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
                let prototype = PrototypeObjectTypeBuilder::new()
                    .set_name(ref_name.concat(&".prototype".into()))
                    .set_implicit_prototype(Some(object_type))
                    .set_native(self.is_native_object_type(reg))
                    .build(reg, ast);
                self.set_prototype(reg, ast, Some(prototype), None);
            } else {
                let unknown_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
                set_prototype_no_check(self, reg, unknown_type, None);
            }
        }
        reg.function_data(self)
            .prototype_slot
            .unwrap()
            .get_type(reg)
    }
    // port: FunctionType#setPrototypeBasedOn
    fn set_prototype_based_on(self, reg: &mut JSTypeRegistry, ast: &Ast, base_type: TypeId) {
        set_prototype_based_on_with_node(self, reg, ast, base_type, None);
    }
    // port: FunctionType#setPrototype
    fn set_prototype(
        self,
        reg: &mut JSTypeRegistry,
        _ast: &Ast,
        prototype: Option<TypeId>,
        property_node: Option<NodeId>,
    ) -> bool {
        let Some(prototype) = prototype else {
            return false;
        };
        if self.is_constructor(reg) && Some(prototype) == self.get_instance_type(reg) {
            return false;
        }
        set_prototype_no_check(self, reg, prototype, property_node)
    }
    // port: FunctionType#getAllImplementedInterfaces
    fn get_all_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        let mut interfaces = Vec::new();
        for type_ in self.get_implemented_interfaces(reg, ast) {
            add_related_interfaces(type_, reg, ast, &mut interfaces);
        }
        interfaces.into_iter().map(|(_, type_)| type_).collect()
    }
    // port: FunctionType#getAncestorInterfaces
    fn get_ancestor_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        let types = if self.is_constructor(reg) {
            self.get_implemented_interfaces(reg, ast)
        } else {
            self.get_extended_interfaces(reg)
        };
        let mut result = Vec::new();
        for t in types {
            insert_structural(reg, ast, &mut result, t);
        }
        result.into_iter().map(|(_, type_)| type_).collect()
    }
    // port: FunctionType#getImplementedInterfaces
    fn get_implemented_interfaces(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        let mut super_ctor = if self.is_constructor(reg) {
            self.get_super_class_constructor(reg, ast)
        } else {
            None
        };
        let mut builder = reg.function_data(self).implemented_interfaces.clone();
        while let Some(current) = super_ctor {
            builder.extend(
                reg.function_data(current)
                    .implemented_interfaces
                    .iter()
                    .copied(),
            );
            super_ctor = current.get_super_class_constructor(reg, ast);
        }
        builder
    }
    // port: FunctionType#getOwnImplementedInterfaces
    fn get_own_implemented_interfaces(self, reg: &JSTypeRegistry) -> Vec<TypeId> {
        reg.function_data(self).implemented_interfaces.clone()
    }
    // port: FunctionType#setImplementedInterfaces
    fn set_implemented_interfaces(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        implemented_interfaces: Vec<TypeId>,
    ) {
        check_state!(self.is_constructor(reg));
        reg.function_data_mut(self).implemented_interfaces = implemented_interfaces.clone();
        for type_ in implemented_interfaces {
            let type_of_this = reg.function_data(self).type_of_this.unwrap();
            type_of_this.merge_supertype_template_types(reg, ast, type_);
        }
    }
    // port: FunctionType#getExtendedInterfaces
    fn get_extended_interfaces(self, reg: &JSTypeRegistry) -> Vec<TypeId> {
        reg.function_data(self).extended_interfaces.clone()
    }
    // port: FunctionType#getExtendedInterfacesCount
    fn get_extended_interfaces_count(self, reg: &JSTypeRegistry) -> usize {
        reg.function_data(self).extended_interfaces.len()
    }
    // port: FunctionType#setExtendedInterfaces
    fn set_extended_interfaces(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        extended_interfaces: Vec<TypeId>,
    ) {
        check_state!(self.is_interface(reg));
        reg.function_data_mut(self).extended_interfaces = extended_interfaces.clone();
        for type_ in extended_interfaces {
            let type_of_this = reg.function_data(self).type_of_this.unwrap();
            type_of_this.merge_supertype_template_types(reg, ast, type_);
        }
    }
    // port: FunctionType#getBindReturnType
    fn get_bind_return_type(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        args_to_bind: i32,
    ) -> TypeId {
        let template_keys = self.get_template_type_map(reg).get_template_keys().to_vec();
        let mut builder = FunctionTypeBuilder::new()
            .with_return_type(self.get_return_type(reg))
            .with_template_keys(reg, ast, template_keys);
        if args_to_bind < 0 {
            return builder.build(reg, ast);
        }
        let mut params = self.get_parameters(reg);
        for _ in 1..args_to_bind {
            if params.is_empty() || params[0].is_variadic() {
                break;
            }
            params.remove(0);
        }
        builder = builder.with_parameters(params);
        builder.build(reg, ast)
    }
    // port: FunctionType#supAndInfHelper
    fn sup_and_inf_helper(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        that: TypeId,
        least_super: bool,
    ) -> TypeId {
        if self.equals(reg, ast, that) {
            return self;
        }
        let self_call = self.get_internal_arrow_type(reg);
        let that_call = that.get_internal_arrow_type(reg);
        if self.is_ordinary_function(reg)
            && that.is_ordinary_function(reg)
            && !arrow_type::has_unknown_params_or_return(self_call, reg, ast)
            && !arrow_type::has_unknown_params_or_return(that_call, reg, ast)
        {
            let is_subtype_of_that = self.is_subtype_of(reg, ast, that);
            let is_subtype_of_this = that.is_subtype_of(reg, ast, self);
            if is_subtype_of_that && !is_subtype_of_this {
                return if least_super { that } else { self };
            } else if is_subtype_of_this && !is_subtype_of_that {
                return if least_super { self } else { that };
            }
            if let Some(merged) = try_merge_function_piecewise(self, reg, ast, that, least_super) {
                return merged;
            }
        }
        let function_instance = reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
        if function_instance.equals(reg, ast, that) {
            return if least_super { that } else { self };
        } else if function_instance.equals(reg, ast, self) {
            return if least_super { self } else { that };
        }
        reg.get_native_type(if least_super {
            JSTypeNative::FUNCTION_TYPE
        } else {
            JSTypeNative::LEAST_FUNCTION_TYPE
        })
    }
    // port: FunctionType#hasEqualCallType
    fn has_equal_call_type(self, reg: &mut JSTypeRegistry, ast: &Ast, that: TypeId) -> bool {
        let self_call = self.get_internal_arrow_type(reg);
        let that_call = that.get_internal_arrow_type(reg);
        crate::equality_checker::EqualityChecker::new()
            .set_eq_method(crate::equality_checker::EqMethod::IDENTITY)
            .check(reg, ast, self_call, that_call)
    }
    // port: FunctionType#getInstanceType
    fn get_instance_type(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        check_state!(self.has_instance_type(reg));
        reg.function_data(self)
            .type_of_this
            .and_then(|t| t.to_object_type(reg))
    }
    // port: FunctionType#hasInstanceType
    fn has_instance_type(self, reg: &JSTypeRegistry) -> bool {
        self.is_constructor(reg) || self.is_interface(reg)
    }
    // port: FunctionType#getSource
    fn get_source(self, reg: &JSTypeRegistry) -> Option<NodeId> {
        reg.function_data(self).source
    }
    // port: FunctionType#getGoogModuleId
    fn get_goog_module_id(self, reg: &JSTypeRegistry) -> Option<JsString> {
        reg.function_data(self).goog_module_id.clone()
    }
    #[allow(clippy::collapsible_if)] // Keep Java's prototype slot branch.
    // port: FunctionType#setSource
    fn set_source(self, reg: &mut JSTypeRegistry, source: Option<NodeId>) {
        if let Some(slot) = reg.function_data(self).prototype_slot {
            if source.is_none() || slot.get_node(reg).is_none() {
                let name = slot.get_name(reg);
                let type_ = slot.get_type(reg);
                let inferred = slot.is_type_inferred(reg);
                let replacement =
                    reg.alloc_property(PropertyData::new(name, type_, inferred, source));
                reg.function_data_mut(self).prototype_slot = Some(replacement);
            }
        }
        reg.function_data_mut(self).source = source;
    }
    // port: FunctionType#hasProperties
    fn has_properties(self, reg: &JSTypeRegistry) -> bool {
        !reg.prototype_data(self).properties.properties.is_empty()
    }
    // port: FunctionType#setImplicitMatch
    fn set_implicit_match(self, reg: &mut JSTypeRegistry, flag: bool) {
        check_state!(self.is_interface(reg));
        reg.function_data_mut(self).is_structural_interface = flag;
    }
    // port: FunctionType#isAbstract
    fn is_abstract(self, reg: &JSTypeRegistry) -> bool {
        reg.function_data(self).is_abstract
    }
    // port: FunctionType#checkExtendsLoop
    fn check_extends_loop(self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<Vec<TypeId>> {
        check_extends_loop_helper(self, reg, ast, &mut Vec::new(), &mut Vec::new())
    }
    // port: FunctionType#acceptsArguments
    fn accepts_arguments(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        argument_types: &[TypeId],
    ) -> bool {
        let parameters = self.get_parameters(reg);
        let mut parameter_index = 0;
        let mut parameter: Option<Parameter> = None;
        for &argument in argument_types {
            if parameter_index >= parameters.len() && !parameter.is_some_and(|p| p.is_variadic()) {
                break;
            }
            if parameter_index < parameters.len() {
                parameter = Some(parameters[parameter_index]);
                parameter_index += 1;
            }
            if !argument.is_subtype_of(reg, ast, parameter.unwrap().get_jstype()) {
                return false;
            }
        }
        self.get_min_arity(reg) <= argument_types.len()
            && argument_types.len() <= self.get_max_arity(reg)
    }
    // port: FunctionType#forgetParameterAndReturnTypes
    fn forget_parameter_and_return_types(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let result = FunctionTypeBuilder::new()
            .with_name_option(self.get_reference_name(reg))
            .with_source_node(self.get_source(reg))
            .with_type_of_this(self.get_instance_type(reg))
            .with_kind(self.get_kind(reg))
            .with_canonical_representation(Some(self))
            .build(reg, ast);
        result.set_prototype_based_on(reg, ast, self.get_instance_type(reg).unwrap());
        result
    }
    // port: FunctionType#getConstructorOnlyTemplateParameters
    fn get_constructor_only_template_parameters(self, reg: &JSTypeRegistry) -> Vec<TypeId> {
        check_state!(self.is_constructor(reg));
        if self.get_source(reg).is_none() {
            return Vec::new();
        }
        let map = self.get_template_type_map(reg);
        let count = self.get_template_param_count(reg)
            - self
                .get_instance_type(reg)
                .unwrap()
                .get_template_param_count(reg);
        map.get_template_keys()[map.size() - count..].to_vec()
    }
    // port: FunctionType#isAmbiguousConstructor
    fn is_ambiguous_constructor(self, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
        if reg.function_data(self).constructor_ambiguity == ConstructorAmbiguity::UNKNOWN {
            let value = calculate_constructor_ambiguity(self, reg, ast);
            reg.function_data_mut(self).constructor_ambiguity = value;
        }
        reg.function_data(self).constructor_ambiguity
            == ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
    }
    // port: FunctionType#getClosurePrimitive
    fn get_closure_primitive(self, reg: &JSTypeRegistry) -> Option<ClosurePrimitive> {
        reg.function_data(self).closure_primitive
    }
    // port: FunctionType#toBuilder
    fn to_builder(self, reg: &JSTypeRegistry) -> FunctionTypeBuilder {
        let mut builder = FunctionTypeBuilder::new()
            .set_goog_module_id(self.get_goog_module_id(reg))
            .with_name_option(self.get_reference_name(reg))
            .set_native(self.is_native_object_type(reg))
            .set_template_type_map(self.get_template_type_map(reg))
            .with_canonical_representation(self.get_canonical_representation(reg))
            .set_is_known_ambiguous(
                reg.function_data(self).constructor_ambiguity
                    == ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR,
            )
            .with_closure_primitive_id(self.get_closure_primitive(reg))
            .with_is_abstract(self.is_abstract(reg))
            .with_kind(self.get_kind(reg))
            .with_parameters(self.get_parameters(reg))
            .with_return_type_inferred(self.get_return_type(reg), self.is_return_type_inferred(reg))
            .with_source_node(self.get_source(reg))
            .with_type_of_this(get_type_of_this(self, reg))
            .set_template_param_count(self.get_template_param_count(reg));
        if self.is_constructor(reg) {
            builder = builder
                .with_constructor_template_keys(self.get_constructor_only_template_parameters(reg));
        }
        builder
    }
    // port: FunctionType#getCanonicalRepresentation
    fn get_canonical_representation(self, reg: &JSTypeRegistry) -> Option<TypeId> {
        reg.function_data(self).canonical_representation
    }
}

// port: FunctionType#getTypeClass
pub fn get_type_class(_t: TypeId, _reg: &JSTypeRegistry) -> crate::js_type_class::JSTypeClass {
    crate::js_type_class::JSTypeClass::FUNCTION
}
// port: FunctionType#getConstructor
pub fn get_constructor(_t: TypeId, reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(reg.get_native_type(JSTypeNative::FUNCTION_FUNCTION_TYPE))
}
// port: FunctionType#isInstanceType
pub fn is_instance_type(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t == reg.get_native_type(JSTypeNative::FUNCTION_TYPE)
}
// port: FunctionType#isConstructor
pub fn is_constructor(t: TypeId, reg: &JSTypeRegistry) -> bool {
    reg.function_data(t).kind == Kind::CONSTRUCTOR
}
// port: FunctionType#isInterface
pub fn is_interface(t: TypeId, reg: &JSTypeRegistry) -> bool {
    reg.function_data(t).kind == Kind::INTERFACE
}
// port: FunctionType#isOrdinaryFunction
pub fn is_ordinary_function(t: TypeId, reg: &JSTypeRegistry) -> bool {
    reg.function_data(t).kind == Kind::ORDINARY
}
// port: FunctionType#toMaybeFunctionType
pub fn to_maybe_function_type(t: TypeId, _reg: &JSTypeRegistry) -> Option<TypeId> {
    Some(t)
}
// port: FunctionType#canBeCalled
pub fn can_be_called(_t: TypeId, _reg: &JSTypeRegistry) -> bool {
    true
}
// port: FunctionType#getSlot
pub fn get_slot(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
) -> Option<PropertyId> {
    if name.matches("prototype") {
        t.get_prototype(reg, ast);
        reg.function_data(t).prototype_slot
    } else {
        crate::object_type::get_slot(t, reg, ast, name)
    }
}
// port: FunctionType#getOwnPropertyNames
pub fn get_own_property_names(
    t: TypeId,
    reg: &JSTypeRegistry,
) -> closure_rhino::fx_hash::IndexSet<JsString> {
    let mut names = closure_rhino::fx_hash::IndexSet::<_>::default();
    if reg.function_data(t).prototype_slot.is_some() {
        names.insert("prototype".into());
    }
    names.extend(reg.prototype_data(t).properties.get_own_property_names());
    names
}
// port: FunctionType#setPrototypeBasedOn
fn set_prototype_based_on_with_node(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    mut base_type: TypeId,
    property_node: Option<NodeId>,
) {
    if t.is_native_object_type(reg) {
        return;
    }
    if t.get_source(reg).is_some_and(|source| source.is_class(ast)) {
        if let Some(super_ctor) = base_type.get_constructor(reg) {
            t.set_implicit_prototype(reg, Some(super_ctor));
        }
        t.maybe_loosen_typechecking_due_to_forward_referenced_supertype(reg, base_type);
    }
    if base_type.has_reference_name(reg)
        || t.is_native_object_type(reg)
        || base_type.is_function_prototype_type(reg)
    {
        if reg.function_data(t).prototype_slot.is_some()
            && t.has_instance_type(reg)
            && base_type.equals(reg, ast, t.get_instance_type(reg).unwrap())
        {
            return;
        }
        let name = t
            .get_reference_name(reg)
            .unwrap_or_else(|| "null".into())
            .concat(&".prototype".into());
        base_type = PrototypeObjectTypeBuilder::new()
            .set_name(name)
            .set_implicit_prototype(Some(base_type))
            .build(reg, ast);
    }
    t.set_prototype(reg, ast, Some(base_type), property_node);
}
// port: FunctionType#setPrototypeNoCheck
fn set_prototype_no_check(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    prototype: TypeId,
    property_node: Option<NodeId>,
) -> bool {
    let old_prototype = reg.function_data(t).prototype_slot.map(|p| p.get_type(reg));
    let source = t.get_source(reg);
    let slot = reg.alloc_property(PropertyData::new(
        "prototype",
        prototype,
        true,
        property_node.or(source),
    ));
    reg.function_data_mut(t).prototype_slot = Some(slot);
    prototype.set_owner_function(reg, Some(t));
    if let Some(old_prototype) = old_prototype {
        old_prototype.set_owner_function(reg, None);
        t.clear_cached_values(reg);
    }
    true
}
// Java LinkedHashSet<JSType> is insertion ordered and retains the hash computed on insertion.
fn contains_structural(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    set: &[(i32, TypeId)],
    instance: TypeId,
) -> bool {
    let hash = instance.hash_code(reg);
    set.iter().any(|&(stored_hash, item)| {
        stored_hash == hash && (item == instance || instance.equals(reg, ast, item))
    })
}
fn insert_structural(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    set: &mut Vec<(i32, TypeId)>,
    instance: TypeId,
) -> bool {
    let hash = instance.hash_code(reg);
    if set.iter().any(|&(stored_hash, item)| {
        stored_hash == hash && (item == instance || instance.equals(reg, ast, item))
    }) {
        false
    } else {
        set.push((hash, instance));
        true
    }
}
fn remove_structural(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    set: &mut Vec<(i32, TypeId)>,
    instance: TypeId,
) {
    let hash = instance.hash_code(reg);
    if let Some(index) = set.iter().position(|&(stored_hash, item)| {
        stored_hash == hash && (item == instance || instance.equals(reg, ast, item))
    }) {
        set.remove(index);
    }
}
// port: FunctionType#addRelatedInterfaces
fn add_related_interfaces(
    instance: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    set: &mut Vec<(i32, TypeId)>,
) {
    if let Some(constructor) = instance.get_constructor(reg) {
        if !constructor.is_interface(reg) || !insert_structural(reg, ast, set, instance) {
            return;
        }
        for interface_type in instance.get_ctor_extended_interfaces(reg, ast) {
            add_related_interfaces(interface_type, reg, ast, set);
        }
    }
}
// port: FunctionType#getPropertyType
pub fn get_property_type(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
) -> TypeId {
    if !t.has_own_property(reg, ast, name.clone()) {
        let is_call = name.matches("call");
        let is_bind = name.matches("bind");
        if is_call || is_bind {
            let type_ = get_call_or_bind_signature(t, reg, ast, is_call);
            let source = t.get_source(reg);
            t.define_declared_property(reg, ast, name.clone(), type_, source);
        } else if name.matches("apply") {
            let mut builder = crate::function_param_builder::FunctionParamBuilder::new();
            let this_type = reg.create_nullable_type(ast, get_type_of_this(t, reg));
            let object = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
            let object_type = reg.create_nullable_type(ast, object);
            builder.add_optional_params(reg, ast, &[this_type, object_type]);
            let template_keys = t.get_template_type_map(reg).get_template_keys().to_vec();
            let signature = FunctionTypeBuilder::new()
                .with_parameters(builder.build())
                .with_return_type(t.get_return_type(reg))
                .with_template_keys(reg, ast, template_keys)
                .build(reg, ast);
            let source = t.get_source(reg);
            t.define_declared_property(reg, ast, name.clone(), signature, source);
        }
    }
    crate::object_type::get_property_type(t, reg, ast, name)
}
// port: FunctionType#getCallOrBindSignature
fn get_call_or_bind_signature(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    is_call: bool,
) -> TypeId {
    let is_bind = !is_call;
    let return_type = if is_call {
        t.get_return_type(reg)
    } else {
        t.get_bind_return_type(reg, ast, -1)
    };
    let template_keys = t.get_template_type_map(reg).get_template_keys().to_vec();
    let builder = FunctionTypeBuilder::new()
        .with_return_type(return_type)
        .with_template_keys(reg, ast, template_keys);
    let mut params = t.get_parameters(reg);
    let this_type = reg.create_optional_nullable_type(ast, get_type_of_this(t, reg));
    params.insert(0, Parameter::create(this_type, false, false));
    if is_bind {
        for current in &mut params[1..] {
            *current = Parameter::create(current.get_jstype(), true, current.is_variadic());
        }
    } else if params
        .get(1)
        .is_none_or(|p| p.is_optional() || p.is_variadic())
    {
        params[0] = Parameter::create(this_type, true, false);
    }
    builder.with_parameters(params).build(reg, ast)
}
#[allow(clippy::collapsible_if)] // Keep Java's property and prototype slot branches.
// port: FunctionType#defineProperty
pub fn define_property(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    name: &PropertyKey,
    type_: TypeId,
    inferred: bool,
    node: Option<NodeId>,
) -> bool {
    if name.matches("prototype") {
        if let Some(obj_type) = type_.to_object_type(reg) {
            if let Some(slot) = reg.function_data(t).prototype_slot {
                if obj_type.equals(reg, ast, slot.get_type(reg)) {
                    return true;
                }
            }
            set_prototype_based_on_with_node(t, reg, ast, obj_type, node);
            true
        } else {
            false
        }
    } else {
        crate::prototype_object_type::define_property(t, reg, ast, name, type_, inferred, node)
    }
}
// port: FunctionType#tryMergeFunctionPiecewise
fn try_merge_function_piecewise(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    other: TypeId,
    least_super: bool,
) -> Option<TypeId> {
    let t_call = t.get_internal_arrow_type(reg);
    let other_call = other.get_internal_arrow_type(reg);
    if !crate::equality_checker::EqualityChecker::new()
        .set_eq_method(crate::equality_checker::EqMethod::IDENTITY)
        .check_parameters(reg, ast, t_call, other_call)
    {
        return None;
    }
    let params = t.get_parameters(reg);
    let new_return_type = if least_super {
        t.get_return_type(reg)
            .get_least_supertype(reg, ast, other.get_return_type(reg))
    } else {
        t.get_return_type(reg)
            .get_greatest_subtype(reg, ast, other.get_return_type(reg))
    };
    let this_type = reg.function_data(t).type_of_this.unwrap();
    let other_this = reg.function_data(other).type_of_this.unwrap();
    let new_this = if this_type.equals(reg, ast, other_this) {
        this_type
    } else if least_super {
        this_type.get_least_supertype(reg, ast, other_this)
    } else {
        this_type.get_greatest_subtype(reg, ast, other_this)
    };
    let inferred = t.is_return_type_inferred(reg) || other.is_return_type_inferred(reg);
    Some(
        FunctionTypeBuilder::new()
            .with_parameters(params)
            .with_return_type_inferred(new_return_type, inferred)
            .with_type_of_this(new_this)
            .build(reg, ast),
    )
}
// port: FunctionType#getSuperClassConstructor
pub fn get_super_class_constructor(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> Option<TypeId> {
    check_argument!(t.is_constructor(reg) || t.is_interface(reg));
    t.get_prototype(reg, ast)
        .get_implicit_prototype(reg, ast)
        .and_then(|p| p.get_constructor(reg))
}
// port: FunctionType#recursionUnsafeHashCode
pub fn recursion_unsafe_hash_code(t: TypeId, reg: &JSTypeRegistry) -> i32 {
    let d = reg.function_data(t);
    let mut hc = d.kind as i32;
    match d.kind {
        Kind::CONSTRUCTOR | Kind::INTERFACE => hc.wrapping_mul(31).wrapping_add(t.0 as i32),
        Kind::ORDINARY => {
            hc = hc
                .wrapping_mul(31)
                .wrapping_add(d.type_of_this.map_or(0, |t| t.hash_code(reg)));
            hc = hc
                .wrapping_mul(31)
                .wrapping_add(d.call.map_or(0, |t| t.hash_code(reg)));
            hc.wrapping_mul(31)
                .wrapping_add(d.closure_primitive.map_or(0, |p| p as i32))
        }
        Kind::NONE => panic!("AssertionError"),
    }
}
// port: FunctionType#appendTo
pub fn append_to(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast, sb: &mut TypeStringBuilder) {
    if !t.is_pretty_print(reg) || t == reg.get_native_type(JSTypeNative::FUNCTION_TYPE) {
        sb.append(if sb.is_for_annotations() {
            "!Function"
        } else {
            "Function"
        });
        return;
    }
    if t.has_instance_type(reg) && t.get_source(reg).is_some() {
        sb.append("(typeof ")
            .append_type(reg, ast, t.get_instance_type(reg).unwrap())
            .append(")");
        return;
    }
    t.set_pretty_print(reg, false);
    sb.append("function(");
    let params = t.get_parameters(reg);
    let type_of_this = reg.function_data(t).type_of_this.unwrap();
    let known = !matches!(reg.data(type_of_this).kind, JSTypeKind::Unknown(_));
    if known {
        sb.append(if t.is_constructor(reg) {
            "new:"
        } else {
            "this:"
        })
        .append_type(reg, ast, type_of_this);
    }
    if !params.is_empty() {
        if known {
            sb.append(", ");
        }
        append_arg_string(reg, ast, sb, &params[0]);
        for p in &params[1..] {
            sb.append(", ");
            append_arg_string(reg, ast, sb, p);
        }
    }
    sb.append("): ")
        .append_non_null(reg, ast, t.get_return_type(reg));
    t.set_pretty_print(reg, true);
}
// port: FunctionType#appendArgString
fn append_arg_string(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
    p: &Parameter,
) {
    if p.is_variadic() {
        append_var_args_string(reg, ast, sb, p.get_jstype());
    } else if p.is_optional() {
        append_optional_arg_string(reg, ast, sb, p.get_jstype());
    } else {
        sb.append_non_null(reg, ast, p.get_jstype());
    }
}
// port: FunctionType#appendVarArgsString
fn append_var_args_string(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
    param_type: TypeId,
) {
    sb.append("...").append_non_null(reg, ast, param_type);
}
// port: FunctionType#appendOptionalArgString
fn append_optional_arg_string(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    sb: &mut TypeStringBuilder,
    mut param_type: TypeId,
) {
    if param_type.is_union_type(reg) {
        let void = reg.get_native_type(JSTypeNative::VOID_TYPE);
        param_type = param_type.get_restricted_union(reg, ast, void);
    }
    sb.append_non_null(reg, ast, param_type).append("=");
}
// port: FunctionType#visit
pub fn visit<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::visitor::Visitor<T>,
) -> T {
    visitor.case_function_type(reg, ast, t)
}
// port: FunctionType#visit
pub fn visit_relationship<T>(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    visitor: &mut impl crate::relationship_visitor::RelationshipVisitor<T>,
    that: TypeId,
) -> T {
    visitor.case_function_type(reg, ast, t, that)
}
// port: FunctionType#getTypeOfThis
pub fn get_type_of_this(t: TypeId, reg: &JSTypeRegistry) -> TypeId {
    let type_of_this = reg
        .function_data(t)
        .type_of_this
        .expect("NullPointerException");
    if type_of_this.is_empty_type(reg) {
        reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)
    } else {
        type_of_this
    }
}
#[allow(clippy::collapsible_if)] // Keep Java's hasInstanceType branch.
// port: FunctionType#clearCachedValues
pub fn clear_cached_values(t: TypeId, reg: &mut JSTypeRegistry) {
    reg.object_data_mut(t).unknown = true;
    if !t.is_native_object_type(reg) {
        if t.has_instance_type(reg) {
            if let Some(instance) = t.get_instance_type(reg) {
                instance.clear_cached_values(reg);
            }
        }
        if let Some(slot) = reg.function_data(t).prototype_slot {
            let prototype = slot.get_type(reg);
            prototype.clear_cached_values(reg);
        }
    }
}
// port: FunctionType#hasCachedValues
pub fn has_cached_values(t: TypeId, reg: &JSTypeRegistry) -> bool {
    reg.function_data(t).prototype_slot.is_some() || !reg.object_data(t).unknown
}
// port: FunctionType#resolveInternal
pub fn resolve_internal(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> TypeId {
    if let Some(call) = reg.function_data(t).call {
        let resolved = call.resolve_with_reporter(reg, ast, reporter);
        reg.function_data_mut(t).call = Some(resolved);
    }
    if let Some(slot) = reg.function_data(t).prototype_slot {
        let type_ = slot.get_type(reg).resolve_with_reporter(reg, ast, reporter);
        slot.set_type(reg, type_);
    }
    if let Some(type_of_this) = reg.function_data(t).type_of_this {
        let mut maybe_type_of_this = Some(type_of_this.resolve_with_reporter(reg, ast, reporter));
        if let Some(type_) = maybe_type_of_this {
            if type_.is_null_type(reg) || type_.is_void_type(reg) {
                reg.function_data_mut(t).type_of_this = Some(type_);
            } else {
                let restricted = type_.restrict_by_not_null_or_undefined(reg, ast);
                maybe_type_of_this = restricted.to_object_type(reg);
                if maybe_type_of_this.is_some() {
                    reg.function_data_mut(t).type_of_this = maybe_type_of_this;
                }
            }
        }
    }
    let implemented = reg.function_data(t).implemented_interfaces.clone();
    if let Some(resolved) = resolve_type_list_helper(t, reg, ast, implemented, reporter) {
        reg.function_data_mut(t).implemented_interfaces = resolved;
    }
    let extended = reg.function_data(t).extended_interfaces.clone();
    if let Some(resolved) = resolve_type_list_helper(t, reg, ast, extended, reporter) {
        reg.function_data_mut(t).extended_interfaces = resolved;
    }
    crate::prototype_object_type::resolve_internal(t, reg, ast, reporter)
}
// port: FunctionType#resolveTypeListHelper
fn resolve_type_list_helper(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    list: Vec<TypeId>,
    reporter: &mut dyn closure_rhino::error_reporter::ErrorReporter,
) -> Option<Vec<TypeId>> {
    let mut changed = false;
    let mut resolved_list = Vec::new();
    for type_ in list {
        let rt = type_.resolve_with_reporter(reg, ast, reporter);
        if !rt.is_object_type(reg, ast) {
            let message = format!(
                "not an object type: {} (at {})",
                rt.to_string(reg, ast),
                t.to_string(reg, ast)
            );
            let source = t.get_source(reg).expect("NullPointerException");
            reporter.warning(
                &message,
                &source.get_source_file_name(ast).unwrap_or_default(),
                source.get_lineno(ast),
                source.get_charno(ast),
            );
            continue;
        }
        let resolved = rt.to_object_type(reg).unwrap();
        resolved_list.push(resolved);
        changed |= resolved != type_;
    }
    if changed { Some(resolved_list) } else { None }
}
// port: FunctionType#hasAnyTemplateTypesInternal
pub fn has_any_template_types_internal(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) -> bool {
    let type_of_this = reg.function_data(t).type_of_this.unwrap();
    let call = reg.function_data(t).call.unwrap();
    t.get_template_param_count(reg) > 0
        || type_of_this.has_any_template_types(reg, ast)
        || call.has_any_template_types(reg, ast)
}
// port: FunctionType#isStructuralInterface
pub fn is_structural_interface(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.is_interface(reg) && reg.function_data(t).is_structural_interface
}
// port: FunctionType#getPropertyTypeMap
pub fn get_property_type_map(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> closure_rhino::fx_hash::IndexMap<JsString, TypeId> {
    let mut map = closure_rhino::fx_hash::IndexMap::<_, _>::default();
    update_property_type_map(t, reg, ast, &mut map, &mut Vec::new());
    map
}
// port: FunctionType#updatePropertyTypeMap
fn update_property_type_map(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    map: &mut closure_rhino::fx_hash::IndexMap<JsString, TypeId>,
    cache: &mut Vec<(i32, TypeId)>,
) {
    let prototype = t.get_prototype(reg, ast);
    for name in prototype.get_own_property_names(reg) {
        if !map.contains_key(&name) {
            let type_ = prototype.get_property_type(reg, ast, &name);
            map.insert(name, type_);
        }
    }
    for interface_type in t.get_extended_interfaces(reg) {
        let Some(super_constructor) = interface_type.get_constructor(reg) else {
            continue;
        };
        if contains_structural(reg, ast, cache, super_constructor) {
            continue;
        }
        insert_structural(reg, ast, cache, super_constructor);
        update_property_type_map(super_constructor, reg, ast, map, cache);
        remove_structural(reg, ast, cache, super_constructor);
    }
}
// port: FunctionType#checkExtendsLoop
fn check_extends_loop_helper(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    cache: &mut Vec<(i32, TypeId)>,
    path: &mut Vec<TypeId>,
) -> Option<Vec<TypeId>> {
    for interface_type in t.get_extended_interfaces(reg) {
        let Some(super_constructor) = interface_type.get_constructor(reg) else {
            continue;
        };
        if contains_structural(reg, ast, cache, super_constructor) {
            path.push(super_constructor);
            while path[0] != super_constructor {
                path.remove(0);
            }
            return Some(path.clone());
        }
        insert_structural(reg, ast, cache, super_constructor);
        path.push(super_constructor);
        if let Some(result) = check_extends_loop_helper(super_constructor, reg, ast, cache, path) {
            return Some(result);
        }
        remove_structural(reg, ast, cache, super_constructor);
        path.pop();
    }
    None
}
// port: FunctionType#calculateConstructorAmbiguity
fn calculate_constructor_ambiguity(
    t: TypeId,
    reg: &mut JSTypeRegistry,
    ast: &Ast,
) -> ConstructorAmbiguity {
    if t.is_unknown_type(reg, ast) {
        ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
    } else if t.is_native_object_type(reg) {
        ConstructorAmbiguity::IS_UNAMBIGUOUS_CONSTRUCTOR
    } else if let Some(super_constructor) = t.get_super_class_constructor(reg, ast) {
        if super_constructor.is_ambiguous_constructor(reg, ast) {
            ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
        } else if t.get_source(reg).is_some() || is_delegate_proxy(t, reg) {
            ConstructorAmbiguity::IS_UNAMBIGUOUS_CONSTRUCTOR
        } else {
            ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
        }
    } else {
        ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR
    }
}
// port: FunctionType#isDelegateProxy
fn is_delegate_proxy(t: TypeId, reg: &JSTypeRegistry) -> bool {
    t.get_reference_name(reg)
        .is_some_and(|name| name.ends_with("(Proxy)"))
}
