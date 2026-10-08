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
//   test/com/google/javascript/jscomp/GuardedCallbackTest.java.

//! Port of GuardedCallbackTest.java: unit tests for GuardedCallback.
use closure_jscomp::{
    guarded_callback::{GuardedCallback, GuardedCallbackSubclass},
    node_traversal::NodeTraversal,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    testing::test_externs_builder::TestExternsBuilder,
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

struct GuardedCallbackTest {
    ctx: Ctx,
}

impl GuardedCallbackTest {
    fn new() -> Self {
        Self {
            ctx: Ctx::new(
                "GuardedCallbackTest".into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::new(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
        }
    }
}

impl CompilerTestCaseHooks for GuardedCallbackTest {
    // port: GuardedCallbackTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            |compiler: &mut Compiler, _externs: NodeId, root: NodeId| {
                NodeTraversal::traverse(compiler, root, &mut GuardSwitchingCallback::new());
            },
        )))))
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// Replaces all guarded name and property references with "GUARDED_NAME" and "GUARDED_PROP",
/// respectively.
struct GuardSwitchingCallback {
    base: GuardedCallback<JsString>,
}

impl GuardSwitchingCallback {
    // port: GuardedCallbackTest.GuardSwitchingCallback#GuardSwitchingCallback
    fn new() -> Self {
        Self {
            base: GuardedCallback::new(),
        }
    }
}

impl GuardedCallbackSubclass for GuardSwitchingCallback {
    type Resource = JsString;

    fn guarded_callback(&mut self) -> &mut GuardedCallback<JsString> {
        &mut self.base
    }

    // port: GuardedCallbackTest.GuardSwitchingCallback#visitGuarded
    fn visit_guarded(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) {
        if n.is_name(traversal) && self.is_guarded(n.get_string(traversal)) {
            n.set_string(traversal, "GUARDED_NAME");
            traversal.report_code_change();
        } else if n.is_get_prop(traversal) || n.is_opt_chain_get_prop(traversal) {
            // prefix guarded resource name with "." to keep properties distinct from names
            if self.is_guarded(JsString::from(".").concat(&n.get_string(traversal))) {
                n.set_string(traversal, "GUARDED_PROP");
                traversal.report_code_change();
            }
        }
    }
}

fn set_up() -> (CompilerTestCase, GuardedCallbackTest) {
    let mut harness = CompilerTestCase::new("");
    harness.set_up();
    (harness, GuardedCallbackTest::new())
}

fn externs_console_promise() -> TestPart {
    TestPart::Externs(CompilerTestCase::externs(
        TestExternsBuilder::new()
            .add_console()
            .add_promise()
            .build(),
    ))
}

fn srcs(code: &str) -> TestPart {
    TestPart::Sources(CompilerTestCase::srcs(code))
}

fn expected(code: &str) -> TestPart {
    TestPart::Expected(CompilerTestCase::expected(code))
}

// port: GuardedCallbackTest#unguardedTest
#[test]
fn unguarded_test() {
    let (mut h, mut t) = set_up();
    h.test_same(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise.allSettled);"),
        ],
    )
    .unwrap();
}

// port: GuardedCallbackTest#guardedTest
#[test]
fn guarded_test() {
    let (mut h, mut t) = set_up();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise && Promise.allSettled && Promise.allSettled);"),
            expected(
                "console.log(GUARDED_NAME && GUARDED_NAME.GUARDED_PROP && GUARDED_NAME.GUARDED_PROP)",
            ),
        ],
    )
    .unwrap();
}

// port: GuardedCallbackTest#ifGuardTest
#[test]
fn if_guard_test() {
    let (mut h, mut t) = set_up();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs(
                "if (Promise && Promise.allSettled) {\n  Promise.allSettled([]).then(() => console.log('done'));\n}\n",
            ),
            expected(
                "if (GUARDED_NAME && GUARDED_NAME.GUARDED_PROP) {\n  GUARDED_NAME.GUARDED_PROP([]).then(() => console.log('done'));\n}\n",
            ),
        ],
    )
    .unwrap();
}

// port: GuardedCallbackTest#ifGuardWithOptChainTest
#[test]
fn if_guard_with_opt_chain_test() {
    let (mut h, mut t) = set_up();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs(
                "if (Promise?.allSettled) {\n  Promise.allSettled([]).then(() => console.log('done'));\n}\n",
            ),
            expected(
                "if (GUARDED_NAME?.GUARDED_PROP) {\n  GUARDED_NAME.GUARDED_PROP([]).then(() => console.log('done'));\n}\n",
            ),
        ],
    )
    .unwrap();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs(
                "if (Promise?.allSettled([])) {\n  Promise.allSettled([]).then(() => console.log('done'));\n}\n",
            ),
            expected(
                "if (GUARDED_NAME?.allSettled([])) {\n  GUARDED_NAME.allSettled([]).then(() => console.log('done'));\n}\n",
            ),
        ],
    )
    .unwrap();
}

// port: GuardedCallbackTest#guardedAndUnguardedTest
#[test]
fn guarded_and_unguarded_test() {
    let (mut h, mut t) = set_up();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise && Promise.allSettled && Promise.finally);"),
            // The values of `Promise` and `Promise.allSettled` are only checked for truthiness,
            // so they can be considered guarded. However, the value of `Promise.finally` may end up
            // getting passed to `console.log()`, so it is not guarded.
            expected(
                "console.log(GUARDED_NAME && GUARDED_NAME.GUARDED_PROP && GUARDED_NAME.finally)",
            ),
        ],
    )
    .unwrap();
}

// port: GuardedCallbackTest#optChainTest
#[test]
fn opt_chain_test() {
    let (mut h, mut t) = set_up();
    h.test_same(
        &mut t,
        vec![
            externs_console_promise(),
            // Nothing guarded
            srcs("console.log(Promise.allSettled([Promise.resolve(), x.allSettled]))"),
        ],
    )
    .unwrap();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise?.allSettled([Promise.resolve(), x.allSettled]))"),
            expected(
                "console.log(GUARDED_NAME?.allSettled([GUARDED_NAME.resolve(), x.allSettled]))",
            ),
        ],
    )
    .unwrap();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise.allSettled?.([Promise.resolve(), x.allSettled]))"),
            expected("console.log(Promise.GUARDED_PROP?.([Promise.resolve(), x.GUARDED_PROP]))"),
        ],
    )
    .unwrap();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs("console.log(Promise?.allSettled?.([Promise.resolve(), x.allSettled]))"),
            expected(
                "console.log(GUARDED_NAME?.GUARDED_PROP?.([GUARDED_NAME.resolve(), x.GUARDED_PROP]))",
            ),
        ],
    )
    .unwrap();
    h.test(
        &mut t,
        vec![
            externs_console_promise(),
            srcs(
                "console.log(\n    Promise?.resolve(Promise)\n        .then(x.finally)\n        .finally?.(x.finally))\n",
            ),
            expected(
                "console.log(\n    GUARDED_NAME?.resolve(GUARDED_NAME)\n        .then(x.finally)\n        .GUARDED_PROP?.(x.GUARDED_PROP))\n",
            ),
        ],
    )
    .unwrap();
}
