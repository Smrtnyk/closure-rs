/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeRemoveDeadCodeTest.java.

//! Port of `PeepholeRemoveDeadCodeTest.java`: tests for PeepholeRemoveDeadCodeTest in isolation.
//! Tests for the interaction of multiple peephole passes are in PeepholeIntegrationTest.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
};
use closure_rhino::fast_hash::IndexMap;
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

// port: PeepholeRemoveDeadCodeTest#MATH
const MATH: &str = "/** @const */ var Math = {};\n/** @nosideeffects */ Math.random = function(){};\n/** @nosideeffects */ Math.sin = function(){};\n";

struct PeepholeRemoveDeadCodeTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeRemoveDeadCodeTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let pass: Box<dyn CompilerPass> = Box::new(
            // port: PeepholeRemoveDeadCodeTest#getProcessor (anonymous CompilerPass)
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    vec![Box::new(PeepholeRemoveDeadCode::new())];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: PeepholeRemoveDeadCodeTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        2
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeRemoveDeadCodeTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeRemoveDeadCodeTest {
    // port: PeepholeRemoveDeadCodeTest#PeepholeRemoveDeadCodeTest
    // port: PeepholeRemoveDeadCodeTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(MATH);
        harness.set_up();
        // This pass doesn't need the AST to be normalized as it runs both before and after
        // `denormalize`. Since it runs PureFunctionsIdentifier pass which expects the AST to be
        // normalized, we're doing `enableNormalize` for these tests.
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_compute_side_effects().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeRemoveDeadCodeTest".into(),
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

    // port: PeepholeRemoveDeadCodeTest#foldSame
    fn fold_same(&mut self, js: &str) {
        self.test_same(js);
    }

    // port: PeepholeRemoveDeadCodeTest#fold
    fn fold(&mut self, js: &str, expected: &str) {
        self.test(js, expected);
    }

    // port: PeepholeRemoveDeadCodeTest#testInFn
    fn test_in_fn(&mut self, js: &str, expected: &str) {
        let pre = "function f() {";
        let post = "}";
        self.test(
            &format!("{pre}{js}{post}"),
            &format!("{pre}{expected}{post}"),
        );
    }

    // port: PeepholeRemoveDeadCodeTest#testInLoop(String, String)
    fn test_in_loop(&mut self, js: &str, expected: &str) {
        self.test_in_loop_with_expected_before_loop(js, "", expected);
    }

    // port: PeepholeRemoveDeadCodeTest#testInLoop(String, String, String)
    fn test_in_loop_with_expected_before_loop(
        &mut self,
        js: &str,
        expected_before_loop: &str,
        expected: &str,
    ) {
        let pre = "for (;;) {";
        let post = "}";
        self.test(
            &format!("{pre}{js}{post}"),
            &format!("{expected_before_loop}{pre}{expected}{post}"),
        );
    }
}

// port: PeepholeRemoveDeadCodeTest#testRemoveNoOpLabelledStatement
#[test]
fn test_remove_no_op_labelled_statement() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a: break a;", "");
    t.fold("a: { break a; }", "");
    t.fold("a: { break a; console.log('unreachable'); }", "");
    t.fold("a: { break a; var x = 1; } x = 2;", "var x; x = 2;");
    t.fold_same("b: { var x = 1; } x = 2;");
    t.fold_same("a: b: { var x = 1; } x = 2;");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUselessLabelWithFollowingBreak
#[test]
fn test_remove_useless_label_with_following_break() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a:b: break b;", "");
    // Note: the break is only removed if the parent
    // is the break target.
    t.fold_same("a:b: break a;");
}

// port: PeepholeRemoveDeadCodeTest#testFoldBlock
#[test]
fn test_fold_block() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("{{foo()}}", "foo()");
    t.fold("{foo();{}}", "foo()");
    t.fold("{{foo()}{}}", "foo()");
    t.fold("{{foo()}{bar()}}", "foo();bar()");
    t.fold("{if(false)foo(); {bar()}}", "bar()");
    t.fold("{if(false)if(false)if(false)foo(); {bar()}}", "bar()");
    t.fold("{'hi'}", "");
    t.fold("{x==3}", "");
    t.fold("{`hello ${foo}`}", "");
    t.fold("{ (function(){x++}) }", "");
    t.fold_same("function f(){return;}");
    t.fold("function f(){return 3;}", "function f(){return 3}");
    t.fold_same("function f(){if(x)return; x=3; return; }");
    t.fold("{x=3;;;y=2;;;}", "x=3;y=2");
    // Cases to test for empty block.
    t.fold("while(x()){x}", "while(x());");
    t.fold("while(x()){x()}", "while(x())x()");
    t.fold("for(x=0;x<100;x++){x}", "for(x=0;x<100;x++);");
    t.fold("for(x in y){x}", "for(x in y);");
    t.fold("for (x of y) {x}", "for(x of y);");
    t.fold_same("for (let x = 1; x <10; x++ ) {}");
    t.fold_same("for (var x = 1; x <10; x++ ) {}");
}

// port: PeepholeRemoveDeadCodeTest#testFoldBlockWithDeclaration_notNormalized
#[test]
fn test_fold_block_with_declaration_not_normalized() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.harness.disable_normalize().unwrap();
    t.harness.disable_compute_side_effects().unwrap();
    t.fold_same("{let x}");
    t.fold_same("function f() {let x}");
    t.fold_same("{const x = 1}");
    t.fold_same("{x = 2; y = 4; let z;}");
    t.fold("{'hi'; let x;}", "{let x}");
    t.fold("{x = 4; {let y}}", "x = 4; {let y}");
    t.fold_same("{class C {}} {class C {}}");
    t.fold("{label: var x}", "label: var x");
    // `{label: let x}` is a syntax error
    t.fold_same("{label: var x; let y;}");
}

// port: PeepholeRemoveDeadCodeTest#testFoldBlockWithDeclaration_normalized
#[test]
fn test_fold_block_with_declaration_normalized() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("{let x}", "let x");
    t.fold_same("function f() {let x}");
    t.fold("{const x = 1}", "const x = 1;");
    t.fold("{x = 2; y = 4; let z;}", "x = 2; y = 4; let z;");
    t.fold("{'hi'; let x;}", "let x;");
    t.fold("{x = 4; {let y}}", "x = 4; let y;");
    t.fold(
        "{class C {}} {class C {}}",
        "class C {} class C$jscomp$1 {}",
    );
    t.fold("{label: var x}", "label: var x");
    // `{label: let x}` is a syntax error
    t.fold("{label: var x; let y;}", "label: var x; let y;");
}

/// Try to remove spurious blocks with multiple children
// port: PeepholeRemoveDeadCodeTest#testFoldBlocksWithManyChildren
#[test]
fn test_fold_blocks_with_many_children() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() { if (false) {} }", "function f(){}");
    t.fold(
        "function f() { { if (false) {} if (true) {} {} } }",
        "function f(){}",
    );
    t.fold(
        "{var x; var y; var z; class Foo { constructor() { var a; { var b; } } } }",
        "var x;var y;var z;class Foo { constructor() { var a;var b} }",
    );
    t.fold(
        "{var x; var y; var z; { { var a; { var b; } } } }",
        "var x;var y;var z; var a;var b",
    );
}

// port: PeepholeRemoveDeadCodeTest#testIf
#[test]
fn test_if() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if (1){ x=1; } else { x = 2;}", "x=1");
    t.fold("if (false){ x = 1; } else { x = 2; }", "x=2");
    t.fold("if (undefined){ x = 1; } else { x = 2; }", "x=2");
    t.fold("if (null){ x = 1; } else { x = 2; }", "x=2");
    t.fold("if (void 0){ x = 1; } else { x = 2; }", "x=2");
    t.fold("if (void foo()){ x = 1; } else { x = 2; }", "foo();x=2");
    t.fold(
        "if (false){ x = 1; } else if (true) { x = 3; } else { x = 2; }",
        "x=3",
    );
    t.fold("if (x){ x = 1; } else if (false) { x = 3; }", "if(x)x=1");
}

// port: PeepholeRemoveDeadCodeTest#testHook
#[test]
fn test_hook() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("true ? a() : b()", "a()");
    t.fold("false ? a() : b()", "b()");
    t.fold("a() ? b() : true", "a() && b()");
    t.fold("a() ? true : b()", "a() || b()");
    t.fold("(a = true) ? b() : c()", "a = true, b()");
    t.fold("(a = false) ? b() : c()", "a = false, c()");
    t.fold(
        "do {f()} while((a = true) ? b() : c())",
        "do {f()} while((a = true) , b())",
    );
    t.fold(
        "do {f()} while((a = false) ? b() : c())",
        "do {f()} while((a = false) , c())",
    );
    t.fold("var x = (true) ? 1 : 0", "var x=1");
    t.fold(
        "var y = (true) ? ((false) ? 12 : (cond ? 1 : 2)) : 13",
        "var y=cond?1:2",
    );
    t.fold_same("var z=x?void 0:y()");
    t.fold_same("z=x?void 0:y()");
    t.fold_same("z*=x?void 0:y()");
    t.fold_same("var z=x?y():void 0");
    t.fold_same("(w?x:void 0).y=z");
    t.fold_same("(w?x:void 0).y+=z");
    t.fold("y = (x ? void 0 : void 0)", "y = void 0");
    t.fold("y = (x ? f() : f())", "y = f()");
    t.fold("(function(){}) ? function(){} : function(){}", "");
}

// port: PeepholeRemoveDeadCodeTest#testConstantConditionWithSideEffect1
#[test]
fn test_constant_condition_with_side_effect1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if (b=true) x=1;", "b=true;x=1");
    t.fold("if (b=/ab/) x=1;", "b=/ab/;x=1");
    t.fold("if (b=/ab/){ x=1; } else { x=2; }", "b=/ab/;x=1");
    t.fold("var b;b=/ab/;if(b)x=1;", "var b;b=/ab/;x=1");
    t.fold_same("var b;b=f();if(b)x=1;");
    t.fold("var b=/ab/;if(b)x=1;", "var b=/ab/;x=1");
    t.fold_same("var b=f();if(b)x=1;");
    t.fold_same("b=b++;if(b)x=b;");
    t.fold("(b=0,b=1);if(b)x=b;", "b=0,b=1;if(b)x=b;");
    t.fold("b=1;if(foo,b)x=b;", "b=1;x=b;");
    t.fold_same("b=1;if(foo=1,b)x=b;");
}

// port: PeepholeRemoveDeadCodeTest#testConstantConditionWithSideEffect2
#[test]
fn test_constant_condition_with_side_effect2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("(b=true)?x=1:x=2;", "b=true,x=1");
    t.fold("(b=false)?x=1:x=2;", "b=false,x=2");
    t.fold("if (b=/ab/) x=1;", "b=/ab/;x=1");
    t.fold("var b;b=/ab/;(b)?x=1:x=2;", "var b;b=/ab/;x=1");
    t.fold_same("var b;b=f();(b)?x=1:x=2;");
    t.fold("var b=/ab/;(b)?x=1:x=2;", "var b=/ab/;x=1");
    t.fold_same("var b=f();(b)?x=1:x=2;");
}

// port: PeepholeRemoveDeadCodeTest#testConstantConditionWithSideEffect_coalesce
#[test]
fn test_constant_condition_with_side_effect_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("b = null; b ?? (x = 1)", "b = null; void 0 ?? (x = 1)");
    t.fold(
        "b = undefined; b ?? (x = 1)",
        "b = undefined; void 0 ?? (x = 1)",
    );
    t.fold(
        "b = (fn(), null); b ?? (x = 1)",
        "b = (fn(), null); void 0 ?? (x = 1)",
    );
    t.fold("b = 34; b ?? (x = 1)", "b = 34; 0 ?? (x = 1)");
    t.fold("b = 'test'; b ?? (x = 1)", "b = 'test'; 0 ?? (x = 1)");
    t.fold("b = []; b ?? (x = 1)", "b = []; 0 ?? (x = 1)");
    t.fold("b = (fn(), 0); b ?? (x = 1)", " b= (fn(), 0); 0 ?? (x = 1)");
    t.fold_same("b = fn(); b ?? (x = 1)");
}

// port: PeepholeRemoveDeadCodeTest#testVarLifting
#[test]
fn test_var_lifting() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if(true)var a", "var a");
    t.fold("if(false)var a", "var a");
    // More var lifting tests in PeepholeIntegrationTests
}

// port: PeepholeRemoveDeadCodeTest#testLetConstLifting
#[test]
fn test_let_const_lifting() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if(true) {const x = 1}", "const x = 1;");
    t.fold("if(false) {const x = 1}", "");
    t.fold("if(true) {let x}", "let x;");
    t.fold("if(false) {let x}", "");
    t.fold("if(false) {const x = 1;  function f() { return x; }}", "");
}

// port: PeepholeRemoveDeadCodeTest#testLetConstLifting_removePartOfBlock
#[test]
fn test_let_const_lifting_remove_part_of_block() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "function f() {\n  return 0;\n  let x = 0;\n  x++;\n}\n",
        "function f() {\n  let x;\n  return 0;\n}\n",
    );
    t.fold(
        "function f() {\n  return 0;\n  const [x, y, [[z]]] = 1;\n}\n",
        "function f() {\n  let x;\n  let y;\n  let z;\n  return 0;\n}\n",
    );
    t.fold(
        "function f() {\n  return 0;\n  const C = class Bar {};\n}\n",
        "function f() {\n  let C;\n  return 0;\n}\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testLetConstLifting_removePartOfBlock_withHoistedFunction
#[test]
fn test_let_const_lifting_remove_part_of_block_with_hoisted_function() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  return 0;\n  // Everything after this is dead code, except for the function declaration, which is\n  // hoisted. `foo` could in theory reference x, y, or C. Add a stub 'let' declaration for\n  // each to avoid violating the invariant that all NAME nodes in the AST are declared.\n  const x = 1;\n  let y;\n  class C {}\n  function foo() {}\n}\n", "function f() {\n  function foo() {}\n  let C;\n  let y;\n  let x;\n  return 0;\n}\n");
    t.fold("function f(param) {\n  return 0;\n  // everything after this is dead code.\n  if (param) {\n    function foo() {}\n    const x = 1;\n    let y;\n    class C {}\n  }\n}\n", "function f(param) {\n  return 0;\n}\n");
    t.fold("function f(param) {\n  if (param) {\n    return 0;\n    const x = 1;\n  }\n  return 1;\n}\n", "function f(param) {\n  if (param) {\n    let x;\n    return 0;\n  }\n  return 1;\n}\n");
}

// port: PeepholeRemoveDeadCodeTest#testFoldUselessFor
#[test]
fn test_fold_useless_for() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("for(;false;) { foo() }", "");
    t.fold("for(;void 0;) { foo() }", "");
    t.fold("for(;undefined;) { foo() }", "");
    t.fold("for(;true;) foo() ", "for(;;) foo() ");
    t.fold_same("for(;;) foo()");
    t.fold("for(;false;) { var a = 0; }", "var a");
    t.fold("for(;false;) { const a = 0; }", "");
    t.fold("for(;false;) { let a = 0; }", "");
    // Make sure it plays nice with minimizing
    t.fold("for(;false;) { foo(); continue }", "");
    t.fold("l1:for(;false;) {  }", "");
}

// port: PeepholeRemoveDeadCodeTest#testFoldUselessDo
#[test]
fn test_fold_useless_do() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("do { foo() } while(false);", "foo()");
    t.fold("do { foo() } while(void 0);", "foo()");
    t.fold("do { foo() } while(undefined);", "foo()");
    t.fold_same("do { foo() } while(true);");
    t.fold("do { var a = 0; } while(false);", "var a=0");
    t.fold("do { var a = 0; } while(!{a:foo()});", "var a=0;foo()");
    // Can't fold with break or continues.
    t.fold_same("do { foo(); continue; } while(0)");
    t.fold_same("do { try { foo() } catch (e) { break; } } while (0);");
    t.fold_same("do { foo(); break; } while(0)");
    t.fold(
        "do { for (;;) {foo(); continue;} } while(0)",
        "for (;;) {foo(); continue;}",
    );
    t.fold_same("l1: do { for (;;) { foo() } } while(0)");
    t.fold(
        "do { switch (1) { default: foo(); break} } while(0)",
        "foo();",
    );
    t.fold(
        "do { switch (1) { default: foo(); continue} } while(0)",
        "do { foo(); continue } while(0)",
    );
    t.fold(
        "l1: { do { x = 1; break l1; } while (0); x = 2; }",
        "l1: { x = 1; break l1; }",
    );
    t.fold("do { x = 1; } while (x = 0);", "x = 1; x = 0;");
    t.fold(
        "let x = 1; (function() { do { let x = 2; } while (x = 10, false); })();",
        "let x = 1; (function() { let x$jscomp$1 = 2; x = 10 })();",
    );
}

// port: PeepholeRemoveDeadCodeTest#testFoldEmptyDo
#[test]
fn test_fold_empty_do() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("do { } while(true);", "for (;;);");
}

// port: PeepholeRemoveDeadCodeTest#testMinimizeLoop_withConstantCondition_vanillaFor
#[test]
fn test_minimize_loop_with_constant_condition_vanilla_for() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("for(;true;) foo()", "for(;;) foo()");
    t.fold("for(;0;) foo()", "");
    t.fold("for(;0.0;) foo()", "");
    t.fold("for(;NaN;) foo()", "");
    t.fold("for(;null;) foo()", "");
    t.fold("for(;undefined;) foo()", "");
    t.fold("for(;'';) foo()", "");
}

// port: PeepholeRemoveDeadCodeTest#testMinimizeLoop_withConstantCondition_doWhile
#[test]
fn test_minimize_loop_with_constant_condition_do_while() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("do { foo(); } while (true)", "do { foo(); } while (true);");
    t.fold("do { foo(); } while (0)", "foo();");
    t.fold("do { foo(); } while (0.0)", "foo();");
    t.fold("do { foo(); } while (NaN)", "foo();");
    t.fold("do { foo(); } while (null)", "foo();");
    t.fold("do { foo(); } while (undefined)", "foo();");
    t.fold("do { foo(); } while ('')", "foo();");
}

// port: PeepholeRemoveDeadCodeTest#testFoldConstantCommaExpressions
#[test]
fn test_fold_constant_comma_expressions() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if (true, false) {foo()}", "");
    t.fold("if (false, true) {foo()}", "foo()");
    t.fold("true, foo()", "foo()");
    t.fold("true, foo?.()", "foo?.()");
    t.fold("(1 + 2 + ''), foo()", "foo()");
    t.fold("(1 + 2 + ''), foo?.()", "foo?.()");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUselessOps1
#[test]
fn test_remove_useless_ops1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("(function () { f(); })();");
}

// port: PeepholeRemoveDeadCodeTest#testCallSideEffectsPreserved
#[test]
fn test_call_side_effects_preserved() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // Functions calls known to be free of side effects are removed.
    t.fold("Math.random()", "");
    t.fold("Math?.random()", "");
    t.fold("Math.random(f() + g())", "f(),g();");
    t.fold_same("Math?.random(f() + g());");
    t.fold("Math.random(f(),g(),h())", "f(),g(),h();");
    t.fold_same("Math?.random(f(),g(),h());");
    t.fold("Symbol()", "");
    t.fold("Symbol('desc')", "");
    t.fold("Symbol(f())", "f();");
    // Calls to functions with unknown side-effects are preserved.
    t.fold_same("f();");
    t.fold_same("f?.();");
    t.fold_same("(function () { f(); })();");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUselessOps2
#[test]
fn test_remove_useless_ops2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // There are four place where expression results are discarded:
    //  - a top-level expression EXPR_RESULT
    //  - the LHS of a COMMA
    //  - the FOR init expression
    //  - the FOR increment expression
    // We know that this function has no side effects because of the
    // PureFunctionIdentifier.
    t.fold("(function () {})();", "");
    // Uncalled function expressions are removed
    t.fold("(function () {});", "");
    t.fold("(function f() {});", "");
    t.fold("(function* f() {})", "");
    // ... including any code they contain.
    t.fold("(function () {foo();});", "");
    // Useless operators are removed.
    t.fold("+f()", "f()");
    t.fold("+f?.()", "f?.()");
    t.fold("a=(+f(),g())", "a=(f(),g())");
    t.fold("a=(+f?.(),g())", "a=(f?.(),g())");
    t.fold("a=(true,g())", "a=g()");
    t.fold("f(),true", "f()");
    t.fold("f() + g()", "f(),g()");
    t.fold("for(;;+f()){}", "for(;;f()){}");
    t.fold("for(+f();;g()){}", "for(f();;g()){}");
    t.fold("for(;;Math.random(f(),g(),h())){}", "for(;;f(),g(),h()){}");
    // The optimization cascades into conditional expressions:
    t.fold("g() && +f()", "g() && f()");
    t.fold("g() || +f()", "g() || f()");
    t.fold("x ? g() : +f()", "x ? g() : f()");
    t.fold("+x()", "x()");
    t.fold("+x() * 2", "x()");
    t.fold("-(+x() * 2)", "x()");
    t.fold("2 -(+x() * 2)", "x()");
    t.fold("x().foo", "x()");
    t.fold_same("x().foo()");
    t.fold_same("x++");
    t.fold_same("++x");
    t.fold_same("x--");
    t.fold_same("--x");
    t.fold_same("x = 2");
    t.fold_same("x *= 2");
    // Sanity check, other expression are left alone.
    t.fold_same("function f() {}");
    t.fold_same("var x;");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitch
#[test]
fn test_optimize_switch() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("switch(a){}", "");
    t.fold("switch(foo()){}", "foo()");
    t.fold("switch(x++){}", "x++");
    t.fold("switch(a=b){}", "a=b");
    t.fold("switch(x++){default:foo();}", "x++; foo()");
    t.fold("switch(a=b){default:foo();}", "a=b; foo()");
    t.fold("switch(a){default:}", "");
    t.fold("switch(a){default:break;}", "");
    t.fold("switch(a){default:var b;break;}", "var b");
    t.fold("switch(a){case 1: default:}", "");
    t.fold("switch(a){default: case 1:}", "");
    t.fold("switch(a){default: break; case 1:break;}", "");
    t.fold(
        "switch(a){default: var b; break; case 1: var c; break;}",
        "var c; var b;",
    );
    t.fold("var x=1; switch(x) { case 1: var y; }", "var y; var x=1;");
    // Can't remove cases if a default exists and is not the last case.
    t.fold_same("function f() {switch(a){default: return; case 1: break;}}");
    t.fold_same("function f() {switch(1){default: return; case 1: break;}}");
    // foldable
    t.fold_same("function f() {switch(a){case 1: foo();}}");
    t.fold_same("function f() {switch(a){case 3: case 2: case 1: foo();}}");
    t.fold(
        "function f() {switch(a){case 2: case 1: default: foo();}}",
        "function f() { foo(); }",
    );
    t.fold(
        "switch(a){case 1: default:break; case 2: foo()}",
        "switch(a){case 2: foo()}",
    );
    t.fold_same("switch(a){case 1: goo(); default:break; case 2: foo()}");
    // TODO(johnlenz): merge the useless "case 2"
    t.fold_same("switch(a){case 1: goo(); case 2:break; case 3: foo()}");
    // Can't remove unused code with a "var" in it.
    t.fold("switch(1){case 2: var x=0;}", "var x;");
    t.fold("switch ('repeated') {\ncase 'repeated':\n  foo();\n  break;\ncase 'repeated':\n  var x=0;\n  break;\n}\n", "var x; foo();");
    // Can't remove cases if something useful is done.
    t.fold_same("switch(a){case 1: var c =2; break;}");
    t.fold_same("function f() {switch(a){case 1: return;}}");
    t.fold_same("x:switch(a){case 1: break x;}");
    t.fold(
        "switch ('foo') {\ncase 'foo':\n  foo();\n  break;\ncase 'bar':\n  bar();\n  break;\n}\n",
        "foo();",
    );
    t.fold("switch ('noMatch') {\ncase 'foo':\n  foo();\n  break;\ncase 'bar':\n  bar();\n  break;\n}\n", "");
    t.fold("switch ('fallThru') {\ncase 'fallThru':\n  if (foo(123) > 0) {\n    foobar(1);\n    break;\n  }\n  foobar(2);\ncase 'bar':\n  bar();\n}\n", "switch ('fallThru') {\ncase 'fallThru':\n  if (foo(123) > 0) {\n    foobar(1);\n    break;\n  }\n  foobar(2);\n  bar();\n}\n");
    t.fold(
        "switch ('fallThru') {\ncase 'fallThru':\n  foo();\ncase 'bar':\n  bar();\n}\n",
        "foo();\nbar();\n",
    );
    t.fold("switch ('hasDefaultCase') {\n  case 'foo':\n    foo();\n    break;\n  default:\n    bar();\n    break;\n}\n", "bar();");
    t.fold("switch ('repeated') {\ncase 'repeated':\n  foo();\n  break;\ncase 'repeated':\n  bar();\n  break;\n}\n", "foo();");
    t.fold_same("switch ('foo') {\ncase 'bar':\n  bar();\n  break;\ncase notConstant:\n  foobar();\n  break;\ncase 'foo':\n  foo();\n  break;\n}\n");
    t.fold(
        "switch (1) {\ncase 1:\n  foo();\n  break;\ncase 2:\n  bar();\n  break;\n}\n",
        "foo();",
    );
    t.fold(
        "switch (1) {\ncase 1.1:\n  foo();\n  break;\ncase 2:\n  bar();\n  break;\n}\n",
        "",
    );
    t.fold("switch (0) {\ncase NaN:\n  foobar();\n  break;\ncase -0.0:\n  foo();\n  break;\ncase 2:\n  bar();\n  break;\n}\n", "foo();");
    t.fold_same("switch ('\\v') {\ncase '\\u000B':\n  foo();\n}\n");
    t.fold(
        "switch ('empty') {\ncase 'empty':\ncase 'foo':\n  foo();\n}\n",
        "foo()",
    );
    t.fold(
        "let x;\nswitch (use(x)) {\n  default: {let y;}\n}\n",
        "let x;\nuse(x);\nlet y;\n",
    );
    t.fold(
        "let x;\nswitch (use?.(x)) {\n  default: {let y;}\n}\n",
        "let x;\nuse?.(x);\nlet y;\n",
    );
    t.fold(
        "let x;\nswitch (use(x)) {\n  default: let y;\n}\n",
        "let x;\nuse(x);\nlet y;\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchBug335145701
#[test]
fn test_optimize_switch_bug335145701() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function foo() { alert('foo()'); }\nswitch (1) {\n  case 1: break;\n  case foo(): break;\n}\n");
    t.fold_same("function foo() { alert('foo()'); }\nswitch (1) {\n  case 0: break;\n  case 1: break;\n  case foo(): break;\n}\n");
    t.fold_same("function foo() { alert('foo()'); }\nswitch (1) {\n  case 0: alert('bar'); break;\n  case 1: break;\n  case foo(): break;\n}\n");
    t.fold_same("function foo() { alert('foo()'); return 1; }\nswitch (1) {\n  case 0: break;\n  case foo(): break;\n  case 2: break;\n}\n");
    t.fold("function foo() { alert('foo()'); }\nswitch (1) {\n  case foo(): break;\n  case (0,1): break;\n}\n", "function foo() { alert('foo()'); }\nswitch (1) {\n  case foo(): break;\n  case 1: break;\n}\n");
    t.fold_same("function foo() { alert('foo()'); }\nswitch (x) {\n  case 1: break;\n  case foo(): break;\n}\n");
    t.fold("// not valid to remove the useless case 1,\n// it would cause the default to run and it has side-effects\nswitch (1) {\n  case 1: break;\n  default:\n    bar();\n    break;\n}\n", "");
    t.fold_same("function foo() { alert('foo()'); }\nswitch (1) {\n  case 0: alert('bar'); break;\n  case 1: break;\n  case foo(): break;\n}\n");
    t.fold_same("    function foo() { alert('foo()'); }\n    switch (bar()) {\n      case 1: break;\n      case foo(): break;\n    }\n");
    t.fold("// is not valid to remove the first useless case 1,\n// because it matches and the second should not run\nswitch (1) {\n  case 1: break;\n  case 1: bar(); break;\n}\n", "");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchBug11536863
#[test]
fn test_optimize_switch_bug11536863() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "outer: {\n  switch (2) {\n    case 2:\n      f();\n      break outer;\n  }\n}\n",
        "outer: {f(); break outer;}",
    );
}

// port: PeepholeRemoveDeadCodeTest#testUnusedGetElemRemoved
#[test]
fn test_unused_get_elem_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a[b]", "");
    t.fold("a?.[b]", "");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitch2
#[test]
fn test_optimize_switch2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "outer: switch (2) {\n  case 2:\n    f();\n    break outer;\n}\n",
        "outer: {f(); break outer;}",
    );
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitch3
#[test]
fn test_optimize_switch3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("switch (1) {\n  case 1:\n  case 2:\n  case 3: {\n    break;\n  }\n  case 4:\n  case 5:\n  case 6:\n  default:\n    fail('Should not get here');\n    break;\n}\n", "");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithLabellessBreak
#[test]
fn test_optimize_switch_with_labelless_break() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  switch('x') {\n    case 'x': var x = 1; break;\n    case 'y': break;\n  }\n}\n", "function f() { var x = 1; }");
    // TODO(moz): Convert this to an if statement for better optimization
    t.fold_same(
        "function f() {\n  switch(x) {\n    case 'y': break;\n    default: var x = 1;\n  }\n}\n",
    );
    t.fold("var exit;\nswitch ('a') {\n  case 'a':\n    break;\n  default:\n    exit = 21;\n    break;\n}\nswitch(exit) {\n  case 21: throw 'x';\n  default : console.log('good');\n}\n", "var exit;\nswitch(exit) {\n  case 21: throw 'x';\n  default : console.log('good');\n}\n");
    t.fold(
        "let x = 1;\nswitch('x') {\n  case 'x': let x = 2; break;\n}\n",
        "let x = 1;\nlet x$jscomp$1 = 2\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithLabelledBreak
#[test]
fn test_optimize_switch_with_labelled_break() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  label:\n  switch('x') {\n    case 'x': break label;\n    case 'y': throw f;\n  }\n}\n", "function f() { }");
    t.fold("function f() {\n  label:\n  switch('x') {\n    case 'x': break label;\n    default: throw f;\n  }\n}\n", "function f() { }");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithReturn
#[test]
fn test_optimize_switch_with_return() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  switch('x') {\n    case 'x': return 1;\n    case 'y': return 2;\n  }\n}\n", "function f() { return 1; }");
    t.fold("function f() {\n  let x = 1;\n  switch('x') {\n    case 'x': { let x = 2; } return 3;\n    case 'y': return 4;\n  }\n}\n", "function f() {\n  let x = 1;\n  let x$jscomp$1 = 2;\n  return 3;\n}\n");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithThrow
#[test]
fn test_optimize_switch_with_throw() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "function f() {\n  switch('x') {\n    case 'x': throw f;\n    case 'y': throw f;\n  }\n}\n",
        "function f() { throw f; }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithContinue
#[test]
fn test_optimize_switch_with_continue() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  for (;;) {\n    switch('x') {\n      case 'x': continue;\n      case 'y': continue;\n    }\n  }\n}\n", "function f() { for (;;) { continue; } }");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithDefaultCaseWithFallthru
#[test]
fn test_optimize_switch_with_default_case_with_fallthru() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f() {\n  switch(a) {\n    case 'x':\n    case foo():\n    default: return 3\n  }\n}\n");
}

// port: PeepholeRemoveDeadCodeTest#testOptimizeSwitchWithDefaultCase
#[test]
fn test_optimize_switch_with_default_case() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f() {\n  switch('x') {\n    case 'x': return 1;\n    case 'y': return 2;\n    default: return 3\n }\n}\n", "function f() { return 1; }");
    t.fold("switch ('hasDefaultCase') {\n  case 'foo':\n    foo();\n    break;\n  default:\n    bar();\n    break;\n}\n", "bar();");
    t.fold_same("switch (x) { default: if (a) { break; } bar(); }");
    // Potentially foldable
    t.fold_same("switch (x) {\n  case x:\n    foo();\n    break;\n  default:\n    if (a) { break; }\n    bar();\n}\n");
    t.fold("switch ('hasDefaultCase') {\n  case 'foo':\n    foo();\n    break;\n  default:\n    if (true) { break; }\n    bar();\n}\n", "");
    t.fold("switch ('hasDefaultCase') {\n  case 'foo':\n    foo();\n    break;\n  default:\n    if (a) { break; }\n    bar();\n}\n", "switch ('hasDefaultCase') { default: if (a) { break; } bar(); }");
    t.fold("l: switch ('hasDefaultCase') {\n  case 'foo':\n    foo();\n    break;\n  default:\n    if (a) { break l; }\n    bar();\n    break;\n}\n", "l:{ if (a) { break l; } bar(); }");
    t.fold("switch ('hasDefaultCase') {\n  case 'foo':\n    bar();\n    break;\n  default:\n    foo();\n    break;\n}\n", "foo();");
    t.fold("switch (a()) { default: bar(); break;}", "a(); bar();");
    t.fold("switch (a?.()) { default: bar(); break;}", "a?.(); bar();");
    t.fold("switch (a()) { default: break; bar();}", "a();");
    t.fold("loop:\nfor (;;) {\n  switch (a()) {\n    default:\n      bar();\n      break loop;\n  }\n}\n", "loop: for (;;) { a(); bar(); break loop; }");
}

// port: PeepholeRemoveDeadCodeTest#testTreatSwitchAsExit
#[test]
fn test_treat_switch_as_exit() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "a: {switch(x){ case 1: case 2: break a; default: foo(); break a;} bar(); }",
        "a: {switch(x){ case 1: case 2: break a; default: foo(); break a;} }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testDontTreatSwitchAsExit
#[test]
fn test_dont_treat_switch_as_exit() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a: {switch(x){ case 1:break a; default: b: {foo(); break b;}} bar(); }");
    t.fold_same("a: {switch(x){ case 1:break a; default: foo(); break;} bar(); }");
    t.fold_same("a: {b: switch(x){ case 1:break a; default: foo(); break b;} bar(); }");
    t.fold_same("a: {switch(x){ case 1: b: { foo(); break b; } default: break a; } bar(); }");
    t.fold_same("a: {switch(x){ case 1: break a; } bar(); }");
    t.fold_same("a: {switch(x){ case 1: if (y) { break; } default: break a;} bar(); }");
    t.fold_same("a: {switch(x){ case 1: if (y) { break; } break a; default: break a;} bar(); }");
}

// port: PeepholeRemoveDeadCodeTest#testTreatTryAsExit
#[test]
fn test_treat_try_as_exit() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "a: {try { foo(); break a; } catch (e) { foo(); break a; } bar(); }",
        "a: {try { foo(); break a; } catch (e) { foo(); break a; } }",
    );
    t.fold(
        "a: {try { foo(); } finally { foo(); break a; } bar(); }",
        "a: {try { foo(); } finally { foo(); break a; } }",
    );
    t.fold(
        "a: {try { foo(); break a; } finally { foo(); } bar(); }",
        "a: {try { foo(); break a; } finally { foo(); } }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testDontTreatTryAsExit
#[test]
fn test_dont_treat_try_as_exit() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a: {try { foo(); break a; } catch (e) { foo(); } bar(); }");
    t.fold_same("a: {try { foo(); break a; } catch (e) { } bar(); }");
    t.fold_same("a: {try { foo(); } catch (e) { foo(); break a; } bar(); }");
    t.fold_same("a: {try { foo(); } finally { foo(); } bar(); }");
    t.fold_same("a: {try { b: { foo(); break b; } } finally { foo(); } bar(); }");
    t.fold_same("a: { b: try { foo(); break a; } finally { foo(); } bar(); }");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveNumber
#[test]
fn test_remove_number() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("3", "");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveVarGet1
#[test]
fn test_remove_var_get1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a", "");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveVarGet2
#[test]
fn test_remove_var_get2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = 1;a", "var a = 1");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnusedGetProp
#[test]
fn test_remove_unused_get_prop() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a.b", "var a = {}");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnusedOptChainGetProp
#[test]
fn test_remove_unused_opt_chain_get_prop() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a?.b", "var a = {}");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnusedGetProp2
#[test]
fn test_remove_unused_get_prop2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a.b=1;a.b", "var a = {};a.b=1");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnusedOptChainGetProp2
#[test]
fn test_remove_unused_opt_chain_get_prop2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a.b=1;a?.b", "var a = {};a.b=1");
}

// port: PeepholeRemoveDeadCodeTest#testRemovePrototypeGet1
#[test]
fn test_remove_prototype_get1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a.prototype.b", "var a = {}");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveOptChainPrototypeGet1
#[test]
fn test_remove_opt_chain_prototype_get1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var a = {};a?.prototype.b", "var a = {}");
}

// port: PeepholeRemoveDeadCodeTest#testRemovePrototypeGet2
#[test]
fn test_remove_prototype_get2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "var a = {};a.prototype.b = 1;a.prototype.b",
        "var a = {};a.prototype.b = 1",
    );
}

// port: PeepholeRemoveDeadCodeTest#testRemoveOptChainPrototypeGet2
#[test]
fn test_remove_opt_chain_prototype_get2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "var a = {};a.prototype.b = 1;a?.prototype.b",
        "var a = {};a.prototype.b = 1",
    );
}

// port: PeepholeRemoveDeadCodeTest#testNotRemovePrototypeGet2
#[test]
fn test_not_remove_prototype_get2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {};a.prototype.b = 1; let x = a.prototype.b");
}

// port: PeepholeRemoveDeadCodeTest#testNotRemoveOptChainPrototypeGet2
#[test]
fn test_not_remove_opt_chain_prototype_get2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {};a.prototype.b = 1; let x = a?.prototype.b");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveAdd1
#[test]
fn test_remove_add1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 + 2", "");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveVar1
#[test]
fn test_no_remove_var1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = 1");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveVar2
#[test]
fn test_no_remove_var2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = 1, b = 2");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign1
#[test]
fn test_no_remove_assign1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a = 1");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign2
#[test]
fn test_no_remove_assign2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a = b = 1");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign3
#[test]
fn test_no_remove_assign3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 + (a = 2)", "a = 2");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign4
#[test]
fn test_no_remove_assign4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("x.a = 1");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign5
#[test]
fn test_no_remove_assign5() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("x.a = x.b = 1");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveAssign6
#[test]
fn test_no_remove_assign6() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 + (x.a = 2)", "x.a = 2");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall1
#[test]
fn test_no_remove_call1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall1
#[test]
fn test_no_remove_opt_chain_call1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall2
#[test]
fn test_no_remove_call2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a()+b()", "a(),b()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall2
#[test]
fn test_no_remove_opt_chain_call2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a?.()+b?.()", "a?.(),b?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall3
#[test]
fn test_no_remove_call3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() && b()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall3
#[test]
fn test_no_remove_opt_chain_call3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a?.() && b?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall4
#[test]
fn test_no_remove_call4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() || b()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall4
#[test]
fn test_no_remove_opt_chain_call4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a?.() || b?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall4NullishCoalesce
#[test]
fn test_no_remove_call4_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() ?? b()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall4NullishCoalesce
#[test]
fn test_no_remove_opt_chain_call4_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a?.() ?? b?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall5NullishCoalesce
#[test]
fn test_no_remove_call5_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() ?? 1", "a()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptChainCall5NullishCoalesce
#[test]
fn test_no_remove_opt_chain_call5_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a?.() ?? 1", "a?.()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveOptionalChainSideEffects
#[test]
fn test_no_remove_optional_chain_side_effects() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a?.[f()];");
    t.fold_same("a?.[x++];");
    t.fold_same("a?.[delete cache[k]];");
    t.fold_same("a?.b[f()];");
    t.fold_same("Math?.sin(mutate());");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall6NullishCoalesce
#[test]
fn test_no_remove_call6_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("1 ?? a()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall5
#[test]
fn test_no_remove_call5() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() || 1", "a()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveCall6
#[test]
fn test_no_remove_call6() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("1 || a()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveThrow1
#[test]
fn test_no_remove_throw1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f(){throw a()}");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveThrow2
#[test]
fn test_no_remove_throw2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f(){throw a}");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveThrow3
#[test]
fn test_no_remove_throw3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f(){throw 10}");
}

// port: PeepholeRemoveDeadCodeTest#testRedundantIfRemoved
#[test]
fn test_redundant_if_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("if(x()) 1", "x()");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveInControlStructure3
#[test]
fn test_remove_in_control_structure3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("for(1;2;3) 4", "for(;;);");
}

// port: PeepholeRemoveDeadCodeTest#testHook1
#[test]
fn test_hook1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 ? 2 : 3", "");
}

// port: PeepholeRemoveDeadCodeTest#testHook2
#[test]
fn test_hook2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("x ? a() : 3", "x && a()");
}

// port: PeepholeRemoveDeadCodeTest#testHook3
#[test]
fn test_hook3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("x ? 2 : a()", "x || a()");
}

// port: PeepholeRemoveDeadCodeTest#testHook4
#[test]
fn test_hook4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("x ? a() : b()");
}

// port: PeepholeRemoveDeadCodeTest#testHook5
#[test]
fn test_hook5() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() ? 1 : 2", "a()");
}

// port: PeepholeRemoveDeadCodeTest#testHook6
#[test]
fn test_hook6() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() ? b() : 2", "a() && b()");
}

// port: PeepholeRemoveDeadCodeTest#testHook7
#[test]
fn test_hook7() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() ? 1 : b()", "a() || b()");
}

// port: PeepholeRemoveDeadCodeTest#testHook8
#[test]
fn test_hook8() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() ? b() : c()");
}

// port: PeepholeRemoveDeadCodeTest#testHook9
#[test]
fn test_hook9() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("true ? a() : (function f() {})()", "a()");
    t.fold(
        "false ? a() : (function f() {alert(x)})()",
        "(function f() {alert(x)})()",
    );
}

// port: PeepholeRemoveDeadCodeTest#testHook10
#[test]
fn test_hook10() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("((function () {}), true) ? a() : b()", "a()");
    t.fold(
        "((function () {alert(x)})(), true) ? a() : b()",
        "(function(){alert(x)})(),a()",
    );
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit1
#[test]
fn test_short_circuit1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("1 && a()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit2NullishCoalesce
#[test]
fn test_short_circuit2_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 ?? a() ?? 2", "1 ?? a()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit3NullishCoalesce
#[test]
fn test_short_circuit3_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() ?? 1 ?? 2", "a()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit4NullishCoalesce
#[test]
fn test_short_circuit4_nullish_coalesce() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() ?? 1 ?? b()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit2
#[test]
fn test_short_circuit2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 && a() && 2", "1 && a()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit3
#[test]
fn test_short_circuit3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a() && 1 && 2", "a()");
}

// port: PeepholeRemoveDeadCodeTest#testShortCircuit4
#[test]
fn test_short_circuit4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("a() && 1 && b()");
}

// port: PeepholeRemoveDeadCodeTest#testComplex1
#[test]
fn test_complex1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 && a() + b() + c()", "1 && (a(), b(), c())");
}

// port: PeepholeRemoveDeadCodeTest#testComplex2
#[test]
fn test_complex2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 && (a() ? b() : 1)", "1 && (a() && b())");
}

// port: PeepholeRemoveDeadCodeTest#testComplex3
#[test]
fn test_complex3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 && (a() ? b() : 1 + c())", "1 && (a() ? b() : c())");
}

// port: PeepholeRemoveDeadCodeTest#testComplex4
#[test]
fn test_complex4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("1 && (a() ? 1 : 1 + c())", "1 && (a() || c())");
}

// port: PeepholeRemoveDeadCodeTest#testComplex5
#[test]
fn test_complex5() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // can't simplify LHS of short circuit statements with side effects
    t.fold_same("(a() ? 1 : 1 + c()) && foo()");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveFunctionDeclaration1
#[test]
fn test_no_remove_function_declaration1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function foo(){}");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveFunctionDeclaration2
#[test]
fn test_no_remove_function_declaration2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var foo = function (){}");
}

// port: PeepholeRemoveDeadCodeTest#testNoSimplifyFunctionArgs1
#[test]
fn test_no_simplify_function_args1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("f(1 + 2, 3 + g())");
}

// port: PeepholeRemoveDeadCodeTest#testNoSimplifyFunctionArgs2
#[test]
fn test_no_simplify_function_args2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("1 && f(1 + 2, 3 + g())");
}

// port: PeepholeRemoveDeadCodeTest#testNoSimplifyFunctionArgs3
#[test]
fn test_no_simplify_function_args3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("1 && foo(a() ? b() : 1 + c())");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveInherits1
#[test]
fn test_no_remove_inherits1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {}; this.b = {}; var goog = {}; goog.inherits(b, a)");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveInherits2
#[test]
fn test_no_remove_inherits2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "var a = {}; this.b = {}; var goog = {}; goog.inherits(b, a) + 1",
        "var a = {}; this.b = {}; var goog = {}; goog.inherits(b, a)",
    );
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveInherits3
#[test]
fn test_no_remove_inherits3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("this.a = {}; var b = {}; b.inherits(a);");
}

// port: PeepholeRemoveDeadCodeTest#testNoRemoveInherits4
#[test]
fn test_no_remove_inherits4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "this.a = {}; var b = {}; b.inherits(a) + 1;",
        "this.a = {}; var b = {}; b.inherits(a)",
    );
}

// port: PeepholeRemoveDeadCodeTest#testRemoveFromLabel1
#[test]
fn test_remove_from_label1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("LBL: void 0", "");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveFromLabel2
#[test]
fn test_remove_from_label2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("LBL: foo() + 1 + bar()", "LBL: foo(),bar()");
}

// port: PeepholeRemoveDeadCodeTest#testCall
#[test]
fn test_call() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("foo(0)");
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("Math.sin(0);", "");
    t.fold("1 + Math.sin(0);", "");
}

// port: PeepholeRemoveDeadCodeTest#testCall_toString
#[test]
fn test_call_to_string() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("(10).toString();", "");
    t.fold("(10).toString(16);", "");
    t.fold("x.toString(2);", "");
    t.fold("foo().toString(16);", "foo();");
    t.fold("(10)?.toString(16);", "");
    t.fold_same("(10).toString(16, 2);");
}

// port: PeepholeRemoveDeadCodeTest#testCall_containingSpread
#[test]
fn test_call_containing_spread() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("Math.sin(...c)", "([...c])");
    t.fold("Math.sin(4, ...c, a)", "([...c])");
    t.fold("Math.sin(foo(), ...c, bar())", "(foo(), [...c], bar())");
    t.fold("Math.sin(...a, b, ...c)", "([...a], [...c])");
    t.fold("Math.sin(...b, ...c)", "([...b], [...c])");
}

// port: PeepholeRemoveDeadCodeTest#testOptChainCall_containingSpread
#[test]
fn test_opt_chain_call_containing_spread() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold_same("Math?.sin(...c);");
    t.fold_same("Math?.sin(4, ...c, a);");
    t.fold_same("Math?.sin(foo(), ...c, bar());");
    t.fold_same("Math?.sin(...a, b, ...c);");
    t.fold_same("Math?.sin(...b, ...c);");
}

// port: PeepholeRemoveDeadCodeTest#testNew
#[test]
fn test_new() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("new foo(0)");
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("new Date;", "");
    t.fold("1 + new Date;", "");
    t.fold_same("new (class { x = foo(); })()");
    t.fold("new (class { x = 1; })()", "");
}

// port: PeepholeRemoveDeadCodeTest#testNew_containingSpread
#[test]
fn test_new_containing_spread() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("new Date(...c)", "([...c])");
    t.fold("new Date(4, ...c, a)", "([...c])");
    t.fold("new Date(foo(), ...c, bar())", "(foo(), [...c], bar())");
    t.fold("new Date(...a, b, ...c)", "([...a], [...c])");
    t.fold("new Date(...b, ...c)", "([...b], [...c])");
}

// port: PeepholeRemoveDeadCodeTest#testTaggedTemplateLit_simpleTemplate
#[test]
fn test_tagged_template_lit_simple_template() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("foo`Simple`");
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("Math.sin`Simple`", "");
    t.fold("1 + Math.sin`Simple`", "");
}

// port: PeepholeRemoveDeadCodeTest#testTaggedTemplateLit_substitutingTemplate
#[test]
fn test_tagged_template_lit_substituting_template() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("foo`Complex ${butSafe}`");
    // We use a function with no side-effects, otherwise the entire invocation would be preserved.
    t.fold("Math.sin`Complex ${butSafe}`", "");
    t.fold("Math.sin`Complex ${andDangerous()}`", "andDangerous()");
}

// port: PeepholeRemoveDeadCodeTest#testFoldAssign
#[test]
fn test_fold_assign() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("x=x", "");
    t.fold_same("x=xy");
    t.fold_same("x=x + 1");
    t.fold_same("x.a=x.a");
    t.fold("var y=(x=x)", "var y=x");
    t.fold("y=1 + (x=x)", "y=1 + x");
}

// port: PeepholeRemoveDeadCodeTest#testTryCatchFinally
#[test]
fn test_try_catch_finally() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("try {foo()} catch (e) {bar()}");
    t.fold_same("try { try {foo()} catch (e) {bar()}} catch (x) {bar()}");
    t.fold("try {var x = 1} finally {}", "var x = 1;");
    t.fold_same("try {var x = 1} finally {x()}");
    t.fold(
        "function f() { return; try{ var x = 1; }finally{} }",
        "function f() { var x; return; }",
    );
    t.fold("try {} finally {x()}", "x()");
    t.fold("try {} catch (e) { bar()} finally {x()}", "x()");
    t.fold("try {} catch (e) { bar()}", "");
    t.fold(
        "try {} catch (e) { var a = 0; } finally {x()}",
        "var a; x()",
    );
    t.fold("try {} catch (e) {}", "");
    t.fold("try {} finally {}", "");
    t.fold("try {} catch (e) {} finally {}", "");
    t.fold("L1:try {} catch (e) {} finally {}", "");
    t.fold("L2:L1:try {} catch (e) {} finally {}", "");
}

// port: PeepholeRemoveDeadCodeTest#testObjectLiteral
#[test]
fn test_object_literal() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("({})", "");
    t.fold("({a:1})", "");
    t.fold("({a:foo()})", "foo()");
    t.fold("({'a':foo()})", "foo()");
    // Object-spread may tigger getters.
    t.fold_same("({...a})");
    t.fold_same("({...foo()})");
}

// port: PeepholeRemoveDeadCodeTest#testArrayLiteral
#[test]
fn test_array_literal() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("([])", "");
    t.fold("([1])", "");
    t.fold("([a])", "");
    t.fold("([foo()])", "foo()");
}

// port: PeepholeRemoveDeadCodeTest#testArrayLiteral_containingSpread
#[test]
fn test_array_literal_containing_spread() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("([...c])");
    t.fold("([4, ...c, a])", "([...c])");
    t.fold("([foo(), ...c, bar()])", "(foo(), [...c], bar())");
    t.fold("([...a, b, ...c])", "([...a], [...c])");
    t.fold_same("([...b, ...c])");
    // It would also be fine if the spreads were split apart.
}

// port: PeepholeRemoveDeadCodeTest#testAwait
#[test]
fn test_await() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("async function f() { await something(); }");
    t.fold_same("async function f() { await some.thing(); }");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyPatternInDeclarationRemoved
#[test]
fn test_empty_pattern_in_declaration_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("var [] = [];", "");
    t.fold("let [] = [];", "");
    t.fold("const [] = [];", "");
    t.fold("var {} = [];", "");
    t.fold("var [] = foo();", "foo()");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyArrayPatternInAssignRemoved
#[test]
fn test_empty_array_pattern_in_assign_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("({} = {});", "");
    t.fold("({} = foo());", "foo()");
    t.fold("[] = [];", "");
    t.fold("[] = foo();", "foo()");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyPatternInParamsNotRemoved
#[test]
fn test_empty_pattern_in_params_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f([], a) {}");
    t.fold_same("function f({}, a) {}");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyPatternInForOfLoopNotRemoved
#[test]
fn test_empty_pattern_in_for_of_loop_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("for (let [] of foo()) {}");
    t.fold_same("for (const [] of foo()) {}");
    t.fold_same("for ([] of foo()) {}");
    t.fold_same("for ({} of foo()) {}");
}

// port: PeepholeRemoveDeadCodeTest#testEmptySlotInArrayPatternRemoved
#[test]
fn test_empty_slot_in_array_pattern_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("[,,] = foo();", "foo()");
    t.fold("[a,b,,] = foo();", "[a,b] = foo();");
    t.fold("[a,[],b,[],[]] = foo();", "[a,[],b] = foo();");
    t.fold("[a,{},b,{},{}] = foo();", "[a,{},b] = foo();");
    t.fold("function f([,,,]) {}", "function f([]) {}");
    t.fold_same("[[], [], [], ...rest] = foo()");
}

// port: PeepholeRemoveDeadCodeTest#testEmptySlotInArrayPatternWithDefaultValueMaybeRemoved
#[test]
fn test_empty_slot_in_array_pattern_with_default_value_maybe_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("[a,[] = 0] = [];", "[a] = [];");
    t.fold_same("[a,[] = foo()] = [];");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyKeyInObjectPatternRemoved
#[test]
fn test_empty_key_in_object_pattern_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("const {f: {}} = {};", "");
    t.fold("const {f: []} = {};", "");
    t.fold("const {f: {}, g} = {};", "const {g} = {};");
    t.fold("const {f: [], g} = {};", "const {g} = {};");
    t.fold_same("const {[foo()]: {}} = {};");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyKeyInObjectPatternWithDefaultValueMaybeRemoved
#[test]
fn test_empty_key_in_object_pattern_with_default_value_maybe_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("const {f: {} = 0} = {};", "");
    // In theory the following case could be reduced to `foo()`, but that gets more complicated to
    // implement for object patterns with multiple keys with side effects.
    // Instead the pass backs off for any default with a possible side effect
    t.fold_same("const {f: {} = foo()} = {};");
}

// port: PeepholeRemoveDeadCodeTest#testEmptyKeyInObjectPatternNotRemovedWithObjectRest
#[test]
fn test_empty_key_in_object_pattern_not_removed_with_object_rest() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("const {f: {}, ...g} = foo()");
    t.fold_same("const {f: [], ...g} = foo()");
}

// port: PeepholeRemoveDeadCodeTest#testUndefinedDefaultParameterRemoved
#[test]
fn test_undefined_default_parameter_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "function f(x=undefined,y) {  }",
        "function f(x,y)             {  }",
    );
    t.fold(
        "function f(x,y=undefined,z) {  }",
        "function f(x,y          ,z) {  }",
    );
    t.fold(
        "function f(x=undefined,y=undefined,z=undefined) {  }",
        "function f(x,          y,          z)           {  }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testPureVoidDefaultParameterRemoved
#[test]
fn test_pure_void_default_parameter_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("function f(x = void 0) {  }", "function f(x         ) {  }");
    t.fold(
        "function f(x = void \"XD\") {  }",
        "function f(x              ) {  }",
    );
    t.fold(
        "function f(x = void f()) {  }",
        "function f(x)            {  }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testNoDefaultParameterNotRemoved
#[test]
fn test_no_default_parameter_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f(x,y) {  }");
    t.fold_same("function f(x) {  }");
    t.fold_same("function f() {  }");
}

// port: PeepholeRemoveDeadCodeTest#testEffectfulDefaultParameterNotRemoved
#[test]
fn test_effectful_default_parameter_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("function f(x = void console.log(1)) {  }");
    t.fold_same("function f(x = void f()) { alert(x); }");
}

// port: PeepholeRemoveDeadCodeTest#testDestructuringUndefinedDefaultParameter
#[test]
fn test_destructuring_undefined_default_parameter() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "function f({a=undefined,b=1,c}) {  }",
        "function f({a          ,b=1,c}) {  }",
    );
    t.fold(
        "function f({a={},b=0}=undefined) {  }",
        "function f({a={},b=0}) {  }",
    );
    t.fold(
        "function f({a=undefined,b=0}) {  }",
        "function f({a,b=0}) {  }",
    );
    t.fold(
        " function f({a: {b = undefined}}) {  }",
        " function f({a: {b}}) {  }",
    );
    t.fold_same("function f({a,b}) {  }");
    t.fold_same("function f({a=0, b=1}) {  }");
    t.fold_same("function f({a=0,b=0}={}) {  }");
    t.fold_same("function f({a={},b=0}={}) {  }");
}

// port: PeepholeRemoveDeadCodeTest#testUndefinedDefaultObjectPatterns
#[test]
fn test_undefined_default_object_patterns() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("const {a = undefined} = obj;", "const {a} = obj;");
    t.fold("const {a = void 0} = obj;", "const {a} = obj;");
}

// port: PeepholeRemoveDeadCodeTest#testBatchC_destructuringOptimization
#[test]
fn test_batch_c_destructuring_optimization() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // OPP-015: Pure undefined / void default values
    t.fold(
        "const {a = void 0, b = undefined} = obj;",
        "const {a, b} = obj;",
    );
    t.fold(
        "const [a = void 0, b = undefined] = arr;",
        "const [a, b] = arr;",
    );
    // Empty nested patterns in object patterns
    t.fold("const {a: {}, b: {}} = obj;", "");
    t.fold("const {a: {}, b} = obj;", "const {b} = obj;");
    t.fold("const {a: [], b} = obj;", "const {b} = obj;");
    // Trailing empty slots and empty nested patterns in array patterns
    t.fold("const [a, , ,] = arr;", "const [a] = arr;");
    t.fold("const [a, [], {}] = arr;", "const [a] = arr;");
    t.fold("let [a, [] = 0] = arr;", "let [a] = arr;");
    // Guard cases: Object rest property exclusion, effectful defaults, computed keys
    t.fold_same("const {a: {}, ...rest} = obj;");
    t.fold_same("const {a: [] = foo()} = obj;");
    t.fold_same("const {[foo()]: {}} = obj;");
    t.fold_same("function f({}, x) {}");
    t.fold_same("for (const {} of iter) {}");
    t.fold_same("const [, a] = arr;");
}

// port: PeepholeRemoveDeadCodeTest#testDoNotRemoveGetterOnlyAccess
#[test]
fn test_do_not_remove_getter_only_access() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  get property() {}\n};\na.property;\n");
    t.fold_same("var a = {\n  get property() {}\n};\na?.property;\n");
    t.fold_same(
        "var a = {};\nObject.defineProperty(a, 'property', {\n  get() {}\n});\na.property;\n",
    );
    t.fold_same(
        "var a = {};\nObject.defineProperty(a, 'property', {\n  get() {}\n});\na?.property;\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testDoNotRemoveNestedGetterOnlyAccess
#[test]
fn test_do_not_remove_nested_getter_only_access() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  b: { get property() {} }\n};\na.b.property;\n");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveAfterNestedGetterOnlyAccess
#[test]
fn test_remove_after_nested_getter_only_access() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "var a = {\n  b: { get property() {} }\n};\na.b.property.d.e;\n",
        "var a = {\n  b: { get property() {} }\n};\na.b.property;\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testFoldLabelledEmptyBlock
#[test]
fn test_fold_labelled_empty_block() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("a:{}", "");
    t.fold("a:b:{}", "");
    t.fold("a:b:c:{}", "");
}

// port: PeepholeRemoveDeadCodeTest#testRetainSetterOnlyAccess
#[test]
fn test_retain_setter_only_access() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  set property(v) {}\n};\na.property;\n");
    t.fold_same("var a = {\n  set property(v) {}\n};\na?.property;\n");
}

// port: PeepholeRemoveDeadCodeTest#testDoNotRemoveGetterSetterAccess
#[test]
fn test_do_not_remove_getter_setter_access() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  get property() {},\n  set property(x) {}\n};\na.property;\n");
}

// port: PeepholeRemoveDeadCodeTest#testDoNotRemoveSetSetterToGetter
#[test]
fn test_do_not_remove_set_setter_to_getter() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same(
        "var a = {\n  get property() {},\n  set property(x) {}\n};\na.property = a.property;\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testDoNotRemoveAccessIfOtherPropertyIsGetter
#[test]
fn test_do_not_remove_access_if_other_property_is_getter() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  get property() {}\n};\nvar b = {\n  property: 0,\n};\n// This pass should be conservative and not remove this since it sees a getter for\n// \"property\"\nb.property;\n");
    t.fold_same("var a = {};\nObject.defineProperty(a, 'property', {\n  get() {}\n});\nvar b = {\n  property: 0,\n};\nb.property;\n");
}

// port: PeepholeRemoveDeadCodeTest#testFunctionCallReferencesGetterIsNotRemoved
#[test]
fn test_function_call_references_getter_is_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same("var a = {\n  get property() {}\n};\nfunction foo() { a.property; }\nfoo();\n");
}

// port: PeepholeRemoveDeadCodeTest#testFunctionCallReferencesSetterIsNotRemoved
#[test]
fn test_function_call_references_setter_is_not_removed() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold_same(
        "var a = {\n  set property(v) {}\n};\nfunction foo() { a.property = 0; }\nfoo();\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testClassField
#[test]
fn test_class_field() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("class C {\n  f1 = (5,2);\n}\n", "class C {\n  f1 = 2;\n}\n");
}

// port: PeepholeRemoveDeadCodeTest#testThis
#[test]
fn test_this() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold(
        "class C {\n  constructor() {\n    this.f1 = (5,2);\n  }\n}\n",
        "class C {\n  constructor() {\n    this.f1 = 2;\n  }\n}\n",
    );
}

// port: PeepholeRemoveDeadCodeTest#testClassStaticBlock
#[test]
fn test_class_static_block() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("class C {\n  static {\n  }\n}\n", "class C {\n}\n");
    t.fold_same("class C {\n  static {\n    this.x = 0;\n  }\n}\n");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnreachableOptionalChainingCall
#[test]
fn test_remove_unreachable_optional_chaining_call() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.fold("(null)?.();", "");
    t.fold("(void 0)?.();", "");
    t.fold("(undefined)?.();", "");
    t.fold("(void 0)?.(0)", "");
    t.fold("(void 0)?.(function f() {})", "");
    t.fold("(null)?.x;", "");
    t.fold("(void 0)?.x;", "");
    t.fold("(null)?.['x'];", "");
    t.fold("(void 0)?.['x'];", "");
    t.fold("(null)?.[x];", "");
    t.fold("(void 0)?.[x];", "");
    // arguments with unknown side effects are also removed
    t.fold("(void 0)?.(f(), g())", "");
    // void arguments with unknown side effects are preserved
    t.fold("(void f())?.();", "f();");
    t.fold("g((void f())?.());", "g(void f());");
    t.fold_same("(f(), null)?.()");
    t.fold_same("f?.()");
    t.fold("a?.x;", "");
    t.fold("a?.['x'];", "");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveUnusedVoid
#[test]
fn test_remove_unused_void() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // remove void at statement level
    t.fold("void 0;", "");
    t.fold("void foo();", "foo();");
    // preserve void when passed somewhere else
    t.fold_same("use(void 0);");
    t.fold_same("use(void foo());");
    t.fold_same("use(() => void foo());");
    t.fold("void use(() => void foo());", "use(() => void foo());");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveDeadStatements1
#[test]
fn test_remove_dead_statements1() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test("throw 1; x;", "throw 1;");
    t.test("throw 1; alert(1)", "throw 1;");
    t.test("throw 1; var x = 1", "var x; throw 1;");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveDeadStatements2
#[test]
fn test_remove_dead_statements2() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test_in_fn("return; x;", "return;");
    t.test_in_fn("return; alert(1)", "return;");
    t.test_in_fn("return; var x = 1", "var x; return;");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveDeadStatements3
#[test]
fn test_remove_dead_statements3() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test_in_loop("break; x;", "break;");
    t.test_in_loop("break; alert(1)", "break;");
    t.test_in_loop_with_expected_before_loop("break; var x = 1", "var x;", "break;");
}

// port: PeepholeRemoveDeadCodeTest#testRemoveDeadStatements4
#[test]
fn test_remove_dead_statements4() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test_in_loop("continue; x;", "continue;");
    t.test_in_loop("continue; alert(1)", "continue;");
    t.test_in_loop_with_expected_before_loop("continue; var x = 1", "var x;", "continue;");
}

// port: PeepholeRemoveDeadCodeTest#testRemovalRequiresRedeclaration
#[test]
fn test_removal_requires_redeclaration() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test("while(1) { break; var x = 1}", "var x; for(;;) { break }");
    t.test(
        "while(1) { break; var x=1; var y=1 }",
        "var y; var x; for(;;) { break }",
    );
    t.test(
        "while(1) { break; var [x, [[[y]]]] = [];}",
        "var y; var x; for(;;) { break }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testRemovalRequiresRedeclaration_normalizeDisabled
#[test]
fn test_removal_requires_redeclaration_normalize_disabled() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.harness.disable_normalize().unwrap();
    t.harness.disable_compute_side_effects().unwrap();
    t.test(
        "while(1) { break; var x = 1}",
        "var x; while (1) { break; }",
    );
    t.test(
        "while(1) { break; var x=1; var y=1 }",
        "var y; var x; while (1) { break; }",
    );
    t.test(
        "while(1) { break; var [x, [[[y]]]] = [];}",
        "var y; var x; while (1) { break; }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testRemoveDo
#[test]
fn test_remove_do() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test(
        "do { print(1); break } while(1)",
        "do { print(1); break } while(1)",
    );
    t.test(
        "while(1) { break; do { print(1); break } while(1) }",
        "for (;;) { break; }",
    );
}

// port: PeepholeRemoveDeadCodeTest#testSwitchCase
#[test]
fn test_switch_case() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    t.test(
        "function f() { switch(x) { case 1: break; default: return 5; foo()}}",
        "function f() { switch(x) { case 1: break; default: return 5;}}",
    );
    t.test(
        "function f() { switch(x) { default: return; case 1: foo(); bar()}}",
        "function f() { switch(x) { default: return; case 1: foo(); bar()}}",
    );
    t.test(
        "function f() { switch(x) { default: return; case 1: return 5;bar()}}",
        "function f() { switch(x) { default: return; case 1: return 5;}}",
    );
}

// port: PeepholeRemoveDeadCodeTest#testBatchE_switchConstantDiscriminantFolding
#[test]
fn test_batch_e_switch_constant_discriminant_folding() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // OPP-023: Switch Constant Discriminant Folding & Dead Case Pruning
    t.fold("switch ('foo') {\n  case 'foo':\n    foo();\n    break;\n  case 'bar':\n    bar();\n    break;\n}\n", "foo();");
    t.fold("switch ('noMatch') {\n  case 'foo':\n    foo();\n    break;\n  case 'bar':\n    bar();\n    break;\n}\n", "");
    t.fold("switch(1){case 2: var x=0;}", "var x;");
    // Guard cases
    t.fold_same("switch (x) { case 1: foo(); break; }");
    t.fold_same("function f() { switch(a) { case 1: foo(); } }");
    t.fold_same("function f() { switch(a) { default: return; case 1: break; } }");
}

// port: PeepholeRemoveDeadCodeTest#testBatchE_pureExpressionStatementAndBlockElimination
#[test]
fn test_batch_e_pure_expression_statement_and_block_elimination() {
    let mut t = PeepholeRemoveDeadCodeTest::new();
    // OPP-024: Pure Expression Statement & Block Elimination
    t.fold("{ 'hi' }", "");
    t.fold("{ x == 3 }", "");
    t.fold("{ `hello ${foo}` }", "");
    t.fold("{ (function(){ x++; }) }", "");
    t.fold("{{ foo() }}", "foo()");
    t.fold("{ foo(); {} }", "foo()");
    t.fold("{{ foo() } { bar() }}", "foo(); bar()");
    // Guard cases: side-effectful statements and declarations
    t.fold_same("foo();");
    t.fold_same("function f() { return 3; }");
    t.fold_same("for (let x = 1; x < 10; x++) {}");
}
