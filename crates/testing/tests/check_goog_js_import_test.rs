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
//   test/com/google/javascript/jscomp/CheckGoogJsImportTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of CheckGoogJsImportTest (a CompilerTestCase) on the Rust CompilerTestCase port.
//! getProcessor is the corpus helper `CheckGoogJsImportTest_Helpers$GetProcessorPass`
//! (GatherModuleMetadata, ModuleMapCreator, then RewriteGoogJsImports in LINT_ONLY mode).
use closure_jscomp::{
    deps::module_loader::LOAD_WARNING,
    rewrite_goog_js_imports::{
        CANNOT_HAVE_MODULE_VAR_NAMED_GOOG, GOOG_JS_IMPORT_MUST_BE_GOOG_STAR, GOOG_JS_REEXPORTED,
    },
};
use closure_rhino::fx_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        rewrite_goog_js_imports_helpers::check_goog_js_import_test_get_processor_pass,
    },
    throwable::Throwable,
};

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: CheckGoogJsImportTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        check_goog_js_import_test_get_processor_pass(
            &mut self.ctx,
            vec![DslValue::Compiler(compiler)],
        )
    }
    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: CheckGoogJsImportTest#setUp
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.ignore_warnings(&[&LOAD_WARNING]).unwrap();
    let hooks = Hooks {
        ctx: Ctx::new(
            "CheckGoogJsImportTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
    };
    (harness, hooks)
}

#[test]
fn test_no_goog_js_import() {
    let (mut t, mut h) = set_up();
    t.test_same_string(&mut h, "var x").unwrap();
    t.test_same_string(&mut h, "import {stuff} from './somethingelse.js';")
        .unwrap();
    t.test_same_string(&mut h, "import * as o from './closure/other.js';")
        .unwrap();
    t.test_same_string(&mut h, "export * from './closure/other.js';")
        .unwrap();
    t.test_same_string(&mut h, "import {stuff} from './othergoog.js';")
        .unwrap();
}

#[test]
fn test_valid_imports() {
    let (mut t, mut h) = set_up();
    t.test_same_string(&mut h, "import * as goog from './closure/goog.js';")
        .unwrap();
    t.test_same_string(&mut h, "import * as goog from './goog.js';")
        .unwrap();
}

#[test]
fn test_import_star_must_be_named_goog() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "import * as closure from './goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_import_spec_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "import {require} from './closure/goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_import_default_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "import d from './closure/goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_mixed_import_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "import d, * as goog from './closure/goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
    t.test_error(
        &mut h,
        "import d, {require} from './closure/goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_path_only_import_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "import './closure/goog.js';",
        &GOOG_JS_IMPORT_MUST_BE_GOOG_STAR,
    )
    .unwrap();
}

#[test]
fn test_export_from_goog_js_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "export {require} from './closure/goog.js';",
        &GOOG_JS_REEXPORTED,
    )
    .unwrap();

    t.test_error(
        &mut h,
        "export * from './closure/goog.js';",
        &GOOG_JS_REEXPORTED,
    )
    .unwrap();

    t.test_error(
        &mut h,
        "import * as goog from './closure/goog.js';\nexport {goog};\n",
        &GOOG_JS_REEXPORTED,
    )
    .unwrap();

    t.test_error(
        &mut h,
        "import * as goog from './closure/goog.js';\nexport {goog as GOOG};\n",
        &GOOG_JS_REEXPORTED,
    )
    .unwrap();

    t.test_error(
        &mut h,
        "import * as goog from './closure/goog.js';\nexport default goog;\n",
        &GOOG_JS_REEXPORTED,
    )
    .unwrap();
}

#[test]
fn test_other_module_goog_var_is_error() {
    let (mut t, mut h) = set_up();
    t.test_error(
        &mut h,
        "export const goog = 0;",
        &CANNOT_HAVE_MODULE_VAR_NAMED_GOOG,
    )
    .unwrap();
    t.test_error(
        &mut h,
        "export {}; const goog = 0;",
        &CANNOT_HAVE_MODULE_VAR_NAMED_GOOG,
    )
    .unwrap();
    t.test_error(
        &mut h,
        "export function goog() {}",
        &CANNOT_HAVE_MODULE_VAR_NAMED_GOOG,
    )
    .unwrap();
}
