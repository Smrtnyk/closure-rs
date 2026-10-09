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
//   test/com/google/javascript/jscomp/lint/CheckEs6ModuleFileStructureTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    deps::module_loader::INVALID_MODULE_PATH,
    lint::check_es6_module_file_structure::{CheckEs6ModuleFileStructure, MUST_COME_BEFORE},
};

// port: CheckEs6ModuleFileStructureTest#getProcessor
// port: CheckEs6ModuleFileStructureTest#setUp
fn case() -> LintTestCase {
    let mut t = LintTestCase::new(|c| Box::new(CheckEs6ModuleFileStructure::new(c)));
    t.ignored_warnings = vec![&INVALID_MODULE_PATH];
    t
}

// port: CheckEs6ModuleFileStructureTest#testImportsMustBeFirstIfPresent
#[test]
fn test_imports_must_be_first_if_present() {
    let t = case();
    t.test_same("var notAModule;");
    t.test_same("export let noImports;");
    t.test_same("import importsFirst from 'file'; let notFirst;");
    t.test_warning("let first; import notFirst from 'file';", &MUST_COME_BEFORE);
}

// port: CheckEs6ModuleFileStructureTest#testDeclareModuleIdAfterImportsIfPresent
#[test]
fn test_declare_module_id_after_imports_if_present() {
    let t = case();
    t.test_same("import noDeclareNamespace from 'file';");
    t.test_same("goog.declareModuleId('name.space'); export let noImports;");
    t.test_same("import first from 'file'; goog.declareModuleId('name.space');");
    t.test_warning(
        "export let first; goog.declareModuleId('name.space');",
        &MUST_COME_BEFORE,
    );
    t.test_warning(
        "import first from 'file'; let second; goog.declareModuleId('name.space');",
        &MUST_COME_BEFORE,
    );
}

// port: CheckEs6ModuleFileStructureTest#testGoogRequireAfterImportsAndDeclareNamesButBeforeOthers
#[test]
fn test_goog_require_after_imports_and_declare_names_but_before_others() {
    let t = case();
    t.test_same("const bar = goog.require('bar');");
    t.test_same("const bar = goog.require('bar'); let notAModule;");
    t.test_same("let notAModule; const bar = goog.require('bar');");
    t.test_same("const bar = goog.require('bar'); export {};");
    t.test_same_warning(
        "export {}; const bar = goog.require('bar');",
        &MUST_COME_BEFORE,
    );
    t.test_same("goog.declareModuleId('name'); const bar = goog.require('bar'); export {};");
    t.test_warning(
        "const bar = goog.require('bar'); goog.declareModuleId('name'); export {};",
        &MUST_COME_BEFORE,
    );
    t.test_same("import 'file'; const bar = goog.require('bar');");
    t.test_warning(
        "const bar = goog.require('bar'); import 'file';",
        &MUST_COME_BEFORE,
    );
    t.test_same("import 'file'; goog.require('bar');");
    t.test_warning("goog.require('bar'); import 'file';", &MUST_COME_BEFORE);
    t.test_same(
        r#"import 'file';
goog.declareModuleId('name');
const bar = goog.require('bar');
let rest;
"#,
    );
}
