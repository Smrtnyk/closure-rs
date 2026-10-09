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
//   test/com/google/javascript/jscomp/lint/CheckPrototypePropertiesTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_prototype_properties::{
    CheckPrototypeProperties, ILLEGAL_PROTOTYPE_MEMBER,
};

// port: CheckPrototypePropertiesTest#getProcessor
// port: CheckPrototypePropertiesTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckPrototypeProperties::new(c))).with_lint_checks_warning()
}

// port: CheckPrototypePropertiesTest#testNoWarning
#[test]
fn test_no_warning() {
    let t = case();
    t.test_same("function C() {}; C.prototype.foo = null;");
    t.test_same("function C() {}; C.prototype.foo = undefined;");
    t.test_same("function C() {}; C.prototype.foo;");
    t.test_same("function C() {}; C.prototype.foo = 0;");
    t.test_same("function C() {}; C.prototype.foo = 'someString';");
    t.test_same("function C() {}; C.prototype.foo = function() {};");
    t.test_same("function C() {}; /** @enum {number} */ C.prototype.foo = { BAR: 0 };");
}

// port: CheckPrototypePropertiesTest#testNoWarning_withES6Modules
#[test]
fn test_no_warning_with_es6_modules() {
    let t = case();
    t.test_same("export function C() {}; C.prototype.foo = null;");
}

// port: CheckPrototypePropertiesTest#testWarnings
#[test]
fn test_warnings() {
    let t = case();
    t.test_same_warning(
        "function C() {}; C.prototype.foo = [];",
        &ILLEGAL_PROTOTYPE_MEMBER,
    );
    t.test_same_warning(
        "function C() {}; C.prototype.foo = {};",
        &ILLEGAL_PROTOTYPE_MEMBER,
    );
    t.test_same_warning(
        "function C() {}; C.prototype.foo = { BAR: 0 };",
        &ILLEGAL_PROTOTYPE_MEMBER,
    );
}

// port: CheckPrototypePropertiesTest#testWarnings_withES6Modules
#[test]
fn test_warnings_with_es6_modules() {
    let t = case();
    t.test_same_warning(
        "export function C() {}; C.prototype.foo = [];",
        &ILLEGAL_PROTOTYPE_MEMBER,
    );
}
