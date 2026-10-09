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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/RewriteJsonToModuleTest.java.

//! Port of RewriteJsonToModuleTest. CompilerTestCase's `test(srcs, expected)` is reproduced with
//! the direct Compiler API: RewriteJsonToModule runs inside Compiler#parseInputs
//! (processJsonInputs), the processor is a no-op, and the output is compared with the parsed
//! expected source through the code printer.
use closure_jscomp::{
    Compiler,
    code_printer::Builder,
    compiler_options::CompilerOptions,
    deps::module_loader::{ModuleLoader, ResolutionMode},
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexMap;
use std::sync::Arc;

// port: RewriteJsonToModuleTest#getOptions
fn get_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    // Trigger module processing after parsing.
    options.set_process_common_js_modules(true);
    options.set_module_resolution_mode(ResolutionMode::NODE);
    options.set_package_json_entry_names(vec!["browser".to_string(), "main".to_string()]);
    options
}

fn print(name: &str, code: &str, options: CompilerOptions) -> (String, Compiler) {
    let mut compiler = Compiler::new();
    compiler.init(
        &[Arc::new(SourceFile::from_code("externs", ""))],
        &[Arc::new(SourceFile::from_code(name, code))],
        options,
    );
    compiler.parse_inputs().unwrap();
    assert!(
        compiler.get_errors().is_empty(),
        "{:?}",
        compiler.get_errors()
    );
    assert!(compiler.get_warnings().is_empty());
    let root = compiler.get_js_root().unwrap();
    (
        Builder::new(root).build(&compiler).to_string_lossy(),
        compiler,
    )
}

/// `test(srcs(SourceFile.fromCode(name, code)), expected(expected))`; returns the compiler's
/// `getModuleLoader().getPackageJsonMainEntries()`.
fn test(name: &str, code: &str, expected: &str) -> IndexMap<String, String> {
    let (actual, compiler) = print(name, code, get_options());
    let (expected, _) = print("expected0", expected, CompilerOptions::new());
    assert_eq!(actual, expected);
    compiler.get_module_loader().get_package_json_main_entries()
}

fn entries(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// port: RewriteJsonToModuleTest#testJsonFile
#[test]
fn test_json_file() {
    let main_entries = test(
        "/test.json",
        "{ 'foo': 'bar'}",
        "/** @fileoverview @suppress {undefinedVars} */\ngoog.provide('module$test_json'); var module$test_json = { 'foo': 'bar'};\n",
    );

    assert!(main_entries.is_empty());
}

// port: RewriteJsonToModuleTest#testPackageJsonFile
#[test]
fn test_package_json_file() {
    let main_entries = test(
        "/package.json",
        "{ 'main': 'foo/bar/baz.js'}",
        "/** @fileoverview @suppress {undefinedVars} */\ngoog.provide('module$package_json')\nvar module$package_json = {'main': 'foo/bar/baz.js'};\n",
    );

    assert_eq!(main_entries.len(), 1);
    assert!(main_entries.contains_key("/package.json"));
    assert_eq!(
        main_entries.get("/package.json").map(String::as_str),
        Some("/foo/bar/baz.js")
    );
}

// port: RewriteJsonToModuleTest#testPackageJsonWithoutMain
#[test]
fn test_package_json_without_main() {
    let main_entries = test(
        "/package.json",
        "{'other': { 'main': 'foo/bar/baz.js'}}",
        "/** @fileoverview @suppress {undefinedVars} */\ngoog.provide('module$package_json')\nvar module$package_json = {'other': { 'main': 'foo/bar/baz.js'}};\n",
    );

    assert!(main_entries.is_empty());
}

// port: RewriteJsonToModuleTest#testPackageJsonFileBrowserField
#[test]
fn test_package_json_file_browser_field() {
    let main_entries = test(
        "/package.json",
        "{ 'main': 'foo/bar/baz.js', 'browser': 'browser/foo.js' }",
        "/** @fileoverview @suppress {undefinedVars} */
goog.provide('module$package_json')
var module$package_json = {
  'main': 'foo/bar/baz.js',
  'browser': 'browser/foo.js'
};
",
    );

    assert_eq!(
        main_entries,
        entries(&[("/package.json", "/browser/foo.js")])
    );
}

// port: RewriteJsonToModuleTest#testPackageJsonFileBrowserFieldAdvancedUsage
#[test]
fn test_package_json_file_browser_field_advanced_usage() {
    let package_json_main_entries = test(
        "/package.json",
        "{ 'main': 'foo/bar/baz.js',
  'browser': { 'dont/include.js': false,
               'foo/bar/baz.js': 'replaced/main.js',
               'override/relative.js': './with/this.js',
               'override/explicitly.js': 'with/other.js'} }
",
        "/** @fileoverview @suppress {undefinedVars} */
goog.provide('module$package_json')
var module$package_json = {
  'main': 'foo/bar/baz.js',
  'browser': {
    'dont/include.js': false,
    'foo/bar/baz.js': 'replaced/main.js',
    'override/relative.js': './with/this.js',
    'override/explicitly.js': 'with/other.js'
  }
};
",
    );

    assert_eq!(package_json_main_entries.len(), 5);
    let get = |k: &str| package_json_main_entries.get(k).map(String::as_str);
    assert_eq!(get("/package.json"), Some("/foo/bar/baz.js"));
    assert_eq!(get("/foo/bar/baz.js"), Some("/replaced/main.js"));
    // NodeModuleResolver knows how to normalize this entry's value
    assert_eq!(get("/override/relative.js"), Some("/./with/this.js"));
    assert_eq!(
        get("/dont/include.js"),
        Some(ModuleLoader::JSC_BROWSER_SKIPLISTED_MARKER)
    );
    assert_eq!(get("/override/explicitly.js"), Some("/with/other.js"));
}

// port: RewriteJsonToModuleTest#testPackageJsonBrowserFieldAdvancedUsageGH2625
#[test]
fn test_package_json_browser_field_advanced_usage_gh2625() {
    let package_json_main_entries = test(
        "/package.json",
        "{ 'main': 'foo/bar/baz.js',
  'browser': { './a/b.js': './c/d.js',
               './server.js': 'client.js'} }
",
        "/** @fileoverview @suppress {undefinedVars} */
goog.provide('module$package_json')
var module$package_json = {
  'main': 'foo/bar/baz.js',
  'browser': {
    './a/b.js': './c/d.js',
    './server.js': 'client.js'
  }
};
",
    );

    assert_eq!(
        package_json_main_entries,
        entries(&[
            ("/package.json", "/foo/bar/baz.js"),
            // Test that we have normalized the key, value is normalized by NodeModuleResolver
            ("/a/b.js", "/./c/d.js"),
            ("/server.js", "/client.js"),
        ])
    );
}
