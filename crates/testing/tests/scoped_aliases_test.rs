/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ScopedAliasesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Rust unit tests of ScopedAliasesTest.java whose post-call assertions the corpus records cannot
//! capture (corpus/unit/rust_unit_tests/ScopedAliasesTest.md): they read source positions of
//! nodes of the last compiler's output AST.
use closure_jscomp::{
    compiler_pass::CompilerPass,
    scoped_aliases::{InvalidModuleGetHandling, ScopedAliases},
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::{Throwable, assert_that},
};
use std::{cell::RefCell, rc::Rc};

const GOOG_SCOPE_START_BLOCK: &str = "goog.scope(function() {";
const GOOG_SCOPE_END_BLOCK: &str = "});";

/// The fields and overrides of ScopedAliasesTest (invalidModuleGetHandling, getProcessor).
struct ScopedAliasesTest {
    ctx: Ctx,
    invalid_module_get_handling: Option<InvalidModuleGetHandling>,
}

impl ScopedAliasesTest {
    // port: ScopedAliasesTest#ScopedAliasesTest
    fn new() -> (CompilerTestCase, Self) {
        let externs = CompilerTestCase::minimal_externs()
            .unwrap()
            .to_string_lossy()
            + "var window;";
        let mut harness = CompilerTestCase::new(externs);
        let mut test = Self {
            ctx: Ctx::new(
                "ScopedAliasesTest".into(),
                closure_testing::replay::replay_values::object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            invalid_module_get_handling: None,
        };
        test.set_up(&mut harness).unwrap();
        (harness, test)
    }

    // port: ScopedAliasesTest#setUp
    fn set_up(&mut self, harness: &mut CompilerTestCase) -> Result<(), Throwable> {
        harness.set_up();
        harness.disable_type_check()?;
        harness.enable_run_type_check_after_processing()?;
        harness.enable_create_module_map()?;
        self.invalid_module_get_handling = Some(InvalidModuleGetHandling::GIVE_UNIQUE_NAME);
        harness.set_generic_name_replacements(IndexMap::<_, _>::from_iter([(
            "SCOPED_ALIASES".to_string(),
            "jscomp$scopedAliases$".to_string(),
        )]));
        Ok(())
    }

    // port: ScopedAliasesTest#testScoped
    fn test_scoped(
        &mut self,
        harness: &mut CompilerTestCase,
        code: &str,
        expected: &str,
    ) -> Result<(), Throwable> {
        let parts = vec![
            TestPart::Sources(CompilerTestCase::srcs(format!(
                "{GOOG_SCOPE_START_BLOCK}{code}{GOOG_SCOPE_END_BLOCK}"
            ))),
            TestPart::Expected(CompilerTestCase::expected(expected)),
        ];
        harness.test(self, parts)
    }
}

impl CompilerTestCaseHooks for ScopedAliasesTest {
    // port: ScopedAliasesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let invalid_module_get_handling = self.invalid_module_get_handling.unwrap();
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                ScopedAliases::builder()
                    .set_module_metadata_map(compiler.get_module_metadata_map().cloned())
                    .set_invalid_module_get_handling(invalid_module_get_handling)
                    .build(compiler)
                    .process(compiler, externs, root);
            },
        )))))
    }

    // port: ReplayDsl.Ctx#Ctx
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

// port: ScopedAliasesTest#testSourceInfo
#[test]
fn test_source_info() {
    let (mut harness, mut test) = ScopedAliasesTest::new();
    test.test_scoped(
        &mut harness,
        "var d = dom;\nvar e = event;\nalert(e.EventType.MOUSEUP);\nalert(d.TagName.DIV);\n",
        "alert(event.EventType.MOUSEUP); alert(dom.TagName.DIV);",
    )
    .unwrap();
    let compiler = harness.get_last_compiler().unwrap();
    let compiler = compiler.borrow();
    let root = compiler.get_root().unwrap();
    let dom = CompilerTestCase::find_qualified_name_node(&compiler, &"dom".into(), root).unwrap();
    let event =
        CompilerTestCase::find_qualified_name_node(&compiler, &"event".into(), root).unwrap();
    assert_that(
        dom.get_lineno(&compiler) > event.get_lineno(&compiler),
        "Dom line should be after event line.",
    )
    .unwrap();
}
