/*
 * Copyright 2020 The Closure Compiler Authors.
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
/*
 * Copyright (C) 2011 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/serialization/JSTypeColorIdHasher.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/hash/AbstractHasher.java,
//   com/google/common/hash/AbstractNonStreamingHashFunction.java,
//   com/google/common/hash/HashCode.java, com/google/common/hash/Hashing.java.

//! Port of serialization/JSTypeColorIdHasher.java.
use closure_jstype::prelude::*;
use closure_rhino::common_hash::farm_hash_fingerprint64;
use closure_rhino::java_lang::string::write_bytes_utf8;
use closure_rhino::js_string::JsString;
use closure_rhino::jscomp_colors::color_id::ColorId;
use closure_rhino::jscomp_colors::standard_colors;
use closure_rhino::node::Ast;

// The lookup in the standard color ids by JSType#hashCode and JSType#equals, which names
// Closure's Rhino-derived JSType (MPL-1.1 / GPL-2.0-or-later), is in its own file.
#[path = "js_type_color_id_hasher_rhino.rs"]
mod rhino;

/// port: JSTypeColorIdHasher
pub struct JSTypeColorIdHasher {
    /// Java's `ImmutableMap<ObjectType, ColorId>`, looked up with `JSType#equals`.
    standard_color_object_ids: Vec<(i32, TypeId, ColorId)>,
    /// Rust-only (performance divergence): 64-bit Bloom mask over `standard_color_object_ids`
    /// hashes to skip the 13-entry scan for non-matching types.
    standard_hash_bloom: u64,
    /// Rust-only (performance divergence): reusable byte buffer for `farmHashFingerprint64`,
    /// avoiding a heap allocation per hashed object type and per string.
    scratch_bytes: std::cell::RefCell<Vec<u8>>,
}

/// port: Hashing#farmHashFingerprint64 (a non-streaming hash function: its Hasher buffers the bytes
/// it is given and fingerprints them all at once)
struct Hasher<'a> {
    bytes: &'a mut Vec<u8>,
}

impl<'a> Hasher<'a> {
    fn new(bytes: &'a mut Vec<u8>) -> Self {
        bytes.clear();
        Self { bytes }
    }

    // port: AbstractHasher#putInt (little-endian)
    fn put_int(&mut self, i: i32) -> &mut Self {
        self.bytes.extend_from_slice(&i.to_le_bytes());
        self
    }

    // port: AbstractHasher#putString(CharSequence, Charset)
    fn put_string_utf8(&mut self, s: &JsString) -> &mut Self {
        write_bytes_utf8(s, self.bytes);
        self
    }

    // port: AbstractNonStreamingHashFunction.BufferingHasher#hash (HashCode#asLong)
    fn hash_as_long(&self) -> i64 {
        farm_hash_fingerprint64::hash_bytes(self.bytes)
    }
}

/// Arbitrary constants that may get hashed into ColorId.
///
/// Since a hash may end up including combinations of markers, using int values reduces that
/// chance that two combinations will concatenate to the same bit sequence.
// port: JSTypeColorIdHasher.Marker
struct Marker;
impl Marker {
    const HAS_INSTANCE_TYPE: i32 = 0x8c8b70db_u32 as i32;
    const NO_GOOG_MODULE_ID: i32 = 0x2593c5ff;
    const UNKNOWN_SOURCEREF: i32 = 0x660be782;
}

impl JSTypeColorIdHasher {
    // port: JSTypeColorIdHasher#NATIVE_TYPE_TO_ID
    pub const NATIVE_TYPE_TO_ID: [(JSTypeNative, ColorId); 13] = [
        (JSTypeNative::ARGUMENTS_TYPE, standard_colors::ARGUMENTS_ID),
        (JSTypeNative::ARRAY_TYPE, standard_colors::ARRAY_ID),
        (
            JSTypeNative::READONLY_ARRAY_TYPE,
            standard_colors::READONLY_ARRAY_ID,
        ),
        (
            JSTypeNative::ASYNC_ITERATOR_ITERABLE_TYPE,
            standard_colors::ASYNC_ITERATOR_ITERABLE_ID,
        ),
        (JSTypeNative::GENERATOR_TYPE, standard_colors::GENERATOR_ID),
        (
            JSTypeNative::I_TEMPLATE_ARRAY_TYPE,
            standard_colors::I_TEMPLATE_ARRAY_ID,
        ),
        (JSTypeNative::ITERATOR_TYPE, standard_colors::ITERATOR_ID),
        (JSTypeNative::PROMISE_TYPE, standard_colors::PROMISE_ID),
        (
            JSTypeNative::BIGINT_OBJECT_TYPE,
            standard_colors::BIGINT_OBJECT_ID,
        ),
        (
            JSTypeNative::BOOLEAN_OBJECT_TYPE,
            standard_colors::BOOLEAN_OBJECT_ID,
        ),
        (
            JSTypeNative::NUMBER_OBJECT_TYPE,
            standard_colors::NUMBER_OBJECT_ID,
        ),
        (
            JSTypeNative::STRING_OBJECT_TYPE,
            standard_colors::STRING_OBJECT_ID,
        ),
        (
            JSTypeNative::SYMBOL_OBJECT_TYPE,
            standard_colors::SYMBOL_OBJECT_ID,
        ),
    ];

    // port: JSTypeColorIdHasher#<init>
    pub fn new(registry: &JSTypeRegistry) -> Self {
        let mut standard_color_object_ids: Vec<(i32, TypeId, ColorId)> = Vec::with_capacity(13);
        let mut standard_hash_bloom = 0u64;
        for (native, id) in Self::NATIVE_TYPE_TO_ID {
            let key = registry.get_native_object_type(native);
            // ImmutableMap#toImmutableMap throws on a duplicate key.
            closure_rhino::check_argument!(
                standard_color_object_ids.iter().all(|&(_, k, _)| k != key),
                "Multiple entries with same key"
            );
            let hash = key.hash_code(registry);
            standard_hash_bloom |= 1u64 << ((hash as u32) & 63);
            standard_color_object_ids.push((hash, key, id));
        }
        Self {
            standard_color_object_ids,
            standard_hash_bloom,
            scratch_bytes: std::cell::RefCell::new(Vec::with_capacity(64)),
        }
    }

    // port: JSTypeColorIdHasher#hashObjectType
    pub fn hash_object_type(
        &self,
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        r#type: TypeId,
    ) -> ColorId {
        let valid = !r#type.is_enum_element_type(registry)
            && !r#type.is_no_resolved_type(registry)
            && !r#type.is_templatized_type(registry)
            && !r#type.is_unknown_type(registry, ast)
            && !r#type.is_template_type(registry);
        if !valid {
            let message = r#type.to_string(registry, ast);
            closure_rhino::check_state!(valid, "%s", message);
        }

        if let Some(box_id) = self.standard_color_object_id(registry, ast, r#type) {
            return box_id;
        }

        let mut scratch = self.scratch_bytes.borrow_mut();
        let mut hasher = Hasher::new(&mut scratch);

        if r#type.is_function_type(registry) {
            let fn_type = r#type.to_maybe_function_type(registry).unwrap();
            if fn_type.has_instance_type(registry) {
                hasher.put_int(Marker::HAS_INSTANCE_TYPE);
            }
        }

        if let Some(name) = r#type.get_reference_name(registry) {
            Self::put_reference_name_type(&mut hasher, registry, r#type, &name);
        } else {
            closure_jstype::object_type::for_each_own_property_name(r#type, registry, |name| {
                Self::put_utf8(&mut hasher, name);
            });
        }

        ColorId::from_unsigned(hasher.hash_as_long())
    }

    // port: JSTypeColorIdHasher#putReferenceNameType
    fn put_reference_name_type(
        hasher: &mut Hasher<'_>,
        registry: &JSTypeRegistry,
        r#type: TypeId,
        name: &JsString,
    ) {
        Self::put_utf8(hasher, name);

        let Some(source_ref) = Self::source_ref_for(registry, r#type) else {
            // TODO(b/185519307): This is a hack to work around bugs in the typesystem.
            hasher.put_int(Marker::UNKNOWN_SOURCEREF);
            return;
        };

        match closure_jstype::js_type::WithSourceRef::get_goog_module_id(source_ref, registry) {
            None => {
                hasher.put_int(Marker::NO_GOOG_MODULE_ID);
            }
            Some(module_id) => {
                Self::put_utf8(hasher, &module_id);
            }
        }
    }

    // port: JSTypeColorIdHasher#sourceRefFor
    fn source_ref_for(registry: &JSTypeRegistry, r#type: TypeId) -> Option<TypeId> {
        if r#type.is_enum_type(registry) {
            return r#type.to_maybe_enum_type(registry);
        } else if r#type.is_enum_element_type(registry) {
            // EnumElementTypes are proxied to their underlying type when transformed to colors.
            panic!("AssertionError: {type:?}");
        }

        if r#type.is_function_type(registry) {
            return r#type.to_maybe_function_type(registry);
        } else if r#type.is_function_prototype_type(registry) {
            return r#type.get_owner_function(registry);
        } else if r#type.get_constructor(registry).is_some() {
            return r#type.get_constructor(registry);
        }

        None
    }

    // port: JSTypeColorIdHasher#putUtf8
    fn put_utf8(hasher: &mut Hasher<'_>, s: &JsString) {
        hasher.put_string_utf8(s);
    }
}
