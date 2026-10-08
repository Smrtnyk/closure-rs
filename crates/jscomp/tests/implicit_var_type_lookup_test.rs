/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only regression tests (no Java counterpart): a type name rooted at `this` resolves
//! through the implicit `this` var that Java's AbstractScope#getOwnSlot creates on the first
//! lookup of JSTypeRegistry#getLookupScope (getTopmostScopeOfEventualDeclaration),
//! isNameDefinedLocally and resolveViaProperties. Expected diagnostics are the reference jar's
//! (--compilation_level=ADVANCED on the same code); without the implicit var the lookups
//! reported "Bad type annotation. Unknown type this".
use closure_testing::type_check_test_case::{TypeCheckTestCase, TypeTestBuilder};

fn new_test() -> (TypeCheckTestCase, TypeTestBuilder) {
    let mut t = TypeCheckTestCase::default();
    t.set_up().unwrap();
    let builder = t.new_test_legacy("".into()).unwrap();
    (t, builder)
}

#[test]
fn test_this_return_type_in_nested_function_of_static_method() {
    // The jar compiles this without a warning.
    let (_t, mut builder) = new_test();
    builder
        .add_source(
            "class B { static f() { /** @return {this} */ function inner(){} return inner; } }\n\
             var x = B.f();",
        )
        .run()
        .unwrap();
}

#[test]
fn test_templatized_this_return_type_in_nested_function_under_this_annotation() {
    let (_t, mut builder) = new_test();
    builder
        .add_source(
            "/**\n * @constructor\n * @template T\n */\nfunction E() {}\n\
             /**\n * @this {typeof E}\n */\n\
             E.make = function() {\n\
               /** @return {this<number>} */\n\
               function inner() { return 'x'; }\n\
               return inner();\n\
             };\n\
             var y = E.make();",
        )
        .add_diagnostic_description(
            "inconsistent return type\nfound   : string\nrequired: (E<number>|null)",
        )
        .run()
        .unwrap();
}

#[test]
fn test_this_return_type_in_nested_function_under_this_annotation() {
    let (_t, mut builder) = new_test();
    builder
        .add_source(
            "/**\n * @constructor\n * @template T\n */\nfunction E() {}\n\
             /**\n * @this {typeof E}\n */\n\
             E.make = function() {\n\
               /** @return {this} */\n\
               function inner() { return 'x'; }\n\
               return inner();\n\
             };\n\
             var y = E.make();",
        )
        .add_diagnostic_description(
            "inconsistent return type\nfound   : string\nrequired: (E|null)",
        )
        .run()
        .unwrap();
}
