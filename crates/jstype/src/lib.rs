/*
 * Copyright 2026 The closure-rs Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]
// Arena handles use the Java receiver-first API; constructors return TypeId.
#![allow(clippy::wrong_self_convention, clippy::new_ret_no_self)]
//! Port of com.google.javascript.rhino.jstype.

pub use closure_rhino::jstype::TypeId;

pub mod all_type;
pub mod arrow_type;
pub mod big_int_type;
pub mod boolean_literal_set;
pub mod boolean_type;
pub mod can_cast_to_visitor;
pub mod contains_upper_bound_super_type_visitor;
pub mod enum_element_type;
pub mod enum_type;
pub mod equality_checker;
pub mod equivalence_method;
pub mod function_param_builder;
pub mod function_type;
pub mod instance_object_type;
pub mod js_type;
pub mod js_type_class;
pub mod js_type_iterations;
pub mod js_type_native;
pub mod js_type_registry;
pub mod js_type_resolver;
pub mod known_symbol_type;
pub mod named_type;
pub mod no_object_type;
pub mod no_resolved_type;
pub mod no_type;
pub mod null_type;
pub mod number_type;
pub mod object_type;
pub mod property;
pub mod property_map;
pub mod prototype_object_type;
pub mod proxy_object_type;
pub mod record_type;
pub mod record_type_builder;
pub mod relationship_visitor;
pub mod rhino;
pub mod simple_reference;
pub mod simple_slot;
pub mod static_typed_ref;
pub mod static_typed_scope;
pub mod static_typed_slot;
pub mod string_type;
pub mod subtype_checker;
pub mod symbol_type;
pub mod template_type;
pub mod template_type_map;
pub mod template_type_replacer;
pub mod templatized_type;
pub mod testing;
pub mod type_string_builder;
pub mod union_type;
pub mod unknown_type;
pub mod value_type;
pub mod visitor;
pub mod void_type;
pub use js_type_native::JSTypeNative;
pub use js_type_registry::JSTypeRegistry;
pub mod prelude {
    pub use crate::enum_element_type::EnumElementType;
    pub use crate::enum_type::EnumType;
    pub use crate::function_type::FunctionType;
    pub use crate::js_type::JSType;
    pub use crate::named_type::NamedType;
    pub use crate::object_type::ObjectType;
    pub use crate::prototype_object_type::PrototypeObjectType;
    pub use crate::proxy_object_type::ProxyObjectType;
    pub use crate::record_type::RecordType;
    pub use crate::template_type::TemplateType;
    pub use crate::templatized_type::TemplatizedType;
    pub use crate::union_type::UnionType;
    pub use crate::{JSTypeNative, JSTypeRegistry, TypeId};
}
