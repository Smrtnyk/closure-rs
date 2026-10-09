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
//   test/com/google/javascript/jscomp/InvalidatingTypesTest.java.

//! Port of InvalidatingTypesTest.java.
use closure_jscomp::invalidating_types::Builder;
use closure_jscomp::type_mismatch::TypeMismatch;
use closure_jstype::JSTypeNative::{ARRAY_TYPE, NUMBER_TYPE, OBJECT_TYPE, STRING_TYPE};
use closure_jstype::JSTypeRegistry;
use closure_jstype::prelude::*;
use closure_rhino::node::Ast;
use closure_rhino::testing::test_error_reporter::TestErrorReporter;

// port: InvalidatingTypesTest#setUp
fn set_up() -> (Ast, JSTypeRegistry) {
    let mut ast = Ast::new();
    let registry = JSTypeRegistry::new(&mut ast, Box::new(TestErrorReporter::new()), Vec::new());
    (ast, registry)
}

// port: InvalidatingTypesTest#objectIsInvalidating
#[test]
fn object_is_invalidating() {
    let (mut ast, mut registry) = set_up();
    let invalidating_types = Builder::new(&registry).build(&mut registry, &mut ast);
    let object = registry.get_native_object_type(OBJECT_TYPE);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, Some(object)));
}

// port: InvalidatingTypesTest#templatizedObjectIsInvalidating
#[test]
fn templatized_object_is_invalidating() {
    let (mut ast, mut registry) = set_up();
    let invalidating_types = Builder::new(&registry).build(&mut registry, &mut ast);
    let object = registry.get_native_object_type(OBJECT_TYPE);
    let number = registry.get_native_type(NUMBER_TYPE);
    let object_of_string = registry.create_templatized_type(&ast, object, &[number]);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, Some(object_of_string)));
}

// port: InvalidatingTypesTest#invaliatingRawTypeAlsoInvalidatesTemplatization
#[test]
fn invaliating_raw_type_also_invalidates_templatization() {
    let (mut ast, mut registry) = set_up();
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let mismatch = TypeMismatch::create_for_testing(&mut ast, array, string);
    let invalidating_types = Builder::new(&registry)
        .add_all_type_mismatches(&mut registry, &ast, &[mismatch])
        .build(&mut registry, &mut ast);
    let number = registry.get_native_type(NUMBER_TYPE);
    let array_of_number = registry.create_templatized_type(&ast, array, &[number]);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, Some(array_of_number)));
}

// port: InvalidatingTypesTest#invalidatingOneTemplatizationInvalidatesAll
#[test]
fn invalidating_one_templatization_invalidates_all() {
    let (mut ast, mut registry) = set_up();
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let array_of_string = registry.create_templatized_type(&ast, array, &[string]);
    let mismatch = TypeMismatch::create_for_testing(&mut ast, array_of_string, string);
    let invalidating_types = Builder::new(&registry)
        .add_all_type_mismatches(&mut registry, &ast, &[mismatch])
        .build(&mut registry, &mut ast);
    let number = registry.get_native_type(NUMBER_TYPE);
    let array_of_number = registry.create_templatized_type(&ast, array, &[number]);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, Some(array_of_number)));
}

// port: InvalidatingTypesTest#invalidatingOneTemplatizationInvalidatesRawType
#[test]
fn invalidating_one_templatization_invalidates_raw_type() {
    let (mut ast, mut registry) = set_up();
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let array_of_string = registry.create_templatized_type(&ast, array, &[string]);
    let mismatch = TypeMismatch::create_for_testing(&mut ast, array_of_string, string);
    let invalidating_types = Builder::new(&registry)
        .add_all_type_mismatches(&mut registry, &ast, &[mismatch])
        .build(&mut registry, &mut ast);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, Some(array)));
}

// port: InvalidatingTypesTest#arrayNotInvalidating
#[test]
fn array_not_invalidating() {
    let (mut ast, mut registry) = set_up();
    let invalidating_types = Builder::new(&registry).build(&mut registry, &mut ast);
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let array_of_string = registry.create_templatized_type(&ast, array, &[string]);
    assert!(!invalidating_types.is_invalidating(&mut registry, &ast, Some(array_of_string)));
}

// port: InvalidatingTypesTest#invalidatingInstanceTypeInvalidatesCtor
#[test]
fn invalidating_instance_type_invalidates_ctor() {
    let (mut ast, mut registry) = set_up();
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let mismatch = TypeMismatch::create_for_testing(&mut ast, array, string);
    let invalidating_types = Builder::new(&registry)
        .add_all_type_mismatches(&mut registry, &ast, &[mismatch])
        .build(&mut registry, &mut ast);
    let ctor = array.get_constructor(&registry);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, ctor));
}

// port: InvalidatingTypesTest#invalidatingTemplatizedInstanceTypeInvalidatesCtor
#[test]
fn invalidating_templatized_instance_type_invalidates_ctor() {
    let (mut ast, mut registry) = set_up();
    let array = registry.get_native_object_type(ARRAY_TYPE);
    let string = registry.get_native_type(STRING_TYPE);
    let array_of_string = registry.create_templatized_type(&ast, array, &[string]);
    let mismatch = TypeMismatch::create_for_testing(&mut ast, array_of_string, string);
    let invalidating_types = Builder::new(&registry)
        .add_all_type_mismatches(&mut registry, &ast, &[mismatch])
        .build(&mut registry, &mut ast);
    let ctor = array.get_constructor(&registry);
    assert!(invalidating_types.is_invalidating(&mut registry, &ast, ctor));
}
