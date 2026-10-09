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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/InjectTranspilationRuntimeLibrariesTest.java.

//! Port of InjectTranspilationRuntimeLibrariesTest.java.
use closure_jscomp::{
    Compiler,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    inject_transpilation_runtime_libraries::InjectTranspilationRuntimeLibraries,
    js::runtime_js_lib_manager::RuntimeLibraryMode,
    source_file::SourceFile,
};
use closure_rhino::fast_hash::IndexSet;
use std::sync::Arc;

struct InjectTranspilationRuntimeLibrariesTest {
    compiler: Compiler,
    language_out: LanguageMode,
    instrument_async_context: bool,
}

impl InjectTranspilationRuntimeLibrariesTest {
    // port: InjectTranspilationRuntimeLibrariesTest#setup
    fn setup() -> Self {
        Self {
            compiler: Compiler::new(),
            language_out: LanguageMode::ECMASCRIPT5,
            instrument_async_context: false,
        }
    }

    /// Parses the given code and runs the `InjectTranspilationRuntimeLibraries` pass over the
    /// resulting AST
    ///
    /// Returns the set of paths to all the injected libraries
    // port: InjectTranspilationRuntimeLibrariesTest#parseAndRunInjectionPass
    fn parse_and_run_injection_pass(&mut self, js: &str) -> IndexSet<String> {
        let mut options = CompilerOptions::new();
        options.set_language_out(self.language_out);
        options.set_instrument_async_context(self.instrument_async_context);
        options.set_runtime_library_mode(RuntimeLibraryMode::RECORD_ONLY);

        let compiler = &mut self.compiler;
        compiler.init(
            &[Arc::new(SourceFile::from_code("externs", ""))],
            &[Arc::new(SourceFile::from_code("testcode", js))],
            options,
        );
        compiler.parse();
        let externs_root = compiler.get_externs_root().unwrap();
        let js_root = compiler.get_js_root().unwrap();
        InjectTranspilationRuntimeLibraries::new(compiler).process(compiler, externs_root, js_root);

        let runtime_libs = compiler.get_runtime_js_lib_manager();
        let injected = runtime_libs.lock().unwrap().get_injected_libraries();
        injected.into_iter().collect()
    }
}

/// Truth's `assertThat(set).containsExactly(...)` (order-insensitive).
fn assert_contains_exactly(actual: &IndexSet<String>, expected: &[&str]) {
    let expected: IndexSet<String> = expected.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "{actual:?} contains exactly {expected:?}"
    );
    assert!(
        expected.iter().all(|e| actual.contains(e)),
        "{actual:?} contains exactly {expected:?}"
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testEmptyInjected
#[test]
fn test_empty_injected() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("");

    assert!(injected.is_empty(), "{injected:?} is empty");
}

// port: InjectTranspilationRuntimeLibrariesTest#testMakeIteratorAndObjectAssignInjectedForObjectPatternRest
#[test]
fn test_make_iterator_and_object_assign_injected_for_object_pattern_rest() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("const {a, ...rest} = something();");
    assert_contains_exactly(&injected, &["es6/util/makeiterator", "es6/object/assign"]);
}

// port: InjectTranspilationRuntimeLibrariesTest#testObjectAssignInjectedForObjectSpread
#[test]
fn test_object_assign_injected_for_object_spread() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("const obj = {a, ...rest};");
    assert_contains_exactly(&injected, &["es6/object/assign"]);
}

// port: InjectTranspilationRuntimeLibrariesTest#testForOf_injectsMakeIterator
#[test]
fn test_for_of_injects_make_iterator() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("for (x of []) {}");

    assert_contains_exactly(
        &injected,
        &["es6/util/makeiterator", "es6/util/iteratorclose"],
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testArrayPattern_injectsMakeIterator
#[test]
fn test_array_pattern_injects_make_iterator() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("var [a] = [];");

    assert_contains_exactly(&injected, &["es6/util/makeiterator"]);
}

// port: InjectTranspilationRuntimeLibrariesTest#testObjectPattern_injectsNothing
#[test]
fn test_object_pattern_injects_nothing() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("var {a} = {};");

    assert!(injected.is_empty(), "{injected:?} is empty");
}

// port: InjectTranspilationRuntimeLibrariesTest#testArrayPatternRest_injectsArrayFromIterator
#[test]
fn test_array_pattern_rest_injects_array_from_iterator() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("var [...a] = [];");

    assert_contains_exactly(
        &injected,
        &["es6/util/makeiterator", "es6/util/arrayfromiterator"],
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testInjectsExecuteAsyncFunctionSupport_es5Out_includesGeneratorEngine
#[test]
fn test_injects_execute_async_function_support_es5_out_includes_generator_engine() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("async function foo() {}");

    assert_contains_exactly(
        &injected,
        &["es6/execute_async_generator", "es6/generator_engine"],
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testInjectsExecuteAsyncFunctionSupport_es6Out_doesNotIncludeGeneratorEngine
#[test]
fn test_injects_execute_async_function_support_es6_out_does_not_include_generator_engine() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    t.language_out = LanguageMode::ECMASCRIPT_2015;
    let injected = t.parse_and_run_injection_pass("async function foo() {}");

    assert_contains_exactly(&injected, &["es6/execute_async_generator"]);
}

// port: InjectTranspilationRuntimeLibrariesTest#testTaggedTemplateFirstArgCreaterInjected
#[test]
fn test_tagged_template_first_arg_creater_injected() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("function tag(...a) {}; tag`hello`;");
    assert_contains_exactly(
        &injected,
        &[
            "es6/util/createtemplatetagfirstarg",
            "es6/util/restarguments",
        ],
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testClassInheritance_injectsInheritsAndConstruct
#[test]
fn test_class_inheritance_injects_inherits_and_construct() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("class A {} class B extends A {}");
    assert_contains_exactly(
        &injected,
        &[
            "es6/util/inherits",
            "es6/util/construct",
            "es6/util/arrayfromiterable",
        ],
    );
}

// port: InjectTranspilationRuntimeLibrariesTest#testClass_noInheritances_doesNotInject
#[test]
fn test_class_no_inheritances_does_not_inject() {
    let mut t = InjectTranspilationRuntimeLibrariesTest::setup();
    let injected = t.parse_and_run_injection_pass("class A {}");
    assert!(injected.is_empty(), "{injected:?} is empty");
}
