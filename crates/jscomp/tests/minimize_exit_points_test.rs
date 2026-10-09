/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/MinimizeExitPointsTest.java.

//! Port of `MinimizeExitPointsTest.java`.
use closure_jscomp::{
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    minimize_exit_points::MinimizeExitPoints,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
};
use closure_rhino::fast_hash::IndexMap;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct MinimizeExitPointsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: MinimizeExitPointsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
            vec![Box::new(MinimizeExitPoints::new())];
        let pass: Box<dyn CompilerPass> = Box::new(PeepholeOptimizationsPass::new(
            self.get_name(),
            optimizations,
        ));
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "MinimizeExitPointsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl MinimizeExitPointsTest {
    // port: MinimizeExitPointsTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_normalize().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "MinimizeExitPointsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
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

    // port: MinimizeExitPointsTest#foldSame
    fn fold_same(&mut self, js: &str) {
        self.test_same(js);
    }

    // port: MinimizeExitPointsTest#fold
    fn fold(&mut self, js: &str, expected: &str) {
        self.test(js, expected);
    }
}

// port: MinimizeExitPointsTest#testBreakOptimization
#[test]
fn test_break_optimization() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "f:{if(true){a();break f;}else;b();}",
        "f:{if(true){a()}else{b()}}",
    );
    t.fold(
        "f:{if(false){a();break f;}else;b();break f;}",
        "f:{if(false){a()}else{b()}}",
    );
    t.fold(
        "f:{if(a()){b();break f;}else;c();}",
        "f:{if(a()){b();}else{c();}}",
    );
    t.fold(
        "f:{if(a()){b()}else{c();break f;}}",
        "f:{if(a()){b()}else{c();}}",
    );
    t.fold("f:{if(a()){b();break f;}else;}", "f:{if(a()){b();}else;}");
    t.fold("f:{if(a()){break f;}else;}", "f:{if(a()){}else;}");
    t.fold("f:for(;a();)break f;", "f:for(;a();)break f");
    t.fold_same("f:for(x in a())break f");
    t.fold_same("f:{for(;a();)break;}");
    t.fold_same("f:{for(x in a())break}");
    t.fold(
        "f:try{break f;}catch(e){break f;}",
        "f: { try{}catch(e){} }",
    );
    t.fold(
        "f:try{if(a()){break f;}else{break f;} break f;}catch(e){}",
        "f:{ try{if(a()){}else{}}catch(e){} }",
    );
    t.fold("f:g:break f", "f: g: {}");
    t.fold(
        "f:g:{if(a()){break f;}else{break f;} break f;}",
        "f:g:{if(a()){}else{}}",
    );
    t.fold("function f() { a: break a; }", "function f() { a: {} }");
    t.fold("function f() { a: { break a; } }", "function f() { a: {} }");
}

// port: MinimizeExitPointsTest#testFunctionReturnOptimization1
#[test]
fn test_function_return_optimization1() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold("function f(){return}", "function f(){}");
}

// port: MinimizeExitPointsTest#testFunctionReturnOptimization2
#[test]
fn test_function_return_optimization2() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "function f(){if(a()){b();if(c())return;}}",
        "function f(){if(a()){b();if(c());}}",
    );
    t.fold(
        "function f(){if(x)return; x=3; return; }",
        "function f(){if(x); else x=3}",
    );
    t.fold(
        "function f(){if(true){a();return;}else;b();}",
        "function f(){if(true){a();}else{b();}}",
    );
    t.fold(
        "function f(){if(false){a();return;}else;b();return;}",
        "function f(){if(false){a();}else{b();}}",
    );
    t.fold(
        "function f(){if(a()){b();return;}else;c();}",
        "function f(){if(a()){b();}else{c();}}",
    );
    t.fold(
        "function f(){if(a()){b()}else{c();return;}}",
        "function f(){if(a()){b()}else{c();}}",
    );
    t.fold(
        "function f(){if(a()){b();return;}else;}",
        "function f(){if(a()){b();}else;}",
    );
    t.fold(
        "function f(){if(a()){return;}else{return;} return;}",
        "function f(){if(a()){}else{}}",
    );
    t.fold(
        "function f(){if(a()){return;}else{return;} b();}",
        "function f(){if(a()){}else{return;b()}}",
    );
    t.fold(
        "function f(){ if (x) return; if (y) return; if (z) return; w(); }",
        "function f() {\n  if (x) {} else { if (y) {} else { if (z) {} else w(); }}\n}\n",
    );
    t.fold(
        "function f(){for(;a();)return;}",
        "function f(){for(;a();)return}",
    );
    t.fold_same("function f(){for(x in a())return}");
    t.fold(
        "function f(){for(;a();)break;}",
        "function f(){for(;a();)break}",
    );
    t.fold_same("function f(){for(x in a())break}");
    t.fold(
        "function f(){try{return;}catch(e){throw 9;}finally{return}}",
        "function f(){try{}catch(e){throw 9;}finally{return}}",
    );
    t.fold_same("function f(){try{throw 9;}finally{return;}}");
    t.fold(
        "function f(){try{return;}catch(e){return;}}",
        "function f(){try{}catch(e){}}",
    );
    t.fold(
        "function f(){try{if(a()){return;}else{return;} return;}catch(e){}}",
        "function f(){try{if(a()){}else{}}catch(e){}}",
    );
    t.fold("function f(){g:return}", "function f(){g: {}}");
    t.fold(
        "function f(){g:if(a()){return;}else{return;} return;}",
        "function f(){g:{if(a()){}else{}}}",
    );
    t.fold("function f() {\n  try {\n    g: if (a()) {\n      throw 9;\n    }\n    return;\n  } finally {\n    return;\n  }\n}\n", "function f() {\n  try {\n    g: {\n      if (a()) {\n        throw 9;\n      }\n    }\n  } finally {\n    return;\n  }\n}\n");
}

// port: MinimizeExitPointsTest#testFunctionReturnScoped
#[test]
fn test_function_return_scoped() {
    let mut t = MinimizeExitPointsTest::new();
    t.test_same("function f(a) {\n  if (a) {\n    const aInner = Math.random();\n\n    if (aInner < 0.5) {\n        return aInner;\n    }\n  }\n\n  return a;\n}\n");
}

// port: MinimizeExitPointsTest#testWhileContinueOptimization
#[test]
fn test_while_continue_optimization() {
    let mut t = MinimizeExitPointsTest::new();
    // Normalization should convert all WHILE loops to FOR loops, so just have a simple test
    // verifying that happens & we don't need explicit WHILE loop support.
    t.fold(
        "while(true){if(x)continue; x=3; continue; }",
        "for(;true;)if(x);else x=3",
    );
}

// port: MinimizeExitPointsTest#testDoContinueOptimization
#[test]
fn test_do_continue_optimization() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "do{if(x)continue; x=3; continue; }while(true)",
        "do if(x); else x=3; while(true)",
    );
    t.fold_same("do{a();continue;b()}while(true)");
    t.fold(
        "do{if(true){a();continue;}else;b();}while(true)",
        "do{if(true){a();}else{b();}}while(true)",
    );
    t.fold(
        "do{if(false){a();continue;}else;b();continue;}while(true)",
        "do{if(false){a();}else{b();}}while(true)",
    );
    t.fold(
        "do{if(a()){b();continue;}else;c();}while(true)",
        "do{if(a()){b();}else{c()}}while(true)",
    );
    t.fold(
        "do{if(a()){b();}else{c();continue;}}while(true)",
        "do{if(a()){b();}else{c();}}while(true)",
    );
    t.fold(
        "do{if(a()){b();continue;}else;}while(true)",
        "do{if(a()){b();}else;}while(true)",
    );
    t.fold(
        "do{if(a()){continue;}else{continue;} continue;}while(true)",
        "do{if(a()){}else{}}while(true)",
    );
    t.fold(
        "do{if(a()){continue;}else{continue;} b();}while(true)",
        "do{if(a()){}else{continue; b();}}while(true)",
    );
    t.fold(
        "do{for(;a();)continue;}while(true)",
        "do for(;a(););while(true)",
    );
    t.fold(
        "do{for(x in a())continue}while(true)",
        "do for(x in a());while(true)",
    );
    t.fold(
        "do{for(;a();)break;}while(true)",
        "do for(;a();)break;while(true)",
    );
    t.fold_same("do for(x in a())break;while(true)");
    t.fold(
        "do{try{continue;}catch(e){continue;}}while(true)",
        "do{try{}catch(e){}}while(true)",
    );
    t.fold(
        "do{try{if(a()){continue;}else{continue;} continue;}catch(e){}}while(true)",
        "do{try{if(a()){}else{}}catch(e){}}while(true)",
    );
    t.fold("do{g:continue}while(true)", "do{g: {}}while(true)");
    // This case could be improved.
    t.fold(
        "do{g:if(a()){continue;}else{continue;} continue;}while(true)",
        "do{g: { if(a());else; } }while(true)",
    );
    t.fold(
        "do { foo(); continue; } while(false)",
        "do { foo(); } while(false)",
    );
    t.fold(
        "do { foo(); break; } while(false)",
        "do { foo(); } while(false)",
    );
    t.fold("do{break}while(!new Date());", "do{}while(!new Date());");
    t.fold_same("do { foo(); switch (x) { case 1: break; default: f()}; } while(false)");
}

// port: MinimizeExitPointsTest#testForContinueOptimization
#[test]
fn test_for_continue_optimization() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "for(x in y){if(x)continue; x=3; continue; }",
        "for(x in y)if(x);else x=3",
    );
    t.fold_same("for(x in y){a();continue;b()}");
    t.fold(
        "for(x in y){if(true){a();continue;}else;b();}",
        "for(x in y){if(true)a();else b();}",
    );
    t.fold(
        "for(x in y){if(false){a();continue;}else;b();continue;}",
        "for(x in y){if(false){a();}else{b()}}",
    );
    t.fold(
        "for(x in y){if(a()){b();continue;}else;c();}",
        "for(x in y){if(a()){b();}else{c();}}",
    );
    t.fold(
        "for(x in y){if(a()){b();}else{c();continue;}}",
        "for(x in y){if(a()){b();}else{c();}}",
    );
    t.fold(
        "for(x of y){if(x)continue; x=3; continue; }",
        "for(x of y)if(x);else x=3",
    );
    t.fold_same("for(x of y){a();continue;b()}");
    t.fold(
        "for(x of y){if(true){a();continue;}else;b();}",
        "for(x of y){if(true)a();else b();}",
    );
    t.fold(
        "for(x of y){if(false){a();continue;}else;b();continue;}",
        "for(x of y){if(false){a();}else{b()}}",
    );
    t.fold(
        "for(x of y){if(a()){b();continue;}else;c();}",
        "for(x of y){if(a()){b();}else{c();}}",
    );
    t.fold(
        "for(x of y){if(a()){b();}else{c();continue;}}",
        "for(x of y){if(a()){b();}else{c();}}",
    );
    t.fold(
        "async () => { for await (x of y){if(x)continue; x=3; continue; }}",
        "async () => { for await (x of y)if(x);else x=3 }",
    );
    t.fold_same("async () => { for await (x of y){a();continue;b()}}");
    t.fold(
        "async () => { for await (x of y){if(true){a();continue;}else;b();}}",
        "async () => { for await (x of y){if(true)a();else b();}}",
    );
    t.fold(
        "async () => { for await (x of y){if(false){a();continue;}else;b();continue;}}",
        "async () => { for await (x of y){if(false){a();}else{b()}}}",
    );
    t.fold(
        "async () => { for await (x of y){if(a()){b();continue;}else;c();}}",
        "async () => { for await (x of y){if(a()){b();}else{c();}}}",
    );
    t.fold(
        "async () => { for await (x of y){if(a()){b();}else{c();continue;}}}",
        "async () => { for await (x of y){if(a()){b();}else{c();}}}",
    );
    t.fold(
        "for(x=0;x<y;x++){if(a()){b();continue;}else;}",
        "x=0;for(;x<y;x++){if(a()){b();}else;}",
    );
    t.fold(
        "for(x=0;x<y;x++){if(a()){continue;}else{continue;} continue;}",
        "x=0;for(;x<y;x++){if(a()){}else{}}",
    );
    t.fold(
        "for(x=0;x<y;x++){if(a()){continue;}else{continue;} b();}",
        "x=0;for(;x<y;x++){if(a()){}else{continue; b();}}",
    );
    t.fold(
        "for(x=0;x<y;x++)for(;a();)continue;",
        "x=0;for(;x<y;x++)for(;a(););",
    );
    t.fold(
        "for(x=0;x<y;x++)for(x in a())continue",
        "x=0;for(;x<y;x++)for(x in a());",
    );
    t.fold(
        "for(x=0;x<y;x++)for(;a();)break;",
        "x=0;for(;x<y;x++)for(;a();)break",
    );
    t.fold_same("x=0;for(;x<y;x++)for(x in a())break");
    t.fold(
        "for(x=0;x<y;x++){try{continue;}catch(e){continue;}}",
        "x=0;for(;x<y;x++){try{}catch(e){}}",
    );
    t.fold(
        "for(x=0;x<y;x++){try{if(a()){continue;}else{continue;} continue;}catch(e){}}",
        "x=0;for(;x<y;x++){try{if(a()){}else{}}catch(e){}}",
    );
    t.fold("for(x=0;x<y;x++){g:continue}", "x=0;for(;x<y;x++){ g: {} }");
    t.fold(
        "for(x=0;x<y;x++){g:if(a()){continue;}else{continue;} continue;}",
        "x=0;for(;x<y;x++){g:{if(a());else;}}",
    );
}

// port: MinimizeExitPointsTest#testCodeMotionDoesntBreakFunctionHoisting
#[test]
fn test_code_motion_doesnt_break_function_hoisting() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "function f() { if (x) return; foo(); function foo() {} }",
        "function f() { function foo() {} if (x) {} else { foo(); } }",
    );
}

// port: MinimizeExitPointsTest#testDontRemoveBreakInTryFinally
#[test]
fn test_dont_remove_break_in_try_finally() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold_same("function f() {b: {try{throw 9} finally {break b}} return 1;}");
}

/// The 'break' prevents the 'b=false' from being evaluated. If we fold the do-while to
/// 'do;while(b=false)' the code will be incorrect.
/// @see https://github.com/google/closure-compiler/issues/554
/// /
// port: MinimizeExitPointsTest#testDontFoldBreakInDoWhileIfConditionHasSideEffects
#[test]
fn test_dont_fold_break_in_do_while_if_condition_has_side_effects() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold_same("var b=true;do{break}while(b=false);");
}

// port: MinimizeExitPointsTest#testSwitchExitPoints1
#[test]
fn test_switch_exit_points1() {
    let mut t = MinimizeExitPointsTest::new();
    t.fold(
        "switch (x) { case 1: f(); break; }",
        "switch (x) { case 1: f();        }",
    );
    t.fold(
        "switch (x) { case 1: f(); break; case 2: g(); break; }",
        "switch (x) { case 1: f(); break; case 2: g();        }",
    );
    t.fold(
        "switch (x) { case 1: if (x) { f(); break; } break; default: g(); break; }",
        "switch (x) { case 1: if (x) { f();        } break; default: g();        }",
    );
}

// port: MinimizeExitPointsTest#testThrowsExceptionIfAstNotNormalized
#[test]
fn test_throws_exception_if_ast_not_normalized() {
    let mut t = MinimizeExitPointsTest::new();
    t.harness.disable_normalize().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| t.fold_same("")));
    assert!(result.is_err(), "expected a RuntimeException");
}

// port: MinimizeExitPointsTest#testFoldBlockScopedVariables
#[test]
fn test_fold_block_scoped_variables() {
    let mut t = MinimizeExitPointsTest::new();
    // When moving block-scoped variable declarations into inner blocks, first convert them to
    // "var" declarations to avoid breaking any references in inner functions.
    // For example, in the following test case, moving "let c = 3;" directly inside the else block
    // would break the function "g"'s reference to "c".
    t.fold(
        "function f() { function g() { return c; } if (x) {return;} let c = 3; }",
        "function f() { function g() { return c; } if (x){} else {var c = 3;} }",
    );
    t.fold(
        "function f() { function g() { return c; } if (x) {return;} const c = 3; }",
        "function f() { function g() { return c; } if (x) {} else {var c = 3;} }",
    );
    // Convert let and const even they're if not referenced by any functions.
    t.fold(
        "function f() { if (x) {return;} const c = 3; }",
        "function f() { if (x) {} else { var c = 3; } }",
    );
    t.fold(
        "function f() { if (x) {return;} let a = 3; let b = () => a; }",
        "function f() { if (x) {} else { var a = 3; var b = () => { return a; }} }",
    );
    t.fold(
        "function f() { if (x) { if (y) {return;} let c = 3; } }",
        "function f() { if (x) { if (y) {} else { var c = 3; } } }",
    );
}

// port: MinimizeExitPointsTest#testDontFoldBlockScopedVariablesInLoops
#[test]
fn test_dont_fold_block_scoped_variables_in_loops() {
    let mut t = MinimizeExitPointsTest::new();
    // Don't move block-scoped declarations into inner blocks inside a loop, since converting
    // let/const declarations to vars in a loop can cause incorrect semantics.
    // See the following test case for an example.
    t.fold_same("function f(param) {\n  let arr = [];\n  for (let x of param) {\n    if (x < 0) continue;\n    let y = x * 2;\n    arr.push(() => {\n      return y;\n    }); // If y were a var, this would capture the wrong value.\n  }\n  return arr;\n}\n");
    // Additional tests for different kinds of loops.
    t.fold_same("function f() { for (;true;) { if (true) {return;} let c = 3; } }");
    t.fold_same("function f() { do { if (true) {return;} let c = 3; } while (x); }");
    t.fold_same("function f() { for (;;) { if (true) { return; } let c = 3; } }");
    t.fold_same("function f(y) { for(x in []){ if(x) { return; } let c = 3; } }");
    t.fold_same("async function f(y) { for await (x in []){ if(x) { return; } let c = 3; } }");
}
