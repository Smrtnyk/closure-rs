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
//   test/com/google/javascript/jscomp/lint/CheckEs6ModulesTest.java.

use super::support::LintTestCase;
use closure_jscomp::{
    deps::module_loader::INVALID_MODULE_PATH,
    lint::check_es6_modules::{CheckEs6Modules, DUPLICATE_IMPORT, NO_DEFAULT_EXPORT},
};

// port: CheckEs6ModulesTest#getProcessor
// port: CheckEs6ModulesTest#setUp
fn case() -> LintTestCase {
    let mut t = LintTestCase::new(|c| Box::new(CheckEs6Modules::new(c)));
    t.ignored_warnings = vec![&INVALID_MODULE_PATH];
    t
}

// port: CheckEs6ModulesTest#testDuplicateImports
#[test]
fn test_duplicate_imports() {
    let t = case();
    t.test_same("import singleImport from 'file';");
    t.test_same("import first from 'first'; import second from 'second';");
    t.test_warning(
        "import * as first from 'file'; import {second} from 'file';",
        &DUPLICATE_IMPORT,
    );
}

// port: CheckEs6ModulesTest#testNoDefaultExport
#[test]
fn test_no_default_export() {
    let t = case();
    t.test_warning("export default 0", &NO_DEFAULT_EXPORT);
}
