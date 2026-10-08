/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/TypeCheckTypedPercentTest.java.

//! Port of TypeCheckTypedPercentTest: tests for TypeCheck#getTypedPercent.
use closure_jscomp::{
    compiler::Compiler, semantic_reverse_abstract_interpreter::SemanticReverseAbstractInterpreter,
    type_check::TypeCheck,
};
use closure_rhino::{ir::IR, js_string::JsString};
use closure_testing::testing::test_externs_builder::TestExternsBuilder;

// port: Truth DoubleSubject#isWithin(tolerance).of(expected)
fn assert_within(actual: f64, tolerance: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} to be within {tolerance} of {expected}"
    );
}

#[test]
fn test_get_typed_percent1() {
    let js = "var id = function(x) { return x; }\n\
              var id2 = function(x) { return id(x); }\n";
    assert_within(get_typed_percent(js), 0.1, 50.0);
}

#[test]
fn test_get_typed_percent2() {
    let js = "var x = {}; x.y = 1;";
    assert_within(get_typed_percent(js), 0.1, 100.0);
}

#[test]
fn test_get_typed_percent3() {
    let js = "var f = function(x) { x.a = x.b; }";
    assert_within(get_typed_percent(js), 0.1, 25.0);
}

#[test]
fn test_get_typed_percent4() {
    let js = "var n = {};\n\
              /** @constructor */ n.T = function() {};\n\
              /** @type {n.T} */ var x = new n.T();\n";
    assert_within(get_typed_percent(js), 0.1, 100.0);
}

#[test]
fn test_get_typed_percent5() {
    let js = "/** @enum {number} */ keys = {A: 1,B: 2,C: 3};";
    assert_within(get_typed_percent(js), 0.1, 100.0);
}

#[test]
fn test_get_typed_percent6() {
    let js = "a = {TRUE: 1, FALSE: 0};";
    assert_within(get_typed_percent(js), 0.1, 100.0);
}

#[test]
fn test_resolving_named_types() {
    let externs = TestExternsBuilder::new().add_object().build();
    let js = "/** @constructor */\n\
              var Foo = function() {}\n\
              /** @param {number} a */\n\
              Foo.prototype.foo = function(a) {\n  \
                return this.baz().toString();\n\
              };\n\
              /** @return {Baz} */\n\
              Foo.prototype.baz = function() { return new Baz(); };\n\
              /** @constructor\n  \
                * @extends Foo */\n\
              var Bar = function() {};\n\
              /** @constructor */\n\
              var Baz = function() {};\n";
    assert_within(get_typed_percent_with_externs(externs, js), 0.1, 100.0);
}

#[test]
fn test_get_typed_percent_const_and_let() {
    // Make sure names declared with `const` and `let` are counted correctly for typed percentage.
    let js = "const id = function(x) { return x; }\n\
              let id2 = function(x) { return id(x); }\n";
    assert_within(get_typed_percent(js), 0.1, 50.0);
}

// port: TypeCheckTypedPercentTest#getTypedPercent
fn get_typed_percent(js: &str) -> f64 {
    get_typed_percent_with_externs(JsString::from(""), js)
}

// port: TypeCheckTypedPercentTest#getTypedPercentWithExterns
fn get_typed_percent_with_externs(externs: JsString, js: &str) -> f64 {
    let mut compiler = Compiler::new();
    compiler.get_type_registry();
    let parsed = compiler.parse_test_code(js);
    let js_root = IR::root(&mut compiler, &[parsed]);

    let parsed_externs = compiler.parse_test_code(externs);
    let externs_root = IR::root(&mut compiler, &[parsed_externs]);
    IR::root(&mut compiler, &[externs_root, js_root]);

    let interpreter = SemanticReverseAbstractInterpreter::new(compiler.get_type_registry());
    let mut t = TypeCheck::new(&mut compiler, interpreter);
    t.process_for_testing(&mut compiler, Some(externs_root), js_root);
    assert!(compiler.get_errors().is_empty());
    t.get_typed_percent()
}
