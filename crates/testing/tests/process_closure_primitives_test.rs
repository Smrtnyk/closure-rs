/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ProcessClosurePrimitivesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Rust unit tests of ProcessClosurePrimitivesTest.java whose post-call assertions the corpus
//! records cannot capture (corpus/unit/rust_unit_tests/ProcessClosurePrimitivesTest.md): they
//! query the last compiler's JSTypeRegistry for forward-declared types.
use closure_jscomp::{
    compiler_pass::CompilerPass, process_closure_primitives::ProcessClosurePrimitives,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    harness_passes,
    jscomp_api::{CheckLevel, Compiler, CompilerOptions},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

/// The overrides of ProcessClosurePrimitivesTest (getOptions, getProcessor).
struct ProcessClosurePrimitivesTest {
    ctx: Ctx,
}

impl ProcessClosurePrimitivesTest {
    // port: ProcessClosurePrimitivesTest#ProcessClosurePrimitivesTest
    fn new() -> (CompilerTestCase, Self) {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        (
            harness,
            Self {
                ctx: Ctx::new(
                    "ProcessClosurePrimitivesTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
            },
        )
    }
}

impl CompilerTestCaseHooks for ProcessClosurePrimitivesTest {
    // port: ProcessClosurePrimitivesTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        harness.enable_create_module_map()?;

        options.set_warning_level(
            harness_passes::diagnostic_group("MODULE_LOAD")?,
            CheckLevel::OFF,
        );
        Ok(options)
    }

    // port: ProcessClosurePrimitivesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                ProcessClosurePrimitives::new(compiler).process(compiler, externs, root);
            },
        )))))
    }

    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: CompilerTestCase#test(String, String)
fn test(
    harness: &mut CompilerTestCase,
    hooks: &mut ProcessClosurePrimitivesTest,
    js: &str,
    expected: &str,
) -> Result<(), Throwable> {
    harness.test(
        hooks,
        vec![
            TestPart::Sources(CompilerTestCase::srcs(js)),
            TestPart::Expected(CompilerTestCase::expected(expected)),
        ],
    )
}

// port: CompilerTestCase#testSame(String)
fn test_same(
    harness: &mut CompilerTestCase,
    hooks: &mut ProcessClosurePrimitivesTest,
    js: &str,
) -> Result<(), Throwable> {
    harness.test_same(hooks, vec![TestPart::Sources(CompilerTestCase::srcs(js))])
}

fn is_forward_declared_type(harness: &CompilerTestCase, name: &str) -> bool {
    let compiler = harness.get_last_compiler().unwrap();
    let mut compiler = compiler.borrow_mut();
    compiler.get_type_registry().is_forward_declared_type(name)
}

// port: ProcessClosurePrimitivesTest#testAddDependency
#[test]
fn test_add_dependency() {
    let (mut harness, mut hooks) = ProcessClosurePrimitivesTest::new();
    test(
        &mut harness,
        &mut hooks,
        "goog.addDependency('x.js', ['A', 'B'], []);",
        "0",
    )
    .unwrap();

    assert!(!is_forward_declared_type(&harness, "A"));
    assert!(!is_forward_declared_type(&harness, "B"));
    assert!(!is_forward_declared_type(&harness, "C"));
}

// port: ProcessClosurePrimitivesTest#testForwardDeclarations
#[test]
fn test_forward_declarations() {
    let (mut harness, mut hooks) = ProcessClosurePrimitivesTest::new();
    test_same(&mut harness, &mut hooks, "goog.forwardDeclare('A.B')").unwrap();

    assert!(is_forward_declared_type(&harness, "A.B"));
    assert!(!is_forward_declared_type(&harness, "C.D"));

    test_same(
        &mut harness,
        &mut hooks,
        "goog.module('mod'); goog.forwardDeclare('A.B');",
    )
    .unwrap();

    assert!(is_forward_declared_type(&harness, "A.B"));

    harness
        .test_same(
            &mut hooks,
            vec![TestPart::Sources(CompilerTestCase::srcs_strings(&[
                "goog.provide('A.B');".into(),
                "goog.module('mod'); const B = goog.forwardDeclare('A.B');".into(),
            ]))],
        )
        .unwrap();

    // This is a valid forward declaration, but for historical reasons, does not actually forward
    // declare the type 'A.B'.
    assert!(!is_forward_declared_type(&harness, "A.B"));
}
