/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/RewriteGoogJsImportsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of RewriteGoogJsImportsTest (a CompilerTestCase) on the Rust CompilerTestCase port.
//! getProcessor is the corpus helper `RewriteGoogJsImportsTest_Helpers$GetProcessorPass`
//! (GatherModuleMetadata, ModuleMapCreator, then RewriteGoogJsImports in LINT_AND_REWRITE mode).
use closure_jscomp::{
    deps::module_loader::LOAD_WARNING,
    rewrite_goog_js_imports::{GOOG_JS_IMPORT_MUST_BE_GOOG_STAR, GOOG_JS_REEXPORTED},
};
use closure_rhino::fast_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::SourceFile,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        rewrite_goog_js_imports_helpers::rewrite_goog_js_imports_test_get_processor_pass,
    },
    throwable::Throwable,
};
use std::sync::Arc;

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: RewriteGoogJsImportsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        rewrite_goog_js_imports_test_get_processor_pass(
            &mut self.ctx,
            vec![DslValue::Compiler(compiler)],
        )
    }
    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: CompilerTestCase#setUp (RewriteGoogJsImportsTest does not override it)
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    let hooks = Hooks {
        ctx: Ctx::new(
            "RewriteGoogJsImportsTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
    };
    (harness, hooks)
}

// JsFileRegexParser determines if this file is base.js by looking at the first comment of the
// file.
// port: RewriteGoogJsImportsTest#BASE
fn base() -> Arc<SourceFile> {
    Arc::new(SourceFile::from_code(
        "/closure/base.js",
        "/**\n * @fileoverview\n * @provideGoog\n */\n",
    ))
}

// port: RewriteGoogJsImportsTest#GOOG
fn goog() -> Arc<SourceFile> {
    Arc::new(SourceFile::from_code(
        "/closure/goog.js",
        "export const require = goog.require;\nexport function foo() {}\nexport class MyClass {}\nexport const constant = 0;\n",
    ))
}

fn code(name: &str, code: &str) -> Arc<SourceFile> {
    Arc::new(SourceFile::from_code(name, code))
}

fn srcs(files: Vec<Arc<SourceFile>>) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs_files(files))
}

fn expected(files: Vec<Arc<SourceFile>>) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected_files(files))
}

/// `getLastCompiler().getModuleMap().getModule(getModuleLoader().resolve("testcode"))
/// .boundNames().keySet()`.
fn testcode_bound_names(harness: &CompilerTestCase) -> Vec<String> {
    let compiler = harness.get_last_compiler().unwrap();
    let compiler = compiler.borrow();
    let path = compiler.get_module_loader().resolve("testcode");
    compiler
        .get_module_map()
        .unwrap()
        .get_module_by_path(&path)
        .unwrap()
        .bound_names()
        .keys()
        .map(|k| k.to_string())
        .collect()
}

#[test]
fn test_base_and_goog_untouched() {
    let (mut t, mut h) = set_up();
    t.test_same(&mut h, vec![srcs(vec![base(), goog()])])
        .unwrap();
}

#[test]
fn test_if_cannot_detect_goog_js_then_globalizes_all() {
    let (mut t, mut h) = set_up();
    let testcode = code(
        "testcode",
        "import * as goog from './closure/goog.js'; use(goog.bad);",
    );

    let expected_code = code("testcode", "import './closure/goog.js'; use(goog.bad);");

    t.ignore_warnings(&[&LOAD_WARNING]).unwrap();

    // No base.js = no detecting goog.js
    t.test(
        &mut h,
        vec![
            srcs(vec![goog(), testcode.clone()]),
            expected(vec![goog(), expected_code.clone()]),
        ],
    )
    .unwrap();

    // No goog.js
    t.test(
        &mut h,
        vec![
            srcs(vec![base(), testcode.clone()]),
            expected(vec![base(), expected_code.clone()]),
        ],
    )
    .unwrap();
    t.test(
        &mut h,
        vec![srcs(vec![testcode]), expected(vec![expected_code])],
    )
    .unwrap();

    // Linting still happens.
    t.test_error(
        &mut h,
        "import * as notgoog from './goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_import_star() {
    let (mut t, mut h) = set_up();
    t.test(
        &mut h,
        vec![
            srcs(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import * as goog from './closure/goog.js';\nuse(goog.require, goog.foo, goog.MyClass, goog.constant);\n",
                ),
            ]),
            expected(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import './closure/goog.js';\nuse(goog.require, goog.foo, goog.MyClass, goog.constant);\n",
                ),
            ]),
        ],
    )
    .unwrap();
    assert!(testcode_bound_names(&t).is_empty());
}

#[test]
fn test_goog_and_base_in_externs() {
    let (mut t, mut h) = set_up();
    t.ignore_warnings(&[&LOAD_WARNING]).unwrap();

    t.test(
        &mut h,
        vec![
            TestPart::Externs(CompilerTestCase::externs_files(vec![base(), goog()])),
            srcs(vec![code(
                "testcode",
                "import * as goog from './closure/goog.js';\nuse(goog.require, goog.foo, goog.MyClass, goog.constant);\n",
            )]),
            expected(vec![code(
                "testcode",
                "import './closure/goog.js';\nuse(goog.require, goog.foo, goog.MyClass, goog.constant);\n",
            )]),
        ],
    )
    .unwrap();
}

#[test]
fn test_known_bad_property_access() {
    let (mut t, mut h) = set_up();
    t.test(
        &mut h,
        vec![
            srcs(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import * as goog from './closure/goog.js';\nuse(goog.require, goog.bad);\n",
                ),
            ]),
            expected(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import * as $goog from './closure/goog.js';\nuse(goog.require, $goog.bad);\n",
                ),
            ]),
        ],
    )
    .unwrap();
    assert_eq!(testcode_bound_names(&t), vec!["$goog".to_string()]);
}

#[test]
fn test_reexport_goog() {
    let (mut t, mut h) = set_up();
    t.test_error_sources(
        &mut h,
        CompilerTestCase::srcs_files(vec![
            base(),
            goog(),
            code(
                "testcode",
                "import * as goog from './closure/goog.js';\nexport {goog};\n",
            ),
        ]),
        CompilerTestCase::error(&GOOG_JS_REEXPORTED),
    )
    .unwrap();

    t.test_error_sources(
        &mut h,
        CompilerTestCase::srcs_files(vec![
            base(),
            goog(),
            code(
                "testcode",
                "import * as goog from './closure/goog.js';\nexport default goog;\n",
            ),
        ]),
        CompilerTestCase::error(&GOOG_JS_REEXPORTED),
    )
    .unwrap();

    t.test_error_sources(
        &mut h,
        CompilerTestCase::srcs_files(vec![
            base(),
            goog(),
            code("testcode", "export * from './closure/goog.js';"),
        ]),
        CompilerTestCase::error(&GOOG_JS_REEXPORTED),
    )
    .unwrap();

    t.test_error_sources(
        &mut h,
        CompilerTestCase::srcs_files(vec![
            base(),
            goog(),
            code("testcode", "export {require} from './closure/goog.js';"),
        ]),
        CompilerTestCase::error(&GOOG_JS_REEXPORTED),
    )
    .unwrap();
}

/// this is just to make sure the presence of import.meta does not cause a compiler failure
#[test]
fn test_import_meta() {
    let (mut t, mut h) = set_up();
    t.test(
        &mut h,
        vec![
            srcs(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import * as goog from './closure/goog.js';\nuse(import.meta);\n",
                ),
            ]),
            expected(vec![
                base(),
                goog(),
                code(
                    "testcode",
                    "import './closure/goog.js';\nuse(import.meta);\n",
                ),
            ]),
        ],
    )
    .unwrap();
}
