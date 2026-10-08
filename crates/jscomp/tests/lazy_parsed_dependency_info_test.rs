/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/LazyParsedDependencyInfoTest.java.

//! Port of LazyParsedDependencyInfoTest.java.
use closure_jscomp::{
    compiler::Compiler,
    compiler_input::CompilerInput,
    compiler_options::CompilerOptions,
    deps::{dependency_info::Require, simple_dependency_info::SimpleDependencyInfo},
    error_manager::ErrorManager,
    js_error::JSError,
    lazy_parsed_dependency_info::LazyParsedDependencyInfo,
    source_file::SourceFile,
};
use indexmap::IndexMap;

fn flags(entries: &[(&str, &str)]) -> IndexMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
// port: LazyParsedDependencyInfoTest#testDelegation
fn test_delegation() {
    let baz = Require::goog_require_symbol("baz");
    let qux = Require::goog_require_symbol("qux");
    let _compiler = Compiler::new();
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "// nothing here"));
    let delegate = SimpleDependencyInfo::builder("path/to/1.js", "path/2.js")
        .set_provides(["foo", "bar"])
        .set_requires([baz.clone(), qux.clone()])
        .build();
    let info = LazyParsedDependencyInfo::new(delegate, ast);

    assert_eq!(info.get_name(), "path/2.js");
    assert_eq!(info.get_path_relative_to_closure_base(), "path/to/1.js");
    assert_eq!(info.get_provides(), ["foo", "bar"]);
    assert_eq!(info.get_requires(), [baz, qux]);
}

#[test]
// port: LazyParsedDependencyInfoTest#testLoadFlagsParsesEs3
fn test_load_flags_parses_es3() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "// nothing here"));
    let delegate = SimpleDependencyInfo::builder("", "")
        .set_load_flags(flags(&[("foo", "bar")]))
        .build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    assert_eq!(
        info.get_load_flags(&mut compiler),
        &flags(&[("foo", "bar")])
    );
    assert!(!info.is_goog_module());
}

#[test]
// port: LazyParsedDependencyInfoTest#testLoadFlagsParsesEs5
fn test_load_flags_parses_es5() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "var x = [1, 2,];"));
    let delegate = SimpleDependencyInfo::builder("", "")
        .set_load_flags(flags(&[("module", "goog")]))
        .build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    assert_eq!(
        info.get_load_flags(&mut compiler),
        &flags(&[("module", "goog"), ("lang", "es5")])
    );
    assert!(info.is_goog_module());
}

#[test]
// port: LazyParsedDependencyInfoTest#testLoadFlagsParsesEs6Impl
fn test_load_flags_parses_es6_impl() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "class X {}"));
    let delegate = SimpleDependencyInfo::builder("", "")
        .set_load_flags(flags(&[("foo", "bar")]))
        .build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    assert_eq!(
        info.get_load_flags(&mut compiler),
        &flags(&[("foo", "bar"), ("lang", "es6")])
    );
    assert!(!info.is_goog_module());
}

#[test]
// port: LazyParsedDependencyInfoTest#testLoadFlagsParsesEs6
fn test_load_flags_parses_es6() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "let [a, b] = [1, 2];"));
    let delegate = SimpleDependencyInfo::builder("", "")
        .set_load_flags(flags(&[("foo", "bar")]))
        .build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    assert_eq!(
        info.get_load_flags(&mut compiler),
        &flags(&[("foo", "bar"), ("lang", "es6")])
    );
    assert!(!info.is_goog_module());
}

#[test]
// port: LazyParsedDependencyInfoTest#testParseIsLazy
fn test_parse_is_lazy() {
    let mut compiler = Compiler::new();
    compiler.init_options(CompilerOptions::new());
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "parse error"));
    let delegate = SimpleDependencyInfo::builder("", "").build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    info.get_name();
    info.get_path_relative_to_closure_base();
    info.get_provides();
    info.get_requires();

    assert_eq!(compiler.get_error_manager().get_error_count(), 0);
    info.get_load_flags(&mut compiler);
    assert!(compiler.get_error_manager().get_error_count() >= 1);
}

#[test]
// port: LazyParsedDependencyInfoTest#testModuleConflict
fn test_module_conflict() {
    let mut compiler = Compiler::new();
    let options = CompilerOptions::new();
    compiler.init_options(options);
    let ast = CompilerInput::new(SourceFile::from_code("file.js", "export let foo = 42;"));
    let delegate = SimpleDependencyInfo::builder("", "my/js.js")
        .set_load_flags(flags(&[("module", "goog")]))
        .build();
    let mut info = LazyParsedDependencyInfo::new(delegate, ast);

    assert!(compiler.get_error_manager().get_warnings().is_empty());
    assert_eq!(
        info.get_load_flags(&mut compiler),
        &flags(&[("module", "es6"), ("lang", "es6")])
    );
    assert!(!info.is_es6_module());
    assert_eq!(
        compiler.get_error_manager().get_warnings(),
        vec![JSError::make_with_source_location(
            "my/js.js",
            /* lineno= */ -1,
            /* charno= */ -1,
            &closure_jscomp::deps::module_loader::MODULE_CONFLICT,
            &["my/js.js"],
        )]
    );
}
