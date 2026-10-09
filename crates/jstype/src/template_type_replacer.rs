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
 *   John Lenz
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
//   src/com/google/javascript/rhino/jstype/TemplateTypeReplacer.java.

use crate::TypeId;
use crate::function_param_builder::FunctionParamBuilder;
use crate::function_type::FunctionType;
use crate::js_type::JSType;
use crate::js_type_native::JSTypeNative;
use crate::js_type_registry::JSTypeRegistry;
use crate::object_type::ObjectType;
use crate::proxy_object_type::ProxyObjectType;
use crate::record_type_builder::RecordTypeBuilder;
use crate::template_type_map::TemplateTypeMap;
use crate::templatized_type::TemplatizedType;
use crate::union_type::UnionType;
use crate::visitor::Visitor;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{check_state, node::Ast};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TemplateTypeReplacer {
    bindings: Arc<TemplateTypeMap>,
    visit_properties: bool,
    use_unknown_for_missing_keys: bool,
    use_unknown_for_missing_values: bool,
    has_made_replacement: bool,
    key_type: Option<TypeId>,
    own_sub_map_boundary: i32,
    // Java identity sets/maps are lookup-only; insertion order cannot affect replacement output.
    seen_types: Option<IndexSet<TypeId>>,
    visited_object_types: Option<IndexMap<TypeId, TypeId>>,
}

impl TemplateTypeReplacer {
    // port: TemplateTypeReplacer#forInference
    pub fn for_inference(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        bindings: &IndexMap<TypeId, Option<TypeId>>,
    ) -> Self {
        let keys = bindings.keys().copied().collect::<Vec<_>>();
        let values = bindings
            .values()
            .map(|value| value.unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE)))
            .collect::<Vec<_>>();
        let empty = reg.get_empty_template_type_map();
        let map = empty.copy_with_extension(reg, ast, &keys, &values);
        Self::new(map, true, true, true)
    }
    // port: TemplateTypeReplacer#forTotalReplacement
    pub fn for_total_replacement(bindings: Arc<TemplateTypeMap>) -> Self {
        Self::new(bindings, false, false, true)
    }
    // port: TemplateTypeReplacer#forPartialReplacement
    pub fn for_partial_replacement(bindings: Arc<TemplateTypeMap>) -> Self {
        Self::new(bindings, false, false, false)
    }
    // port: TemplateTypeReplacer#TemplateTypeReplacer
    fn new(
        bindings: Arc<TemplateTypeMap>,
        visit_properties: bool,
        use_unknown_for_missing_keys: bool,
        use_unknown_for_missing_values: bool,
    ) -> Self {
        Self {
            bindings,
            visit_properties,
            use_unknown_for_missing_keys,
            use_unknown_for_missing_values,
            has_made_replacement: false,
            key_type: None,
            own_sub_map_boundary: -1,
            seen_types: None,
            visited_object_types: None,
        }
    }
    // port: TemplateTypeReplacer#hasMadeReplacement
    pub fn has_made_replacement(&self) -> bool {
        self.has_made_replacement
    }
    // port: TemplateTypeReplacer#initSeenTypes
    fn init_seen_types(&mut self) {
        if self.seen_types.is_none() {
            self.seen_types = Some(IndexSet::<_>::default());
        }
    }
    // port: TemplateTypeReplacer#caseFunctionTypeUnguarded
    fn case_function_type_unguarded(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        if self.is_native_function_type(reg, type_) {
            return type_;
        }
        if !type_.is_ordinary_function(reg) && !type_.is_constructor(reg) {
            return type_;
        }
        let mut changed = false;
        let before_this = type_.get_type_of_this(reg).expect("");
        let visited_this = before_this.visit(reg, ast, self);
        let after_this = self.coerse_to_this_type(reg, Some(visited_this));
        if before_this != after_this {
            changed = true;
        }
        let before_return = type_.get_return_type(reg);
        let after_return = before_return.visit(reg, ast, self);
        if before_return != after_return {
            changed = true;
        }
        let mut params_changed = false;
        let parameters = type_.get_parameters(reg);
        let mut param_builder = FunctionParamBuilder::with_capacity(parameters.len());
        for parameter in &parameters {
            let before_param_type = parameter.get_jstype();
            let after_param_type = before_param_type.visit(reg, ast, self);
            if before_param_type != after_param_type {
                changed = true;
                params_changed = true;
                if parameter.is_optional() {
                    param_builder.add_optional_params(reg, ast, &[after_param_type]);
                } else if parameter.is_variadic() {
                    param_builder.add_var_args(after_param_type);
                } else {
                    param_builder.add_required_params(&[after_param_type]);
                }
            } else {
                param_builder.new_parameter_from(*parameter);
            }
        }
        if changed {
            let parameters = if params_changed {
                param_builder.build()
            } else {
                parameters
            };
            return type_
                .to_builder(reg)
                .with_parameters(parameters)
                .with_return_type(after_return)
                .with_type_of_this(after_this)
                .with_is_abstract(false)
                .build(reg, ast);
        }
        type_
    }
    // port: TemplateTypeReplacer#coerseToThisType
    fn coerse_to_this_type(&self, reg: &JSTypeRegistry, type_: Option<TypeId>) -> TypeId {
        type_.unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE))
    }
    // port: TemplateTypeReplacer#caseObjectTypeUnguarded
    fn case_object_type_unguarded(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        obj_type: TypeId,
    ) -> TypeId {
        if !self.visit_properties
            || obj_type.is_nominal_type(reg)
            || matches!(
                &reg.data(obj_type).kind,
                crate::js_type::JSTypeKind::ProxyObject(_)
                    | crate::js_type::JSTypeKind::Named(_)
                    | crate::js_type::JSTypeKind::Template(_)
                    | crate::js_type::JSTypeKind::Templatized(_)
            )
            || !obj_type.is_record_type(reg)
        {
            return obj_type;
        }
        if let Some(cached) = self.get_visited_object_type_or_null(obj_type) {
            return cached;
        }
        let mut changed = false;
        let mut builder = RecordTypeBuilder::new();
        for prop in obj_type.get_own_property_names(reg) {
            let property_node = obj_type.get_property_node(reg, ast, &prop);
            let before_type = obj_type.get_property_type(reg, ast, &prop);
            let after_type = before_type.visit(reg, ast, self);
            if before_type != after_type {
                changed = true;
            }
            builder.add_property(prop, after_type, property_node);
        }
        let result = if changed {
            builder.build(reg, ast)
        } else {
            obj_type
        };
        self.visited_object_types
            .as_mut()
            .unwrap()
            .insert(obj_type, result);
        result
    }
    // port: TemplateTypeReplacer#caseTemplatizedTypeUnguarded
    fn case_templatized_type_unguarded(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut type_: TypeId,
    ) -> TypeId {
        let mut changed = false;
        let before_base_type = type_.get_referenced_type(reg);
        let after_base_type = before_base_type
            .visit(reg, ast, self)
            .to_maybe_object_type(reg)
            .expect("ClassCastException");
        if before_base_type != after_base_type {
            changed = true;
        }
        let mut builder = Vec::new();
        for before_template_type in type_.get_template_types(reg).unwrap() {
            let after_template_type = before_template_type.visit(reg, ast, self);
            if before_template_type != after_template_type {
                changed = true;
            }
            builder.push(after_template_type);
        }
        if changed {
            type_ = reg.create_templatized_type(ast, after_base_type, &builder);
        }
        type_
    }
    // port: TemplateTypeReplacer#caseUnionTypeUnguarded
    fn case_union_type_unguarded(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        let mut changed = false;
        let mut results = Vec::new();
        for alternative in type_.get_alternates(reg, ast).iter().copied() {
            let replacement = alternative.visit(reg, ast, self);
            if replacement != alternative {
                changed = true;
            }
            results.push(replacement);
        }
        if changed {
            reg.create_union_type(ast, &results)
        } else {
            type_
        }
    }
    // port: TemplateTypeReplacer#getNativeType
    fn get_native_type(&self, reg: &JSTypeRegistry, native_type: JSTypeNative) -> TypeId {
        reg.get_native_type(native_type)
    }
    // port: TemplateTypeReplacer#isNativeFunctionType
    fn is_native_function_type(&self, reg: &JSTypeRegistry, type_: TypeId) -> bool {
        type_.is_native_object_type(reg)
    }
    // port: TemplateTypeReplacer#caseProxyObjectTypeUnguarded
    fn case_proxy_object_type_unguarded(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        let before_type = type_.get_referenced_type_internal(reg);
        let replacement = before_type.visit(reg, ast, self);
        if replacement != before_type {
            replacement
        } else {
            type_
        }
    }
    // port: TemplateTypeReplacer#setKeyType
    pub(crate) fn set_key_type(&mut self, key_type: TypeId, own_sub_map_boundary: i32) {
        self.key_type = Some(key_type);
        self.own_sub_map_boundary = own_sub_map_boundary;
    }
    // port: TemplateTypeReplacer#guardAgainstCycles
    fn guard_against_cycles(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
        mapper: fn(&mut Self, &mut JSTypeRegistry, &Ast, TypeId) -> TypeId,
    ) -> TypeId {
        self.init_seen_types();
        if !self.seen_types.as_mut().unwrap().insert(type_) {
            return type_;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mapper(self, reg, ast, type_)
        }));
        self.seen_types.as_mut().unwrap().shift_remove(&type_);
        match result {
            Ok(result) => result,
            Err(error) => std::panic::resume_unwind(error),
        }
    }
    // port: TemplateTypeReplacer#getVisitedObjectTypeOrNull
    fn get_visited_object_type_or_null(&mut self, type_: TypeId) -> Option<TypeId> {
        match &mut self.visited_object_types {
            Some(visited) => visited.get(&type_).copied(),
            None => {
                self.visited_object_types = Some(IndexMap::<_, _>::default());
                None
            }
        }
    }
}

impl Visitor<TypeId> for TemplateTypeReplacer {
    // port: TemplateTypeReplacer#caseNoType
    fn case_no_type(&mut self, _reg: &mut JSTypeRegistry, _ast: &Ast, type_: TypeId) -> TypeId {
        type_
    }
    // port: TemplateTypeReplacer#caseEnumElementType
    fn case_enum_element_type(
        &mut self,
        _reg: &mut JSTypeRegistry,
        _ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        type_
    }
    // port: TemplateTypeReplacer#caseAllType
    fn case_all_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::ALL_TYPE)
    }
    // port: TemplateTypeReplacer#caseBooleanType
    fn case_boolean_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::BOOLEAN_TYPE)
    }
    // port: TemplateTypeReplacer#caseNoObjectType
    fn case_no_object_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::NO_OBJECT_TYPE)
    }
    // port: TemplateTypeReplacer#caseFunctionType
    fn case_function_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        self.guard_against_cycles(reg, ast, type_, Self::case_function_type_unguarded)
    }
    // port: TemplateTypeReplacer#caseObjectType
    fn case_object_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        self.guard_against_cycles(reg, ast, type_, Self::case_object_type_unguarded)
    }
    // port: TemplateTypeReplacer#caseTemplatizedType
    fn case_templatized_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        self.guard_against_cycles(reg, ast, type_, Self::case_templatized_type_unguarded)
    }
    // port: TemplateTypeReplacer#caseUnknownType
    fn case_unknown_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::UNKNOWN_TYPE)
    }
    // port: TemplateTypeReplacer#caseNullType
    fn case_null_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::NULL_TYPE)
    }
    // port: TemplateTypeReplacer#caseNumberType
    fn case_number_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::NUMBER_TYPE)
    }
    // port: TemplateTypeReplacer#caseBigIntType
    fn case_big_int_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::BIGINT_TYPE)
    }
    // port: TemplateTypeReplacer#caseStringType
    fn case_string_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::STRING_TYPE)
    }
    // port: TemplateTypeReplacer#caseSymbolType
    fn case_symbol_type(&mut self, _reg: &mut JSTypeRegistry, _ast: &Ast, type_: TypeId) -> TypeId {
        type_
    }
    // port: TemplateTypeReplacer#caseVoidType
    fn case_void_type(&mut self, reg: &mut JSTypeRegistry, _ast: &Ast) -> TypeId {
        self.get_native_type(reg, JSTypeNative::VOID_TYPE)
    }
    // port: TemplateTypeReplacer#caseUnionType
    fn case_union_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        self.guard_against_cycles(reg, ast, type_, Self::case_union_type_unguarded)
    }
    // port: TemplateTypeReplacer#caseTemplateType
    fn case_template_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        self.has_made_replacement = true;
        if Some(type_) != self.key_type
            && !self
                .bindings
                .has_template_key_from(type_, self.own_sub_map_boundary)
        {
            return if self.use_unknown_for_missing_keys {
                self.get_native_type(reg, JSTypeNative::UNKNOWN_TYPE)
            } else {
                type_
            };
        }
        self.init_seen_types();
        if self.seen_types.as_ref().unwrap().contains(&type_) {
            return type_;
        }
        if !self.bindings.has_template_type(type_) {
            return if self.use_unknown_for_missing_values {
                self.get_native_type(reg, JSTypeNative::UNKNOWN_TYPE)
            } else {
                type_
            };
        }
        let replacement = self
            .bindings
            .get_unresolved_original_template_type(reg, type_);
        if Some(replacement) == self.key_type
            || self.seen_types.as_ref().unwrap().contains(&replacement)
        {
            return type_;
        }
        self.seen_types.as_mut().unwrap().insert(type_);
        let visited_replacement = replacement.visit(reg, ast, self);
        self.seen_types.as_mut().unwrap().shift_remove(&type_);
        check_state!(
            Some(visited_replacement) != self.key_type,
            "Trying to replace key %s with the same value",
            self.key_type
                .map(|key| key.to_string(reg, ast))
                .unwrap_or_else(|| "null".into())
        );
        visited_replacement
    }
    // port: TemplateTypeReplacer#caseNamedType
    fn case_named_type(&mut self, _reg: &mut JSTypeRegistry, _ast: &Ast, type_: TypeId) -> TypeId {
        type_
    }
    // port: TemplateTypeReplacer#caseProxyObjectType
    fn case_proxy_object_type(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> TypeId {
        self.guard_against_cycles(reg, ast, type_, Self::case_proxy_object_type_unguarded)
    }
}
