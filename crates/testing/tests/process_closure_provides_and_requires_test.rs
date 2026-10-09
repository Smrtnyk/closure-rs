/*
 * Copyright 2019 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   test/com/google/javascript/jscomp/ProcessClosureProvidesAndRequiresTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Rust unit tests of ProcessClosureProvidesAndRequiresTest.java that call
//! ProcessClosureProvidesAndRequires#collectProvidedNames directly (getProvidedNameCollection)
//! and so make no hooked CompilerTestCase call the corpus could record, plus the post-call
//! source-position assertions of testSourcePositionPreservation
//! (corpus/unit/rust_unit_tests/ProcessClosureProvidesAndRequiresTest.md).
use closure_jscomp::process_closure_provides_and_requires::ProcessClosureProvidesAndRequires;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    ir::IR, js_string::JsString, node::NodeId, testing::node_subject::assert_node,
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
    unit_test_utils::externs_builder_call,
};
use std::{cell::RefCell, rc::Rc};

/// The fields of ProcessClosureProvidesAndRequiresTest that getProvidedNameCollection reads.
struct ProcessClosureProvidesAndRequiresTest {
    harness: CompilerTestCase,
    preserve_goog_provides_and_requires: bool,
}

impl ProcessClosureProvidesAndRequiresTest {
    // port: ProcessClosureProvidesAndRequiresTest#ProcessClosureProvidesAndRequiresTest
    fn new() -> Self {
        // MINIMAL_EXTERNS + new TestExternsBuilder().addClosureExterns().build()
        let builder = closure_testing::harness_passes::test_externs_builder().unwrap();
        let builder = externs_builder_call(&builder, "addClosureExterns", vec![]).unwrap();
        let DslValue::String(closure_externs) =
            externs_builder_call(&builder, "build", vec![]).unwrap()
        else {
            panic!("TestExternsBuilder.build must return a String");
        };
        let externs = CompilerTestCase::minimal_externs()
            .unwrap()
            .to_string_lossy()
            + &closure_externs.to_string_lossy();
        let mut test = Self {
            harness: CompilerTestCase::new(externs),
            preserve_goog_provides_and_requires: false,
        };
        test.set_up();
        test
    }

    // port: ProcessClosureProvidesAndRequiresTest#setUp
    fn set_up(&mut self) {
        self.harness.set_up();
        self.harness
            .set_accepted_language(DslValue::Enum {
                class: "com.google.javascript.jscomp.CompilerOptions$LanguageMode".into(),
                name: "ECMASCRIPT_2017".into(),
            })
            .unwrap();
        self.preserve_goog_provides_and_requires = false;
        self.harness.enable_type_check().unwrap();
        self.harness.enable_create_module_map().unwrap(); // necessary for the typechecker
        self.harness.replace_types_with_colors().unwrap();
    }

    // port: ProcessClosureProvidesAndRequiresTest#createClosureProcessor
    fn create_closure_processor(
        &self,
        compiler: &CompilerHandle,
    ) -> ProcessClosureProvidesAndRequires {
        ProcessClosureProvidesAndRequires::new(
            &mut compiler.borrow_mut(),
            self.preserve_goog_provides_and_requires,
        )
    }

    // port: ProcessClosureProvidesAndRequiresTest#getProvidedNameCollection
    fn get_provided_name_collection(&self, js: &str) -> Vec<JsString> {
        let compiler = self.harness.create_compiler().unwrap();
        let mut processor = self.create_closure_processor(&compiler);
        let mut c = compiler.borrow_mut();
        let js_root = c.parse_test_code(js);
        let externs_root = IR::root(&mut c, &[]);
        let js_root_root = IR::root(&mut c, &[js_root]);
        let scope_root = IR::root(&mut c, &[externs_root, js_root_root]);
        let first = scope_root.get_first_child(&c).unwrap();
        let second = scope_root.get_second_child(&c).unwrap();
        let provided_name_map = processor
            .collect_provided_names(&mut c, first, second)
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert!(c.get_warnings().is_empty());
        assert!(c.get_errors().is_empty());
        provided_name_map
    }
}

// Truth's assertThat(keySet()).containsExactly(...) (any order, no duplicates in a key set).
fn assert_contains_exactly(actual: &[JsString], expected: &[&str]) {
    let mut actual = actual
        .iter()
        .map(JsString::to_string_lossy)
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|s| (*s).to_string())
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

// port: ProcessClosureProvidesAndRequiresTest#testSimpleProvidedNameCollection
#[test]
fn test_simple_provided_name_collection() {
    let test = ProcessClosureProvidesAndRequiresTest::new();
    let provided_name_map = test.get_provided_name_collection(
        "goog.provide('a.b');\ngoog.provide('a.b.c');\ngoog.provide('a.b.d');\n",
    );

    assert_contains_exactly(&provided_name_map, &["goog", "a", "a.b", "a.b.c", "a.b.d"]);
}

// port: ProcessClosureProvidesAndRequiresTest#testLegacyGoogModule
#[test]
fn test_legacy_goog_module() {
    let test = ProcessClosureProvidesAndRequiresTest::new();
    let provided_name_map = test.get_provided_name_collection(
        "goog.module('a.b.c');\ngoog.module.declareLegacyNamespace();\n\nexports = class {};\n",
    );

    assert_contains_exactly(&provided_name_map, &["goog", "a", "a.b", "a.b.c"]);
}

// port: ProcessClosureProvidesAndRequiresTest#testLegacyGoogModule_inLoadModuleCall
#[test]
fn test_legacy_goog_module_in_load_module_call() {
    let test = ProcessClosureProvidesAndRequiresTest::new();
    let provided_name_map = test.get_provided_name_collection(
        "goog.loadModule(function(exports) {\n  goog.module('a.b.c');\n  goog.module.declareLegacyNamespace();\n  exports = class {};\n  return exports;\n});\n",
    );

    assert_contains_exactly(&provided_name_map, &["goog", "a", "a.b", "a.b.c"]);
}

// port: ProcessClosureProvidesAndRequiresTest#testEsModule_ignored
#[test]
fn test_es_module_ignored() {
    let test = ProcessClosureProvidesAndRequiresTest::new();
    let provided_name_map =
        test.get_provided_name_collection("goog.declareModuleId('a.b.c'); export const x = 0;");

    assert_contains_exactly(&provided_name_map, &["goog"]);
}

/// The getProcessor override of ProcessClosureProvidesAndRequiresTest (with the fields it reads).
struct GetProcessorHooks {
    ctx: Ctx,
    preserve_goog_provides_and_requires: bool,
}

impl CompilerTestCaseHooks for GetProcessorHooks {
    // port: ProcessClosureProvidesAndRequiresTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let preserve_goog_provides_and_requires = self.preserve_goog_provides_and_requires;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                verify_collect_provided_names_doesnt_change_ast(
                    externs,
                    root,
                    compiler,
                    preserve_goog_provides_and_requires,
                );
                let mut last_processor = ProcessClosureProvidesAndRequires::new(
                    compiler,
                    preserve_goog_provides_and_requires,
                );
                last_processor.rewrite_provides_and_requires(compiler, externs, root);
            },
        )))))
    }

    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// Validates that running {@link ProcessClosureProvidesAndRequires#collectProvidedNames(Node,
/// Node)} does not modify the AST.
// port: ProcessClosureProvidesAndRequiresTest#verifyCollectProvidedNamesDoesntChangeAst
fn verify_collect_provided_names_doesnt_change_ast(
    externs: NodeId,
    root: NodeId,
    compiler: &mut Compiler,
    preserve_goog_provides_and_requires: bool,
) {
    // Validate that this does not modify the AST at all!
    let original_externs = externs.clone_tree(compiler);
    let original_root = root.clone_tree(compiler);
    let mut processor =
        ProcessClosureProvidesAndRequires::new(compiler, preserve_goog_provides_and_requires);
    processor.collect_provided_names(compiler, externs, root);

    assert_node(externs).is_equal_including_js_doc_to(compiler, original_externs);
    assert_node(root).is_equal_including_js_doc_to(compiler, original_root);
}

// port: ProcessClosureProvidesAndRequiresTest#testSourcePositionPreservation
#[test]
fn test_source_position_preservation() {
    let ProcessClosureProvidesAndRequiresTest {
        mut harness,
        preserve_goog_provides_and_requires,
    } = ProcessClosureProvidesAndRequiresTest::new();
    let mut hooks = GetProcessorHooks {
        ctx: Ctx::new(
            "ProcessClosureProvidesAndRequiresTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
        preserve_goog_provides_and_requires,
    };
    harness
        .test(
            &mut hooks,
            vec![
                TestPart::Sources(CompilerTestCase::srcs("goog.provide('foo.bar.baz');")),
                TestPart::Expected(CompilerTestCase::expected(
                    "/** @const */ var foo = {};\n/** @const */ foo.bar = {};\n/** @const */ foo.bar.baz = {};\n",
                )),
            ],
        )
        .unwrap();

    let compiler = harness.get_last_compiler().unwrap();
    let compiler = compiler.borrow();
    let root = compiler.get_root().unwrap();

    let foo_decl =
        CompilerTestCase::find_qualified_name_node(&compiler, &"foo".into(), root).unwrap();
    let foo_bar_decl =
        CompilerTestCase::find_qualified_name_node(&compiler, &"foo.bar".into(), root).unwrap();
    let foo_bar_baz_decl =
        CompilerTestCase::find_qualified_name_node(&compiler, &"foo.bar.baz".into(), root).unwrap();

    assert_eq!(foo_decl.get_lineno(&compiler), 1);
    assert_eq!(foo_decl.get_charno(&compiler), 14);

    assert_eq!(foo_bar_decl.get_lineno(&compiler), 1);
    assert_eq!(foo_bar_decl.get_charno(&compiler), 18);

    assert_eq!(foo_bar_baz_decl.get_lineno(&compiler), 1);
    assert_eq!(foo_bar_baz_decl.get_charno(&compiler), 22);
}
