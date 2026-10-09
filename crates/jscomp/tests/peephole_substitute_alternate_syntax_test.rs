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
//   test/com/google/javascript/jscomp/PeepholeSubstituteAlternateSyntaxTest.java.

//! Port of `PeepholeSubstituteAlternateSyntaxTest.java`: tests for PeepholeSubstituteAlternateSyntax
//! in isolation. Tests for the interaction of multiple peephole passes are in
//! PeepholeIntegrationTest.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    abstract_peephole_optimization::AbstractPeepholeOptimization, compiler_pass::CompilerPass,
    peephole_optimizations_pass::PeepholeOptimizationsPass,
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

// port: PeepholeSubstituteAlternateSyntaxTest#FOLD_CONSTANTS_TEST_EXTERNS
// Externs for built-in constructors
// Needed for testFoldLiteralObjectConstructors(),
// testFoldLiteralArrayConstructors() and testFoldRegExp...()
const FOLD_CONSTANTS_TEST_EXTERNS: &str = "var window = {};\nvar Object = function f(){};\nvar RegExp = function f(a){};\nvar Array = function f(a){};\nwindow.foo = null;\n";

struct PeepholeSubstituteAlternateSyntaxTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
    late: bool,
    retraverse_on_change: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: PeepholeSubstituteAlternateSyntaxTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let name = self.get_name();
        let late = self.late;
        let retraverse_on_change = self.retraverse_on_change;
        let pass: Box<dyn CompilerPass> = Box::new(
            move |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                let optimizations: Vec<Box<dyn AbstractPeepholeOptimization>> =
                    vec![Box::new(PeepholeSubstituteAlternateSyntax::new(late))];
                let mut peephole_pass = PeepholeOptimizationsPass::new(name.clone(), optimizations);
                peephole_pass.set_retraverse_on_change(retraverse_on_change);
                peephole_pass.process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    // port: CompilerTestCase#getName (this.getClass().getSimpleName())
    fn get_name(&self) -> String {
        "PeepholeSubstituteAlternateSyntaxTest".into()
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl PeepholeSubstituteAlternateSyntaxTest {
    // port: PeepholeSubstituteAlternateSyntaxTest#PeepholeSubstituteAlternateSyntaxTest
    // port: PeepholeSubstituteAlternateSyntaxTest#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(FOLD_CONSTANTS_TEST_EXTERNS);
        harness.set_up();
        let late = true;
        let retraverse_on_change = false;
        harness.disable_normalize().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "PeepholeSubstituteAlternateSyntaxTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
                late,
                retraverse_on_change,
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

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldRegExpConstructor
#[test]
fn test_fold_reg_exp_constructor() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // Cannot fold all the way to a literal because there are too few arguments.
    t.test("x = new RegExp", "x = RegExp()");
    // Could fold all the way to /foobar/, but the complexity of that optimization wasn't worth the
    // code size improvements, mainly because there are a lot special cases to check.
    t.test("x = new RegExp(\"foobar\")", "x = RegExp(\"foobar\")");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldLiteralObjectConstructors
#[test]
fn test_fold_literal_object_constructors() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    // Can fold when normalized
    t.test("x = new Object", "x = ({})");
    t.test("x = new Object()", "x = ({})");
    t.test("x = Object()", "x = ({})");
    t.harness.disable_normalize().unwrap();
    // Cannot fold above when not normalized
    t.test_same("x = new Object");
    t.test_same("x = new Object()");
    t.test_same("x = Object()");
    t.harness.enable_normalize().unwrap();
    // Cannot fold, the constructor being used is actually a local function
    t.test_same("x =\n(function f(){function Object(){this.x=4};return new Object();})();\n");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldLiteralObjectConstructors_onWindow
#[test]
fn test_fold_literal_object_constructors_on_window() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    // Can fold when normalized
    t.test("x = new window.Object", "x = ({})");
    t.test("x = new window.Object()", "x = ({})");
    // Mustn't fold optional chains
    t.test("x = window.Object()", "x = ({})");
    t.test("x = window.Object?.()", "x = Object?.()");
    t.harness.disable_normalize().unwrap();
    // Cannot fold above when not normalized
    t.test_same("x = new window.Object");
    t.test_same("x = new window.Object()");
    t.test_same("x = window.Object()");
    t.test_same("x = window?.Object()");
    t.harness.enable_normalize().unwrap();
    // Can fold, the window namespace ensures it's not a conflict with the local Object.
    t.test(
        "x = (function f(){function Object(){this.x=4};return new window.Object;})();",
        "x = (function f(){function Object(){this.x=4};return {};})();",
    );
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldLiteralArrayConstructors
#[test]
fn test_fold_literal_array_constructors() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // No arguments - can fold when normalized
    t.test("x = new Array", "x = []");
    t.test("x = new Array()", "x = []");
    t.test("x = Array()", "x = []");
    t.test_same("x = Array?.()");
    // Mustn't fold optional chains
    // One argument - can be fold when normalized
    t.test("x = new Array(0)", "x = []");
    t.test("x = Array(0)", "x = []");
    t.test("x = new Array(\"a\")", "x = [\"a\"]");
    t.test("x = Array(\"a\")", "x = [\"a\"]");
    // One argument - cannot be fold when normalized
    t.test("x = new Array(7)", "x = Array(7)");
    t.test_same("x = Array(7)");
    t.test("x = new Array(y)", "x = Array(y)");
    t.test_same("x = Array(y)");
    t.test("x = new Array(foo())", "x = Array(foo())");
    t.test_same("x = Array(foo())");
    // More than one argument - can be fold when normalized
    t.test("x = new Array(1, 2, 3, 4)", "x = [1, 2, 3, 4]");
    t.test("x = Array(1, 2, 3, 4)", "x = [1, 2, 3, 4]");
    t.test(
        "x = new Array('a', 1, 2, 'bc', 3, {}, 'abc')",
        "x = ['a', 1, 2, 'bc', 3, {}, 'abc']",
    );
    t.test(
        "x = Array('a', 1, 2, 'bc', 3, {}, 'abc')",
        "x = ['a', 1, 2, 'bc', 3, {}, 'abc']",
    );
    t.test(
        "x = new Array(Array(1, '2', 3, '4'))",
        "x = [[1, '2', 3, '4']]",
    );
    t.test("x = Array(Array(1, '2', 3, '4'))", "x = [[1, '2', 3, '4']]");
    t.test(
        "x = new Array(Object(), Array(\"abc\", Object(), Array(Array())))",
        "x = [{}, [\"abc\", {}, [[]]]]",
    );
    t.test(
        "x = new Array(Object(), Array(\"abc\", Object(), Array(Array())))",
        "x = [{}, [\"abc\", {}, [[]]]]",
    );
    t.harness.disable_normalize().unwrap();
    // Cannot fold above when not normalized
    t.test_same("x = new Array");
    t.test_same("x = new Array()");
    t.test_same("x = Array()");
    t.test_same("x = new Array(0)");
    t.test_same("x = Array(0)");
    t.test_same("x = new Array(\"a\")");
    t.test_same("x = Array(\"a\")");
    t.test_same("x = new Array(7)");
    t.test_same("x = Array(7)");
    t.test_same("x = new Array(foo())");
    t.test_same("x = Array(foo())");
    t.test_same("x = new Array(1, 2, 3, 4)");
    t.test_same("x = Array(1, 2, 3, 4)");
    t.test_same("x = new Array('a', 1, 2, 'bc', 3, {}, 'abc')");
    t.test_same("x = Array('a', 1, 2, 'bc', 3, {}, 'abc')");
    t.test_same("x = new Array(Array(1, '2', 3, '4'))");
    t.test_same("x = Array(Array(1, '2', 3, '4'))");
    t.test_same("x = new Array(\nObject(), Array(\"abc\", Object(), Array(Array())))\n");
    t.test_same("x = new Array(\nObject(), Array(\"abc\", Object(), Array(Array())))\n");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testRemoveWindowRefs
#[test]
fn test_remove_window_refs() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test("x = window.Object", "x = Object");
    t.test("x = window.Object.keys", "x = Object.keys");
    t.test("if (window.Object) {}", "if (Object) {}");
    t.test("x = window.Object", "x = Object");
    t.test("x = window.Array", "x = Array");
    t.test("x = window.Error", "x = Error");
    t.test("x = window.RegExp", "x = RegExp");
    t.test("x = window.Math", "x = Math");
    // Not currently handled by the pass but should be folded in the future.
    t.test_same("x = window.String");
    // Don't fold properties on the window.
    t.test_same("x = window.foo");
    t.harness.disable_normalize().unwrap();
    // Cannot fold when not normalized
    t.test_same("x = window.Object");
    t.test_same("x = window.Object.keys");
    t.harness.enable_normalize().unwrap();
    t.test_same("var x =\n(function f(){var window = {Object: function() {}};return new window.Object;})();\n");
}

/// Tests that it's safe to fold access on window that is non-optional but is under an optional
/// chain in the AST.
/// /
// port: PeepholeSubstituteAlternateSyntaxTest#testRemoveWindowRef_childOfOptionalChain
#[test]
fn test_remove_window_ref_child_of_optional_chain() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // ref on window can be folded preserving the optional access.
    t.test("x = window.Object?.keys", "x = Object?.keys");
    t.test("x = window.Object?.(keys)", "x = Object?.(keys)");
    // Show that the above works only on `BUILTIN_EXTERNS` such as Object but not on regular prop
    // accesses
    t.test_same("x = window.prop?.keys");
    t.test_same("x = window.prop?.(keys)");
}

/// There isn't an obvious reason to write `window?.Boolean()` or `window?.Error()`, but if one
/// did, presumably there's some reason to believe that `window` won't be there. We don't optimize
/// away these checks.
/// /
// port: PeepholeSubstituteAlternateSyntaxTest#testDontRemoveWindowRefs_optChain
#[test]
fn test_dont_remove_window_refs_opt_chain() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.harness.enable_normalize().unwrap();
    // Don't fold the optional chain check on window
    t.test_same("x = window?.Object");
    t.test_same("if (window?.Object) {}");
    t.test_same("x = window?.Object");
    t.test_same("x = window?.Array");
    t.test_same("x = window?.Error");
    t.test_same("x = window?.RegExp");
    t.test_same("x = window?.Math");
    t.test_same("x = window?.String");
    // Don't fold properties on the window anyway (non-optional or optional).
    t.test_same("x = window.foo");
    t.test_same("x = window?.foo");
    t.harness.disable_normalize().unwrap();
    // Cannot fold when not normalized
    t.test_same("x = window?.Object");
    t.test_same("x = window.Object?.keys");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldStandardConstructors
#[test]
fn test_fold_standard_constructors() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test_same("new Foo('a')");
    t.test_same("var x = new goog.Foo(1)");
    t.test_same("var x = new String(1)");
    t.test_same("var x = new Number(1)");
    t.test_same("var x = new Boolean(1)");
    t.harness.enable_normalize().unwrap();
    t.test("var x = new Object('a')", "var x = Object('a')");
    t.test("var x = new RegExp('')", "var x = RegExp('')");
    t.test("var x = new Error('20')", "var x = Error(\"20\")");
    t.test("var x = new Array(20)", "var x = Array(20)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldTrueFalse
#[test]
fn test_fold_true_false() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("x = true", "x = !0");
    t.test("x = false", "x = !1");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldTrueFalseComparison
#[test]
fn test_fold_true_false_comparison() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("x == true", "x == 1");
    t.test("x == false", "x == 0");
    t.test("x != true", "x != 1");
    t.test("x < true", "x < 1");
    t.test("x <= true", "x <= 1");
    t.test("x > true", "x > 1");
    t.test("x >= true", "x >= 1");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldSubtractionAssignment
#[test]
fn test_fold_subtraction_assignment() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("x -= 1", "--x");
    t.test("x -= -1", "++x");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testFoldReturnResult
#[test]
fn test_fold_return_result() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test_same("function f(){return !1;}");
    t.test_same("function f(){return null;}");
    t.test("function f(){return void 0;}", "function f(){return}");
    t.test_same("function f(){return void foo();}");
    t.test("function f(){return undefined;}", "function f(){return}");
    t.test(
        "function f(){if(a()){return undefined;}}",
        "function f(){if(a()){return}}",
    );
}

// port: PeepholeSubstituteAlternateSyntaxTest#testUndefined
#[test]
fn test_undefined() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test_same("var x = undefined");
    t.test_same("function f(f) {var undefined=2;var x = undefined;}");
    t.harness.enable_normalize().unwrap();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test("var x = undefined", "var x=void 0");
    t.test_same("var undefined = 1;\nfunction f() {var undefined=2;var x = undefined;}\n");
    t.test_same("function f(undefined) {}");
    t.test_same("try {} catch(undefined) {}");
    t.test_same("for (undefined in {}) {}");
    t.test_same("undefined++;");
    t.harness.disable_normalize().unwrap();
    t.test_same("undefined += undefined;");
    t.harness.enable_normalize().unwrap();
    t.test("undefined += undefined;", "undefined = void 0 + void 0;");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testSplitCommaExpressions
#[test]
fn test_split_comma_expressions() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    // Don't try to split in expressions.
    t.test_same("while (foo(), !0) boo()");
    t.test_same("var a = (foo(), !0);");
    t.test_same("a = (foo(), !0);");
    // Don't try to split COMMA under LABELs.
    t.test_same("a:a(),b()");
    t.test("1, 2, 3, 4", "1; 2; 3; 4");
    t.test("x = 1, 2, 3", "x = 1; 2; 3");
    t.test_same("x = (1, 2, 3)");
    t.test("1, (2, 3), 4", "1; 2; 3; 4");
    t.test("(x=2), foo()", "x=2; foo()");
    t.test("foo(), boo();", "foo(); boo()");
    t.test("(a(), b()), (c(), d());", "a(); b(); c(); d()");
    t.test("a(); b(); (c(), d());", "a(); b(); c(); d();");
    t.test("foo(), true", "foo();true");
    t.test_same("foo();true");
    t.test("function x(){foo(), !0}", "function x(){foo(); !0}");
    t.test_same("function x(){foo(); !0}");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testComma1
#[test]
fn test_comma1() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    t.test("1, 2", "1; 2");
    t.hooks.late = true;
    t.test_same("1, 2");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testComma2
#[test]
fn test_comma2() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    t.test("1, a()", "1; a()");
    t.test("1, a?.()", "1; a?.()");
    t.hooks.late = true;
    t.test_same("1, a()");
    t.test_same("1, a?.()");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testComma3
#[test]
fn test_comma3() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    t.test("1, a(), b()", "1; a(); b()");
    t.test("1, a?.(), b?.()", "1; a?.(); b?.()");
    t.hooks.late = true;
    t.test_same("1, a(), b()");
    t.test_same("1, a?.(), b?.()");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testComma4
#[test]
fn test_comma4() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    t.test("a(), b()", "a();b()");
    t.test("a?.(), b?.()", "a?.();b?.()");
    t.hooks.late = true;
    t.test_same("a(), b()");
    t.test_same("a?.(), b?.()");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testComma5
#[test]
fn test_comma5() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.hooks.late = false;
    t.test("a(), b(), 1", "a(); b(); 1");
    t.test("a?.(), b?.(), 1", "a?.(); b?.(); 1");
    t.hooks.late = true;
    t.test_same("a(), b(), 1");
    t.test_same("a?.(), b?.(), 1");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testStringArraySplitting
#[test]
fn test_string_array_splitting() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test_same("var x=['1','2','3','4']");
    t.test_same("var x=['1','2','3','4','5']");
    t.test(
        "var x=['1','2','3','4','5','6']",
        "var x='123456'.split('')",
    );
    t.test(
        "var x=['1','2','3','4','5','00']",
        "var x='1 2 3 4 5 00'.split(' ')",
    );
    t.test(
        "var x=['1','2','3','4','5','6','7']",
        "var x='1234567'.split('')",
    );
    t.test(
        "var x=['1','2','3','4','5','6','00']",
        "var x='1 2 3 4 5 6 00'.split(' ')",
    );
    t.test(
        "var x=[' ,',',',',',',',',',',']",
        "var x=' ,;,;,;,;,;,'.split(';')",
    );
    t.test(
        "var x=[',,',' ',',',',',',',',']",
        "var x=',,; ;,;,;,;,'.split(';')",
    );
    t.test(
        "var x=['a,',' ',',',',',',',',']",
        "var x='a,; ;,;,;,;,'.split(';')",
    );
    // all possible delimiters used, leave it alone
    t.test_same("var x=[',', ' ', ';', '{', '}']");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testTemplateStringToString
#[test]
fn test_template_string_to_string() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("`abcde`", "'abcde'");
    t.test("`ab cd ef`", "'ab cd ef'");
    t.test_same("`hello ${name}`");
    t.test_same("tag `hello ${name}`");
    t.test_same("tag `hello`");
    t.test("`hello ${'foo'}`", "'hello foo'");
    t.test("`${2} bananas`", "'2 bananas'");
    t.test("`This is ${true}`", "'This is true'");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testBindToCall1
#[test]
fn test_bind_to_call1() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("(goog.bind(f))()", "f()");
    t.test("(goog.bind(f,a))()", "f.call(a)");
    t.test("(goog.bind(f,a,b))()", "f.call(a,b)");
    t.test("(goog.bind(f))(a)", "f(a)");
    t.test("(goog.bind(f,a))(b)", "f.call(a,b)");
    t.test("(goog.bind(f,a,b))(c)", "f.call(a,b,c)");
    t.test("(goog.partial(f))()", "f()");
    t.test("(goog.partial(f,a))()", "f(a)");
    t.test("(goog.partial(f,a,b))()", "f(a,b)");
    t.test("(goog.partial(f))(a)", "f(a)");
    t.test("(goog.partial(f,a))(b)", "f(a,b)");
    t.test("(goog.partial(f,a,b))(c)", "f(a,b,c)");
    t.test("((function(){}).bind())()", "((function(){}))()");
    t.test("((function(){}).bind(a))()", "((function(){})).call(a)");
    t.test("((function(){}).bind(a,b))()", "((function(){})).call(a,b)");
    t.test("((function(){}).bind())(a)", "((function(){}))(a)");
    t.test("((function(){}).bind(a))(b)", "((function(){})).call(a,b)");
    t.test(
        "((function(){}).bind(a,b))(c)",
        "((function(){})).call(a,b,c)",
    );
    // Without using type information we don't know "f" is a function.
    t.test_same("(f.bind())()");
    t.test_same("(f.bind(a))()");
    t.test_same("(f.bind())(a)");
    t.test_same("(f.bind(a))(b)");
    // Don't rewrite if the bind isn't the immediate call target
    t.test_same("(goog.bind(f)).call(g)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testBindToCall2
#[test]
fn test_bind_to_call2() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("(goog$bind(f))()", "f()");
    t.test("(goog$bind(f,a))()", "f.call(a)");
    t.test("(goog$bind(f,a,b))()", "f.call(a,b)");
    t.test("(goog$bind(f))(a)", "f(a)");
    t.test("(goog$bind(f,a))(b)", "f.call(a,b)");
    t.test("(goog$bind(f,a,b))(c)", "f.call(a,b,c)");
    t.test("(goog$partial(f))()", "f()");
    t.test("(goog$partial(f,a))()", "f(a)");
    t.test("(goog$partial(f,a,b))()", "f(a,b)");
    t.test("(goog$partial(f))(a)", "f(a)");
    t.test("(goog$partial(f,a))(b)", "f(a,b)");
    t.test("(goog$partial(f,a,b))(c)", "f(a,b,c)");
    // Don't rewrite if the bind isn't the immediate call target
    t.test_same("(goog$bind(f)).call(g)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testBindToCall3
#[test]
fn test_bind_to_call3() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    // TODO(johnlenz): The code generator wraps free calls with (0,...) to
    // prevent leaking "this", but the parser doesn't unfold it, making a
    // AST comparison fail.  For now do a string comparison to validate the
    // correct code is in fact generated.
    // The FREE call wrapping should be moved out of the code generator
    // and into a denormalizing pass.
    t.harness.disable_compare_as_tree().unwrap();
    t.hooks.retraverse_on_change = true;
    t.hooks.late = false;
    t.test("(goog.bind(f.m))()", "(0,f.m)()");
    t.test("(goog.bind(f.m,a))()", "f.m.call(a)");
    t.test("(goog.bind(f.m))(a)", "(0,f.m)(a)");
    t.test("(goog.bind(f.m,a))(b)", "f.m.call(a,b)");
    t.test("(goog.partial(f.m))()", "(0,f.m)()");
    t.test("(goog.partial(f.m,a))()", "(0,f.m)(a)");
    t.test("(goog.partial(f.m))(a)", "(0,f.m)(a)");
    t.test("(goog.partial(f.m,a))(b)", "(0,f.m)(a,b)");
    // Without using type information we don't know "f" is a function.
    t.test_same("f.m.bind()()");
    t.test_same("f.m.bind(a)()");
    t.test_same("f.m.bind()(a)");
    t.test_same("f.m.bind(a)(b)");
    // Don't rewrite if the bind isn't the immediate call target
    t.test_same("goog.bind(f.m).call(g)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testSimpleFunctionCall1
#[test]
fn test_simple_function_call1() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("var a = String(23)", "var a = '' + 23");
    // Don't fold the existence check to preserve behavior
    t.test_same("var a = String?.(23)");
    t.test("var a = String('hello')", "var a = '' + 'hello'");
    // Don't fold the existence check to preserve behavior
    t.test_same("var a = String?.('hello')");
    t.test_same("var a = String('hello', bar());");
    t.test_same("var a = String({valueOf: function() { return 1; }});");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testSimpleFunctionCall2
#[test]
fn test_simple_function_call2() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("var a = Boolean(true)", "var a = !0");
    // Don't fold the existence check to preserve behavior
    t.test("var a = Boolean?.(true)", "var a = Boolean?.(!0)");
    t.test("var a = Boolean(false)", "var a = !1");
    // Don't fold the existence check to preserve behavior
    t.test("var a = Boolean?.(false)", "var a = Boolean?.(!1)");
    t.test("var a = Boolean(1)", "var a = !!1");
    // Don't fold the existence check to preserve behavior
    t.test_same("var a = Boolean?.(1)");
    t.test("var a = Boolean(x)", "var a = !!x");
    // Don't fold the existence check to preserve behavior
    t.test_same("var a = Boolean?.(x)");
    t.test("var a = Boolean({})", "var a = !!{}");
    // Don't fold the existence check to preserve behavior
    t.test_same("var a = Boolean?.({})");
    t.test_same("var a = Boolean()");
    t.test_same("var a = Boolean(!0, !1);");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testRotateAssociativeOperators
#[test]
fn test_rotate_associative_operators() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    // Multiplication is not associative because it can include floating point numbers e.g.
    // 1e-300 * 1e300 * 1e9 does not equal 1e-300 * (1e300 * 1e9).
    t.test(
        "a || (b || c); a * (b * c); a | (b | c)",
        "a || b || c; b * c * a; a | b | c",
    );
    t.test_same("a % (b % c); a / (b / c); a - (b - c);");
    t.test_same("(a / b) & (c % d)");
    t.test_same("(c = 5) & (c % d)");
    t.test_same("(a + b) * c * (d % e)");
    t.test("(a + b) * (c % d)", "c % d * (a + b)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testRotateCommutativeeOperators
#[test]
fn test_rotate_commutativee_operators() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("a * (b % c);", "b % c * a");
    t.test_same("a * b * (c / d)");
    t.test_same("!a * c * (d % e)");
}

// port: PeepholeSubstituteAlternateSyntaxTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("a ?? (b ?? c);", "(a ?? b) ?? c");
}

// port: PeepholeSubstituteAlternateSyntaxTest#testNoRotateInfiniteLoop
#[test]
fn test_no_rotate_infinite_loop() {
    let mut t = PeepholeSubstituteAlternateSyntaxTest::new();
    t.test("1/x || (y/1 ||(1/z))", "1/x || (y/1) || (1/z)");
    t.test_same("1/x || (y/1) || (1/z)");
}
