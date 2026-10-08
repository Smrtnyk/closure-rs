/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/Es6RewriteScriptsToModulesTest.java.

use closure_jscomp::{
    Compiler,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    es6_rewrite_scripts_to_modules::Es6RewriteScriptsToModules,
    source_file::SourceFile,
};
use std::sync::Arc;

// port: Es6RewriteScriptsToModulesTest#getOptions
fn options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options
}
// port: Es6RewriteScriptsToModulesTest#testImportedScript
#[test]
fn test_imported_script() {
    let mut compiler = Compiler::new();
    compiler.init(
        &[],
        &[
            Arc::new(SourceFile::from_code("/script.js", "")),
            Arc::new(SourceFile::from_code("/module.js", "import '/script.js';")),
        ],
        options(),
    );
    let root = compiler.parse_inputs().unwrap();
    assert!(compiler.get_errors().is_empty());
    let externs_root = root.get_first_child(&compiler).unwrap();
    let main_root = externs_root.get_next(&compiler).unwrap();
    Es6RewriteScriptsToModules::new().process(&mut compiler, externs_root, main_root);
    assert!(
        main_root
            .get_first_first_child(&compiler)
            .unwrap()
            .is_module_body(&compiler)
    );
}
// port: Es6RewriteScriptsToModulesTest#testNonImportedScript
#[test]
fn test_non_imported_script() {
    let mut compiler = Compiler::new();
    compiler.init(
        &[],
        &[
            Arc::new(SourceFile::from_code("/script.js", "")),
            Arc::new(SourceFile::from_code("/module.js", "export default 'foo';")),
        ],
        options(),
    );
    compiler.parse_inputs().unwrap();
    assert!(compiler.get_errors().is_empty());
    assert!(compiler.get_warnings().is_empty());
    let externs_root = compiler.get_externs_root().unwrap();
    let main_root = compiler.get_js_root().unwrap();
    let expected = main_root.clone_tree(&mut compiler);
    Es6RewriteScriptsToModules::new().process(&mut compiler, externs_root, main_root);
    assert!(main_root.is_equivalent_to(&compiler, expected));
}
