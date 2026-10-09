/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckNoMutatedEs6ExportsTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_no_mutated_es6_exports::{
    CheckNoMutatedEs6Exports, MUTATED_EXPORT,
};

// port: CheckNoMutatedEs6ExportsTest#getProcessor
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckNoMutatedEs6Exports::new(c)))
}

// port: CheckNoMutatedEs6ExportsTest#testNeverMutatedExportIsOk
#[test]
fn test_never_mutated_export_is_ok() {
    let t = case();
    t.test_same("export let x = 0;");
}

// port: CheckNoMutatedEs6ExportsTest#testLocalWithExportedNameCanBeMutated
#[test]
fn test_local_with_exported_name_can_be_mutated() {
    let t = case();
    t.test_same("export let x = 0; () => { let x = 0; x++; }");
}

// port: CheckNoMutatedEs6ExportsTest#testMutatedDuringInitializationIsOk
#[test]
fn test_mutated_during_initialization_is_ok() {
    let t = case();
    t.test_same("export let x = 0; x++;");
    t.test_same("let x = 0; export {x as y}; x++;");
}

// port: CheckNoMutatedEs6ExportsTest#testMutatedInInnerScopeIsError
#[test]
fn test_mutated_in_inner_scope_is_error() {
    let t = case();
    t.test_warning("export let x = 0; () => x++;", &MUTATED_EXPORT);
    t.test_warning("let x = 0; export {x as y}; () => x++;", &MUTATED_EXPORT);
    t.test_warning("export function foo() {}; () => foo = 0;", &MUTATED_EXPORT);
    t.test_warning(
        "export default function foo() {}; () => foo = 0;",
        &MUTATED_EXPORT,
    );
    t.test_warning(
        "export default class Foo {}; () => Foo = 0;",
        &MUTATED_EXPORT,
    );
    t.test_warning("export function foo() { foo = 0; };", &MUTATED_EXPORT);
    t.test_warning(
        "export let x = 0; export function foo() { x++; };",
        &MUTATED_EXPORT,
    );
}

// port: CheckNoMutatedEs6ExportsTest#testExportDefaultExpressionIsOk
#[test]
fn test_export_default_expression_is_ok() {
    let t = case();
    t.test_same("export default 0;");
    t.test_same("export default {prop: 0}");
    t.test_same("export default function() {};");
    t.test_same("export default class {};");
}
