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
//   test/com/google/javascript/jscomp/PeepholeMinimizeConditionsTest.java.

//! Port of `PeepholeMinimizeConditionsTest.java`: tests for PeepholeMinimizeConditions in isolation.
//! Tests for the interaction of multiple peephole passes are in PeepholeIntegrationTest.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, DEFAULT_EXTERNS},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct PeepholeMinimizeConditionsTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    late: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeMinimizeConditionsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let late = self.late;
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    vec![Box::new(PeepholeMinimizeConditions::new(late))];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.set_retraverse_on_change(false);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeMinimizeConditionsTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeMinimizeConditionsTest {
    // port: PeepholeMinimizeConditionsTest#PeepholeMinimizeConditionsTest
    // port: PeepholeMinimizeConditionsTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(DEFAULT_EXTERNS.clone().unwrap());
        harness.set_up();
        let late = true;
        harness.disable_type_check().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeMinimizeConditionsTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                late,
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

    // port: PeepholeMinimizeConditionsTest#foldSame
    fn fold_same(&mut self, js: &str) {
        self.test_same(js);
    }

    // port: PeepholeMinimizeConditionsTest#fold
    fn fold(&mut self, js: &str, expected: &str) {
        self.test(js, expected);
    }
}

/// Try to minimize assignments
// port: PeepholeMinimizeConditionsTest#testFoldAssignments
#[test]
fn test_fold_assignments() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("function f(){if(x)y=3;else y=4;}", "function f(){y=x?3:4}");
    t.fold(
        "function f(){if(x)y=1+a;else y=2+a;}",
        "function f(){y=x?1+a:2+a}",
    );
    // and operation assignments
    t.fold(
        "function f(){if(x)y+=1;else y+=2;}",
        "function f(){y+=x?1:2}",
    );
    t.fold(
        "function f(){if(x)y-=1;else y-=2;}",
        "function f(){y-=x?1:2}",
    );
    t.fold(
        "function f(){if(x)y%=1;else y%=2;}",
        "function f(){y%=x?1:2}",
    );
    t.fold(
        "function f(){if(x)y|=1;else y|=2;}",
        "function f(){y|=x?1:2}",
    );
    // Don't fold if the 2 ops don't match.
    t.fold_same("function f(){x ? y-=1 : y+=2}");
    // Don't fold if the 2 LHS don't match.
    t.fold_same("function f(){x ? y-=1 : z-=1}");
    // Don't fold if there are potential effects.
    t.fold_same("function f(){x ? y().a=3 : y().a=4}");
}

// port: PeepholeMinimizeConditionsTest#testDontRemoveDuplicateStatementsWithoutNormalization
#[test]
fn test_dont_remove_duplicate_statements_without_normalization() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // In the following test case, we can't remove the duplicate "alert(x);" lines since each "x"
    // refers to a different variable.
    // We only try removing duplicate statements if the AST is normalized and names are unique.
    t.test_same(
        "if (Math.random() < 0.5) { const x = 3; alert(x); } else { const x = 5; alert(x); }",
    );
}

// port: PeepholeMinimizeConditionsTest#testDontFoldDirectAndIndirectEval
#[test]
fn test_dont_fold_direct_and_indirect_eval() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "if (a) { eval(x); } else { (0, eval)(x); }",
        "a ? eval(x) : (0, eval)(x);",
    );
}

// port: PeepholeMinimizeConditionsTest#testNotCond
#[test]
fn test_not_cond() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("function f(){if(!x)foo()}", "function f(){x||foo()}");
    t.fold("function f(){if(!x)b=1}", "function f(){x||(b=1)}");
    t.fold("if(!x)z=1;else if(y)z=2", "if(x){y&&(z=2);}else{z=1;}");
    t.fold("if(x)y&&(z=2);else z=1;", "x ? y&&(z=2) : z=1");
    t.fold(
        "function f(){if(!(x=1))a.b=1}",
        "function f(){(x=1)||(a.b=1)}",
    );
}

// port: PeepholeMinimizeConditionsTest#testAndParenthesesCount
#[test]
fn test_and_parentheses_count() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "function f(){if(x||y)a.foo()}",
        "function f(){(x||y)&&a.foo()}",
    );
    t.fold("function f(){if(x.a)x.a=0}", "function f(){x.a&&(x.a=0)}");
    t.fold("function f(){if(x?.a)x.a=0}", "function f(){x?.a&&(x.a=0)}");
    t.fold_same("function f(){if(x()||y()){x()||y()}}");
}

// port: PeepholeMinimizeConditionsTest#testFoldLogicalOpStringCompare
#[test]
fn test_fold_logical_op_string_compare() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // side-effects
    // There is two way to parse two &&'s and both are correct.
    t.fold("if (foo() && false) z()", "(foo(), 0) && z()");
}

// port: PeepholeMinimizeConditionsTest#testFoldNot
#[test]
fn test_fold_not() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("while(!(x==y)){a=b;}", "while(x!=y){a=b;}");
    t.fold("while(!(x!=y)){a=b;}", "while(x==y){a=b;}");
    t.fold("while(!(x===y)){a=b;}", "while(x!==y){a=b;}");
    t.fold("while(!(x!==y)){a=b;}", "while(x===y){a=b;}");
    // Because !(x<NaN) != x>=NaN don't fold < and > cases.
    t.fold_same("while(!(x>y)){a=b;}");
    t.fold_same("while(!(x>=y)){a=b;}");
    t.fold_same("while(!(x<y)){a=b;}");
    t.fold_same("while(!(x<=y)){a=b;}");
    t.fold_same("while(!(x<=NaN)){a=b;}");
    // NOT forces a boolean context
    t.fold("x = !(y() && true)", "x = !y()");
    // This will be further optimized by PeepholeFoldConstants.
    t.fold("x = !true", "x = !1");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeExprCondition
#[test]
fn test_minimize_expr_condition() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("(x ? true : false) && y()", "x&&y()");
    t.fold("(x ? false : true) && y()", "(!x)&&y()");
    t.fold("(x ? true : y) && y()", "(x || y)&&y()");
    t.fold("(x ? y : false) && y()", "(x && y)&&y()");
    t.fold("(x && true) && y()", "x && y()");
    t.fold("(x && false) && y()", "0&&y()");
    t.fold("(x || true) && y()", "1&&y()");
    t.fold("(x || false) && y()", "x&&y()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeWhileCondition
#[test]
fn test_minimize_while_condition() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // This test uses constant folding logic, so is only here for completeness.
    t.fold("while(!!true) foo()", "while(1) foo()");
    // These test tryMinimizeCondition
    t.fold("while(!!x) foo()", "while(x) foo()");
    t.fold("while(!(!x&&!y)) foo()", "while(x||y) foo()");
    t.fold("while(x||!!y) foo()", "while(x||y) foo()");
    t.fold("while(!(!!x&&y)) foo()", "while(!x||!y) foo()");
    t.fold("while(!(!x&&y)) foo()", "while(x||!y) foo()");
    t.fold("while(!(x||!y)) foo()", "while(!x&&y) foo()");
    t.fold("while(!(x||y)) foo()", "while(!x&&!y) foo()");
    t.fold("while(!(!x||y-z)) foo()", "while(x&&!(y-z)) foo()");
    t.fold("while(!(!(x/y)||z+w)) foo()", "while(x/y&&!(z+w)) foo()");
    t.fold_same("while(!(x+y||z)) foo()");
    t.fold_same("while(!(x&&y*z)) foo()");
    t.fold("while(!(!!x&&y)) foo()", "while(!x||!y) foo()");
    t.fold("while(x&&!0) foo()", "while(x) foo()");
    t.fold("while(x||!1) foo()", "while(x) foo()");
    t.fold("while(!((x,y)&&z)) foo()", "while((x,!y)||!z) foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorganRemoveLeadingNot
#[test]
fn test_minimize_demorgan_remove_leading_not() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if(!(!a||!b)&&c) foo()", "((a&&b)&&c)&&foo()");
    t.fold("if(!(x&&y)) foo()", "x&&y||foo()");
    t.fold("if(!(x||y)) foo()", "(x||y)||foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan1
#[test]
fn test_minimize_demorgan1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if(!a&&!b)foo()", "(a||b)||foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan2
#[test]
fn test_minimize_demorgan2() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // Make sure trees with cloned functions are marked as changed
    t.fold(
        "(!(a&&!((function(){})())))||foo()",
        "!a||(function(){})()||foo()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan2b
#[test]
fn test_minimize_demorgan2b() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // Make sure unchanged trees with functions are not marked as changed
    t.fold_same("!a||(function(){})()||foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan3
#[test]
fn test_minimize_demorgan3() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if((!a||!b)&&(c||d)) foo()", "(a&&b||!c&&!d)||foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan5
#[test]
fn test_minimize_demorgan5() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if((!a||!b)&&c) foo()", "(a&&b||!c)||foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan11
#[test]
fn test_minimize_demorgan11() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "if (x && (y===2 || !f()) && (y===3 || !h())) foo()",
        "(!x || y!==2 && f() || y!==3 && h()) || foo()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan20a
#[test]
fn test_minimize_demorgan20a() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "if (0===c && (2===a || 1===a)) f(); else g()",
        "if (0!==c || 2!==a && 1!==a) g(); else f()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan20b
#[test]
fn test_minimize_demorgan20b() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "if (0!==c || 2!==a && 1!==a) g(); else f()",
        "(0!==c || 2!==a && 1!==a) ? g() : f()",
    );
}

// port: PeepholeMinimizeConditionsTest#testPreserveIf
#[test]
fn test_preserve_if() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold_same("if(!a&&!b)for(;f(););");
}

// port: PeepholeMinimizeConditionsTest#testNoSwapWithDanglingElse
#[test]
fn test_no_swap_with_dangling_else() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold_same("if(!x) {for(;;)foo(); for(;;)bar()} else if(y) for(;;) f()");
    t.fold_same("if(!a&&!b) {for(;;)foo(); for(;;)bar()} else if(y) for(;;) f()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeHook
#[test]
fn test_minimize_hook() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("x ? x : y", "x || y");
    // We assume GETPROPs don't have side effects.
    t.fold("x.y ? x.y : x.z", "x.y || x.z");
    t.fold("x?.y ? x?.y : x.z", "x?.y || x.z");
    t.fold("x?.y ? x?.y : x?.z", "x?.y || x?.z");
    // This can be folded if x() does not have side effects.
    t.fold_same("x() ? x() : y()");
    t.fold_same("x?.() ? x?.() : y()");
    t.fold("!x ? foo() : bar()", "x ? bar() : foo()");
    t.fold("while(!(x ? y : z)) foo();", "while(x ? !y : !z) foo();");
    t.fold(
        "(x ? !y : !z) ? foo() : bar()",
        "(x ? y : z) ? bar() : foo()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeComma
#[test]
fn test_minimize_comma() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "while(!(inc(), test())) foo();",
        "while(inc(), !test()) foo();",
    );
    t.fold(
        "(inc(), !test()) ? foo() : bar()",
        "(inc(), test()) ? bar() : foo()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeExprResult
#[test]
fn test_minimize_expr_result() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("!x||!y", "x&&y");
    t.fold("if(!(x&&!y)) foo()", "(!x||y)&&foo()");
    t.fold("if(!x||y) foo()", "(!x||y)&&foo()");
    t.fold("(!x||y)&&foo()", "x&&!y||!foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeDemorgan21
#[test]
fn test_minimize_demorgan21() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "if (0===c && (2===a || 1===a)) f()",
        "(0!==c || 2!==a && 1!==a) || f()",
    );
}

// port: PeepholeMinimizeConditionsTest#testMinimizeAndOr1
#[test]
fn test_minimize_and_or1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if ((!a || !b) && (d || e)) f()", "(a&&b || !d&&!e) || f()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeForCondition
#[test]
fn test_minimize_for_condition() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // This test uses constant folding logic, so is only here for completeness.
    // These could be simplified to "for(;;) ..."
    t.fold("for(;!!true;) foo()", "for(;1;) foo()");
    // Verify function deletion tracking.
    t.fold("if(!!true||function(){}) {}", "if(1) {}");
    // Don't bother with FOR inits as there are normalized out.
    t.fold("for(!!true;;) foo()", "for(!0;;) foo()");
    // These test tryMinimizeCondition
    t.fold("for(;!!x;) foo()", "for(;x;) foo()");
    t.fold_same("for(a in b) foo()");
    t.fold_same("for(a in {}) foo()");
    t.fold_same("for(a in []) foo()");
    t.fold("for(a in !!true) foo()", "for(a in !0) foo()");
    t.fold_same("for(a of b) foo()");
    t.fold_same("for(a of {}) foo()");
    t.fold_same("for(a of []) foo()");
    t.fold("for(a of !!true) foo()", "for(a of !0) foo()");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeCondition_example1
#[test]
fn test_minimize_condition_example1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // Based on a real failing code sample.
    t.fold(
        "if(!!(f() > 20)) {foo();foo()}",
        "if(f() > 20){foo();foo()}",
    );
}

// port: PeepholeMinimizeConditionsTest#testFoldLoopBreakLate
#[test]
fn test_fold_loop_break_late() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = true;
    t.fold("for(;;) if (a) break", "for(;!a;);");
    t.fold_same("for(;;) if (a) { f(); break }");
    t.fold("for(;;) if (a) break; else f()", "for(;!a;) { { f(); } }");
    t.fold("for(;a;) if (b) break", "for(;a && !b;);");
    t.fold(
        "for(;a;) { if (b) break; if (c) break; }",
        "for(;(a && !b);) if (c) break;",
    );
    t.fold("for(;(a && !b);) if (c) break;", "for(;(a && !b) && !c;);");
    t.fold(
        "for(;;) { if (foo) { break; var x; } } x;",
        "var x; for(;!foo;) {} x;",
    );
    // 'while' is normalized to 'for'
    t.harness.enable_normalize().unwrap();
    t.fold("while(true) if (a) break", "for(;1&&!a;);");
    t.harness.disable_normalize().unwrap();
}

// port: PeepholeMinimizeConditionsTest#testFoldLoopBreakEarly
#[test]
fn test_fold_loop_break_early() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.fold_same("for(;;) if (a) break");
    t.fold_same("for(;;) if (a) { f(); break }");
    t.fold_same("for(;;) if (a) break; else f()");
    t.fold_same("for(;a;) if (b) break");
    t.fold_same("for(;a;) { if (b) break; if (c) break; }");
    t.fold_same("while(1) if (a) break");
    t.harness.enable_normalize().unwrap();
    t.fold_same("for (; 1; ) if (a) break");
}

// port: PeepholeMinimizeConditionsTest#testFoldConditionalVarDeclaration
#[test]
fn test_fold_conditional_var_declaration() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if(x) var y=1;else y=2", "var y=x?1:2");
    t.fold("if(x) y=1;else var y=2", "var y=x?1:2");
    t.fold_same("if(x) var y = 1; z = 2");
    t.fold_same("if(x||y) y = 1; var z = 2");
    t.fold_same("if(x) { var y = 1; print(y)} else y = 2 ");
    t.fold_same("if(x) var y = 1; else {y = 2; print(y)}");
}

// port: PeepholeMinimizeConditionsTest#testFoldIfWithLowerOperatorsInside
#[test]
fn test_fold_if_with_lower_operators_inside() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if (x + (y=5)) z && (w,z);", "x + (y=5) && (z && (w,z))");
    t.fold("if (!(x+(y=5))) z && (w,z);", "x + (y=5) || z && (w,z)");
    t.fold(
        "if (x + (y=5)) if (z && (w,z)) for(;;) foo();",
        "if (x + (y=5) && (z && (w,z))) for(;;) foo();",
    );
}

// port: PeepholeMinimizeConditionsTest#testNestedIfCombine
#[test]
fn test_nested_if_combine() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if(x)if(y){while(1){}}", "if(x&&y){while(1){}}");
    t.fold("if(x||z)if(y){while(1){}}", "if((x||z)&&y){while(1){}}");
    t.fold("if(x)if(y||z){while(1){}}", "if((x)&&(y||z)){while(1){}}");
    t.fold_same("if(x||z)if(y||z){while(1){}}");
    t.fold("if(x)if(y){if(z){while(1){}}}", "if(x&&(y&&z)){while(1){}}");
}

// port: PeepholeMinimizeConditionsTest#testIssue291
#[test]
fn test_issue291() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold("if (true) { f.onchange(); }", "if (1) f.onchange();");
    t.fold_same("if (f) { f.onchange(); }");
    t.fold_same("if (f) { f.bar(); } else { f.onchange(); }");
    t.fold("if (f) { f.bonchange(); }", "f && f.bonchange();");
    t.fold_same("if (f) { f['x'](); }");
    // optional versions
    t.fold("if (true) { f?.onchange(); }", "if (1) f?.onchange();");
    t.fold_same("if (f) { f?.onchange(); }");
    t.fold_same("if (f) { f?.bar(); } else { f?.onchange(); }");
    t.fold("if (f) { f?.bonchange(); }", "f && f?.bonchange();");
    t.fold_same("if (f) { f?.['x'](); }");
}

// port: PeepholeMinimizeConditionsTest#testObjectLiteral
#[test]
fn test_object_literal() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test("({})", "1");
    t.test("({a:1})", "1");
    t.test_same("({a:foo()})");
    t.test_same("({'a':foo()})");
}

// port: PeepholeMinimizeConditionsTest#testArrayLiteral
#[test]
fn test_array_literal() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test("([])", "1");
    t.test("([1])", "1");
    t.test("([a])", "1");
    t.test_same("([foo()])");
}

// port: PeepholeMinimizeConditionsTest#testRemoveElseCause3
#[test]
fn test_remove_else_cause3() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test_same("function f() { a:{if (x) break a; else f() } }");
    t.test_same("function f() { if (x) { a:{ break a } } else f() }");
    t.test_same("function f() { if (x) a:{ break a } else f() }");
}

// port: PeepholeMinimizeConditionsTest#testIssue925
#[test]
fn test_issue925() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test(
        "if (x[--y] === 1) {\n    x[y] = 0;\n} else {\n    x[y] = 1;\n}\n",
        "(x[--y] === 1) ? x[y] = 0 : x[y] = 1;",
    );
    t.test(
        "if (x[--y]) {\n    a = 0;\n} else {\n    a = 1;\n}\n",
        "a = (x[--y]) ? 0 : 1;",
    );
    t.test(
        "if (x?.[--y]) {\n    a = 0;\n} else {\n    a = 1;\n}\n",
        "a = (x?.[--y]) ? 0 : 1;",
    );
    t.test(
        "if (x++) { x += 2 } else { x += 3 }",
        "x++ ? x += 2 : x += 3",
    );
    t.test(
        "if (x++) { x = x + 2 } else { x = x + 3 }",
        "x = x++ ? x + 2 : x + 3",
    );
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_disabled
#[test]
fn test_coercion_substitution_disabled() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {}; if (x != null) throw 'a';");
    t.test_same("var x = {}; var y = x != null;");
    t.test_same("var x = 1; if (x != 0) throw 'a';");
    t.test_same("var x = 1; var y = x != 0;");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_booleanResult0
#[test]
fn test_coercion_substitution_boolean_result0() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {}; var y = x != null;");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_booleanResult1
#[test]
fn test_coercion_substitution_boolean_result1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {}; var y = x == null;");
    t.test_same("var x = {}; var y = x !== null;");
    t.test_same("var x = undefined; var y = x !== null;");
    t.test_same("var x = {}; var y = x === null;");
    t.test_same("var x = undefined; var y = x === null;");
    t.test_same("var x = 1; var y = x != 0;");
    t.test_same("var x = 1; var y = x == 0;");
    t.test_same("var x = 1; var y = x !== 0;");
    t.test_same("var x = 1; var y = x === 0;");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_if
#[test]
fn test_coercion_substitution_if() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test(
        "var x = {};\nif (x != null) throw 'a';\n",
        "var x={}; if (x!=null) throw 'a'",
    );
    t.test_same("var x = {};\nif (x == null) throw 'a';\n");
    t.test_same("var x = {};\nif (x !== null) throw 'a';\n");
    t.test_same("var x = {};\nif (x === null) throw 'a';\n");
    t.test_same("var x = {};\nif (null != x) throw 'a';\n");
    t.test_same("var x = {};\nif (null == x) throw 'a';\n");
    t.test_same("var x = {};\nif (null !== x) throw 'a';\n");
    t.test_same("var x = {};\nif (null === x) throw 'a';\n");
    t.test_same("var x = 1;\nif (x != 0) throw 'a';\n");
    t.test_same("var x = 1;\nif (x != 0) throw 'a';\n");
    t.test_same("var x = 1;\nif (x == 0) throw 'a';\n");
    t.test_same("var x = 1;\nif (x !== 0) throw 'a';\n");
    t.test_same("var x = 1;\nif (x === 0) throw 'a';\n");
    t.test_same("var x = 1;\nif (0 != x) throw 'a';\n");
    t.test_same("var x = 1;\nif (0 == x) throw 'a';\n");
    t.test_same("var x = 1;\nif (0 !== x) throw 'a';\n");
    t.test_same("var x = 1;\nif (0 === x) throw 'a';\n");
    t.test_same("var x = NaN;\nif (0 === x) throw 'a';\n");
    t.test_same("var x = NaN;\nif (x === 0) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_expression
#[test]
fn test_coercion_substitution_expression() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {}; x != null && alert('b');");
    t.test_same("var x = 1; x != 0 && alert('b');");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_hook
#[test]
fn test_coercion_substitution_hook() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {};\nvar y = x != null ? 1 : 2;\n");
    t.test_same("var x = 1;\nvar y = x != 0 ? 1 : 2;\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_not
#[test]
fn test_coercion_substitution_not() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test(
        "var x = {};\nvar y = !(x != null) ? 1 : 2;\n",
        "var x = {};\nvar y = (x == null) ? 1 : 2;\n",
    );
    t.test(
        "var x = 1;\nvar y = !(x != 0) ? 1 : 2;\n",
        "var x = 1;\nvar y = x == 0 ? 1 : 2;\n",
    );
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_while
#[test]
fn test_coercion_substitution_while() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {}; while (x != null) throw 'a';");
    t.test_same("var x = 1; while (x != 0) throw 'a';");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_unknownType
#[test]
fn test_coercion_substitution_unknown_type() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = /** @type {?} */ ({});\nif (x != null) throw 'a';\n");
    t.test_same("var x = /** @type {?} */ (1);\nif (x != 0) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_allType
#[test]
fn test_coercion_substitution_all_type() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = /** @type {*} */ ({});\nif (x != null) throw 'a';\n");
    t.test_same("var x = /** @type {*} */ (1);\nif (x != 0) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_primitivesVsNull
#[test]
fn test_coercion_substitution_primitives_vs_null() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = 0;\nif (x != null) throw 'a';\n");
    t.test_same("var x = '';\nif (x != null) throw 'a';\n");
    t.test_same("var x = false;\nif (x != null) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_nonNumberVsZero
#[test]
fn test_coercion_substitution_non_number_vs_zero() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = {};\nif (x != 0) throw 'a';\n");
    t.test_same("var x = '';\nif (x != 0) throw 'a';\n");
    t.test_same("var x = false;\nif (x != 0) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_boxedNumberVsZero
#[test]
fn test_coercion_substitution_boxed_number_vs_zero() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = new Number(0);\nif (x != 0) throw 'a';\n");
}

// port: PeepholeMinimizeConditionsTest#testCoercionSubstitution_boxedPrimitives
#[test]
fn test_coercion_substitution_boxed_primitives() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_type_check().unwrap();
    t.test_same("var x = new Number(); if (x != null) throw 'a';");
    t.test_same("var x = new String(); if (x != null) throw 'a';");
    t.test_same("var x = new Boolean();\nif (x != null) throw 'a';");
}

// port: PeepholeMinimizeConditionsTest#testBatchE_booleanAbsorptionAndContextSimplifications
#[test]
fn test_batch_e_boolean_absorption_and_context_simplifications() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // OPP-022: Boolean Absorption & Context Simplifications
    // Hook inversion on negated condition
    t.test("var res = !x ? a : b;", "var res = x ? b : a;");
    // if-statement minimization preserves Boolean(x) wrapper in current pass
    t.test("if (Boolean(x)) { foo(); }", "Boolean(x) && foo();");
    // Guard cases: non-constant / side-effectful / shadowed expressions
    t.test_same("while (Boolean(x)) foo();");
    t.test_same("var res = Boolean(x) ? a : b;");
    t.test_same("var res = !Boolean(x);");
    t.test_same("x = a || (a && b);");
    t.test_same("x = a && (a || b);");
    t.test_same("x = foo() || (foo() && b);");
    t.test_same("function f(Boolean) { Boolean(x) ? a : b; }");
}

// port: PeepholeMinimizeConditionsTest#testCombineIfs1
#[test]
fn test_combine_ifs1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "function f() {if (x) return 1; if (y) return 1}",
        "function f() {if (x||y) return 1;}",
    );
    t.fold(
        "function f() {if (x) return 1; if (y) foo(); else return 1}",
        "function f() {if ((!x)&&y) foo(); else return 1;}",
    );
}

// port: PeepholeMinimizeConditionsTest#testCombineIfs2
#[test]
fn test_combine_ifs2() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // combinable but not yet done
    t.fold_same("function f() {if (x) throw 1; if (y) throw 1}");
    // Can't combine, side-effect
    t.fold(
        "function f(){ if (x) g(); if (y) g() }",
        "function f(){ x&&g(); y&&g() }",
    );
    t.fold(
        "function f(){ if (x) g?.(); if (y) g?.() }",
        "function f(){ x&&g?.(); y&&g?.() }",
    );
    // Can't combine, side-effect
    t.fold(
        "function f(){ if (x) y = 0; if (y) y = 0; }",
        "function f(){ x&&(y = 0); y&&(y = 0); }",
    );
}

// port: PeepholeMinimizeConditionsTest#testCombineIfs3
#[test]
fn test_combine_ifs3() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold_same("function f() {if (x) return 1; if (y) {g();f()}}");
}

/// Check that removing blocks with 1 child works
// port: PeepholeMinimizeConditionsTest#testFoldOneChildBlocks
#[test]
fn test_fold_one_child_blocks() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.fold("function f(){if(x)a();x=3}", "function f(){x&&a();x=3}");
    t.fold("function f(){if(x)a?.();x=3}", "function f(){x&&a?.();x=3}");
    t.fold("function f(){if(x){a()}x=3}", "function f(){x&&a();x=3}");
    t.fold(
        "function f(){if(x){a?.()}x=3}",
        "function f(){x&&a?.();x=3}",
    );
    t.fold(
        "function f(){if(x){return 3}}",
        "function f(){if(x)return 3}",
    );
    t.fold("function f(){if(x){a()}}", "function f(){x&&a()}");
    t.fold(
        "function f(){if(x){throw 1}}",
        "function f(){if(x)throw 1;}",
    );
    // Try it out with functions
    t.fold("function f(){if(x){foo()}}", "function f(){x&&foo()}");
    t.fold(
        "function f(){if(x){foo()}else{bar()}}",
        "function f(){x?foo():bar()}",
    );
    // Try it out with properties and methods
    t.fold("function f(){if(x){a.b=1}}", "function f(){if(x)a.b=1}");
    t.fold("function f(){if(x){a.b*=1}}", "function f(){x&&(a.b*=1)}");
    t.fold("function f(){if(x){a.b+=1}}", "function f(){x&&(a.b+=1)}");
    t.fold("function f(){if(x){++a.b}}", "function f(){x&&++a.b}");
    t.fold("function f(){if(x){a.foo()}}", "function f(){x&&a.foo()}");
    t.fold("function f(){if(x){a?.foo()}}", "function f(){x&&a?.foo()}");
    // Try it out with throw/catch/finally [which should not change]
    t.fold_same("function f(){try{foo()}catch(e){bar(e)}finally{baz()}}");
    // Try it out with switch statements
    t.fold_same("function f(){switch(x){case 1:break}}");
    // Do while loops stay in a block if that's where they started
    t.fold_same("function f(){if(e1){do foo();while(e2)}else foo2()}");
    // Test an obscure case with do and while
    t.fold(
        "if(x){do{foo()}while(y)}else bar()",
        "if(x){do foo();while(y)}else bar()",
    );
    // Play with nested IFs
    t.fold(
        "function f(){if(x){if(y)foo()}}",
        "function f(){x && (y && foo())}",
    );
    t.fold(
        "function f(){if(x){if(y)foo();else bar()}}",
        "function f(){x&&(y?foo():bar())}",
    );
    t.fold(
        "function f(){if(x){if(y)foo()}else bar()}",
        "function f(){x?y&&foo():bar()}",
    );
    t.fold(
        "function f(){if(x){if(y)foo();else bar()}else{baz()}}",
        "function f(){x?y?foo():bar():baz()}",
    );
    t.fold(
        "if(e1){while(e2){if(e3){foo()}}}else{bar()}",
        "if(e1)while(e2)e3&&foo();else bar()",
    );
    t.fold(
        "if(e1){with(e2){if(e3){foo()}}}else{bar()}",
        "if(e1)with(e2)e3&&foo();else bar()",
    );
    t.fold("if(a||b){if(c||d){var x;}}", "if(a||b)if(c||d)var x");
    t.fold(
        "if(x){ if(y){var x;}else{var z;} }",
        "if(x)if(y)var x;else var z",
    );
    // NOTE - technically we can remove the blocks since both the parent
    // and child have elses. But we don't since it causes ambiguities in
    // some cases where not all descendent ifs having elses
    t.fold(
        "if(x){ if(y){var x;}else{var z;} }else{var w}",
        "if(x)if(y)var x;else var z;else var w",
    );
    t.fold(
        "if (x) {var x;}else { if (y) { var y;} }",
        "if(x)var x;else if(y)var y",
    );
    // Here's some of the ambiguous cases
    t.fold(
        "if(a){if(b){f1();f2();}else if(c){f3();}}else {if(d){f4();}}",
        "if(a)if(b){f1();f2()}else c&&f3();else d&&f4()",
    );
    t.fold("function f(){foo()}", "function f(){foo()}");
    t.fold("switch(x){case y: foo()}", "switch(x){case y:foo()}");
    t.fold(
        "try{foo()}catch(ex){bar()}finally{baz()}",
        "try{foo()}catch(ex){bar()}finally{baz()}",
    );
}

/// Try to minimize returns
// port: PeepholeMinimizeConditionsTest#testFoldReturns
#[test]
fn test_fold_returns() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.fold(
        "function f(){if(x)return 1;else return 2}",
        "function f(){return x?1:2}",
    );
    t.fold(
        "function f(){if(x)return 1;return 2}",
        "function f(){return x?1:2}",
    );
    t.fold(
        "function f(){if(x)return;return 2}",
        "function f(){return x?void 0:2}",
    );
    t.fold(
        "function f(){if(x)return 1+x;else return 2-x}",
        "function f(){return x?1+x:2-x}",
    );
    t.fold(
        "function f(){if(x)return 1+x;return 2-x}",
        "function f(){return x?1+x:2-x}",
    );
    t.fold(
        "function f(){if(x)return y += 1;else return y += 2}",
        "function f(){return x?(y+=1):(y+=2)}",
    );
    t.fold(
        "function f(){if(x)return;else return 2-x}",
        "function f(){if(x);else return 2-x}",
    );
    t.fold(
        "function f(){if(x)return;return 2-x}",
        "function f(){return x?void 0:2-x}",
    );
    t.fold(
        "function f(){if(x)return x;else return}",
        "function f(){if(x)return x;{}}",
    );
    t.fold(
        "function f(){if(x)return x;return}",
        "function f(){if(x)return x}",
    );
    t.fold_same("function f(){for(var x in y) { return x.y; } return k}");
}

// port: PeepholeMinimizeConditionsTest#testFoldReturnsIntegration2
#[test]
fn test_fold_returns_integration2() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = true;
    t.harness.disable_normalize().unwrap();
    // if-then-else duplicate statement removal handles this case:
    t.test_same("function test(a) {if (a) {const a = Math.random();if(a) {return a;}} return a; }");
}

// port: PeepholeMinimizeConditionsTest#testMinimizeIfWithNewTargetCondition
#[test]
fn test_minimize_if_with_new_target_condition() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    // Related to https://github.com/google/closure-compiler/issues/3097
    t.test(
        "function x() {\n  if (new.target) {\n    return 1;\n  } else {\n    return 2;\n  }\n}\n",
        "function x() {\n  return new.target ? 1 : 2;\n}\n",
    );
}

// port: PeepholeMinimizeConditionsTest#testRemoveDuplicateReturn
#[test]
fn test_remove_duplicate_return() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.harness.enable_normalize().unwrap();
    t.fold("function f() { return; }", "function f(){}");
    t.fold_same("function f() { return a; }");
    t.fold(
        "function f() { if (x) { return a } return a; }",
        "function f() { if (x) {} return a; }",
    );
    t.fold_same("function f() { try { if (x) { return a } } catch(e) {} return a; }");
    t.fold_same("function f() { try { if (x) {} } catch(e) {} return 1; }");
    // finally clauses may have side effects
    t.fold_same("function f() { try { if (x) { return a } } finally { a++ } return a; }");
    // but they don't matter if the result doesn't have side effects and can't
    // be affect by side-effects.
    t.fold(
        "function f() { try { if (x) { return 1 } } finally {} return 1; }",
        "function f() { try { if (x) {} } finally {} return 1; }",
    );
    t.fold(
        "function f() { switch(a){ case 1: return a; } return a; }",
        "function f() { switch(a){ case 1: } return a; }",
    );
    t.fold(
        "function f() { switch(a){\n  case 1: return a; case 2: return a; } return a; }\n",
        "function f() { switch(a){\n  case 1: break; case 2: } return a; }\n",
    );
}

// port: PeepholeMinimizeConditionsTest#testRemoveDuplicateStatements
#[test]
fn test_remove_duplicate_statements() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.fold(
        "if (a) { x = 1; x++ } else { x = 2; x++ }",
        "x=(a) ? 1 : 2; x++",
    );
    t.fold(
        "if (a) { x = 1; x++; y += 1; z = pi; }\n else  { x = 2; x++; y += 1; z = pi; }\n",
        "x=(a) ? 1 : 2; x++; y += 1; z = pi;",
    );
    t.fold(
        "function z() {\nif (a) { foo(); return !0 } else { goo(); return !0 }\n}\n",
        "function z() {(a) ? foo() : goo(); return !0}",
    );
    t.fold("function z() {if (a) { foo(); x = true; return true\n} else { goo(); x = true; return true }}\n", "function z() {(a) ? foo() : goo(); x = true; return true}");
    t.fold("function z() {\n  if (a) { bar(); foo(); return true }\n    else { bar(); goo(); return true }\n}\n", "function z() {\n  if (a) { bar(); foo(); }\n    else { bar(); goo(); }\n  return true;\n}\n");
}

// port: PeepholeMinimizeConditionsTest#testRemoveDuplicateThrow
#[test]
fn test_remove_duplicate_throw() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.harness.enable_normalize().unwrap();
    t.fold_same("function f() { throw a; }");
    t.fold(
        "function f() { if (x) { throw a } throw a; }",
        "function f() { if (x) {} throw a; }",
    );
    t.fold_same("function f() { try { if (x) {throw a} } catch(e) {} throw a; }");
    t.fold_same("function f() { try { if (x) {throw 1} } catch(e) {f()} throw 1; }");
    t.fold_same("function f() { try { if (x) {throw 1} } catch(e) {f()} throw 1; }");
    t.fold_same("function f() { try { if (x) {throw 1} } catch(e) {throw 1}}");
    t.fold(
        "function f() { try { if (x) {throw 1} } catch(e) {throw 1} throw 1; }",
        "function f() { try { if (x) {throw 1} } catch(e) {} throw 1; }",
    );
    // finally clauses may have side effects
    t.fold_same("function f() { try { if (x) { throw a } } finally { a++ } throw a; }");
    // but they don't matter if the result doesn't have side effects and can't
    // be affect by side-effects.
    t.fold(
        "function f() { try { if (x) { throw 1 } } finally {} throw 1; }",
        "function f() { try { if (x) {} } finally {} throw 1; }",
    );
    t.fold(
        "function f() { switch(a){ case 1: throw a; } throw a; }",
        "function f() { switch(a){ case 1: } throw a; }",
    );
    t.fold(
        "function f() { switch(a){\ncase 1: throw a; case 2: throw a; } throw a; }\n",
        "function f() { switch(a){ case 1: break; case 2: } throw a; }",
    );
}

// port: PeepholeMinimizeConditionsTest#testRemoveElseCause
#[test]
fn test_remove_else_cause() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test(
        "function f() {\n if(x) return 1;\n else if(x) return 2;\n else if(x) return 3 }\n",
        "function f() {\n if(x) return 1;\n{ if(x) return 2;\n{ if(x) return 3 } } }\n",
    );
}

// port: PeepholeMinimizeConditionsTest#testRemoveElseCause1
#[test]
fn test_remove_else_cause1() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test(
        "function f() { if (x) throw 1; else f() }",
        "function f() { if (x) throw 1; { f() } }",
    );
}

// port: PeepholeMinimizeConditionsTest#testRemoveElseCause2
#[test]
fn test_remove_else_cause2() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test(
        "function f() { if (x) return 1; else f() }",
        "function f() { if (x) return 1; { f() } }",
    );
    t.test(
        "function f() { if (x) return; else f() }",
        "function f() { if (x) {} else { f() } }",
    );
    // This case is handled by minimize exit points.
    t.test_same("function f() { if (x) return; f() }");
}

// port: PeepholeMinimizeConditionsTest#testRemoveElseCause4
#[test]
fn test_remove_else_cause4() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.test_same("function f() { if (x) { if (y) { return 1; } } else f() }");
}

// port: PeepholeMinimizeConditionsTest#testSubsituteBreakForThrow
#[test]
fn test_subsitute_break_for_throw() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.fold_same("function f() { while(x) { throw Error }}");
    t.fold(
        "function f() { while(x) { throw Error } throw Error }",
        "function f() { while(x) { break } throw Error}",
    );
    t.fold_same("function f() { while(x) { throw Error(1) } throw Error(2)}");
    t.fold_same("function f() { while(x) { throw Error(1) } return Error(2)}");
    t.fold_same("function f() { while(x) { throw 5 } }");
    t.fold_same("function f() { a: { throw 5 } }");
    t.fold(
        "function f() { while(x) { throw 5}  throw 5}",
        "function f() { while(x) { break }   throw 5}",
    );
    t.fold(
        "function f() { while(x) { throw x}  throw x}",
        "function f() { while(x) { break }   throw x}",
    );
    t.fold_same("function f() { while(x) { if (y) { throw Error }}}");
    t.fold(
        "function f() { while(x) { if (y) { throw Error }} throw Error}",
        "function f() { while(x) { if (y) { break }} throw Error}",
    );
    t.fold(
        "function f() { while(x) { if (y) { throw 5 }} throw 5}",
        "function f() { while(x) { if (y) { break    }} throw 5}",
    );
    // It doesn't matter if x is changed between them. We are still throwing
    // x at whatever x value current holds. The whole x = 1 is skipped.
    t.fold(
        "function f() { while(x) { if (y) { throw x } x = 1} throw x}",
        "function f() { while(x) { if (y) { break    } x = 1} throw x}",
    );
    t.fold(
        "function f() { while(x) { if (y) { throw x } throw x} throw x}",
        "function f() { while(x) { if (y) {} break }throw x}",
    );
    // A break here only breaks out of the inner loop.
    t.fold_same("function f() { while(x) { while (y) { throw Error } } }");
    t.fold_same("function f() { while(1) { throw 7}  throw 5}");
    t.fold_same("function f() {\n  try { while(x) {throw f()}} catch (e) { } throw f()}\n");
    t.fold_same("function f() {\n  try { while(x) {throw f()}} finally {alert(1)} throw f()}\n");
    // Both throws has the same handler
    t.fold(
        "function f() {\n  try { while(x) { throw f() } throw f() } catch (e) { } }\n",
        "function f() {\n  try { while(x) { break } throw f() } catch (e) { } }\n",
    );
    // We can't fold this because it'll change the order of when foo is called.
    t.fold_same(
        "function f() {\n  try { while(x) { throw foo() } } finally { alert(1) }\n  throw foo()}\n",
    );
    // This is fine, we have no side effect in the throw value.
    t.fold(
        "function f() {\n  try { while(x) { throw 1 } } finally { alert(1) } throw 1}\n",
        "function f() {\n  try { while(x) { break    } } finally { alert(1) } throw 1}\n",
    );
    t.fold_same("function f() { try{ throw a } finally { a = 2 } throw a; }");
    t.fold(
        "function f() { switch(a){ case 1: throw a; default: g();} throw a;}",
        "function f() { switch(a){ case 1: break; default: g();} throw a; }",
    );
    t.fold_same("function f(flag, e) {\n  while (flag) {\n    try { throw e; } finally { e = null; }\n  }\n  throw e;\n}\n");
}

// port: PeepholeMinimizeConditionsTest#testSubsituteReturn
#[test]
fn test_subsitute_return() {
    let mut t = PeepholeMinimizeConditionsTest::new();
    t.hooks.late = false;
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.fold(
        "function f() { while(x) { return }}",
        "function f() { while(x) { break }}",
    );
    t.fold_same("function f() { while(x) { return 5 } }");
    t.fold_same("function f() { a: { return 5 } }");
    t.fold(
        "function f() { while(x) { return 5}  return 5}",
        "function f() { while(x) { break }    return 5}",
    );
    t.fold(
        "function f() { while(x) { return x}  return x}",
        "function f() { while(x) { break }    return x}",
    );
    t.fold(
        "function f() { while(x) { if (y) { return }}}",
        "function f() { while(x) { if (y) { break  }}}",
    );
    t.fold(
        "function f() { while(x) { if (y) { return }} return}",
        "function f() { while(x) { if (y) { break  }}}",
    );
    t.fold(
        "function f() { while(x) { if (y) { return 5 }} return 5}",
        "function f() { while(x) { if (y) { break    }} return 5}",
    );
    // It doesn't matter if x is changed between them. We are still returning
    // x at whatever x value current holds. The whole x = 1 is skipped.
    t.fold(
        "function f() { while(x) { if (y) { return x } x = 1} return x}",
        "function f() { while(x) { if (y) { break    } x = 1} return x}",
    );
    t.fold(
        "function f() { while(x) { if (y) { return x } return x} return x}",
        "function f() { while(x) { if (y) {} break }return x}",
    );
    // A break here only breaks out of the inner loop.
    t.fold_same("function f() { while(x) { while (y) { return } } }");
    t.fold_same("function f() { while(1) { return 7}  return 5}");
    t.fold_same("function f() {\n  try { while(x) {return f()}} catch (e) { } return f()}\n");
    t.fold_same("function f() {\n  try { while(x) {return f()}} finally {alert(1)} return f()}\n");
    // Both returns has the same handler
    t.fold(
        "function f() {\n  try { while(x) { return f() } return f() } catch (e) { } }\n",
        "function f() {\n  try { while(x) { break } return f() } catch (e) { } }\n",
    );
    // We can't fold this because it'll change the order of when foo is called.
    t.fold_same("function f() {\n  try { while(x) { return foo() } } finally { alert(1) }\n  return foo()}\n");
    // This is fine, we have no side effect in the return value.
    t.fold(
        "function f() {\n  try { while(x) { return 1 } } finally { alert(1) } return 1}\n",
        "function f() {\n  try { while(x) { break    } } finally { alert(1) } return 1}\n",
    );
    t.fold_same("function f() { try{ return a } finally { a = 2 } return a; }");
    t.fold(
        "function f() { switch(a){ case 1: return a; default: g();} return a;}",
        "function f() { switch(a){ case 1: break; default: g();} return a; }",
    );
    t.fold_same("function f(flag) {\n  var ok = false;\n  while (flag) {\n    try { return ok; } finally { ok = true; }\n  }\n  return ok;\n}\n");
    t.fold("function f(flag) {\n  var ok = false;\n  while (flag) {\n    try { return 1; } finally { ok = true; }\n  }\n  return 1;\n}\n", "function f(flag) {\n  var ok = false;\n  while (flag) {\n    try { break; } finally { ok = true; }\n  }\n  return 1;\n}\n");
}
