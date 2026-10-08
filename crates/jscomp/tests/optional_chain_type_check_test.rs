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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/OptionalChainTypeCheckTest.java.

//! Port of the OptionalChainTypeCheckTest method whose post-call assertions are classified
//! `rust_unit_test` (corpus/unit/rust_unit_tests/OptionalChainTypeCheckTest.md, D-015 (a)). Every
//! other OptionalChainTypeCheckTest method is a record that replays in the unit-record harness.
use closure_jstype::js_type::JSType;
use closure_testing::type_check_test_case::TypeCheckTestCase;

// port: OptionalChainTypeCheckTest.OptChainTestsNonParameterized#testOptChainGetElemExpressions_nonNullObject
/// Confirms that OPTCHAIN_GETELEM nodes are inferred as unknown type.
#[test]
fn test_opt_chain_get_elem_expressions_non_null_object() {
    let mut t = TypeCheckTestCase::default();
    t.set_up().unwrap();
    let js = "/** @type {({b:number})} */ var a; var x; x = a?.[b]";

    let script = t.parse_and_type_check(js).unwrap().unwrap();
    let c = t.base.compiler.clone().unwrap();
    let mut compiler = c.borrow_mut();
    let assign = script
        .get_last_child(&compiler)
        .unwrap()
        .get_first_child(&compiler)
        .unwrap();
    assert!(assign.is_assign(&compiler));

    let x_name = assign.get_first_child(&compiler).unwrap();
    assert!(x_name.is_name(&compiler));
    let x_type = x_name.get_jstype(&compiler).unwrap();
    let get_elem = assign.get_second_child(&compiler).unwrap();
    assert!(get_elem.is_opt_chain_get_elem(&compiler) || get_elem.is_get_elem(&compiler));
    let get_elem_type = get_elem.get_jstype(&compiler).unwrap();

    let (reg, ast) = compiler.get_type_registry_and_ast();
    assert!(x_type.is_unknown_type(reg, ast));
    assert!(get_elem_type.is_unknown_type(reg, ast));
    drop(compiler);
    t.base.validate_warnings_and_errors().unwrap();
}
