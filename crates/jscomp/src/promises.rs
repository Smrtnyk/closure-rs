/*
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Promises.java.

//! Port of `Promises.java`: models different Javascript Promise-related operations.

use closure_jstype::{prelude::*, union_type};
use closure_rhino::node::Ast;

// port: Promises
pub struct Promises;

impl Promises {
    /// If this object is known to be an IThenable, returns the type it resolves to.
    ///
    /// Returns unknown otherwise.
    ///
    /// (This is different from `getResolvedType`, which will attempt to model the then type of an
    /// expression after calling Promise.resolve() on it.
    // port: Promises#getTemplateTypeOfThenable
    pub fn get_template_type_of_thenable(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        maybe_thenable: TypeId,
    ) -> TypeId {
        // Without ".restrictByNotNullOrUndefined" we'd get the unknown type for "?IThenable<null>"
        let template_type = registry.get_i_thenable_template();
        maybe_thenable
            // Without ".restrictByNotNullOrUndefined" we'd get the unknown type for
            // "?IThenable<null>"
            .restrict_by_not_null_or_undefined(registry, ast)
            .get_template_type_map(registry)
            .get_resolved_template_type(registry, ast, template_type)
    }

    /// Returns the type of `await [expr]`.
    ///
    /// This is equivalent to the type of `result` in `Promise.resolve([expr]).then(result => `
    ///
    /// For example: `!Promise<number>` becomes `number`; `!IThenable<number>` becomes `number`;
    /// `string` becomes `string`; `(!Promise<number>|string)` becomes `(number|string)`;
    /// `?Promise<number>` becomes `(null|number)`.
    // port: Promises#getResolvedType
    pub fn get_resolved_type(registry: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> TypeId {
        if type_.is_unknown_type(registry, ast) {
            return type_;
        }

        if type_.is_union_type(registry) {
            let mut union_type_builder = union_type::builder();
            let alternates = type_
                .to_maybe_union_type(registry)
                .unwrap()
                .get_alternates(registry, ast);
            for &alternate in alternates.iter() {
                let resolved = Self::get_resolved_type(registry, ast, alternate);
                union_type_builder.add_alternate(registry, ast, resolved);
            }
            return union_type_builder.build(registry, ast);
        }

        // If we can find the "IThenable" template key (which is true for Promise and IThenable),
        // return the resolved value. e.g. for "!Promise<string>" return "string".
        let templates = type_.get_template_type_map(registry);
        if templates.has_template_key(registry.get_i_thenable_template()) {
            // Call getResolvedPromiseType again in case someone does something unusual like
            // !Promise<!Promise<number>>
            // TODO(lharker): we don't need to handle this case and should report an error for this
            // in a type annotation (not here, maybe in TypeCheck). A Promise cannot resolve to
            // another Promise
            let key = registry.get_i_thenable_template();
            let resolved = templates.get_resolved_template_type(registry, ast, key);
            return Self::get_resolved_type(registry, ast, resolved);
        }

        // Awaiting anything with a ".then" property (other than IThenable, handled above) should
        // return unknown, rather than the type itself.
        let thenable_type = registry.get_native_type(JSTypeNative::THENABLE_TYPE);
        if type_.is_subtype_of(registry, ast, thenable_type) {
            return registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        }

        type_
    }

    /// Wraps the given type in an IThenable.
    ///
    /// If the given type is already IThenable it is first unwrapped. For example: `number`
    /// becomes `IThenable<number>`; `IThenable<number>` becomes `IThenable<number>`;
    /// `Promise<number>` becomes `IThenable<number>`; `IThenable<number>|string` becomes
    /// `IThenable<number|string>`; `IThenable<number>|IThenable<string>` becomes
    /// `IThenable<number|string>`.
    // port: Promises#wrapInIThenable
    pub fn wrap_in_i_thenable(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        maybe_thenable: TypeId,
    ) -> TypeId {
        // Unwrap for simplicity first in the event it is a thenable.
        let unwrapped = Self::get_resolved_type(registry, ast, maybe_thenable);
        let i_thenable = registry.get_native_object_type(JSTypeNative::I_THENABLE_TYPE);
        registry.create_templatized_type(ast, i_thenable, &[unwrapped])
    }

    /// Synthesizes a type representing the legal types of a return expression within async code
    /// (i.e.`Promise` callbacks, async functions) based on the expected return type of that code.
    ///
    /// The return type will generally be a union but may not be in the case of top-like types. If
    /// the expected return type is a union, any synchronous elements will be dropped, since they
    /// can never occur. For example:
    ///
    /// - `!Promise<number>` => `number|!IThenable<number>`
    /// - `number` => `?`
    /// - `number|!Promise<string>` => `string|!IThenable<string>`
    /// - `!IThenable<number>|!Promise<string>` => `number|string|!IThenable<number|string>`
    /// - `!IThenable<number|string>` => `number|string|!IThenable<number|string>`
    /// - `?` => `?`
    /// - `*` => `?`
    // port: Promises#createAsyncReturnableType
    pub fn create_async_returnable_type(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        maybe_thenable: TypeId,
    ) -> TypeId {
        let unknown_type = registry.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let i_thenable_type = registry.get_native_object_type(JSTypeNative::I_THENABLE_TYPE);

        let i_thenable_of_unknown_type =
            registry.create_templatized_type(ast, i_thenable_type, &[unknown_type]);

        let alternates: Vec<TypeId> = if maybe_thenable.is_union_type(registry) {
            maybe_thenable
                .to_maybe_union_type(registry)
                .unwrap()
                .get_alternates(registry, ast)
                .to_vec()
        } else {
            vec![maybe_thenable]
        };
        let mut async_template_alternates = Vec::new();
        for t in alternates {
            // Discard "synchronous" types.
            if t.is_subtype_of(registry, ast, i_thenable_of_unknown_type) {
                // Unwrap "asynchronous" types.
                async_template_alternates
                    .push(Self::get_template_type_of_thenable(registry, ast, t));
            }
        }

        if async_template_alternates.is_empty() {
            return unknown_type;
        }

        let async_template_union = registry.create_union_type(ast, &async_template_alternates);
        let templatized =
            registry.create_templatized_type(ast, i_thenable_type, &[async_template_union]);
        registry.create_union_type(ast, &[async_template_union, templatized])
    }
}
