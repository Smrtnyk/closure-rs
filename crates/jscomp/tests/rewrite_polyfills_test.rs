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
//   test/com/google/javascript/jscomp/RewritePolyfillsTest.java.

//! Port of RewritePolyfillsTest.java: unit tests for the RewritePolyfills compiler pass.
use closure_jscomp::{
    compiler_options::LanguageMode,
    diagnostic_groups::MISSING_POLYFILL,
    diagnostic_type::DiagnosticType,
    js::runtime_js_lib_manager::{RuntimeJsLibManager, RuntimeLibraryMode},
    polyfill_usage_finder::Polyfills,
    rewrite_polyfills::{INSUFFICIENT_OUTPUT_VERSION_ERROR, RewritePolyfills},
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::{CheckLevel, Compiler, CompilerOptions},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use indexmap::{IndexMap, IndexSet};
use std::{cell::RefCell, rc::Rc};

const LANGUAGE_MODE: &str = "com.google.javascript.jscomp.CompilerOptions$LanguageMode";

// port: RewritePolyfillsTest#ES_2020
const ES_2020: &str = "ECMASCRIPT_2020";
// port: RewritePolyfillsTest#ES6
const ES6: &str = "ECMASCRIPT_2015";
// port: RewritePolyfillsTest#ES5
const ES5: &str = "ECMASCRIPT5_STRICT";
// port: RewritePolyfillsTest#ES3
const ES3: &str = "ECMASCRIPT3";

/// `LanguageMode.<name>` as the harness's option value.
fn language_mode(name: &str) -> DslValue {
    DslValue::Enum {
        class: LANGUAGE_MODE.into(),
        name: name.into(),
    }
}

struct RewritePolyfillsTest {
    ctx: Ctx,
    injectable_libraries: IndexMap<String, String>,
    inject_before_pass: IndexSet<String>,
    polyfill_table: Vec<String>,
    isolate_polyfills: bool,
    inject_polyfills: bool,
    inject_polyfills_newer_than: Option<LanguageMode>,
}

impl RewritePolyfillsTest {
    fn new() -> Self {
        Self {
            ctx: Ctx::new(
                "RewritePolyfillsTest".into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::new(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            injectable_libraries: IndexMap::new(),
            inject_before_pass: IndexSet::new(),
            polyfill_table: Vec::new(),
            isolate_polyfills: false,
            inject_polyfills: true,
            inject_polyfills_newer_than: None,
        }
    }

    // port: RewritePolyfillsTest#addLibrary
    fn add_library(&mut self, name: &str, from: &str, to: &str, library: Option<&str>) {
        if let Some(library) = library {
            self.injectable_libraries.insert(
                library.to_string(),
                format!("$jscomp.polyfill('{name}', function() {{}}, '{from}', '{to}');\n"),
            );
        }
        self.polyfill_table
            .push(format!("{name} {from} {to} {}", library.unwrap_or("")));
    }

    // port: RewritePolyfillsTest#createRuntimeJsLibManager
    fn create_runtime_js_lib_manager(&self, compiler: &mut Compiler) -> RuntimeJsLibManager {
        let injectable_libraries = self.injectable_libraries.clone();
        let mut runtime_libs = RuntimeJsLibManager::create(
            RuntimeLibraryMode::INJECT,
            // stub out the resource parsing
            Box::new(
                move |compiler: &mut Compiler, resource: &str, _path: &str| {
                    let code = injectable_libraries.get(resource).unwrap_or_else(|| {
                        panic!("NullPointerException: injectableLibraries.get({resource})")
                    });
                    Some(compiler.parse_test_code(code.as_str()))
                },
            ),
            Compiler::get_change_tracker_and_ast,
            Box::new(|compiler: &mut Compiler| compiler.get_node_for_code_insertion(None)),
        );
        for to_inject in &self.inject_before_pass {
            runtime_libs.ensure_library_injected(compiler, to_inject, /* force= */ false);
        }
        runtime_libs
    }

    // port: RewritePolyfillsTest#addLibraries
    fn add_libraries(&self, code: &str, libraries: &[&str]) -> String {
        let mut expected = String::new();
        for library in libraries {
            expected.push_str(
                self.injectable_libraries
                    .get(*library)
                    .map_or("null", |s| s),
            );
        }
        expected.push_str(code);
        expected
    }
}

impl CompilerTestCaseHooks for RewritePolyfillsTest {
    // port: RewritePolyfillsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let runtime_libs = self.create_runtime_js_lib_manager(&mut compiler.borrow_mut());
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            RewritePolyfills::new_for_testing(
                runtime_libs,
                Polyfills::from_table(&self.polyfill_table.join("\n")),
                self.inject_polyfills,
                self.isolate_polyfills,
                self.inject_polyfills_newer_than,
            ),
        )))))
    }

    // port: RewritePolyfillsTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_warning_level(MISSING_POLYFILL.clone(), CheckLevel::WARNING);
        options.set_runtime_library_mode(RuntimeLibraryMode::RECORD_ONLY);
        Ok(options)
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: RewritePolyfillsTest,
}

impl Fixture {
    // port: RewritePolyfillsTest#setUp
    fn new() -> Self {
        let mut h = CompilerTestCase::new("");
        h.set_up();
        let t = RewritePolyfillsTest::new();
        h.set_language_out(language_mode("ECMASCRIPT5")).unwrap();
        Self { h, t }
    }

    fn add_library(&mut self, name: &str, from: &str, to: &str, library: Option<&str>) {
        self.t.add_library(name, from, to, library);
    }

    fn set_language(&mut self, lang_in: &str, lang_out: &str) {
        self.h
            .set_language(language_mode(lang_in), language_mode(lang_out))
            .unwrap();
    }

    fn set_accepted_language(&mut self, lang: &str) {
        self.h.set_accepted_language(language_mode(lang)).unwrap();
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.h
            .test(
                &mut self.t,
                vec![
                    TestPart::Sources(CompilerTestCase::srcs(js)),
                    TestPart::Expected(CompilerTestCase::expected(expected)),
                ],
            )
            .unwrap();
    }

    fn test_same(&mut self, js: &str) {
        self.h
            .test_same(
                &mut self.t,
                vec![TestPart::Sources(CompilerTestCase::srcs(js))],
            )
            .unwrap();
    }

    fn test_extern_changes(&mut self, input: &str, expected_extern: &str) {
        self.h
            .test_extern_changes_default(
                &mut self.t,
                &CompilerTestCase::srcs(input),
                &CompilerTestCase::expected(expected_extern),
                &[],
            )
            .unwrap();
    }

    // port: RewritePolyfillsTest#testDoesNotInject
    fn test_does_not_inject(&mut self, code: &str) {
        self.test_injects(code, &[]); // empty list of injections
    }

    // port: RewritePolyfillsTest#testInjects(String,String...)
    fn test_injects(&mut self, code: &str, libraries: &[&str]) {
        let expected = self.t.add_libraries(code, libraries);
        self.test(code, &expected);
    }

    // port: RewritePolyfillsTest#testInjects(String,DiagnosticType,String...)
    fn test_injects_warning(
        &mut self,
        code: &str,
        warning: &'static DiagnosticType,
        libraries: &[&str],
    ) {
        let expected = self.t.add_libraries(code, libraries);
        self.h
            .test(
                &mut self.t,
                vec![
                    TestPart::Sources(CompilerTestCase::srcs(code)),
                    TestPart::Expected(CompilerTestCase::expected(expected.as_str())),
                    TestPart::Diagnostic(CompilerTestCase::warning(warning)),
                ],
            )
            .unwrap();
    }
}

// port: RewritePolyfillsTest#testEmpty
#[test]
fn test_empty() {
    let mut f = Fixture::new();
    f.set_language(ES6, ES5);
    f.test_injects("", &[]);
}

// port: RewritePolyfillsTest#testClassesInjected
#[test]
fn test_classes_injected() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.add_library("Set", "es6", "es3", Some("es6/set"));
    f.set_language(ES6, ES5);
    f.test_injects("var m = new Map();", &["es6/map"]);
    f.test_injects("var m = new goog.global.Map();", &["es6/map"]);
    f.test_injects("var Map = goog.global.Map; new Map();", &["es6/map"]);
    f.test_injects("var m = new window.Map();", &["es6/map"]);
    f.set_language(ES6, ES3);
    f.test_injects("var s = new Set();", &["es6/set"]);
}

// port: RewritePolyfillsTest#testLibrariesOnlyInjectedOnce
#[test]
fn test_libraries_only_injected_once() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_injects("var m = new Map(); m = new Map();", &["es6/map"]);
}

// port: RewritePolyfillsTest#testClassesNotInjectedIfSufficientLanguageOut
#[test]
fn test_classes_not_injected_if_sufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("Proxy", "es6", "es6", None);
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.add_library("Set", "es6", "es3", Some("es6/set"));
    f.set_language(ES6, ES6);
    f.test_does_not_inject("new Proxy();");
    f.test_does_not_inject("var m = new Map();");
    f.test_does_not_inject("new Set();");
}

// port: RewritePolyfillsTest#testClassesNotInjectedIfDeclaredInScope
#[test]
fn test_classes_not_injected_if_declared_in_scope() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_does_not_inject("/** @constructor */ var Map = function() {}; new Map();");
}

// port: RewritePolyfillsTest#testClassesWarnIfInsufficientLanguageOut
#[test]
fn test_classes_warn_if_insufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("Proxy", "es6", "es6", None);
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_injects_warning("new Proxy();", &INSUFFICIENT_OUTPUT_VERSION_ERROR, &[]);
    f.set_language(ES6, ES3);
    f.test_injects_warning(
        "new Map();",
        &INSUFFICIENT_OUTPUT_VERSION_ERROR,
        &["es6/map"],
    );
}

// port: RewritePolyfillsTest#testGlobalThisInjected
#[test]
fn test_global_this_injected() {
    let mut f = Fixture::new();
    f.add_library("globalThis", "es_2020", "es3", Some("es6/globalThis"));
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.test_injects("globalThis.String", &["es6/globalThis"]);
    f.test_injects(
        "var m = new globalThis.Map();",
        &["es6/globalThis", "es6/map"],
    );
}

// port: RewritePolyfillsTest#testGlobalThisInjectedNotInjectedGivenSufficientLanguageOut
#[test]
fn test_global_this_injected_not_injected_given_sufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("globalThis", "es_2020", "es3", Some("es6/globalThis"));
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES_2020, ES_2020);
    f.test_does_not_inject("globalThis.String");
    f.test_does_not_inject("var m = new globalThis.Map();");
}

// port: RewritePolyfillsTest#testStaticMethodsInjected
#[test]
fn test_static_methods_injected() {
    let mut f = Fixture::new();
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.add_library("Array.of", "es6", "es3", Some("es6/array/of"));
    f.add_library("Object.keys", "es5", "es3", Some("es5/object/keys"));
    f.set_language(ES6, ES5);
    f.test_injects("Math.clz32(x);", &["es6/math/clz32"]);
    f.set_language(ES6, ES3);
    f.test_injects("Array.of(x);", &["es6/array/of"]);
    f.set_language(ES6, ES3);
    f.test_injects("Object.keys(x);", &["es5/object/keys"]);
}

// port: RewritePolyfillsTest#testStaticMethodsNotInjectedIfSufficientLanguageOut
#[test]
fn test_static_methods_not_injected_if_sufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("Array.from", "es6", "es6", None);
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.add_library("Array.of", "es6", "es3", Some("es6/array/of"));
    f.add_library("Object.keys", "es5", "es3", Some("es5/object/keys"));
    f.set_language(ES6, ES6);
    f.test_does_not_inject("Array.from(x);");
    f.test_does_not_inject("Math.clz32(x);");
    f.test_does_not_inject("Array.of(x);");
    f.set_language(ES5, ES5);
    f.test_does_not_inject("Object.keys(x);");
}

// port: RewritePolyfillsTest#testStaticMethodsNotInjectedIfDeclaredInScope
#[test]
fn test_static_methods_not_injected_if_declared_in_scope() {
    let mut f = Fixture::new();
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.set_language(ES6, ES5);
    f.test_does_not_inject("var Math = {clz32: function() {}}; Math.clz32(x);");
}

// port: RewritePolyfillsTest#testStaticMethodsWarnIfInsufficientLanguageOut
#[test]
fn test_static_methods_warn_if_insufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("Array.from", "es6", "es6", None);
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.set_language(ES6, ES5);
    f.test_injects_warning("Array.from(x);", &INSUFFICIENT_OUTPUT_VERSION_ERROR, &[]);
    f.set_language(ES6, ES3);
    f.test_injects_warning(
        "Math.clz32(x);",
        &INSUFFICIENT_OUTPUT_VERSION_ERROR,
        &["es6/math/clz32"],
    );
}

// port: RewritePolyfillsTest#testStaticMethodsNotInstalledIfGuardedByIf
#[test]
fn test_static_methods_not_installed_if_guarded_by_if() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es5", Some("es6/array/of"));
    f.test_does_not_inject("if (Array.of) { Array.of(); } else { Array.of(); }");
    f.test_does_not_inject("if (x || Array.of) { Array.of(x); }");
    f.test_does_not_inject("if (x && Array.of) { Array.of(x); }");
    f.test_does_not_inject("if (!Array.of) { Array.of(x); }");
    f.test_does_not_inject("if (Array.of != 'x') { Array.of(x); }");
    f.test_does_not_inject("if (Array.of !== 'x') { Array.of(x); }");
    f.test_does_not_inject("if (Array.of == 'x') { Array.of(x); }");
    f.test_does_not_inject("if (Array.of === 'x') { Array.of(x); }");
    f.test_does_not_inject("if (typeof Array.of == 'function') { Array.of(); }");
}

// port: RewritePolyfillsTest#testStaticMethodsNotInstalledIfGuardedByLogicalOperator
#[test]
fn test_static_methods_not_installed_if_guarded_by_logical_operator() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es5", Some("es6/array/of"));
    f.test_does_not_inject("Array.of && Array.of();");
    f.test_does_not_inject("!Array.of || Array.of();");
    f.test_injects("x && Array.of;", &["es6/array/of"]);
    f.test_injects("Array.of || Array.of();", &["es6/array/of"]);
}

// port: RewritePolyfillsTest#testStaticMethodsNotInstalledIfGuardedByNullishCoalesce
#[test]
fn test_static_methods_not_installed_if_guarded_by_nullish_coalesce() {
    let mut f = Fixture::new();
    f.set_accepted_language("UNSTABLE");
    f.add_library("Array.of", "es_unstable", "es5", Some("es6/array/of"));
    f.test_does_not_inject("!Array.of ?? Array.of();");
    f.set_language(ES_2020, ES5);
    f.test_injects("Array.of ?? Array.of();", &["es6/array/of"]);
}

// port: RewritePolyfillsTest#testStaticMethodsNotInstalledIfGuardedByHook
#[test]
fn test_static_methods_not_installed_if_guarded_by_hook() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es5", Some("es6/array/of"));
    f.test_does_not_inject("var x = Array.of ? y : function(z) { Array.of(z); };");
    f.test_does_not_inject("var x = Array.of ? function(y) { Array.of(y); } : z;");
    f.test_does_not_inject("typeof Array.of ? Array.of(x) : Array.of(y);");
    f.test_does_not_inject("String(Array.of) == 'foo' ? Array.of(x) : Array.of(y);");
    f.test_does_not_inject("Array.of instanceof Function ? Array.of(x) : Array.of(y);");
    f.test_injects("x ? (Array.of ? y : z) : Array.of(x)", &["es6/array/of"]);
}

// port: RewritePolyfillsTest#testStaticMethodsNotInstalledIfGuardedByAbruptReturn
#[test]
fn test_static_methods_not_installed_if_guarded_by_abrupt_return() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es5", Some("es6/array/of"));
    f.test_does_not_inject("if (!Array.of) throw 'x'; Array.of();");
    f.test_does_not_inject("!function() { if (!Array.of) return; Array.of(); }");
    f.test_does_not_inject("if (!Array.of) { throw 'x'; } Array.of();");
    f.test_injects(
        "{ if (!Array.of) throw 'x'; } Array.of();",
        &["es6/array/of"],
    );
    f.test_injects(
        "!function() { { if (!Array.of) return; } Array.of(); }()",
        &["es6/array/of"],
    );
    f.test_injects(
        "if (!Array.of) { { throw 'x'; } } Array.of();",
        &["es6/array/of"],
    );
    f.test_injects(
        "{ if (!Array.of) throw 'x'; throw 'x'; } Array.of();",
        &["es6/array/of"],
    );
    f.test_injects(
        "{ if (Array.of) throw 'x'; { throw 'x'; } } Array.of();",
        &["es6/array/of"],
    );
    f.test_injects(
        "if (unrelated) { if (Array.of) throw 'x'; { throw 'x'; } } else Array.of();",
        &["es6/array/of"],
    );
}

// port: RewritePolyfillsTest#testPrototypeMethodsInjected
#[test]
fn test_prototype_methods_injected() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.add_library("Array.prototype.fill", "es6", "es3", Some("es6/array/fill"));
    f.add_library(
        "Array.prototype.forEach",
        "es5",
        "es3",
        Some("es5/array/foreach"),
    );
    f.set_language(ES6, ES5);
    f.test_injects("x.endsWith(y);", &["es6/string/endswith"]);
    f.test_injects("x.fill();", &["es6/array/fill"]);
    f.set_language(ES5, ES3);
    f.test_injects("x.forEach(y);", &["es5/array/foreach"]);
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInjectedIfSufficientLanguageOut
#[test]
fn test_prototype_methods_not_injected_if_sufficient_language_out() {
    let mut f = Fixture::new();
    f.add_library("String.prototype.normalize", "es6", "es6", None);
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.add_library("Array.prototype.fill", "es6", "es3", Some("es6/array/fill"));
    f.add_library(
        "Array.prototype.forEach",
        "es5",
        "es3",
        Some("es5/array/foreach"),
    );
    f.set_language(ES6, ES6);
    f.test_does_not_inject("x.normalize();");
    f.test_does_not_inject("x.endsWith();");
    f.test_does_not_inject("x.fill(y);");
    f.set_language(ES5, ES5);
    f.test_does_not_inject("x.forEach();");
}

// port: RewritePolyfillsTest#testPrototypeMethodsDontWarnIfInsufficientVersionNumber
#[test]
fn test_prototype_methods_dont_warn_if_insufficient_version_number() {
    let mut f = Fixture::new();
    f.add_library("String.prototype.normalize", "es6", "es6", None);
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.add_library("Array.prototype.fill", "es6", "es3", Some("es6/array/fill"));
    f.set_language(ES6, ES3);
    f.test_does_not_inject("x.normalize();");
    f.test_injects("x.endsWith();", &["es6/string/endswith"]);
    f.test_injects("x.fill(y);", &["es6/array/fill"]);
}

// port: RewritePolyfillsTest#testMultiplePrototypeMethodsWithSameName
#[test]
fn test_multiple_prototype_methods_with_same_name() {
    let mut f = Fixture::new();
    f.add_library(
        "Array.prototype.includes",
        "es6",
        "es3",
        Some("es6/array/includes"),
    );
    f.add_library(
        "String.prototype.includes",
        "es5",
        "es3",
        Some("es5/string/includes"),
    );
    f.set_language(ES6, ES5);
    f.test_injects("x.includes();", &["es6/array/includes"]);
    f.set_language(ES5, ES3);
    f.test_injects(
        "x.includes();",
        &["es6/array/includes", "es5/string/includes"],
    );
}

// port: RewritePolyfillsTest#testPrototypeMethodsInstalledIfStaticMethodShadowed
#[test]
fn test_prototype_methods_installed_if_static_method_shadowed() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.set_language(ES6, ES5);
    f.test_injects("var string = {}; string.endsWith = function() {};\nstring.foo = function(string) { return string.endsWith('x'); };\n", &["es6/string/endswith"]);
}

// port: RewritePolyfillsTest#testPrototypeMethodsInstalledIfActuallyStatic
#[test]
fn test_prototype_methods_installed_if_actually_static() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.set_language(ES6, ES5);
    f.test_injects(
        "var string = {}; string.endsWith = function() {}; string.endsWith('x');",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "var string = {endsWith: function() {}}; string.endsWith('x');",
        &["es6/string/endswith"],
    );
    f.test_injects("var string = {}; string.endsWith = function() {};\nstring.foo = function() { return string.endsWith('x'); };\n", &["es6/string/endswith"]);
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInstalledIfGuardedByIf
#[test]
fn test_prototype_methods_not_installed_if_guarded_by_if() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject(
        "if (String.prototype.endsWith) { x.endsWith(); } else { y.endsWith(); }",
    );
    f.test_does_not_inject("if (x || String.prototype.endsWith) { x.endsWith(); }");
    f.test_does_not_inject("if (x && String.prototype.endsWith) { x.endsWith(); }");
    f.test_does_not_inject("if (!String.prototype.endsWith) { x.endsWith(); }");
    f.test_does_not_inject("if (String.prototype.endsWith != 'x') { x.endsWith(); }");
    f.test_does_not_inject("if (String.prototype.endsWith !== 'x') { x.endsWith(); }");
    f.test_does_not_inject("if (String.prototype.endsWith == 'x') { x.endsWith(); }");
    f.test_does_not_inject("if (String.prototype.endsWith === 'x') { x.endsWith(); }");
    f.test_does_not_inject("if (typeof String.prototype.endsWith == 'function') { x.endsWith(); }");
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInstalledIfGuardedByLogicalOperator
#[test]
fn test_prototype_methods_not_installed_if_guarded_by_logical_operator() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject("String.prototype.endsWith && x.endsWith();");
    f.test_does_not_inject("!String.prototype.endsWith || x.endsWith();");
    f.test_does_not_inject("x.endsWith && x.endsWith();");
    f.test_injects("x && x.endsWith;", &["es6/string/endswith"]);
    f.test_injects(
        "String.prototype.endsWith || x.endsWith();",
        &["es6/string/endswith"],
    );
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInstalledIfGuardedByNullishCoalesce
#[test]
fn test_prototype_methods_not_installed_if_guarded_by_nullish_coalesce() {
    let mut f = Fixture::new();
    f.set_accepted_language("UNSTABLE");
    f.add_library(
        "String.prototype.endsWith",
        "es_unstable",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject("!String.prototype.endsWith ?? x.endsWith();");
    f.set_language(ES_2020, ES5);
    f.test_injects(
        "String.prototype.endsWith ?? x.endsWith();",
        &["es6/string/endswith"],
    );
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInstalledIfGuardedByHook
#[test]
fn test_prototype_methods_not_installed_if_guarded_by_hook() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject(
        "var x = String.prototype.endsWith ? y : function(z) { z.endsWith(); };",
    );
    f.test_does_not_inject(
        "var x = String.prototype.endsWith ? function(y) { y.endsWith(); } : z;",
    );
    f.test_does_not_inject("typeof String.prototype.endsWith ? x.endsWith() : y.endsWith();");
    f.test_does_not_inject("String(x.endsWith) == 'foo' ? x.endsWith() : y.endsWith();");
    f.test_does_not_inject("Boolean(x.endsWith) ? x.endsWith() : y.endsWith();");
    f.test_injects(
        "x ? (String.prototype.endsWith ? y : z) : x.endsWith()",
        &["es6/string/endswith"],
    );
}

// port: RewritePolyfillsTest#testPrototypeMethodsNotInstalledIfGuardedByAbruptReturn
#[test]
fn test_prototype_methods_not_installed_if_guarded_by_abrupt_return() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject("if (!x.endsWith) throw 'x'; y.endsWith();");
    f.test_does_not_inject("if (!x.endsWith) { throw 'x'; } y.endsWith();");
    f.test_does_not_inject("!function() { if (!x.endsWith) return; y.endsWith(); }()");
    f.test_injects(
        "{ if (!x.endsWith) throw 'x'; } y.endsWith();",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "if (!x.endsWith) { { throw 'x'; } } y.endsWith();",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "!function() { { if (!x.endsWith) return; } y.endsWith(); }()",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "{ if (!x.endsWith) throw 'x'; throw 'x'; } y.endsWith();",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "{ if (x.endsWith) throw 'x'; { throw 'x'; } } x.endsWith();",
        &["es6/string/endswith"],
    );
    f.test_injects(
        "if (unrelated) { if (x.endsWith) throw 'x'; { throw 'x'; } } else x.endsWith();",
        &["es6/string/endswith"],
    );
}

// port: RewritePolyfillsTest#testCleansUpUnnecessaryPolyfills
#[test]
fn test_cleans_up_unnecessary_polyfills() {
    let mut f = Fixture::new();
    f.t.injectable_libraries.insert("es6/set".into(), "$jscomp.polyfill('Set', function() {}, 'es6', 'es3');\n$jscomp.polyfill('Map', function() {}, 'es5', 'es3');\n".into());
    f.t.polyfill_table.push("Set es6 es3 es6/set".into());
    f.set_language(ES6, ES5);
    f.test(
        "var set = new Set();",
        "$jscomp.polyfill('Set', function() {}, 'es6', 'es3');\nvar set = new Set();\n",
    );
    f.set_language(ES6, ES3);
    f.test("var set = new Set();", "$jscomp.polyfill('Set', function() {}, 'es6', 'es3');\n$jscomp.polyfill('Map', function() {}, 'es5', 'es3');\nvar set = new Set();\n");
}

// port: RewritePolyfillsTest#testRegexFeatureSetException
#[test]
fn test_regex_feature_set_exception() {
    let mut f = Fixture::new();
    f.add_library(
        "Promise.prototype.finally",
        "es_2018",
        "es3",
        Some("es6/promise/finally"),
    );
    f.set_accepted_language(ES_2020);
    f.h.set_browser_featureset_year(Some(2020)).unwrap();
    f.h.disable_validate_ast_change_marking().unwrap();
    f.test_same("Promise.resolve(1).finally(console.log('done'));");
}

// port: RewritePolyfillsTest#testCleansUpUnnecessaryPreviouslyInjectedPolyfills
#[test]
fn test_cleans_up_unnecessary_previously_injected_polyfills() {
    let mut f = Fixture::new();
    f.t.injectable_libraries.insert("es6/set".into(), "$jscomp.polyfill('Set', function() {}, 'es6', 'es3');\n// pretend Map isn't needed for ES5\n$jscomp.polyfill('Map', function() {}, 'es5', 'es3');\n".into());
    f.t.polyfill_table.push("Set es6 es3 es6/set".into());
    f.t.inject_before_pass.insert("es6/set".into());
    f.set_language(ES6, ES5);
    f.test("var set = new Set();", " // Map gets removed even though not added by RewritePolyfills\n$jscomp.polyfill('Set', function() {}, 'es6', 'es3');\nvar set = new Set();\n");
}

// port: RewritePolyfillsTest#testAddsPolyfillMethodToExterns
#[test]
fn test_adds_polyfill_method_to_externs() {
    let mut f = Fixture::new();
    f.t.isolate_polyfills = true;
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_extern_changes("", "var $jscomp$lookupPolyfilledValue");
}

// port: RewritePolyfillsTest#testNoCodeChangesIfInjectionDisabled
#[test]
fn test_no_code_changes_if_injection_disabled() {
    let mut f = Fixture::new();
    f.t.isolate_polyfills = true;
    f.t.inject_polyfills = false;
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_extern_changes("", "var $jscomp$lookupPolyfilledValue");
    f.h.allow_externs_changes().unwrap();
    f.test_same("'x'.endsWith('y');");
}

// port: RewritePolyfillsTest#testForceInject_es5_addsES6AndES8
#[test]
fn test_force_inject_es5_adds_es6_and_es8() {
    let mut f = Fixture::new();
    f.t.inject_polyfills_newer_than = Some(LanguageMode::ECMASCRIPT5);
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.add_library("Object.values", "es8", "es3", Some("es6/object/values"));
    f.test_injects("", &["es6/string/endswith", "es6/object/values"]);
}

// port: RewritePolyfillsTest#testForceInject_es2015_addsES8Polyfill
#[test]
fn test_force_inject_es2015_adds_es8_polyfill() {
    let mut f = Fixture::new();
    f.t.inject_polyfills_newer_than = Some(LanguageMode::ECMASCRIPT5);
    f.add_library("Object.values", "es8", "es3", Some("es6/object/values"));
    f.test_injects("", &["es6/object/values"]);
}

// port: RewritePolyfillsTest#testForceInject_es2015_skipsEs2015Polyfills
#[test]
fn test_force_inject_es2015_skips_es2015_polyfills() {
    let mut f = Fixture::new();
    f.t.inject_polyfills_newer_than = Some(LanguageMode::ECMASCRIPT_2015);
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.test_does_not_inject("");
}

// port: RewritePolyfillsTest#testIteratorPolyfill
#[test]
fn test_iterator_polyfill() {
    let mut f = Fixture::new();
    f.add_library("Iterator", "es_next", "es3", Some("es6/iterator"));
    f.set_language(ES_2020, ES3);
    f.test_injects("Iterator;", &["es6/iterator"]);
    f.test_injects("class MyIter extends Iterator {}", &["es6/iterator"]);
    f.test_does_not_inject("");
}
