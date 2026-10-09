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
//   test/com/google/javascript/jscomp/MissingProvideTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Port of MissingProvideTest (a CompilerTestCase) on the Rust CompilerTestCase port. The
//! processor is a no-op: the test checks the diagnostics of the Closure passes the harness runs
//! first (enableRewriteClosureCode, enableClosurePass). Bodies were converted mechanically from
//! the Java (text blocks with their common indentation stripped, as javac does).
use closure_jscomp::closure_primitive_errors::{
    MISSING_MODULE_OR_PROVIDE, MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: MissingProvideTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        // No-op. We're just checking for warnings during Closure passes.
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            |_compiler: &mut Compiler, _externs: NodeId, _root: NodeId| {},
        )))))
    }
    // port: ReplayDsl.Ctx#Ctx (native test context)
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: MissingProvideTest#setUp
fn set_up() -> (CompilerTestCase, Hooks) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    harness.enable_rewrite_closure_code().unwrap();
    harness.enable_closure_pass().unwrap();
    let hooks = Hooks {
        ctx: Ctx::new(
            "MissingProvideTest".into(),
            closure_testing::replay::replay_values::object([]),
            IndexMap::<_, _>::default(),
            Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n").unwrap(),
        ),
    };
    (harness, hooks)
}

// Test matrix:
// -----------------------------------------------------
// Leaf    Import type       Root    File status  Result
// -----------------------------------------------------
// legacy  goog.require      module  decl leg     pass
// legacy  goog.require      module  normal       pass
// legacy  goog.require      module  missing      fail
// legacy  goog.module.get   module  normal       pass
// legacy  goog.module.get   module  missing      fail
// module  var goog.require  module  normal       pass
// module  var goog.require  module  missing      fail
// module  goog.module.get   module  normal       pass
// module  goog.module.get   module  missing      fail
// module  var goog.require  legacy  normal       pass
// module  var goog.require  legacy  missing      fail
// module  goog.module.get   legacy  normal       pass
// module  goog.module.get   legacy  missing      fail
// legacy  goog.require      legacy  normal       pass
// legacy  goog.require      legacy  missing      fail
// legacy  goog.module.get   legacy  normal       pass
// legacy  goog.module.get   legacy  missing      fail

#[test]
fn test_legacy_require_module_decl_leg_pass() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.A');\ngoog.module.declareLegacyNamespace();\n/** @constructor */ function A() {}\nexports = A;\n";
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.require('normal.goog.module.A');\nnew normal.goog.module.A;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module.into(), legacy_script.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_require_module_normal_pass() {
    let (mut t, mut h) = set_up();
    let goog_module =
        "goog.module('normal.goog.module.A');\n/** @constructor */ function A() {}\nexports = A;\n";
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.require('normal.goog.module.A');\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module.into(), legacy_script.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_require_module_missing_fail() {
    let (mut t, mut h) = set_up();
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.scope(function() {\n  var A = goog.module.get('missing.goog.module.A');\n});\n";
    let msg = "Required namespace \"missing.goog.module.A\" never defined.";
    t.test_error_message(&mut h, legacy_script, &MISSING_MODULE_OR_PROVIDE, msg)
        .unwrap();
}

#[test]
fn test_legacy_module_get_module_normal_pass() {
    let (mut t, mut h) = set_up();
    let goog_module =
        "goog.module('normal.goog.module.A');\n/** @constructor */ function A() {}\nexports = A;\n";
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.scope(function() {\n  var A = goog.module.get('normal.goog.module.A');\n});\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module.into(), legacy_script.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_module_get_module_missing_fail() {
    let (mut t, mut h) = set_up();
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.scope(function() {\n  var A = goog.module.get('missing.goog.module.A');\n});\n";
    let msg = "Required namespace \"missing.goog.module.A\" never defined.";
    t.test_error_message(&mut h, legacy_script, &MISSING_MODULE_OR_PROVIDE, msg)
        .unwrap();
}

#[test]
fn test_module_require_module_normal_pass() {
    let (mut t, mut h) = set_up();
    let goog_module1 =
        "goog.module('normal.goog.module.A');\n/** @constructor */ function A() {}\nexports = A;\n";
    let goog_module2 = "goog.module('normal.goog.module.B');\nvar A = goog.require('normal.goog.module.A');\n/** @constructor */ function B() {}\nB.prototype = new A;\nexports = B;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module1.into(), goog_module2.into()]),
    )
    .unwrap();
}

#[test]
fn test_module_require_module_missing_fail() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.B');\nvar A = goog.require('missing.goog.module.A');\n/** @constructor */ function B() {}\nB.prototype = new A;\nexports = B;\n";
    let warning = "Required namespace \"missing.goog.module.A\" never defined.";
    t.test_error_message(&mut h, goog_module, &MISSING_MODULE_OR_PROVIDE, warning)
        .unwrap();
}

#[test]
fn test_module_module_get_module_normal_pass() {
    let (mut t, mut h) = set_up();
    let goog_module1 =
        "goog.module('normal.goog.module.A');\n/** @constructor */ function A() {}\nexports = A;\n";
    let goog_module2 = "goog.module('normal.goog.module.B');\nvar A = goog.forwardDeclare('normal.goog.module.A');\n/** @constructor */ function B() {}\nB.prototype.createA = function() {\n  A = goog.module.get('normal.goog.module.A');\n  new A;\n}\nexports = B;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module1.into(), goog_module2.into()]),
    )
    .unwrap();
}

#[test]
fn test_module_require_legacy_normal_pass() {
    let (mut t, mut h) = set_up();
    let legacy_script =
        "goog.provide('legacy.script.A');\n/** @constructor */ legacy.script.A = function () {}\n";
    let goog_module = "goog.module('normal.goog.module.B');\nvar A = goog.require('legacy.script.A');\n/** @constructor */ function B() {}\nB.prototype = new A;\nexports = B;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[legacy_script.into(), goog_module.into()]),
    )
    .unwrap();
}

#[test]
fn test_module_require_legacy_missing_fail() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.B');\nvar A = goog.require('legacy.script.A');\n/** @constructor */ function B() {}\nB.prototype = new A;\nexports = B;\n";
    let msg = "Required namespace \"legacy.script.A\" never defined.";
    t.test_error_message(&mut h, goog_module, &MISSING_MODULE_OR_PROVIDE, msg)
        .unwrap();
}

#[test]
fn test_module_module_get_legacy_normal_pass() {
    let (mut t, mut h) = set_up();
    let legacy_script =
        "goog.provide('legacy.script.A');\n/** @constructor */ legacy.script.A = function () {}\n";
    let goog_module = "goog.module('normal.goog.module.B');\nvar A = goog.forwardDeclare('legacy.script.A');\n/** @constructor */ function B() {}\nB.prototype.createA = function() {\n  A = goog.module.get('legacy.script.A');\n  new A;\n}\nexports = B;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[legacy_script.into(), goog_module.into()]),
    )
    .unwrap();
}

#[test]
fn test_module_module_get_missing_fail() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.B');\nfunction f() {\n  return goog.module.get('missing.legacy.script.A');\n}\nexports = f;\n";
    let msg = "Required namespace \"missing.legacy.script.A\" never defined.";
    t.test_error_message(&mut h, goog_module, &MISSING_MODULE_OR_PROVIDE, msg)
        .unwrap();
}

#[test]
fn test_module_forward_declare_missing_fail() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.B');\nvar A = goog.forwardDeclare('missing.legacy.script.A');\n/** @constructor */ function B() {}\nexports = B;\n";
    let msg = "Required namespace \"missing.legacy.script.A\" never defined.";
    t.test_error_message(
        &mut h,
        goog_module,
        &MISSING_MODULE_OR_PROVIDE_FOR_FORWARD_DECLARE,
        msg,
    )
    .unwrap();
}

#[test]
fn test_legacy_forward_declare_missing_pass() {
    let (mut t, mut h) = set_up();
    let goog_module = "goog.module('normal.goog.module.B');\ngoog.forwardDeclare('missing.legacy.script.A');\n/** @constructor */ function B() {}\nexports = B;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[goog_module.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_require_legacy_normal_pass() {
    let (mut t, mut h) = set_up();
    let legacy_script =
        "goog.provide('legacy.script.A');\n/** @constructor */ legacy.script.A = function () {}\n";
    let legacy_script2 = "goog.provide('legacy.script.B');\ngoog.require('legacy.script.A');\nnew legacy.script.A;\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[legacy_script.into(), legacy_script2.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_require_legacy_missing_fail() {
    let (mut t, mut h) = set_up();
    let legacy_script = "goog.provide('legacy.script.B');\ngoog.require('legacy.script.A');\nnew legacy.script.A;\n";
    let msg = "Required namespace \"legacy.script.A\" never defined.";
    t.test(
        &mut h,
        vec![
            TestPart::Sources(CompilerTestCase::srcs_strings(&[legacy_script.into()])),
            TestPart::Diagnostic(
                CompilerTestCase::error(&MISSING_MODULE_OR_PROVIDE)
                    .with_message(msg)
                    .unwrap(),
            ),
        ],
    )
    .unwrap();
}

#[test]
fn test_legacy_module_get_legacy_normal_pass() {
    let (mut t, mut h) = set_up();
    let legacy_script =
        "goog.provide('legacy.script.A');\n/** @constructor */ legacy.script.A = function () {}\n";
    let legacy_script2 = "goog.provide('legacy.script.B');\n/** @constructor */ legacy.script.B = function () {}\nB.prototype.createA = function() {\n  var A = goog.module.get('legacy.script.A');\n  new A;\n}\n";
    t.test_no_warning_sources(
        &mut h,
        CompilerTestCase::srcs_strings(&[legacy_script.into(), legacy_script2.into()]),
    )
    .unwrap();
}

#[test]
fn test_legacy_module_get_legacy_missing_fail() {
    let (mut t, mut h) = set_up();
    let legacy_script = "goog.provide('legacy.script.B');\n/** @constructor */ legacy.script.B = function () {}\nB.prototype.createA = function() {\n  var A = goog.module.get('legacy.script.A');\n  new A;\n}\n";
    let msg = "Required namespace \"legacy.script.A\" never defined.";
    t.test_error_message(&mut h, legacy_script, &MISSING_MODULE_OR_PROVIDE, msg)
        .unwrap();
}
