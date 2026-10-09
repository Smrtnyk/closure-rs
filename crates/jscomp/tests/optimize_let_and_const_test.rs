/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2026 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/OptimizeLetAndConstTest.java.

//! Port of `OptimizeLetAndConstTest.java`: tests for OptimizeLetAndConst.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    optimize_let_and_const_peephole::OptimizeLetAndConstPeephole,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct OptimizeLetAndConstTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    assume_output_is_wrapped: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: OptimizeLetAndConstTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let assume_output_is_wrapped = self.assume_output_is_wrapped;
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![Box::new(
                    OptimizeLetAndConstPeephole::new(assume_output_is_wrapped),
                )];
                let mut peephole_pass =
                    PeepholeOptimizationsPass::new("testOptimizeLetAndConst", optimizations);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "OptimizeLetAndConstTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl OptimizeLetAndConstTest {
    // port: CompilerTestCase#CompilerTestCase()
    // port: OptimizeLetAndConstTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let assume_output_is_wrapped = false;
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "OptimizeLetAndConstTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                assume_output_is_wrapped,
            },
        }
    }

    fn test(&mut self, js: &str, expected: &str) {
        self.harness
            .test_strings(&mut self.hooks, js, expected)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }
}

// port: OptimizeLetAndConstTest#testConstToLetInBlock
#[test]
fn test_const_to_let_in_block() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test("if (true) { const x = 1; }", "if (true) { let x = 1; }");
}

// port: OptimizeLetAndConstTest#testConstToVarInFunction
#[test]
fn test_const_to_var_in_function() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "function f() { const x = 1; }",
        "function f() { var x = 1; }",
    );
}

// port: OptimizeLetAndConstTest#testLetToVarInFunction
#[test]
fn test_let_to_var_in_function() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test("function f() { let x = 1; }", "function f() { var x = 1; }");
}

// port: OptimizeLetAndConstTest#testLetToVarInBlock
#[test]
fn test_let_to_var_in_block() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test_same("function f() { if (true) { let x = 1; } }");
}

// port: OptimizeLetAndConstTest#testConstToLetGlobalUnwrapped
#[test]
fn test_const_to_let_global_unwrapped() {
    let mut t = OptimizeLetAndConstTest::new();
    t.hooks.assume_output_is_wrapped = false;
    t.test_same("const x = 1;");
}

// port: OptimizeLetAndConstTest#testConstToVarGlobalWrapped
#[test]
fn test_const_to_var_global_wrapped() {
    let mut t = OptimizeLetAndConstTest::new();
    t.hooks.assume_output_is_wrapped = true;
    t.test("const x = 1;", "var x = 1;");
}

// port: OptimizeLetAndConstTest#testLetToVarGlobalWrapped
#[test]
fn test_let_to_var_global_wrapped() {
    let mut t = OptimizeLetAndConstTest::new();
    t.hooks.assume_output_is_wrapped = true;
    t.test("let x = 1;", "var x = 1;");
}

// port: OptimizeLetAndConstTest#testLetInLoopNotConvertedToVar
#[test]
fn test_let_in_loop_not_converted_to_var() {
    let mut t = OptimizeLetAndConstTest::new();
    // let in loop header and loop body stay let
    t.test_same("function f() { for (let i = 0; i < 10; i++) { let x = 1; } }");
}

// port: OptimizeLetAndConstTest#testConstToLetInLoopConvertedToLetButNotVar
#[test]
fn test_const_to_let_in_loop_converted_to_let_but_not_var() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "function f() { for (let i = 0; i < 10; i++) { const x = 1; } }",
        "function f() { for (let i = 0; i < 10; i++) { let x = 1; } }",
    );
}

// port: OptimizeLetAndConstTest#testDestructuringConstToLet
#[test]
fn test_destructuring_const_to_let() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "if (true) { const [a, b] = [1, 2]; }",
        "if (true) { let [a, b] = [1, 2]; }",
    );
}

// port: OptimizeLetAndConstTest#testDestructuringLetToVarInFunction
#[test]
fn test_destructuring_let_to_var_in_function() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "function f() { let [a, b] = [1, 2]; }",
        "function f() { var [a, b] = [1, 2]; }",
    );
}

// port: OptimizeLetAndConstTest#testClassStaticBlockLetAndConst
#[test]
fn test_class_static_block_let_and_const() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "class C { static { const x = 1; let y = 2; } }",
        "class C { static { var x = 1; var y = 2; } }",
    );
}

// port: OptimizeLetAndConstTest#testCatchBlockConstAndLet
#[test]
fn test_catch_block_const_and_let() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "function f() { try {} catch (e) { const x = 1; let y = 2; } }",
        "function f() { try {} catch (e) { let x = 1; let y = 2; } }",
    );
}

// port: OptimizeLetAndConstTest#testSwitchConstAndLet
#[test]
fn test_switch_const_and_let() {
    let mut t = OptimizeLetAndConstTest::new();
    t.test(
        "function f(x) { switch (x) { case 1: const y = 1; let z = 2; } }",
        "function f(x) { switch (x) { case 1: let y = 1; let z = 2; } }",
    );
}
