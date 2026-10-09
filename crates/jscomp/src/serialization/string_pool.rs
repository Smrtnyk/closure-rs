/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/StringPool.java.

//! Port of serialization/StringPool.java.
use super::typed_ast_proto::StringPoolProto;
use super::wtf8::Wtf8;
use closure_rhino::check_state;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::rhino_string_pool::LazyInternedStringList;
use std::sync::{Arc, LazyLock};

/// port: StringPool
///
/// Aggregates strings into a `StringPool`.
///
/// The zeroth offset in the string pool is always the empty string. This is validated inside
/// `TypedAstDeserializer`.
///
/// This implies default/unset/0-valued uint32 StringPool pointers in protos are equivalent to the
/// empty string.
pub struct StringPool {
    /// max length of any individual string in the pool
    max_length: i32,
    pool: LazyInternedStringList,
}

static EMPTY: LazyLock<Arc<StringPool>> = LazyLock::new(|| Arc::new(StringPool::builder().build()));

impl StringPool {
    // port: StringPool#fromProto
    pub fn from_proto(proto: &StringPoolProto) -> Self {
        let mut decoder = Wtf8::decoder(proto.get_max_length());

        let mut pool: Vec<JsString> = Vec::new();
        pool.push(JsString::from(""));
        let strings_list = proto.get_strings_list();
        for s in strings_list {
            pool.push(decoder.decode(s));
        }

        Self::new(proto.get_max_length(), pool)
    }

    // port: StringPool#empty
    pub fn empty() -> Arc<StringPool> {
        EMPTY.clone()
    }

    // port: StringPool#<init>
    fn new(max_length: i32, pool: Vec<JsString>) -> Self {
        let first_is_empty = pool[0].is_empty();
        let this = Self {
            max_length,
            pool: LazyInternedStringList::new(pool),
        };

        check_state!(first_is_empty);
        this
    }

    // port: StringPool#get
    pub fn get(&self, offset: i32) -> JsString {
        self.pool.get(offset)
    }

    // port: StringPool#getInternedStrings
    pub fn get_interned_strings(&self) -> &LazyInternedStringList {
        &self.pool
    }

    // port: StringPool#toProto
    pub fn to_proto(&self) -> StringPoolProto {
        let mut builder = StringPoolProto::new_builder().set_max_length(self.max_length);
        for s in self.pool.stream().skip(1) {
            builder = builder.add_strings(Wtf8::encode_to_wtf8(&s));
        }
        builder.build()
    }

    // port: StringPool#builder
    pub fn builder() -> StringPoolBuilder {
        StringPoolBuilder::new()
    }
}

impl std::fmt::Debug for StringPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StringPool")
            .field("max_length", &self.max_length)
            .finish_non_exhaustive()
    }
}

/// port: StringPool.Builder
pub struct StringPoolBuilder {
    max_length: i32,
    pool: IndexMap<JsString, i32>,
}

impl StringPoolBuilder {
    // port: StringPool.Builder#<init>
    fn new() -> Self {
        let mut this = Self {
            max_length: 0,
            pool: IndexMap::<_, _>::default(),
        };
        this.put("");
        this
    }

    /// Inserts the given string into the string pool if not present and returns its index
    // port: StringPool.Builder#put
    pub fn put(&mut self, string: impl Into<JsString>) -> i32 {
        let string = string.into();

        if string.length() as i32 > self.max_length {
            self.max_length = string.length() as i32;
        }

        let size = self.pool.len() as i32;
        *self.pool.entry(string).or_insert(size)
    }

    // port: StringPool.Builder#putAnd
    pub fn put_and(&mut self, string: impl Into<JsString>) -> &mut Self {
        self.put(string);
        self
    }

    /// Rust-only: the private `maxLength` field, read by the unit recorder's object dump.
    pub fn max_length(&self) -> i32 {
        self.max_length
    }

    /// Rust-only: the private `pool` field, read by the unit recorder's object dump.
    pub fn pool(&self) -> &IndexMap<JsString, i32> {
        &self.pool
    }

    // port: StringPool.Builder#build
    pub fn build(&self) -> StringPool {
        StringPool::new(self.max_length, self.pool.keys().cloned().collect())
    }
}
