/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/IsolatePolyfillsTest.java.

//! Port of IsolatePolyfillsTest.java: unit tests for `IsolatePolyfills`.
use closure_jscomp::{
    compiler_options::PropertyCollapseLevel, compiler_pass::CompilerPass,
    isolate_polyfills::IsolatePolyfills, polyfill_usage_finder::Polyfills,
};
use closure_rhino::{ir::IR, js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::{Compiler, CompilerOptions},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use indexmap::{IndexMap, IndexSet};
use std::{cell::RefCell, rc::Rc};

const LANGUAGE_MODE: &str = "com.google.javascript.jscomp.CompilerOptions$LanguageMode";

// port: IsolatePolyfillsTest#ES_2020
const ES_2020: &str = "ECMASCRIPT_2020";
// port: IsolatePolyfillsTest#ES6
const ES6: &str = "ECMASCRIPT_2015";
// port: IsolatePolyfillsTest#ES5
const ES5: &str = "ECMASCRIPT5_STRICT";
// port: IsolatePolyfillsTest#ES3
const ES3: &str = "ECMASCRIPT3";

/// `LanguageMode.<name>` as the harness's option value.
fn language_mode(name: &str) -> DslValue {
    DslValue::Enum {
        class: LANGUAGE_MODE.into(),
        name: name.into(),
    }
}

/// Java's string concatenation `a + b + ...`.
fn cat(parts: &[&str]) -> String {
    parts.concat()
}

struct IsolatePolyfillsTest {
    ctx: Ctx,
    polyfill_table: Vec<String>,
    polyfills_to_inject: IndexSet<String>,
    enable_property_flattening: bool,
}

impl IsolatePolyfillsTest {
    fn new() -> Self {
        Self {
            ctx: Ctx::new(
                "IsolatePolyfillsTest".into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::new(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            polyfill_table: Vec::new(),
            polyfills_to_inject: IndexSet::new(),
            enable_property_flattening: false,
        }
    }

    // port: IsolatePolyfillsTest#addLibrary
    fn add_library(&mut self, name: &str, from: &str, to: &str, library: Option<&str>) {
        self.polyfill_table
            .push(format!("{name} {from} {to} {}", library.unwrap_or("")));
        self.polyfills_to_inject.insert(name.to_string());
    }

    /// Builds a string with all definitions of the polyfills
    // port: IsolatePolyfillsTest#buildPolyfillJs
    fn build_polyfill_js(&self) -> String {
        if self.polyfills_to_inject.is_empty() {
            return String::new();
        }
        let jscomp_polyfill_name = if self.enable_property_flattening {
            "$jscomp$polyfill"
        } else {
            "$jscomp.polyfill"
        };

        let mut synthetic_code = String::from("var $jscomp = {};\n");

        for polyfill in &self.polyfills_to_inject {
            // $jscomp.polyfill('syntheticName');
            synthetic_code.push_str(jscomp_polyfill_name);
            synthetic_code.push_str("('");
            synthetic_code.push_str(polyfill);
            synthetic_code.push_str("');\n");
        }
        synthetic_code
    }
}

impl CompilerTestCaseHooks for IsolatePolyfillsTest {
    // port: IsolatePolyfillsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let table = self.polyfill_table.join("\n");
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                // Synthetic definition of $jscomp$lookupPolyfilledValue
                let input = compiler.get_synthesized_externs_input().clone();
                let ast_root = input.get_ast_root(compiler);
                let name = IR::name(compiler, "$jscomp$lookupPolyfilledValue");
                let var = IR::var(compiler, name);
                ast_root.add_child_to_back(compiler, var);
                IsolatePolyfills::new_with_polyfills(compiler, Polyfills::from_table(&table))
                    .process(compiler, externs, root);
            },
        )))))
    }

    // port: IsolatePolyfillsTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        options.set_collapse_properties_level(if self.enable_property_flattening {
            PropertyCollapseLevel::ALL
        } else {
            PropertyCollapseLevel::NONE
        });
        Ok(options)
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: IsolatePolyfillsTest,
}

impl Fixture {
    // port: IsolatePolyfillsTest#setUp
    fn new() -> Self {
        let mut h = CompilerTestCase::new("");
        h.set_up();
        let t = IsolatePolyfillsTest::new();
        h.allow_externs_changes().unwrap();
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

    /// `test(srcs(String...), expected(String...))`.
    fn test_strings(&mut self, js: &[&str], expected: &[&str]) {
        let js: Vec<JsString> = js.iter().map(|s| JsString::from(*s)).collect();
        let expected: Vec<JsString> = expected.iter().map(|s| JsString::from(*s)).collect();
        self.h
            .test(
                &mut self.t,
                vec![
                    TestPart::Sources(CompilerTestCase::srcs_strings(&js)),
                    TestPart::Expected(CompilerTestCase::expected_strings(&expected)),
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

    fn test_extern_changes(&mut self, externs: &str, input: &str, expected_extern: &str) {
        self.h
            .test_extern_changes(
                &mut self.t,
                &CompilerTestCase::externs(externs),
                &CompilerTestCase::srcs(input),
                &CompilerTestCase::expected(expected_extern),
                &[],
                None,
            )
            .unwrap();
    }

    // port: IsolatePolyfillsTest#testWithPolyfills
    fn test_with_polyfills(&mut self, input: &str, expected: &str) {
        let polyfills = self.t.build_polyfill_js();
        self.test(&cat(&[&polyfills, input]), &cat(&[&polyfills, expected]));
    }

    // port: IsolatePolyfillsTest#testSameWithPolyfills
    fn test_same_with_polyfills(&mut self, input: &str) {
        let polyfills = self.t.build_polyfill_js();
        self.test_same(&cat(&[&polyfills, input]));
    }
}

// port: IsolatePolyfillsTest#testEmpty
#[test]
fn test_empty() {
    let mut f = Fixture::new();
    f.set_language(ES6, ES5);
    f.test_same_with_polyfills("");
}

// port: IsolatePolyfillsTest#testOptChainCall_doesNotCrash
#[test]
fn test_opt_chain_call_does_not_crash() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.add_library(
        "Array.prototype.includes",
        "es6",
        "es5",
        Some("es6/array/includes"),
    );
    f.test(&cat(&[&f.t.build_polyfill_js(), "if (a.b()?.includes()) {}"]), &cat(&["var $jscomp$polyfillTmp;\n", &f.t.build_polyfill_js(), "if (($jscomp$polyfillTmp = a.b(),\n    $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'includes', true))?.\n        call($jscomp$polyfillTmp)) {}\n"]));
}

// port: IsolatePolyfillsTest#testOptChainGetProp_doesNotCrash
#[test]
fn test_opt_chain_get_prop_does_not_crash() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.add_library(
        "Array.prototype.includes",
        "es6",
        "es5",
        Some("es6/array/includes"),
    );
    f.test_with_polyfills(
        "if (a.b?.includes()) {}",
        "if ($jscomp$lookupPolyfilledValue(a.b, 'includes', true)?.call(a.b)) {}",
    );
}

// port: IsolatePolyfillsTest#testClassesAreIsolated
#[test]
fn test_classes_are_isolated() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "var m = new Map();",
        "var m = new $jscomp.polyfills['Map']();",
    );
    f.test_with_polyfills(
        "var m = new window.Map();",
        "var m = new $jscomp.polyfills['Map']();",
    );
    f.test_with_polyfills(
        "var m = new goog.global.Map();",
        "var m = new $jscomp.polyfills['Map']();",
    );
}

// port: IsolatePolyfillsTest#testClassesAreNotIsolatedUnlessPolyfillInjected
#[test]
fn test_classes_are_not_isolated_unless_polyfill_injected() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.t.polyfills_to_inject.shift_remove("Map");
    f.set_language(ES6, ES5);
    f.test_same("var m = new Map();");
    f.test_same("var m = new window.Map();");
    f.test_same("var m = new goog.global.Map();");
}

// port: IsolatePolyfillsTest#testClassesGuardedByIfAreIsolated
#[test]
fn test_classes_guarded_by_if_are_isolated() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "if (Map) { var m = new Map(); }",
        "if ($jscomp.polyfills['Map']) { var m = new $jscomp.polyfills['Map'](); }",
    );
}

// port: IsolatePolyfillsTest#testClassesAccessedViaBrackets_notIsolated
#[test]
fn test_classes_accessed_via_brackets_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_same("var m = new window['Map']();");
    f.test_same("var m = new goog.global['Map']();");
}

// port: IsolatePolyfillsTest#testGlobalThisIsolated
#[test]
fn test_global_this_isolated() {
    let mut f = Fixture::new();
    f.add_library("globalThis", "es_2020", "es3", Some("es6/globalthis"));
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES_2020, ES5);
    f.test_with_polyfills(
        "alert(globalThis);",
        "alert($jscomp.polyfills['globalThis']);",
    );
    f.set_language(ES_2020, ES_2020);
    f.test_same_with_polyfills("var m = new globalThis.Map();");
}

// port: IsolatePolyfillsTest#testClassOnGlobalThisIsolated
#[test]
fn test_class_on_global_this_isolated() {
    let mut f = Fixture::new();
    f.add_library("globalThis", "es_2020", "es3", Some("es6/globalthis"));
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES_2020, ES5);
    f.test_with_polyfills(
        "var m = new globalThis.Map();",
        "var m = new $jscomp.polyfills['Map']();",
    );
    f.set_language(ES_2020, ES6);
    f.test_with_polyfills(
        "var m = new globalThis.Map();",
        "var m = new $jscomp.polyfills['globalThis'].Map();",
    );
}

// port: IsolatePolyfillsTest#testEnablingPropertyFlatteningUsesFlattenedJscompPolyfills
#[test]
fn test_enabling_property_flattening_uses_flattened_jscomp_polyfills() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.t.enable_property_flattening = true;
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "var m = new Map();",
        "var m = new $jscomp$polyfills['Map']();",
    );
    f.test_with_polyfills(
        "var m = new window.Map();",
        "var m = new $jscomp$polyfills['Map']();",
    );
    f.test_with_polyfills(
        "var m = new goog.global.Map();",
        "var m = new $jscomp$polyfills['Map']();",
    );
}

// port: IsolatePolyfillsTest#testMultipleUsagesOfClassAllIsolated
#[test]
fn test_multiple_usages_of_class_all_isolated() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "var m = new Map(); m = new Map();",
        "var m = new $jscomp.polyfills['Map'](); m = new $jscomp.polyfills['Map']();",
    );
}

// port: IsolatePolyfillsTest#testClassesGivenSufficientLanguageOutNotIsolated
#[test]
fn test_classes_given_sufficient_language_out_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Proxy", "es6", "es6", None);
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.add_library("Set", "es6", "es3", Some("es6/set"));
    f.set_language(ES6, ES6);
    f.test_same_with_polyfills("new Proxy();");
    f.test_same_with_polyfills("var m = new Map();");
    f.test_same_with_polyfills("new Set();");
}

// port: IsolatePolyfillsTest#testClassesDeclaredInScopeNotIsolated
#[test]
fn test_classes_declared_in_scope_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Map", "es6", "es5", Some("es6/map"));
    f.set_language(ES6, ES5);
    f.test_same_with_polyfills("/** @constructor */ var Map = function() {}; new Map();");
}

// port: IsolatePolyfillsTest#testStaticMethodsIsolated
#[test]
fn test_static_methods_isolated() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es3", Some("es6/array/of"));
    f.add_library("Object.keys", "es5", "es3", Some("es5/object/keys"));
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "Array.of(x);",
        "$jscomp$lookupPolyfilledValue(Array, 'of').call(Array, x);",
    );
    f.test_with_polyfills(
        "Math.clz32(x);",
        "$jscomp$lookupPolyfilledValue(Math, 'clz32').call(Math, x);",
    );
    f.set_language(ES6, ES3);
    f.test_with_polyfills(
        "Object.keys(x);",
        "$jscomp$lookupPolyfilledValue(Object, 'keys').call(Object, x);",
    );
}

// port: IsolatePolyfillsTest#testStaticMethodsWithSufficientLanguageOutNotIsolated
#[test]
fn test_static_methods_with_sufficient_language_out_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Array.from", "es6", "es6", None);
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.add_library("Array.of", "es6", "es3", Some("es6/array/of"));
    f.add_library("Object.keys", "es5", "es3", Some("es5/object/keys"));
    f.set_language(ES6, ES6);
    f.test_same_with_polyfills("Array.from(x);");
    f.test_same_with_polyfills("Math.clz32(x);");
    f.test_same_with_polyfills("Array.of(x);");
    f.set_language(ES5, ES5);
    f.test_same_with_polyfills("Object.keys(x);");
}

// port: IsolatePolyfillsTest#testStaticMethodsDeclaredInScopeNotIsolated
#[test]
fn test_static_methods_declared_in_scope_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Math.clz32", "es6", "es5", Some("es6/math/clz32"));
    f.set_language(ES6, ES5);
    f.test_same_with_polyfills("var Math = {clz32: function() {}}; Math.clz32(x);");
}

// port: IsolatePolyfillsTest#testStaticMethodsGuardedByIfStillIsolated
#[test]
fn test_static_methods_guarded_by_if_still_isolated() {
    let mut f = Fixture::new();
    f.add_library("Array.of", "es6", "es5", Some("es6/array/of"));
    f.test_with_polyfills("if (Array.of) { Array.of(); } else { Array.of(); }", "if ($jscomp$lookupPolyfilledValue(Array, 'of')) {\n$jscomp$lookupPolyfilledValue(Array, 'of').call(Array);\n} else {\n  $jscomp$lookupPolyfilledValue(Array, 'of').call(Array);\n}\n");
}

// port: IsolatePolyfillsTest#testStaticMethodIsolatedOnPolyfilledNameIsolated
#[test]
fn test_static_method_isolated_on_polyfilled_name_isolated() {
    let mut f = Fixture::new();
    f.add_library("Promise", "es6", "es3", Some("es6/promise/promise"));
    f.add_library(
        "Promise.allSettled",
        "es_2020",
        "es3",
        Some("es6/promise/allSettled"),
    );
    f.set_language(ES_2020, ES3);
    f.test_with_polyfills("Promise.allSettled([p1, p2]);", "$jscomp$lookupPolyfilledValue($jscomp.polyfills['Promise'], 'allSettled')\n   .call($jscomp.polyfills['Promise'], [p1, p2]);\n");
    f.set_language(ES_2020, ES6);
    f.test_with_polyfills(
        "Promise.allSettled([p1, p2]);",
        "$jscomp$lookupPolyfilledValue(Promise, 'allSettled').call(Promise, [p1, p2]);",
    );
}

// port: IsolatePolyfillsTest#testSinglePrototypeMethodIsIsolated
#[test]
fn test_single_prototype_method_is_isolated() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "x.endsWith(y);",
        "$jscomp$lookupPolyfilledValue(x, 'endsWith').call(x, y);",
    );
    f.test(&cat(&[&f.t.build_polyfill_js(), "x().endsWith(y);"]), &cat(&["var $jscomp$polyfillTmp;\n", &f.t.build_polyfill_js(), "($jscomp$polyfillTmp = x(),\n  $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'endsWith'))\n      .call($jscomp$polyfillTmp, y);\n"]));
}

// port: IsolatePolyfillsTest#testMultiplePrototypeMethodsAllIsolated
#[test]
fn test_multiple_prototype_methods_all_isolated() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.startsWith",
        "es6",
        "es5",
        Some("es6/string/startswith"),
    );
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.set_language(ES6, ES5);
    f.test_with_polyfills("x.endsWith(y) || x.startsWith(y);", "$jscomp$lookupPolyfilledValue(x, 'endsWith').call(x, y)\n  || $jscomp$lookupPolyfilledValue(x, 'startsWith').call(x, y);\n");
    f.test(&cat(&[&f.t.build_polyfill_js(), "x().endsWith(y) || x().startsWith(y);"]), &cat(&["var $jscomp$polyfillTmp;", &f.t.build_polyfill_js(), "($jscomp$polyfillTmp = x(),\n    $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'endsWith'))\n  .call($jscomp$polyfillTmp, y)\n || ($jscomp$polyfillTmp = x(),\n      $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'startsWith'))\n    .call($jscomp$polyfillTmp, y);\n"]));
}

// port: IsolatePolyfillsTest#testMethodMatchingMultiplePolyfillsIsolatedOnlyOnce
#[test]
fn test_method_matching_multiple_polyfills_isolated_only_once() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.add_library(
        "Array.prototype.includes",
        "es6",
        "es5",
        Some("es6/array/includes"),
    );
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "x.includes(y);",
        "$jscomp$lookupPolyfilledValue(x, 'includes').call(x, y);",
    );
}

// port: IsolatePolyfillsTest#testMethodsNotIsolatedUnlessPolyfillInjected
#[test]
fn test_methods_not_isolated_unless_polyfill_injected() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.add_library(
        "String.prototype.endsWith",
        "es6",
        "es5",
        Some("es6/string/endswith"),
    );
    f.t.polyfills_to_inject
        .shift_remove("String.prototype.endsWith");
    f.set_language(ES6, ES5);
    f.test_with_polyfills("x.endsWith(y);", "x.endsWith(y);");
    f.test_with_polyfills(
        "x.includes(y) && x.endsWith(z);",
        "$jscomp$lookupPolyfilledValue(x, 'includes').call(x, y) && x.endsWith(z);",
    );
}

// port: IsolatePolyfillsTest#testLvaluePolyfillUsageNotIsolated
#[test]
fn test_lvalue_polyfill_usage_not_isolated() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.set_language(ES6, ES5);
    f.test_same_with_polyfills("x.includes = true;");
}

// port: IsolatePolyfillsTest#testLvaluePolyfillUsage_doesntCauseBackoffForOtherUsages
#[test]
fn test_lvalue_polyfill_usage_doesnt_cause_backoff_for_other_usages() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.set_language(ES6, ES5);
    f.test_with_polyfills(
        "x.includes = true; y.includes(z)",
        "x.includes = true; $jscomp$lookupPolyfilledValue(y, 'includes').call(y, z)",
    );
}

// port: IsolatePolyfillsTest#testLvaluePolyfillUsageInNestedAssignNotIsolated
#[test]
fn test_lvalue_polyfill_usage_in_nested_assign_not_isolated() {
    let mut f = Fixture::new();
    f.add_library(
        "String.prototype.includes",
        "es6",
        "es5",
        Some("es6/string/includes"),
    );
    f.set_language(ES6, ES5);
    f.test_same_with_polyfills("y = x.includes = true");
    f.test_same_with_polyfills("y = x.includes = z.includes = true");
}

// port: IsolatePolyfillsTest#testPolyfilledMethodOnPolyfilledClassAreBothIsolated
#[test]
fn test_polyfilled_method_on_polyfilled_class_are_both_isolated() {
    let mut f = Fixture::new();
    f.add_library("Promise", "es6", "es3", Some("es6/promise/promise"));
    f.add_library(
        "Promise.prototype.finally",
        "es9",
        "es3",
        Some("es6/promise/finally"),
    );
    f.set_language(ES_2020, ES3);
    f.test_with_polyfills(
        "p.finally(cb);",
        "$jscomp$lookupPolyfilledValue(p, 'finally').call(p, cb);",
    );
    f.set_language(ES_2020, ES6);
    f.test_with_polyfills(
        "p.finally(cb);",
        "$jscomp$lookupPolyfilledValue(p, 'finally').call(p, cb);",
    );
}

// port: IsolatePolyfillsTest#testNonPolyfilledMethodOnPolyfilledClassNotIsolated
#[test]
fn test_non_polyfilled_method_on_polyfilled_class_not_isolated() {
    let mut f = Fixture::new();
    f.add_library("Promise", "es6", "es3", Some("es6/promise/promise"));
    f.set_language(ES_2020, ES3);
    f.test_with_polyfills(
        "Promise.all(p1, p2);",
        "$jscomp.polyfills['Promise'].all(p1, p2);",
    );
}

// port: IsolatePolyfillsTest#testExternForJSCompLookupPolyfillDeleted
#[test]
fn test_extern_for_js_comp_lookup_polyfill_deleted() {
    let mut f = Fixture::new();
    f.h.disable_compare_as_tree().unwrap();
    f.test_extern_changes("", "x.includes(y);", "");
}

// port: IsolatePolyfillsTest#testJscompLookupPolyfillDeletedIfNotUsed
#[test]
fn test_jscomp_lookup_polyfill_deleted_if_not_used() {
    let mut f = Fixture::new();
    f.add_library("Promise", "es6", "es3", Some("es6/promise/promise"));
    f.add_library(
        "Promise.prototype.finally",
        "es9",
        "es3",
        Some("es6/promise/promise"),
    );
    f.set_language(ES_2020, ES3);
    f.test_with_polyfills("var $jscomp$lookupPolyfilledValue = function() {};", "");
    f.test_with_polyfills(
        "var $jscomp$lookupPolyfilledValue = function() {};\nnew Promise();\n",
        "new $jscomp.polyfills['Promise']();",
    );
}

// port: IsolatePolyfillsTest#testJscompLookupPolyfillKeptIfUsed
#[test]
fn test_jscomp_lookup_polyfill_kept_if_used() {
    let mut f = Fixture::new();
    f.add_library("Promise", "es6", "es3", Some("es6/promise/promise"));
    f.add_library(
        "Promise.prototype.finally",
        "es9",
        "es3",
        Some("es6/promise/promise"),
    );
    f.set_language(ES_2020, ES3);
    f.test(&cat(&[&f.t.build_polyfill_js(), "var $jscomp$lookupPolyfilledValue = function() {};\nnew Promise().finally(cb);\n"]), &cat(&["var $jscomp$polyfillTmp;\n", &f.t.build_polyfill_js(), "var $jscomp$lookupPolyfilledValue = function() {};\n($jscomp$polyfillTmp = new $jscomp.polyfills['Promise'],\n    $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'finally'))\n  .call($jscomp$polyfillTmp, cb);\n"]));
}

// port: IsolatePolyfillsTest#testPolyfilTemp_multipleFiles_insertedInFirstFile
#[test]
fn test_polyfil_temp_multiple_files_inserted_in_first_file() {
    let mut f = Fixture::new();
    f.t.polyfill_table
        .push("String.prototype.endsWith es6 es5 es6/string/endswith".into());
    f.set_language(ES6, ES5);
    f.test_strings(&["$jscomp.polyfill('String.prototype.endsWith', 'es6', 'es5');", "x().endsWith(y);"], &["var $jscomp$polyfillTmp;\n$jscomp.polyfill('String.prototype.endsWith', 'es6', 'es5');\n", "($jscomp$polyfillTmp = x(),\n  $jscomp$lookupPolyfilledValue($jscomp$polyfillTmp, 'endsWith'))\n      .call($jscomp$polyfillTmp, y);\n"]);
}
