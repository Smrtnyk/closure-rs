/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JsIterables.java.

//! Port of `JsIterables.java`: models type transformations of JavaScript `Iterable` and
//! `Iterator` types.
//!
//! These tranformations can be especially helpful when working with generator functions.
//! `Generator` is a subtype of `Iterable`.

use closure_jstype::prelude::*;
use closure_rhino::node::Ast;

// port: JsIterables
pub struct JsIterables;

impl JsIterables {
    /// Returns the given `Iterable`s element type.
    ///
    /// If the given type is not an `Iterator`, `Iterable`, `AsyncIterator`, or `AsyncIterable`,
    /// returns the unknown type.
    // port: JsIterables#getElementType
    pub fn get_element_type(
        iterable_or_iterator: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> TypeId {
        let template_type_map = iterable_or_iterator
            // Remember that `string` will box to a `Iterable`.
            .autobox(type_registry, ast)
            .get_template_type_map(type_registry);

        if template_type_map.has_template_key(type_registry.get_iterable_value_template()) {
            // `Iterable<SomeElementType>` or `Generator<SomeElementType>`
            let key = type_registry.get_iterable_value_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map.has_template_key(type_registry.get_iterator_value_template()) {
            // `Iterator<SomeElementType>`
            let key = type_registry.get_iterator_value_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_iterator_like_value_template())
        {
            // `IteratorLike<SomeElementType>`
            let key = type_registry.get_iterator_like_value_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_async_iterable_value_template())
        {
            // `AsyncIterable<SomeElementType>` or `AsyncGenerator<SomeElementType>`
            let key = type_registry.get_async_iterable_value_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_async_iterator_value_template())
        {
            // `AsyncIterator<SomeElementType>`
            let key = type_registry.get_async_iterator_value_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        }
        let next_property = iterable_or_iterator.find_property_type(type_registry, ast, "next");
        if let Some(next_property) = next_property {
            let next_fn = next_property.to_maybe_function_type(type_registry);
            if let Some(next_fn) = next_fn {
                let return_type = next_fn.get_return_type(type_registry);
                let value_property = return_type.find_property_type(type_registry, ast, "value");
                if let Some(value_property) = value_property {
                    return value_property;
                }
            }
        }
        type_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE)
    }

    /// Returns the given `Iterable`s return type.
    ///
    /// This corresponds to the `TReturn` template variable in `Iterable<T, TReturn, TNext>`.
    ///
    /// If the given type is not an `Iterator`, `Iterable`, `AsyncIterator`, or `AsyncIterable`,
    /// returns the unknown type.
    // port: JsIterables#getReturnElementType
    pub fn get_return_element_type(
        iterable_or_iterator: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> TypeId {
        let template_type_map = iterable_or_iterator
            // Remember that `string` will box to a `Iterable`.
            .autobox(type_registry, ast)
            .get_template_type_map(type_registry);

        if template_type_map.has_template_key(type_registry.get_iterable_return_template()) {
            // `Iterable<?, SomeElementType>` or `Generator<?, SomeElementType>`
            let key = type_registry.get_iterable_return_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map.has_template_key(type_registry.get_iterator_return_template()) {
            // `Iterator<?, SomeElementType>`
            let key = type_registry.get_iterator_return_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_iterator_like_return_template())
        {
            // `IteratorLike<?, SomeElementType>`
            let key = type_registry.get_iterator_like_return_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_async_iterable_return_template())
        {
            // `AsyncIterable<?, SomeElementType>` or `AsyncGenerator<?, SomeElementType>`
            let key = type_registry.get_async_iterable_return_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        } else if template_type_map
            .has_template_key(type_registry.get_async_iterator_return_template())
        {
            // `AsyncIterator<?, SomeElementType>`
            let key = type_registry.get_async_iterator_return_template();
            return template_type_map.get_resolved_template_type(type_registry, ast, key);
        }
        type_registry.get_native_type(JSTypeNative::UNKNOWN_TYPE)
    }

    /// Returns an `Iterable` type templated on `element_type`.
    ///
    /// Example: `number' => `Iterable<number>`.
    // port: JsIterables#createIterableTypeOf
    pub fn create_iterable_type_of(
        element_type: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> TypeId {
        let iterable = type_registry.get_native_object_type(JSTypeNative::ITERABLE_TYPE);
        type_registry.create_templatized_type(ast, iterable, &[element_type])
    }

    /// Returns the template for iterable types.
    ///
    /// This does not apply to async-iterables.
    ///
    /// If this value is not an Iterable, we return a sentinel.
    // port: JsIterables#maybeBoxIterable
    pub fn maybe_box_iterable(
        type_: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> MaybeBoxedType {
        Self::maybe_box_iterable_and_possibly_async_iterable(
            type_,
            type_registry,
            ast,
            /* include_async_iterable= */ false,
        )
    }

    /// Returns the template for iterable or async-iterable types.
    ///
    /// If this value is not an Iterable or AsyncIterable, we return a sentinel.
    // port: JsIterables#maybeBoxIterableOrAsyncIterable
    pub fn maybe_box_iterable_or_async_iterable(
        type_: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> MaybeBoxedType {
        Self::maybe_box_iterable_and_possibly_async_iterable(
            type_,
            type_registry,
            ast,
            /* include_async_iterable= */ true,
        )
    }

    /// Given a type, if it is an iterable or async iterable, will return its template. If not a
    /// subtype of Iterable|AsyncIterable, returns an object that has no match, and will indicate
    /// the mismatch. e.g. both `number` and `number|Iterable` are not subtypes of
    /// Iterable|AsyncIterable.
    ///
    /// `include_async_iterable`: whether to look for AsyncIterable as opposed to just Iterable
    // port: JsIterables#maybeBoxIterableAndPossiblyAsyncIterable
    fn maybe_box_iterable_and_possibly_async_iterable(
        type_: TypeId,
        type_registry: &mut JSTypeRegistry,
        ast: &Ast,
        include_async_iterable: bool,
    ) -> MaybeBoxedType {
        let mut templated_types = Vec::new();

        // Note: we don't just use JSType.autobox() here because that removes null and undefined.
        // We want to keep null and undefined around because they should cause a mismatch.
        if type_.is_union_type(type_registry) {
            let alternates = type_
                .to_maybe_union_type(type_registry)
                .unwrap()
                .get_alternates(type_registry, ast);
            for &alt in alternates.iter() {
                let alt = if alt.is_boxable_scalar(type_registry) {
                    alt.autoboxes_to(type_registry).unwrap()
                } else {
                    alt
                };
                let iterable_type = type_registry.get_native_type(JSTypeNative::ITERABLE_TYPE);
                let is_iterable = alt.is_subtype_of(type_registry, ast, iterable_type);
                let is_async_iterable = include_async_iterable && {
                    let async_iterable_type =
                        type_registry.get_native_type(JSTypeNative::ASYNC_ITERABLE_TYPE);
                    alt.is_subtype_of(type_registry, ast, async_iterable_type)
                };
                if !is_iterable && !is_async_iterable {
                    return MaybeBoxedType::new(None, Some(alt));
                }
                let value_template = if is_async_iterable {
                    type_registry.get_async_iterable_value_template()
                } else {
                    type_registry.get_iterable_value_template()
                };
                templated_types.push(
                    alt.get_template_type_map(type_registry)
                        .get_resolved_template_type(type_registry, ast, value_template),
                );
            }
        } else {
            let autoboxed_type = if type_.is_boxable_scalar(type_registry) {
                type_.autoboxes_to(type_registry).unwrap()
            } else {
                type_
            };
            let iterable_type = type_registry.get_native_type(JSTypeNative::ITERABLE_TYPE);
            let is_iterable = autoboxed_type.is_subtype_of(type_registry, ast, iterable_type);
            let is_async_iterable = include_async_iterable && {
                let async_iterable_type =
                    type_registry.get_native_type(JSTypeNative::ASYNC_ITERABLE_TYPE);
                autoboxed_type.is_subtype_of(type_registry, ast, async_iterable_type)
            };
            if !is_iterable && !is_async_iterable {
                return MaybeBoxedType::new(None, Some(autoboxed_type));
            }
            let template_type = if is_async_iterable {
                type_registry.get_async_iterable_value_template()
            } else {
                type_registry.get_iterable_value_template()
            };
            templated_types.push(
                autoboxed_type
                    .get_template_type_map(type_registry)
                    .get_resolved_template_type(type_registry, ast, template_type),
            );
        }
        MaybeBoxedType::new(
            Some(type_registry.create_union_type(ast, &templated_types)),
            None,
        )
    }
}

/// The template of an Iterable|AsyncIterable, or a mismatch if subtype of Iterable|AsyncIterable.
// port: JsIterables.MaybeBoxedType
#[derive(Clone, Copy, Debug)]
pub struct MaybeBoxedType {
    templated_type: Option<TypeId>,
    mismatch_type: Option<TypeId>,
}

impl MaybeBoxedType {
    /// `templated_type`: the unwrapped type that is being yieled by the iterable / async iterable.
    /// e.g. in `Iterable<number>|AsyncIterable<Qux>|string`, this should be
    /// `number|Qux|string`. Null if a mismatch.
    ///
    /// `mismatch_type`: the type that caused the mismatch. This can be the entire type (e.g.
    /// `number` is not iterable or async iterable) or a piece of a union that caused the mismatch
    /// (e.g. `number` in `number|Iterable<Qux>`). Null if a match.
    // port: JsIterables.MaybeBoxedType#MaybeBoxedType
    fn new(templated_type: Option<TypeId>, mismatch_type: Option<TypeId>) -> Self {
        Self {
            templated_type,
            mismatch_type,
        }
    }

    /// Returns the template of the iterable / async iterable; panics if not an iterable / async
    /// iterable.
    // port: JsIterables.MaybeBoxedType#getTemplatedType
    pub fn get_templated_type(&self) -> TypeId {
        if !self.is_match() {
            panic!("Type was not boxable to iterable or async iterable!");
        }
        self.templated_type.unwrap()
    }

    /// Returns the type that caused the mismatch, if any.
    // port: JsIterables.MaybeBoxedType#getMismatchType
    pub fn get_mismatch_type(&self) -> Option<TypeId> {
        self.mismatch_type
    }

    // port: JsIterables.MaybeBoxedType#orElse
    pub fn or_else(&self, type_: TypeId) -> TypeId {
        if self.is_match() {
            return self.templated_type.unwrap();
        }

        type_
    }

    // port: JsIterables.MaybeBoxedType#isMatch
    pub fn is_match(&self) -> bool {
        self.templated_type.is_some()
    }
}
