/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/WhitespaceWrapGoogModulesTest.java.

use closure_jscomp::{
    Compiler, code_printer::Builder, compiler_options::CompilerOptions, source_file::SourceFile,
    whitespace_wrap_goog_modules::WhitespaceWrapGoogModules,
};
use std::sync::Arc;

// port: WhitespaceWrapGoogModulesTest#testGoogModuleRewrite
#[test]
fn test_goog_module_rewrite() {
    let mut compiler = Compiler::new();
    compiler.init(
        &[],
        &[Arc::new(SourceFile::from_code(
            "testcode",
            "goog.module('test');\nvar f = 5;\nexports = f;\n",
        ))],
        CompilerOptions::new(),
    );
    compiler.parse_inputs().unwrap();
    assert!(compiler.get_errors().is_empty());
    assert!(compiler.get_warnings().is_empty());
    let externs = compiler.get_externs_root().unwrap();
    let root = compiler.get_js_root().unwrap();
    WhitespaceWrapGoogModules::new().process(&mut compiler, externs, root);
    assert_eq!(
        Builder::new(root).build(&compiler),
        "goog.loadModule(function(exports){\"use strict\";goog.module(\"test\");var f=5;exports=f;return exports})"
    );
}
