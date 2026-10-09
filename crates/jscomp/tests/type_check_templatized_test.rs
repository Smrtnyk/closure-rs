/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/TypeCheckTemplatizedTest.java.

//! Port of the TypeCheckTemplatizedTest method that is not a unit record: it builds JSTypes
//! directly instead of type-checking a source. Every other TypeCheckTemplatizedTest method is a
//! record that replays in the unit-record harness (testTemplatized8 is @Ignore'd in Java).
use closure_jstype::{js_type_native::JSTypeNative, testing::type_subject::TypeSubject};
use closure_testing::type_check_test_case::TypeCheckTestCase;

// port: TypeCheckTemplatizedTest#testTemplatizedTypeSubtypes2
#[test]
fn test_templatized_type_subtypes2() {
    let mut t = TypeCheckTestCase::default();
    t.set_up().unwrap();
    let c = t.base.compiler.clone().unwrap();
    {
        let mut compiler = c.borrow_mut();
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let array = registry.get_native_object_type(JSTypeNative::ARRAY_TYPE);
        let number = registry.get_native_type(JSTypeNative::NUMBER_TYPE);
        let string = registry.get_native_type(JSTypeNative::STRING_TYPE);
        let null_void = registry.get_native_type(JSTypeNative::NULL_VOID);
        let array_of_number = registry.create_templatized_type(ast, array, &[number]);
        let array_of_string = registry.create_templatized_type(ast, array, &[string]);
        let union = registry.create_union_type(ast, &[array_of_number, null_void]);
        TypeSubject::assert_type(array_of_string).is_not_subtype_of(registry, ast, union);
    }
    t.base.validate_warnings_and_errors().unwrap();
}
