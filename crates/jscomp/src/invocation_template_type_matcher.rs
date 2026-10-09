/*
 * Copyright 2007 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/InvocationTemplateTypeMatcher.java.

//! Determines the types that fill any template parameters at a function invocation.
//!
//! Given an invocation of some function with type `F` templated on `T`, this class traverses the
//! arguments of the invocation to determine what type `T` should be. Argument types and `F`'s
//! parameter types are traversed/recursed in parallel to match template types at any depth inside
//! `F`'s signature.
//!
//! Instances of this class are single use. They provide a "scope" for the matching but accumulate
//! state as the matching progresses.
//!
//! Java's `com.google.javascript.jscomp.InvocationTemplateTypeMatcher`.
use crate::node_util::NodeUtil;
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    function_type::{self, Parameter},
    prelude::{FunctionType, JSType, ObjectType, UnionType},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    jscomp_base::{
        linked_identity_hash_map::LinkedIdentityHashMap,
        linked_identity_hash_set::LinkedIdentityHashSet,
    },
    node::{Ast, NodeId},
};

pub struct InvocationTemplateTypeMatcher<'a> {
    matched_types: LinkedIdentityHashMap<TypeId, TypeId>,
    seen_types: LinkedIdentityHashSet<TypeId>,

    registry: &'a mut JSTypeRegistry,
    /// Rust-only: the arena the registry's type operations read (DESIGN §8).
    ast: &'a Ast,
    callee_type: TypeId,
    local_this_type: Option<TypeId>,
    invocation: NodeId,

    unknown_type: TypeId,
}

impl<'a> InvocationTemplateTypeMatcher<'a> {
    // port: InvocationTemplateTypeMatcher#InvocationTemplateTypeMatcher
    pub fn new(
        registry: &'a mut JSTypeRegistry,
        ast: &'a Ast,
        callee_type: TypeId,
        local_this_type: Option<TypeId>,
        invocation: NodeId,
    ) -> Self {
        let unknown_type = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        Self {
            matched_types: LinkedIdentityHashMap::default(),
            seen_types: LinkedIdentityHashSet::default(),
            registry,
            ast,
            callee_type,
            local_this_type,
            invocation,
            unknown_type,
        }
    }

    // port: InvocationTemplateTypeMatcher#match
    pub fn r#match(&mut self) -> IndexMap<TypeId, TypeId> {
        if self
            .callee_type
            .get_template_type_map(self.registry)
            .is_empty()
        {
            return IndexMap::<_, _>::default();
        }

        let target = self.invocation.get_first_child(self.ast).unwrap();
        if NodeUtil::is_normal_get(self.ast, target) {
            let obj = target.get_first_child(self.ast).unwrap();
            let type_of_this_required_by_the_function =
                function_type::get_type_of_this(self.callee_type, self.registry);
            // The type placed on a SUPER node is the superclass type, which allows us to infer the
            // right property types for the GETPROP or GETELEM nodes built on it.
            // However, the type actually passed as `this` when making calls this way is the `this`
            // of the scope where the `super` appears.
            let mut type_of_this_provided_by_the_call = if obj.is_super(self.ast) {
                self.local_this_type.unwrap()
            } else {
                self.get_type_or_unknown(obj)
            };
            // We're looking at a call made as `obj['method']()` or `obj.method()` (see enclosing
            // if), so if the call is successfully made, then the object through which it is made
            // isn't null or undefined.
            type_of_this_provided_by_the_call = type_of_this_provided_by_the_call
                .restrict_by_not_null_or_undefined(self.registry, self.ast);
            self.match_template_types_recursive(
                type_of_this_required_by_the_function,
                type_of_this_provided_by_the_call,
            );
        }

        if self.invocation.is_tagged_template_lit(self.ast) {
            let mut callee_parameters = self.callee_type.get_parameters(self.registry).into_iter();
            let Some(first) = callee_parameters.next() else {
                // TypeCheck will warn if there are too few function parameters
                return self.copy_matched_types();
            };

            // The first argument to the tag function is an array of strings (typed as
            // ITemplateArray) but not an actual AST node
            let i_template_array_type = self
                .registry
                .get_native_type(JSTypeNative::I_TEMPLATE_ARRAY_TYPE);
            self.match_template_types_recursive(first.get_jstype(), i_template_array_type);

            // Resolve the remaining template types from the template literal substitutions.
            let decl_params: Vec<Parameter> = self
                .callee_type
                .get_parameters(self.registry)
                .into_iter()
                .skip(1)
                .collect();
            let call_params = NodeUtil::get_invocation_args_as_iterable(self.ast, self.invocation);
            self.match_template_types_from_nodes(decl_params, call_params);
        } else if self.invocation.has_more_than_one_child(self.ast) {
            let decl_params = self.callee_type.get_parameters(self.registry);
            let call_params = NodeUtil::get_invocation_args_as_iterable(self.ast, self.invocation);
            self.match_template_types_from_nodes(decl_params, call_params);
        }

        self.copy_matched_types()
    }

    // port: InvocationTemplateTypeMatcher#matchTemplateTypesRecursive
    fn match_template_types_recursive(&mut self, param_type: TypeId, arg_type: TypeId) {
        if param_type.is_template_type(self.registry) {
            // Recursive base case.
            // example: @param {T}
            let template = param_type.to_maybe_template_type(self.registry).unwrap();
            self.record_template_match(template, arg_type);
            return;
        }

        // Unpack unions.
        if param_type.is_union_type(self.registry) {
            // example: @param {Array.<T>|NodeList|Arguments|{length:number}}
            let union_type = param_type.to_maybe_union_type(self.registry).unwrap();
            let alternates = union_type.get_alternates(self.registry, self.ast);
            for &alternate in alternates.iter() {
                self.match_template_types_recursive(alternate, arg_type);
            }
            return;
        } else if arg_type.is_union_type(self.registry) {
            let union_type = arg_type.to_maybe_union_type(self.registry).unwrap();
            let alternates = union_type.get_alternates(self.registry, self.ast);
            for &alternate in alternates.iter() {
                self.match_template_types_recursive(param_type, alternate);
            }
            return;
        }

        if param_type.is_function_type(self.registry) {
            let param_function_type = param_type.to_maybe_function_type(self.registry).unwrap();
            let arg_function_type = arg_type
                .restrict_by_not_null_or_undefined(self.registry, self.ast)
                .collapse_union(self.registry, self.ast)
                .unwrap()
                .to_maybe_function_type(self.registry);
            if let Some(arg_function_type) = arg_function_type
                && arg_function_type.is_subtype(self.registry, self.ast, param_type)
            {
                // infer from return type of the function type
                let param_this =
                    function_type::get_type_of_this(param_function_type, self.registry);
                let arg_this = function_type::get_type_of_this(arg_function_type, self.registry);
                self.match_template_types_recursive(param_this, arg_this);
                // infer from return type of the function type
                let param_return = param_function_type.get_return_type(self.registry);
                let arg_return = arg_function_type.get_return_type(self.registry);
                self.match_template_types_recursive(param_return, arg_return);
                // infer from parameter types of the function type
                let decl_params = param_function_type.get_parameters(self.registry);
                let call_params = arg_function_type.get_parameters(self.registry);
                self.match_template_types_from_parameters(decl_params, call_params);
            }
        } else if param_type.is_record_type(self.registry)
            && !param_type.is_nominal_type(self.registry)
        {
            // example: @param {{foo:T}}
            if self.seen_types.add(param_type) {
                let param_record_type = param_type.to_object_type(self.registry).unwrap();
                let arg_object_type = arg_type
                    .restrict_by_not_null_or_undefined(self.registry, self.ast)
                    .to_object_type(self.registry);
                if let Some(arg_object_type) = arg_object_type
                    && !arg_object_type.is_unknown_type(self.registry, self.ast)
                    && !arg_object_type.is_empty_type(self.registry)
                {
                    let names = param_record_type.get_property_names(self.registry, self.ast);
                    for name in names {
                        if param_record_type.has_own_property(self.registry, self.ast, name.clone())
                            && arg_object_type.has_property(self.registry, self.ast, name.clone())
                        {
                            let param_prop = param_record_type.get_property_type(
                                self.registry,
                                self.ast,
                                name.clone(),
                            );
                            let arg_prop =
                                arg_object_type.get_property_type(self.registry, self.ast, name);
                            self.match_template_types_recursive(param_prop, arg_prop);
                        }
                    }
                }
                self.seen_types.remove(param_type);
            }
        } else if param_type.is_templatized_type(self.registry) {
            // example: @param {Array<T>}
            let templatized_param_type =
                param_type.to_maybe_templatized_type(self.registry).unwrap();
            let key_count = templatized_param_type
                .get_template_types(self.registry)
                .map_or(0, |types| types.len());
            // TODO(johnlenz): determine why we are creating TemplatizedTypes for
            // types with no type arguments.
            if key_count == 0 {
                return;
            }

            let arg_object_type = arg_type
                .restrict_by_not_null_or_undefined(self.registry, self.ast)
                .collapse_union(self.registry, self.ast)
                .unwrap();
            // Resolve any template types in common between the argument type and parameter type
            let param_type_map = param_type.get_template_type_map(self.registry);

            let keys = param_type_map.get_template_keys().to_vec();
            let arg_type_map = arg_object_type.get_template_type_map(self.registry);
            for &key in &keys[keys.len() - key_count..] {
                let param_resolved =
                    param_type_map.get_resolved_template_type(self.registry, self.ast, key);
                let arg_resolved =
                    arg_type_map.get_resolved_template_type(self.registry, self.ast, key);
                self.match_template_types_recursive(param_resolved, arg_resolved);
            }
        }
    }

    // port: InvocationTemplateTypeMatcher#matchTemplateTypesFromNodes
    fn match_template_types_from_nodes(
        &mut self,
        decl_params: Vec<Parameter>,
        call_params: Vec<NodeId>,
    ) {
        let mut decl_params = decl_params.into_iter().peekable();
        let mut call_params = call_params.into_iter().peekable();
        while decl_params.peek().is_some() && call_params.peek().is_some() {
            let decl_param = decl_params.next().unwrap();
            let arg = self.get_type_or_unknown(call_params.next().unwrap());
            self.match_template_types_recursive(decl_param.get_jstype(), arg);

            if decl_param.is_variadic() {
                for call_param in call_params.by_ref() {
                    let arg = self.get_type_or_unknown(call_param);
                    self.match_template_types_recursive(decl_param.get_jstype(), arg);
                }
            }
        }
    }

    // port: InvocationTemplateTypeMatcher#matchTemplateTypesFromParameters
    fn match_template_types_from_parameters(
        &mut self,
        decl_params: Vec<Parameter>,
        call_params: Vec<Parameter>,
    ) {
        let mut decl_params = decl_params.into_iter().peekable();
        let mut call_params = call_params.into_iter().peekable();
        while decl_params.peek().is_some() && call_params.peek().is_some() {
            let decl_param = decl_params.next().unwrap();
            let call_param = call_params.next().unwrap();
            self.match_template_types_recursive(decl_param.get_jstype(), call_param.get_jstype());

            if decl_param.is_variadic() {
                for call_param in call_params.by_ref() {
                    self.match_template_types_recursive(
                        decl_param.get_jstype(),
                        call_param.get_jstype(),
                    );
                }
            }
        }
    }

    // port: InvocationTemplateTypeMatcher#recordTemplateMatch
    fn record_template_match(&mut self, template: TypeId, r#match: TypeId) {
        if r#match.is_unknown_type(self.registry, self.ast) {
            return;
        }

        // Don't worry about checking bounds here. We'll validate them once they're all collected.
        let registry = &mut *self.registry;
        let ast = self.ast;
        self.matched_types
            .merge(template, Some(r#match), |old, value| {
                Some(old.get_least_supertype(registry, ast, *value.unwrap()))
            });
    }

    // port: InvocationTemplateTypeMatcher#copyMatchedTypes
    fn copy_matched_types(&self) -> IndexMap<TypeId, TypeId> {
        let mut builder = IndexMap::<_, _>::default();
        self.matched_types.for_each(|k, v| {
            builder.insert(*k, *v.unwrap());
        });
        builder
    }

    // port: InvocationTemplateTypeMatcher#getTypeOrUnknown
    fn get_type_or_unknown(&self, n: NodeId) -> TypeId {
        let type_ = n.get_jstype(self.ast);
        type_.unwrap_or(self.unknown_type)
    }
}
