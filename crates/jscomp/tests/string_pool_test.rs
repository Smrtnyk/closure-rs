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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/serialization/StringPoolTest.java.

//! Port of serialization/StringPoolTest.java.
use closure_jscomp::serialization::string_pool::StringPool;
use closure_jscomp::serialization::typed_ast_proto::StringPoolProto;

fn assert_is_empty_pool(pool: &StringPool) {
    assert!(pool.get(0).is_empty());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pool.get(1)));
    assert!(result.is_err());
}

// port: StringPoolTest#emptyPool_fromBuilderOrConstant
#[test]
fn empty_pool_from_builder_or_constant() {
    assert_is_empty_pool(&StringPool::empty());
    assert_is_empty_pool(&StringPool::builder().build());
}

// port: StringPoolTest#emptyPool_serializesToDefaultProto
#[test]
fn empty_pool_serializes_to_default_proto() {
    assert_eq!(
        StringPool::empty().to_proto(),
        StringPoolProto::get_default_instance()
    );
}

// port: StringPoolTest#emptyPool_parsesFromDefaultProto
#[test]
fn empty_pool_parses_from_default_proto() {
    assert_is_empty_pool(&StringPool::from_proto(
        &StringPoolProto::get_default_instance(),
    ));
}
