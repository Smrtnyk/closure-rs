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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/PeepholeIntegrationTest.java.

//! Port of `PeepholeIntegrationTest.java`: tests for the interaction between multiple peephole
//! passes.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    minimize_exit_points::MinimizeExitPoints, peephole_fold_constants::PeepholeFoldConstants,
    peephole_minimize_conditions::PeepholeMinimizeConditions,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
    peephole_remove_dead_code::PeepholeRemoveDeadCode,
    peephole_replace_known_methods::PeepholeReplaceKnownMethods,
    peephole_substitute_alternate_syntax::PeepholeSubstituteAlternateSyntax,
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

struct PeepholeIntegrationTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    late: bool,
    num_repetitions: i32,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeIntegrationTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let late = self.late;
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> = vec![
                    Box::new(PeepholeMinimizeConditions::new(late)),
                    Box::new(PeepholeSubstituteAlternateSyntax::new(late)),
                    Box::new(PeepholeRemoveDeadCode::new()),
                    Box::new(PeepholeFoldConstants::new(late, false /* useTypes */)),
                    Box::new(PeepholeReplaceKnownMethods::new(
                        late, /* useTypes= */ false,
                    )),
                    Box::new(MinimizeExitPoints::new()),
                ];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: PeepholeIntegrationTest#getNumRepetitions
    fn get_num_repetitions(&self) -> i32 {
        self.num_repetitions
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeIntegrationTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeIntegrationTest {
    // port: CompilerTestCase#CompilerTestCase()
    // port: PeepholeIntegrationTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();

        harness.enable_normalize().unwrap();
        harness.enable_compute_side_effects().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        let late = false;
        let num_repetitions = 2;
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeIntegrationTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                late,
                num_repetitions,
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

// port: PeepholeIntegrationTest#testUselessLabels
#[test]
fn test_useless_labels() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = false;
    t.test("a:b:{break a;}", "");
    t.test("a:b:{break b;}", "");
    t.test("a:{break a;}", "");
    t.hooks.late = true;
    t.test("a:b:{break a;}", "");
    t.test("a:b:{break b;}", "");
    t.test("a:{break a;}", "");
}

// port: PeepholeIntegrationTest#testTrueFalse
#[test]
fn test_true_false() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = false;
    t.test_same("x = true");
    t.test_same("x = false");
    t.test("x = !1", "x = false");
    t.test("x = !0", "x = true");
    t.hooks.late = true;
    t.test("x = true", "x = !0");
    t.test("x = false", "x = !1");
    t.test_same("x = !1");
    t.test_same("x = !0");
}

// port: PeepholeIntegrationTest#testFoldOneChildBlocksStringCompare
#[test]
fn test_fold_one_child_blocks_string_compare() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "if (x) {if (y) { var x; } } else{ var z; }",
        "if (x) { if (y) var x } else var z",
    );
}

/// Test a particularly hairy edge case.
// port: PeepholeIntegrationTest#testNecessaryDanglingElse
#[test]
fn test_necessary_dangling_else() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "if (x) if (y){ y(); z() } else; else x()",
        "if (x) { if(y) { y(); z() } } else x()",
    );
}

// port: PeepholeIntegrationTest#testBug1059649
#[test]
fn test_bug1059649() {
    let mut t = PeepholeIntegrationTest::new();
    // ensure that folding blocks with a single var node doesn't explode
    t.test("if(x){var y=3;}var z=5", "if(x)var y=3;var z=5");
    t.test(
        "for(var i=0;i<10;i++){var y=3;}var z=5",
        "for(var i=0;i<10;i++)var y=3;var z=5",
    );
    t.test(
        "for(var i in x){var y=3;}var z=5",
        "for(var i in x)var y=3;var z=5",
    );
    t.test(
        "do{var y=3;}while(x);var z=5",
        "do var y=3;while(x);var z=5",
    );
}

// port: PeepholeIntegrationTest#testHookIfIntegration
#[test]
fn test_hook_if_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "if (false){ x = 1; } else if (cond) { x = 2; } else { x = 3; }",
        "x=cond?2:3",
    );
    t.test("x?void 0:y()", "x||y()");
    t.test("!x?void 0:y()", "x&&y()");
    t.test("x?y():void 0", "x&&y()");
}

// port: PeepholeIntegrationTest#testFoldLogicalOpIntegration
#[test]
fn test_fold_logical_op_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("if(x && true) z()", "x&&z()");
    t.test("if(x && false) z()", "");
    t.test("if(x || 3) z()", "z()");
    t.test("if(x || false) z()", "x&&z()");
    t.test("if(x==y && false) z()", "");
    t.test("if(y() || x || 3) z()", "y();z()");
}

// port: PeepholeIntegrationTest#testFoldBitwiseOpStringCompareIntegration
#[test]
fn test_fold_bitwise_op_string_compare_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for (;-1 | 0;) {}", "for (;;);");
}

// port: PeepholeIntegrationTest#testVarLiftingIntegration
#[test]
fn test_var_lifting_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("if(true);else var a;", "var a");
    t.test("if(false) foo();else var a;", "var a");
    t.test("if(true)var a;else;", "var a");
    t.test("if(false)var a;else;", "var a");
    t.test("if(false)var a,b;", "var b; var a");
    t.test("if(false){var a;var a;}", "var a");
    t.test("if(false)var a=function(){var b};", "var a");
    t.test("if(a)if(false)var a;else var b;", "var a;if(a)var b");
}

// port: PeepholeIntegrationTest#testBug1438784
#[test]
fn test_bug1438784() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for(var i=0;i<10;i++)if(x)x.y;", "for(var i=0;i<10;i++);");
}

// port: PeepholeIntegrationTest#testFoldUselessForIntegration
#[test]
fn test_fold_useless_for_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for(;!true;) { foo() }", "");
    t.test("for(;void 0;) { foo() }", "");
    t.test("for(;undefined;) { foo() }", "");
    t.test("for(;1;) foo()", "for(;;) foo()");
    t.test("for(;!void 0;) foo()", "for(;;) foo()");
    // Make sure proper empty nodes are inserted.
    t.test("if(foo())for(;false;){foo()}else bar()", "foo()||bar()");
    t.test(
        "if(foo?.())for(;false;){foo?.()}else bar?.()",
        "foo?.()||bar?.()",
    );
}

// port: PeepholeIntegrationTest#testFoldUselessDoIntegration
#[test]
fn test_fold_useless_do_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("do { foo() } while(!true);", "foo()");
    t.test("do { foo() } while(void 0);", "foo()");
    t.test("do { foo() } while(undefined);", "foo()");
    t.test("do { foo() } while(!void 0);", "do { foo() } while(1);");
    // Make sure proper empty nodes are inserted.
    t.test(
        "if(foo())do {foo()} while(false) else bar()",
        "foo()?foo():bar()",
    );
    // Optional chaining version of these tests.
    t.test("do { foo?.() } while(!true);", "foo?.()");
    t.test("do { foo?.() } while(void 0);", "foo?.()");
    t.test("do { foo?.() } while(undefined);", "foo?.()");
    t.test("do { foo?.() } while(!void 0);", "do { foo?.() } while(1);");
    // Make sure proper empty nodes are inserted.
    t.test(
        "if(foo?.())do {foo?.()} while(false) else bar?.()",
        "foo?.() ? foo?.() : bar?.()",
    );
}

// port: PeepholeIntegrationTest#testMinimizeExpr
#[test]
fn test_minimize_expr() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("!!true", "");
    t.test("!!x()", "x()");
    t.test("!(!x()&&!y())", "x()||y()");
    t.test("x()||!!y()", "x()||y()");
    // This is similar to the !!true case
    t.test("!!x()&&y()", "x()&&y()");
    t.test("!!x?.()&&y?.()", "x?.()&&y?.()");
}

// port: PeepholeIntegrationTest#testBug1509085
#[test]
fn test_bug1509085() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.num_repetitions = 1;
    t.hooks.late = true;
    // Such code can be replaced by using the simpler, equivalent optional chaining operator
    // `x?.()`.
    t.test("x ? x() : void 0", "x&&x();");
    t.test_same("y = x ? x() : void 0");
    t.test_same("x?.()");
}

// port: PeepholeIntegrationTest#testBugIssue3
#[test]
fn test_bug_issue3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function foo() {\n  if(sections.length != 1) children[i] = 0;\n  else var selectedid = children[i]\n}\n");
}

// port: PeepholeIntegrationTest#testBugIssue43
#[test]
fn test_bug_issue43() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function foo() {\n  if (a) { var b = 1; } else { a.b = 1; }\n}");
}

// port: PeepholeIntegrationTest#testFoldNegativeBug
#[test]
fn test_fold_negative_bug() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for (;-3;){};", "for (;;);");
}

// port: PeepholeIntegrationTest#testNoNormalizeLabeledExpr
#[test]
fn test_no_normalize_labeled_expr() {
    let mut t = PeepholeIntegrationTest::new();
    t.harness.enable_normalize().unwrap();
    t.test_same("var x; foo:{x = 3;}");
    t.test_same("var x; foo:x = 3;");
    t.harness.disable_normalize().unwrap();
}

// port: PeepholeIntegrationTest#testShortCircuit1
#[test]
fn test_short_circuit1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("1 && a()", "a()");
}

// port: PeepholeIntegrationTest#testShortCircuit2
#[test]
fn test_short_circuit2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("1 && a() && 2", "a()");
}

// port: PeepholeIntegrationTest#testShortCircuit3
#[test]
fn test_short_circuit3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("a() && 1 && 2", "a()");
}

// port: PeepholeIntegrationTest#testShortCircuit4
#[test]
fn test_short_circuit4() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("a() && (1 && b())", "a() && b()");
    t.test("a() && 1 && b()", "a() && b()");
    t.test("(a() && 1) && b()", "a() && b()");
    t.test("a?.() && (1 && b?.())", "a?.() && b?.()");
    t.test("a?.() && 1 && b?.()", "a?.() && b?.()");
    t.test("(a?.() && 1) && b?.()", "a?.() && b?.()");
}

// port: PeepholeIntegrationTest#testMinimizeExprCondition
#[test]
fn test_minimize_expr_condition() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("(x || true) && y()", "y()");
    t.test("(x || false) && y()", "x&&y()");
    t.test("(x && true) && y()", "x && y()");
    t.test("(x && false) && y()", "");
    t.test("a = x || false ? b : c", "a=x?b:c");
    t.test("do {x()} while((x && false) && y())", "x()");
    t.test("do {x?.()} while((x && false) && y?.())", "x?.()");
}

// port: PeepholeIntegrationTest#testMisc
#[test]
fn test_misc() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("x = [foo()] && x", "x = (foo(),x)");
    t.test("x = foo() && false || bar()", "x = (foo(), bar())");
    t.test("if(foo() && false) z()", "foo()");
}

// port: PeepholeIntegrationTest#testTrueFalseFolding
#[test]
fn test_true_false_folding() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = true;
    t.test("x = true", "x = !0");
    t.test("x = false", "x = !1");
    t.test("x = !3", "x = !1");
    t.test("x = true && !0", "x = !0");
    t.test("x = !!!!!!!!!!!!3", "x = !0");
    t.test("if(!3){x()}", "");
    t.test("if(!!3){x()}", "x()");
}

// port: PeepholeIntegrationTest#testCommaSplitingConstantCondition
#[test]
fn test_comma_spliting_constant_condition() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = false;
    t.test("(b=0,b=1);if(b)x=b;", "b=0;b=1;x=b;");
    t.test("(b=0,b=1);if(b)x=b;", "b=0;b=1;x=b;");
}

// port: PeepholeIntegrationTest#testAvoidCommaSplitting
#[test]
fn test_avoid_comma_splitting() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = false;
    t.test("x(),y(),z()", "x();y();z()");
    t.test("x?.(),y?.(),z?.()", "x?.();y?.();z?.()");
    t.hooks.late = true;
    t.test_same("x(),y(),z()");
    t.test_same("x?.(),y?.(),z?.()");
}

// port: PeepholeIntegrationTest#testObjectLiteral
#[test]
fn test_object_literal() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("({})", "");
    t.test("({a:1})", "");
    t.test("({a:foo()})", "foo()");
    t.test("({'a':foo()})", "foo()");
    t.test("({a:foo?.()})", "foo?.()");
    t.test("({'a':foo?.()})", "foo?.()");
}

// port: PeepholeIntegrationTest#testArrayLiteral
#[test]
fn test_array_literal() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("([])", "");
    t.test("([1])", "");
    t.test("([a])", "");
    t.test("([foo()])", "foo()");
}

// port: PeepholeIntegrationTest#testFoldIfs2
#[test]
fn test_fold_ifs2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {if (x) { a(); } else if (y) { a() }}",
        "function f() {x?a():y&&a();}",
    );
}

// port: PeepholeIntegrationTest#testTemplateStringsKnownMethods
#[test]
fn test_template_strings_known_methods() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("x = `abcdef`.indexOf('b')", "x = 1");
    t.test("x = [`a`, `b`, `c`].join(``)", "x='abc'");
    t.test("x = `abcdef`.substr(0,2)", "x = 'ab'");
    t.test("x = `abcdef`.substring(0,2)", "x = 'ab'");
    t.test("x = `abcdef`.slice(0,2)", "x = 'ab'");
    t.test("x = `abcdef`.charAt(0)", "x = 'a'");
    t.test("x = `abcdef`.charCodeAt(0)", "x = 97");
    t.test("x = `abc`.toUpperCase()", "x = 'ABC'");
    t.test("x = `ABC`.toLowerCase()", "x = 'abc'");
    t.test(
        "x = `\t\n\u{feff}\t asd foo bar \r\n`.trim()",
        "x = 'asd foo bar'",
    );
    t.test("x = parseInt(`123`)", "x = 123");
    t.test("x = parseFloat(`1.23`)", "x = 1.23");
}

// port: PeepholeIntegrationTest#testDontFoldKnownMethodsWithOptionalChaining
#[test]
fn test_dont_fold_known_methods_with_optional_chaining() {
    let mut t = PeepholeIntegrationTest::new();
    // Known methods guarded by an optional chain are not folded
    t.test(
        "x = `abcdef`.indexOf?.('b')",
        "x = \"abcdef\".indexOf?.(\"b\");",
    );
    t.test(
        "x = [`a`, `b`, `c`].join?.(``)",
        "x = [\"a\", \"b\", \"c\"].join?.(\"\")",
    );
    t.test("x = `abcdef`.substr?.(0,2)", "x = \"abcdef\".substr?.(0,2)");
    t.test(
        "x = `abcdef`.substring?.(0,2)",
        "x = \"abcdef\".substring?.(0,2)",
    );
    t.test("x = `abcdef`.slice?.(0,2)", "x = \"abcdef\".slice?.(0,2)");
    t.test("x = `abcdef`.charAt?.(0)", "x = \"abcdef\".charAt?.(0)");
    t.test(
        "x = `abcdef`.charCodeAt?.(0)",
        "x = \"abcdef\".charCodeAt?.(0)",
    );
    t.test("x = `abc`.toUpperCase?.()", "x = \"abc\".toUpperCase?.()");
    t.test("x = `ABC`.toLowerCase?.()", "x = \"ABC\".toLowerCase?.()");
    t.test("x = parseInt?.(`123`)", "x = parseInt?.(\"123\")");
    t.test("x = parseFloat?.(`1.23`)", "x = parseFloat?.(\"1.23\")");
}

// port: PeepholeIntegrationTest#testRemoveUselessNameStatements
#[test]
fn test_remove_useless_name_statements() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("a;", "");
    t.test("a.b;", "");
    t.test("a.b.MyClass.prototype.memberName;", "");
}

// port: PeepholeIntegrationTest#testRemoveUselessStrings
#[test]
fn test_remove_useless_strings() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("'a';", "");
}

// port: PeepholeIntegrationTest#testNoRemoveUseStrict
#[test]
fn test_no_remove_use_strict() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("'use strict';", "'use strict'");
}

// port: PeepholeIntegrationTest#testRemoveDo
#[test]
fn test_remove_do() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("do { print(1); break } while(1)");
    // NOTE: This pass should never see while-loops, because normalization replaces them all with
    // for-loops.
    t.test(
        "for (; 1;) { break; do { print(1); break } while(1) }",
        "for (;  ;) { break;                                 }",
    );
}

// port: PeepholeIntegrationTest#testRemoveUselessLiteralValueStatements
#[test]
fn test_remove_useless_literal_value_statements() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("true;", "");
    t.test("'hi';", "");
    t.test("if (x) 1;", "");
    // NOTE: This pass should never see while-loops, because normalization replaces them all with
    // for-loops.
    t.test("for (; x;) 1;", "for (; x;);");
    t.test("do 1; while (x);", "for (;x;);");
    t.test("for (;;) 1;", "for (;;);");
    t.test("switch(x){case 1:true;case 2:'hi';default:true}", "");
}

// port: PeepholeIntegrationTest#testTryCatchFinally1
#[test]
fn test_try_catch_finally1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("try {foo()} catch (e) {bar()}");
    t.test_same("try { try {foo()} catch (e) {bar()}} catch (x) {bar()}");
}

// port: PeepholeIntegrationTest#testTryCatchFinally2
#[test]
fn test_try_catch_finally2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("try {var x = 1} catch (e) {e()}");
}

// port: PeepholeIntegrationTest#testTryCatchFinally3
#[test]
fn test_try_catch_finally3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("try {var x = 1} catch (e) {e()} finally {x()}");
}

// port: PeepholeIntegrationTest#testTryCatchFinally4
#[test]
fn test_try_catch_finally4() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("try {var x = 1} catch (e) {e()} finally {}");
}

// port: PeepholeIntegrationTest#testTryCatchFinally5
#[test]
fn test_try_catch_finally5() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("try {var x = 1} finally {x()}");
    t.test_same("var x = 1");
}

// port: PeepholeIntegrationTest#testRemovalRequiresRedeclaration
#[test]
fn test_removal_requires_redeclaration() {
    let mut t = PeepholeIntegrationTest::new();
    // NOTE: This pass should never see while-loops, because normalization replaces them all with
    // for-loops.
    t.test(
        "for (; 1;) {\n  break;\n  var x = 1\n}\n",
        "var x;\nfor (;;) {\n  break;\n}\n",
    );
    t.test(
        "for (; 1;) {\n  break;\n  var x=1;\n  var y=1;\n}\n",
        "var y;\nvar x;\nfor (;;) {\n  break;\n}\n",
    );
}

// port: PeepholeIntegrationTest#testAssignPropertyOnCreatedObject
#[test]
fn test_assign_property_on_created_object() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("this.foo = 3;");
    t.test_same("a.foo = 3;");
    t.test_same("bar().foo = 3;");
    t.test_same("({}).foo = bar();");
    t.test_same("(new X()).foo = 3;");
    t.test("({}).foo = 3;", "");
    t.test("(function() {}).prototype.toString = function(){};", "");
    t.test("(function() {}).prototype['toString'] = function(){};", "");
    t.test("(function() {}).prototype[f] = function(){};", "");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn9
#[test]
fn test_useless_unconditional_return9() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function f() {switch (a) { case 'a': case foo(): }}");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalContinue
#[test]
fn test_useless_unconditional_continue() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for(;1;) {continue}", "for(;;) {}");
    t.test("for(;0;) {continue}", "");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalContinue2
#[test]
fn test_useless_unconditional_continue2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "X: for(;1;) { for(;1;) { if (x()) {continue X} x = 1}}",
        "X: for(; ;) { for(; ;) { if (x()) {continue X} x = 1}}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak2
#[test]
fn test_useless_unconditional_break2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("switch (a) { case 'a': break }", "");
    t.test("switch (a) { default: break; case 'a': }", "");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak3
#[test]
fn test_useless_unconditional_break3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("switch (a) { case 'a': alert(a); break; default: alert(a); }");
    t.test_same("switch (a) { default: alert(a); break; case 'a': alert(a); }");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak4
#[test]
fn test_useless_unconditional_break4() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("X: {switch (a) { case 'a': break X}}", "");
    t.test(
        "X: {switch (a) { case 'a': if (a()) {break X}  a = 1; }}",
        "X: {switch (a) { case 'a': a() || (a = 1); }}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak5
#[test]
fn test_useless_unconditional_break5() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "X: {switch (a) { case 'a': if (a()) {break X}}}",
        "X: {switch (a) { case 'a':     a()           }}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak6
#[test]
fn test_useless_unconditional_break6() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "X: {switch (a) { case 'a': if (a()) {break X}}}",
        "X: {switch (a) { case 'a':     a();          }}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalBreak7
#[test]
fn test_useless_unconditional_break7() {
    let mut t = PeepholeIntegrationTest::new();
    // There is no reason to keep these
    t.test_same("do { break } while(1);");
    t.test("for(;1;) { break }", "for(;;) { break; }");
}

// port: PeepholeIntegrationTest#testIteratedRemoval1
#[test]
fn test_iterated_removal1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "switch (a) { case 'a': break; case 'b': break; case 'c': break }",
        "",
    );
}

// port: PeepholeIntegrationTest#testIteratedRemoval3
#[test]
fn test_iterated_removal3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for (;;) {\n   switch (a) {\n   case 'a': continue;\n   case 'b': continue;\n   case 'c': continue;\n   }\n }\n", " for (;;) { }");
}

// port: PeepholeIntegrationTest#testIteratedRemoval5
#[test]
fn test_iterated_removal5() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("var x; \n out: { \n   try { break out; } catch (e) { break out; } \n   x = undefined; \n }\n", "var x;");
}

// port: PeepholeIntegrationTest#testIssue4177428a
#[test]
fn test_issue4177428a() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("f = function() {\n  var action;\n  a: {\n    var proto = null;\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      break a // Keep this...\n    }\n  }\n  alert(action) // and this.\n};\n");
}

// port: PeepholeIntegrationTest#testIssue4177428b
#[test]
fn test_issue4177428b() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("f = function() {\n  var action;\n  a: {\n    var proto = null;\n    try {\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      break a // Keep this...\n    }\n    } finally {\n    }\n  }\n  alert(action) // and this.\n};\n", "f = function() {\n  var action;\n  a: {\n    var proto = null;\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      break a // Keep this...\n    }\n  }\n  alert(action) // and this.\n};\n");
}

// port: PeepholeIntegrationTest#testIssue4177428c
#[test]
fn test_issue4177428c() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("f = function() {\n  var action;\n  a: {\n    var proto = null;\n    try {\n    } finally {\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      break a // Keep this...\n    }\n    }\n  }\n  alert(action)\n// and this.\n};\n", "f = function() {\n  var action;\n  a: {\n    var proto = null;\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      break a // Keep this...\n    }\n  }\n  alert(action) // and this.\n};\n");
}

// port: PeepholeIntegrationTest#testIssue4177428_continue
#[test]
fn test_issue4177428_continue() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("f = function() {\n  var action;\n  a: do {\n    var proto = null;\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      continue a\n// Keep this...\n    }\n  } while(false)\n  alert(action)\n// and this.\n};\n", "f = function() {\n  var action;\n  a: do {\n    var proto = null;\n    try {\n      proto = new Proto\n    } finally {\n      action = proto;\n      continue a\n// Keep this...\n    }\n  } while(0)\n  alert(action)\n// and this.\n};\n");
}

// port: PeepholeIntegrationTest#testIssue4177428_multifinally
#[test]
fn test_issue4177428_multifinally() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("a: {\n try {\n   try {\n   } finally {\n     break a;\n   }\n } finally {\n   x = 1;\n }\n}\n", "a: {\n  x = 1;\n}\n");
}

// port: PeepholeIntegrationTest#testForInLoop
#[test]
fn test_for_in_loop() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("var x; for(x in y) {}");
}

// port: PeepholeIntegrationTest#testForOf
#[test]
fn test_for_of() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("for(x of i){ 1; }", "for(x of i) {}");
    t.test_same("for(x of i){}");
}

// port: PeepholeIntegrationTest#testComputedClassPropertyNotRemoved
#[test]
fn test_computed_class_property_not_removed() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("class Foo { ['x']() {} }");
}

// port: PeepholeIntegrationTest#testClassExtendsNotRemoved
#[test]
fn test_class_extends_not_removed() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function f() {}\nclass Foo extends f() {}\n");
}

// port: PeepholeIntegrationTest#testStaticBlockRemoved
#[test]
fn test_static_block_removed() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("class Foo { static {} }", "class Foo {  }");
}

// port: PeepholeIntegrationTest#testRemoveUnreachableCodeInStaticBlock1
#[test]
fn test_remove_unreachable_code_in_static_block1() {
    let mut t = PeepholeIntegrationTest::new();
    // TODO(b/240443227): Unreachable/Useless code isn't removed in static blocks
    t.test("class Foo {\n  static {\n    switch (a) { case 'a': break }\n    try {var x = 1} catch (e) {e()}\n    true;\n    if (x) 1;\n  }\n}\n", "class Foo {\n  static {\n    try {var x = 1} catch (e) {e()}\n  }\n}\n");
}

// port: PeepholeIntegrationTest#disable_testFoldHook1
#[test]
fn disable_test_fold_hook1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f(a) {return (!a)?a:a;}",
        "function f(a) {return a}",
    );
}

// port: PeepholeIntegrationTest#testArrowFunctions
#[test]
fn test_arrow_functions() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("f(x => {return x; j = 1})", "f(x => {return x;})");
    t.test_same("f( () => {return 1;})");
}

// port: PeepholeIntegrationTest#testConditionalDeadCode
#[test]
fn test_conditional_dead_code() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() { if (1) return 5; else return 5; x = 1}",
        "function f() { return 5;}",
    );
}

// port: PeepholeIntegrationTest#testDoNotRemoveDeclarationOfUsedVariable
#[test]
fn test_do_not_remove_declaration_of_used_variable() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("var f = function() {\n  return 1;\n  let b = 5;\n  do {\n    b--;\n  } while (b);\n  return 3;\n};\n", "var f = function() {\n  let b;\n  return 1;\n};\n");
}

// port: PeepholeIntegrationTest#testDontFoldDirectAndIndirectEval
#[test]
fn test_dont_fold_direct_and_indirect_eval() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f(a, x) { if (a) { return eval(x); } else { return (0, eval)(x); } }",
        "function f(a, x) { return a ? eval(x) : (0, eval)(x); }",
    );
}

// port: PeepholeIntegrationTest#testDontRemoveBreakInTryFinally
#[test]
fn test_dont_remove_break_in_try_finally() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function f() {\n  b: {\n    try {\n      throw 9;\n    } finally {\n      break b;\n    }\n  }\n  return 1;\n}\n");
}

// port: PeepholeIntegrationTest#testDontRemoveBreakInTryFinallySwitch
#[test]
fn test_dont_remove_break_in_try_finally_switch() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function f() {\n  b: {\n    try {\n      throw 9;\n    } finally {\n      switch (x) {\n        case 1:\n          break b;\n      }\n    }\n  }\n  return 1;\n}\n");
}

// port: PeepholeIntegrationTest#testDontRemoveExport
#[test]
fn test_dont_remove_export() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() {\n  return 1;\n  alert(2);\n}\nexport { foo as foo };\n",
        "function foo() {\n  return 1;\n}\nexport { foo as foo };\n",
    );
}

// port: PeepholeIntegrationTest#testFoldFollowing
#[test]
fn test_fold_following() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("function f(){return; console.log(1);}", "function f(){}");
}

// port: PeepholeIntegrationTest#testFoldHook2
#[test]
fn test_fold_hook2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f(a) {if (!a) return a; else return a;}",
        "function f(a) {return a}",
    );
}

// port: PeepholeIntegrationTest#testFoldIfs1
#[test]
fn test_fold_ifs1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {if (x) return 1; else if (y) return 1;}",
        "function f() {if (x||y) return 1;}",
    );
    t.test(
        "function f() {if (x) return 1; else {if (y) return 1; else foo();}}",
        "function f() {if (x||y) return 1; foo();}",
    );
}

/// Check that removing blocks with 1 child works
// port: PeepholeIntegrationTest#testFoldOneChildBlocksIntegration
#[test]
fn test_fold_one_child_blocks_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f(){switch(foo()){default:{break}}}",
        "function f(){foo()}",
    );
    t.test("function f(){switch(x){default:{break}}}", "function f(){}");
    t.test(
        "function f(){switch(x){default:x;case 1:return 2}}",
        "function f(){switch(x){default:case 1:return 2}}",
    );
    // ensure that block folding does not break hook ifs
    t.test(
        "if(x){if(true){foo();foo()}else{bar();bar()}}",
        "if(x){foo();foo()}",
    );
    t.test(
        "if(x){if(false){foo();foo()}else{bar();bar()}}",
        "if(x){bar();bar()}",
    );
    // Cases where the then clause has no side effects.
    t.test("if(x()){}", "x()");
    t.test("if(x()){} else {x()}", "x()||x()");
    t.test("if(x){}", "");
    // Even the condition has no side effect.
    t.test(
        "if(a()){A()} else if (b()) {} else {C()}",
        "a()?A():b()||C()",
    );
    t.test(
        "if(a()){} else if (b()) {} else {C()}",
        "a() || (b() || C())",
    );
    t.test(
        "if(a()){A()} else if (b()) {} else if (c()) {} else{D()}",
        "a() ? A() : b() || (c() || D())",
    );
    t.test(
        "if(a()){} else if (b()) {} else if (c()) {} else{D()}",
        "a() || (b() || (c() || D()))",
    );
    t.test(
        "if(a()){A()} else if (b()) {} else if (c()) {} else{}",
        "a()?A():b()||c()",
    );
    // Verify that non-global scope works.
    t.test("function foo(){if(x()){}}", "function foo(){x()}");
    t.test("function foo(){if(x?.()){}}", "function foo(){x?.()}");
}

/// Try to minimize returns
// port: PeepholeIntegrationTest#testFoldReturnsIntegration
#[test]
fn test_fold_returns_integration() {
    let mut t = PeepholeIntegrationTest::new();
    // if-then-else duplicate statement removal handles this case:
    t.test("function f(){if(x)return;else return}", "function f(){}");
}

/// Try to minimize returns
// port: PeepholeIntegrationTest#testFoldReturnsIntegrationWithScoped
#[test]
fn test_fold_returns_integration_with_scoped() {
    let mut t = PeepholeIntegrationTest::new();
    t.hooks.late = true;
    t.harness.disable_compute_side_effects().unwrap();
    // if-then-else duplicate statement removal handles this case:
    t.test_same("function test(a) {if (a) {const a = Math.random();if(a) {return a;}} return a; }");
}

// port: PeepholeIntegrationTest#testIssue311
#[test]
fn test_issue311() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("function a(b) {\n  switch (b.v) {\n    case 'SWITCH':\n      if (b.i >= 0) {\n        return b.o;\n      } else {\n        return;\n      }\n      break;\n  }\n}\n", "function a(b) {\n  switch (b.v) {\n    case 'SWITCH':\n      if (b.i >= 0) {\n        return b.o;\n      }\n  }\n}\n");
}

// port: PeepholeIntegrationTest#testIssue5215541_deadVarDeclar
#[test]
fn test_issue5215541_dead_var_declar() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("       throw 1; var x;", "var x; throw 1;       ");
    t.test_same("throw 1; function x() {}");
    t.test(
        "throw 1; var x; var y;                ",
        "                var y; var x; throw 1;",
    );
    t.test("       throw 1; var x = foo", "var x; throw 1");
}

// port: PeepholeIntegrationTest#testIteratedRemoval2
#[test]
fn test_iterated_removal2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() { switch (a) { case 'a':return; case 'b':return; case 'c':return }}",
        "function foo() { }",
    );
}

// port: PeepholeIntegrationTest#testIteratedRemoval4
#[test]
fn test_iterated_removal4() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() { if (x) { return; } if (x) { return; }}",
        "function foo() {}",
    );
}

// port: PeepholeIntegrationTest#testLabeledBlocks
#[test]
fn test_labeled_blocks() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("function b(m) {\n return m;\n label: {\n   START('debug');\n   label2: {\n     alert('Shouldnt be here' + m);\n   }\n   END('debug');\n  }\n}\n", "function b(m) {\n  return m;\n}\n");
}

// port: PeepholeIntegrationTest#testIssue1001
// This was originally supported by UnreachableCodeElimination, which was removed in favor
// of peephole optimizations. Support this test case if found useful in the real code.
#[test]
fn test_issue1001() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f(x) { x.property = 3; } f({})", //
        "function f(x) { x.property = 3; }",
    );
    t.test(
        "function f(x) { x.property = 3; } new f({})", //
        "function f(x) { x.property = 3; }",
    );
}

// port: PeepholeIntegrationTest#testLetConstBlocks
#[test]
fn test_let_const_blocks() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {return 1; let a; }",
        "function f() {let a; return 1;}",
    );
    t.test(
        "function f() { return 1; const a = 1; }",
        "function f() { let a;  return 1;}",
    );
    t.test(
        "function f() { x = 1; { let g; return x; } let y;}",
        "function f() { let y; x = 1;   let g; return x;         } ",
    );
}

// port: PeepholeIntegrationTest#testLetConstBlocks_inFunction_exportedFromEs6Module
#[test]
fn test_let_const_blocks_in_function_exported_from_es6_module() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {\n  return 1;\n  let a;\n}\nexport { f as f };\n",
        "function f() {\n  let a;\n  return 1;\n}\nexport { f as f };\n",
    );
    t.test(
        "function f() {\n  return 1;\n  const a = 1;\n}\nexport { f as f };\n",
        "function f() {\n  let a;\n  return 1;\n}\nexport { f as f };\n",
    );
    t.test("function f() {\n  let z;\n  x = 1;\n  {\n    let g;\n    return x\n  }\n  let y\n}\nexport { f as f };\n", "function f() {\n  let y;\n  let z;\n  x = 1;\n  let g;\n  return x;\n}\nexport { f as f };\n");
}

// port: PeepholeIntegrationTest#testRemoveDuplicateStatementsIntegration
#[test]
fn test_remove_duplicate_statements_integration() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("function z() {if (a) { return true }\nelse if (b) { return true }\nelse { return true }}\n", "function z() {return true;}");
    t.test("function z() {if (a()) { return true }\nelse if (b()) { return true }\nelse { return true }}\n", "function z() {a()||b();return true;}");
}

// port: PeepholeIntegrationTest#testRemoveUnreachableCode1
#[test]
fn test_remove_unreachable_code1() {
    let mut t = PeepholeIntegrationTest::new();
    // switch statement with stuff after "return"
    t.test("function foo(){\n  switch (foo) {\n    case 1:\n      x=1;\n      return;\n      break;\n    case 2: {\n      x=2;\n      return;\n      break;\n    }\n    default:\n  }\n}\n", "function foo() {\n  switch (foo) {\n    case 1:\n      x=1;\n      break;\n    case 2:\n      x=2;\n  }\n}\n");
}

// port: PeepholeIntegrationTest#testRemoveUnreachableCode2
#[test]
fn test_remove_unreachable_code2() {
    let mut t = PeepholeIntegrationTest::new();
    // if/else statements with returns
    t.test("function bar(){\n  if (foo)\n    x=1;\n  else if(bar) {\n    return;\n    x=2;\n  } else {\n    x=3;\n    return;\n    x=4;\n  }\n  return 5;\n  x=5;\n}\n", "function bar() {\n  if (foo) {\n    x=1;\n    return 5;\n  }\n  bar || (x = 3);\n}\n");
    // if statements without blocks
    // NOTE: This pass should never see while-loops, because normalization replaces them all with
    // for-loops.
    t.test("function foo() {\n  if (x == 3) return;\n  x = 4;\n  y++;\n  for (; y == 4; ) {\n    return;\n    x = 3\n  }\n}\n", "function foo() {\n  if (x != 3) {\n    x = 4;\n    y++;\n    for (; y == 4; ) {\n      break\n    }\n  }\n}\n");
    // for/do/while loops
    t.test("function baz() {\n// Normalize always moves the for-loop initializer out of the loop.\n  i = 0;\n  for (; i < n; i++) {\n    x = 3;\n    break;\n    x = 4\n  }\n  do {\n    x = 2;\n    break;\n    x = 4\n  } while (x == 4);\n  for (; i < 4; ) {\n    x = 3;\n    return;\n    x = 6\n  }\n}\n", "function baz() {\n  i = 0;\n  for (; i < n; i++) {\n    x = 3;\n    break\n  }\n  do {\n    x = 2;\n    break\n  } while (x == 4);\n  for (; i < 4; ) {\n    x = 3;\n    break;\n  }\n}\n");
    // return statements on the same level as conditionals
    t.test("function foo() {\n  if (x == 3) {\n    return\n  }\n  return 5;\n  while (y == 4) {\n    x++;\n    return;\n    x = 4\n  }\n}\n", "function foo() {\n  return x == 3 ? void 0 : 5;\n}\n");
    // return statements on the same level as conditionals
    t.test("function foo() {\n  return 3;\n  for (; y == 4;) {\n    x++;\n    return;\n    x = 4\n  }\n}\n", "function foo() {\n  return 3\n}\n");
    // try/catch statements
    t.test("function foo() {\n  try {\n    x = 3;\n    return x + 1;\n    x = 5\n  } catch (e) {\n    x = 4;\n    return 5;\n    x = 5\n  }\n}\n", "function foo() {\n  try {\n    x = 3;\n    return x + 1\n  } catch (e) {\n    x = 4;\n    return 5\n  }\n}\n");
    // try/finally statements
    t.test("function foo() {\n  try {\n    x = 3;\n    return x + 1;\n    x = 5\n  } finally {\n    x = 4;\n    return 5;\n    x = 5\n  }\n}\n", "function foo() {\n  try {\n    x = 3;\n    return x + 1\n  } finally {\n    x = 4;\n    return 5\n  }\n}\n");
    // try/catch/finally statements
    t.test("function foo() {\n  try {\n    x = 3;\n    return x + 1;\n    x = 5\n  } catch (e) {\n    x = 3;\n    return;\n    x = 2\n  } finally {\n    x = 4;\n    return 5;\n    x = 5\n  }\n}\n", "function foo() {\n  try {\n    x = 3;\n    return x + 1\n  } catch (e) {\n    x = 3;\n  } finally {\n    x = 4;\n    return 5\n  }\n}\n");
    // test a combination of blocks
    t.test("function foo() {\n  x = 3;\n  if (x == 4) {\n    x = 5;\n    return;\n    x = 6\n  } else {\n    x = 7\n  }\n  return 5;\n  x = 3\n}\n", "function foo() {\n  x = 3;\n  if (x == 4) {\n    x = 5;\n  } else {\n    x = 7\n    return 5\n  }\n}\n");
    // test removing multiple statements
    t.test(
        "function foo() {\n  return 1;\n  var x = 2;\n  var y = 10;\n  return 2;\n}\n",
        "function foo() {\n  var y;\n  var x;\n  return 1\n}\n",
    );
    t.test(
        "function foo() {\n  return 1;\n  x = 2;\n  y = 10;\n  return 2;\n}\n",
        "function foo() {\n  return 1\n}\n",
    );
}

// port: PeepholeIntegrationTest#testRemoveUnreachableCodeInComputedPropertIife
#[test]
fn test_remove_unreachable_code_in_computed_propert_iife() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "class Foo {\n  [function() {\n    1; return 'x';\n  }()]() { return 1; }\n}\n",
        "class Foo {\n  [function() {\n    return 'x';\n  }()]() { return 1; }\n}\n",
    );
}

// port: PeepholeIntegrationTest#testRemoveUnreachableCode_withES6Modules
#[test]
fn test_remove_unreachable_code_with_es6_modules() {
    let mut t = PeepholeIntegrationTest::new();
    // Switch statements
    t.test("function foo() {\n  switch (foo) {\n    case 1:\n      x = 1;\n      return;\n      break;\n    case 2: {\n      x = 2;\n      return;\n      break;\n    }\n    default:\n  }\n}\nexport { foo as foo };\n", "function foo() {\n  switch (foo) {\n    case 1:\n      x = 1;\n      break;\n    case 2:\n      x = 2;\n  }\n}\nexport { foo as foo };\n");
    // if/else statements with returns
    t.test("function bar() {\n  if (foo)\n    x=1;\n  else if(bar) {\n    return;\n    x=2;\n  } else {\n    x=3;\n    return;\n    x=4;\n  }\n  return 5;\n  x=5;\n}\nexport { bar as bar };\n", "function bar() {\n  if (foo) {\n    x=1;\n    return 5;\n  }\n  bar || (x = 3);\n}\nexport { bar as bar };\n");
}

// port: PeepholeIntegrationTest#testTryCatchFinally6
#[test]
fn test_try_catch_finally6() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {return; try{var x = 1}catch(e){} }",
        "function f() {var x;}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn1
#[test]
fn test_useless_unconditional_return1() {
    let mut t = PeepholeIntegrationTest::new();
    t.test("function foo() { return }", "function foo() { }");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn2
#[test]
fn test_useless_unconditional_return2() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() { return; return; x=1 }",
        "function foo() { }",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn3
#[test]
fn test_useless_unconditional_return3() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() { return; return; var x=1}",
        "function foo() {var x}",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn4
#[test]
fn test_useless_unconditional_return4() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function foo() { return; function bar() {} }",
        "function foo() {         function bar() {} }",
    );
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn5
#[test]
fn test_useless_unconditional_return5() {
    let mut t = PeepholeIntegrationTest::new();
    t.test_same("function foo() { return 5 }");
}

// port: PeepholeIntegrationTest#testUselessUnconditionalReturn6
#[test]
fn test_useless_unconditional_return6() {
    let mut t = PeepholeIntegrationTest::new();
    t.test(
        "function f() {switch (a) { case 'a': return}}",
        "function f() {}",
    );
}
