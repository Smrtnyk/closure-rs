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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/RemoveUnusedCodeNameAnalyzerTest.java.

//! Port of RemoveUnusedCodeNameAnalyzerTest.java: tests `RemoveUnusedCode` for functionality that
//! was previously implemented in NameAnalyzer aka smartNamePass, which has now been removed. The
//! tests run on the native CompilerTestCase port of crates/testing. The 25 `disabledTest*` methods
//! are `@Ignore`d in Java (JUnit never runs them), so they are not ported as tests.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    pure_function_identifier::Driver, remove_unused_code::RemoveUnusedCode,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc};

/// Java's string concatenation `a + b + ...`.
fn cat(parts: &[&str]) -> String {
    parts.concat()
}

// port: RemoveUnusedCodeNameAnalyzerTest#EXTERNS
const EXTERNS: &str = r#"/**
 * @constructor
 * @param {*=} opt_value
 * @return {!Object}
 */
function Object(opt_value) {}
/** @type {Function} */
Object.defineProperties = function() {};
/**
 * @constructor
 * @param {string} message
 */
function Error(message) {}
var window, top, console;
var document;
var Function;
var Array;
var goog = {};
goog.inherits = function(childClass, parentClass) {};
goog.mixin = function(target, base) {};
function goog$addSingletonGetter(a){}
var externfoo = {};
externfoo.externProp1 = 1;
externfoo.externProp2 = 2;
function doThing1() {}
function doThing2() {}
function use(something) {}
function alert(something) {}
function sideEffect() {}
"#;

// port: RemoveUnusedCodeNameAnalyzerTest.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
struct MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    pure_function_identifier: Driver,
    remove_unused_code: RemoveUnusedCode,
}

impl MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    // port: RemoveUnusedCodeNameAnalyzerTest.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#MarkNoSideEffectCallsAndRemoveUnusedCodeRunner
    fn new(compiler: &AbstractCompiler) -> Self {
        Self {
            pure_function_identifier: Driver::new(),
            remove_unused_code: RemoveUnusedCode::builder(compiler)
                .remove_globals(true)
                .remove_local_vars(true)
                .remove_unused_prototype_properties(true)
                .remove_unused_this_properties(true)
                .remove_unused_object_define_properties_definitions(true)
                // Removal of function expression names isn't what these tests are about.
                // It just adds noise to the tests when we can't use testSame() because of it.
                .preserve_function_expression_names(true)
                .build(),
        }
    }
}

impl CompilerPass for MarkNoSideEffectCallsAndRemoveUnusedCodeRunner {
    // port: RemoveUnusedCodeNameAnalyzerTest.MarkNoSideEffectCallsAndRemoveUnusedCodeRunner#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.pure_function_identifier
            .process(compiler, externs, root);
        self.remove_unused_code.process(compiler, externs, root);
    }
}

struct RemoveUnusedCodeNameAnalyzerTest {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for RemoveUnusedCodeNameAnalyzerTest {
    // port: RemoveUnusedCodeNameAnalyzerTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let runner = MarkNoSideEffectCallsAndRemoveUnusedCodeRunner::new(&compiler.borrow());
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(runner)))))
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: RemoveUnusedCodeNameAnalyzerTest,
}

impl Fixture {
    // port: RemoveUnusedCodeNameAnalyzerTest#RemoveUnusedCodeNameAnalyzerTest
    // port: RemoveUnusedCodeNameAnalyzerTest#setUp
    fn new() -> Self {
        let mut h = CompilerTestCase::new(EXTERNS);
        h.set_up();
        // Allow testing of features that aren't fully supported for output yet.
        h.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        h.enable_normalize_expected_output().unwrap();
        h.enable_gather_extern_properties().unwrap();
        let t = RemoveUnusedCodeNameAnalyzerTest {
            ctx: Ctx::new(
                "RemoveUnusedCodeNameAnalyzerTest".into(),
                object([]),
                IndexMap::new(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
        };
        Self { h, t }
    }

    // port: CompilerTestCase#test(String,String)
    fn test(&mut self, js: &str, expected: &str) {
        self.h.test_strings(&mut self.t, js, expected).unwrap();
    }

    // port: CompilerTestCase#testSame(String)
    fn test_same(&mut self, js: &str) {
        self.h.test_same_string(&mut self.t, js).unwrap();
    }

    // port: CompilerTestCase#test(TestPart...)
    fn test_parts(&mut self, parts: Vec<TestPart>) {
        self.h.test(&mut self.t, parts).unwrap();
    }
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDefaultingAssignmentWithAssignedProperty
#[test]
fn test_defaulting_assignment_with_assigned_property() {
    let mut f = Fixture::new();
    f.test(
        "var x = function() {}; x.externProp1 = function() {}; var x = x || {};",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration1
#[test]
fn test_remove_var_declaration1() {
    let mut f = Fixture::new();
    f.test("var foo = 3;", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration2
#[test]
fn test_remove_var_declaration2() {
    let mut f = Fixture::new();
    f.test(
        "var foo = 3, bar = 4; externfoo = foo;",
        "var foo = 3; externfoo = foo;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration3
#[test]
fn test_remove_var_declaration3() {
    let mut f = Fixture::new();
    // TODO(b/66971163): Don't block removal when the reference to a name is not actually used.
    // e.g. `b; c` below.
    // preserve newline
    f.test(
        "var a = doThing1(),     b = 1, c = 2; b; c",
        "        doThing1(); var b = 1, c = 2; b; c",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration4
#[test]
fn test_remove_var_declaration4() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a = 0, b = doThing1(),     c = 2; a; c",
        "var a = 0;     doThing1(); var c = 2; a; c",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration5
#[test]
fn test_remove_var_declaration5() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a = 0, b = 1, c = doThing1(); a; b",
        "var a = 0, b = 1;     doThing1(); a; b",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration6
#[test]
fn test_remove_var_declaration6() {
    let mut f = Fixture::new();
    f.test("var a = 0, b = a = 1; a", "var a = 0; a = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration7
#[test]
fn test_remove_var_declaration7() {
    let mut f = Fixture::new();
    f.test("var a = 0, b = a = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveVarDeclaration8
#[test]
fn test_remove_var_declaration8() {
    let mut f = Fixture::new();
    f.test("var a;var b = 0, c = a = b = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration1
#[test]
fn test_remove_let_declaration1() {
    let mut f = Fixture::new();
    f.test("let foo = 3;", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration2
#[test]
fn test_remove_let_declaration2() {
    let mut f = Fixture::new();
    f.test(
        "let foo = 3, bar = 4; externfoo = foo;",
        "let foo = 3; externfoo = foo;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration3
#[test]
fn test_remove_let_declaration3() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "let a = doThing1(),     b = 1, c = 2; b; c",
        "        doThing1(); let b = 1, c = 2; b; c",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration4
#[test]
fn test_remove_let_declaration4() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "let a = 0, b = doThing1(),     c = 2; a; c",
        "let a = 0;     doThing1(); let c = 2; a; c",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration5
#[test]
fn test_remove_let_declaration5() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "let a = 0, b = 1, c = doThing1(); a; b",
        "let a = 0, b = 1;     doThing1(); a; b",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration6
#[test]
fn test_remove_let_declaration6() {
    let mut f = Fixture::new();
    f.test("let a = 0, b = a = 1; a", "let a = 0; a = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration7
#[test]
fn test_remove_let_declaration7() {
    let mut f = Fixture::new();
    f.test("let a = 0, b = a = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration8
#[test]
fn test_remove_let_declaration8() {
    let mut f = Fixture::new();
    f.test("let a;let b = 0, c = a = b = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetDeclaration9
#[test]
fn test_remove_let_declaration9() {
    let mut f = Fixture::new();
    // The variable inside the block doesn't get removed (but does get renamed by Normalize).
    f.test(
        "let x = 1; if (true) { let x = 2; x; }",
        "if (true) { let x$jscomp$1 = 2; x$jscomp$1; }",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetInBlock1
#[test]
fn test_remove_let_in_block1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"if (true) { // preserve newline
  let x = 1; alert(x);
}
"#,
    );
    // preserve newline
    f.test("if (true) { let x = 1; }", "if (true)              ;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLetInBlock2
#[test]
fn test_remove_let_in_block2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"if (true) { // preserve newline
  let x = 1; alert(x);
} else {
  let x = 1; alert(x);
}
"#,
    );
    f.test_parts(vec![
        TestPart::Sources(CompilerTestCase::srcs(
            r#"if (true) { // preserve newline
  let x = 1;
} else {
  let x = 1;
}
"#,
        )),
        TestPart::Expected(CompilerTestCase::expected("if (true); else;")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveConstDeclaration1
#[test]
fn test_remove_const_declaration1() {
    let mut f = Fixture::new();
    f.test("const a = 4;", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveConstDeclaration2
#[test]
fn test_remove_const_declaration2() {
    let mut f = Fixture::new();
    f.test_same("const a = 4; window.x = a;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveConstDeclaration3
#[test]
fn test_remove_const_declaration3() {
    let mut f = Fixture::new();
    // The variable inside the block doesn't get removed (but does get renamed by Normalize).
    f.test(
        "const x = 1; if (true) { const x = 2; x; }",
        "if (true) { const x$jscomp$1 = 2; x$jscomp$1; }",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration1
#[test]
fn test_remove_declaration1() {
    let mut f = Fixture::new();
    f.test("var a;var b = 0, c = a = b = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration2
#[test]
fn test_remove_declaration2() {
    let mut f = Fixture::new();
    f.test("var a,b,c; c = a = b = 1", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration4
#[test]
fn test_remove_declaration4() {
    let mut f = Fixture::new();
    f.test(
        "var a,b,c; c = a = b = {}; a.x = 1;alert(c.x);",
        "var a,  c; c = a     = {}; a.x = 1;alert(c.x);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration5
#[test]
fn test_remove_declaration5() {
    let mut f = Fixture::new();
    f.test("var a,b,c; c = a = b = null; use(b)", "var b;b=null;use(b)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration6
#[test]
fn test_remove_declaration6() {
    let mut f = Fixture::new();
    f.test(
        "var a,b,c; c = a = b = 'str';use(b)",
        "var b;b='str';use(b)",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveDeclaration7
#[test]
fn test_remove_declaration7() {
    let mut f = Fixture::new();
    f.test("var a,b,c; c = a = b = true;use(b)", "var b;b=true;use(b)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveFunction1
#[test]
fn test_remove_function1() {
    let mut f = Fixture::new();
    f.test("var foo = function(){};", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveFunction2
#[test]
fn test_remove_function2() {
    let mut f = Fixture::new();
    f.test("var foo; foo = function(){};", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveFunction3
#[test]
fn test_remove_function3() {
    let mut f = Fixture::new();
    f.test("var foo = {}; foo.bar = function() {};", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDontRemoveFunctionOnNamespaceThatEscapes
#[test]
fn test_dont_remove_function_on_namespace_that_escapes() {
    let mut f = Fixture::new();
    f.test_same("var a = doThing1(); a.b = {}; a.b.c = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testReferredToByWindow
#[test]
fn test_referred_to_by_window() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; foo.bar = function() {}; window['fooz'] = foo.bar");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExtern
#[test]
fn test_extern() {
    let mut f = Fixture::new();
    f.test_same("externfoo = 5");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveNamedFunction
#[test]
fn test_remove_named_function() {
    let mut f = Fixture::new();
    f.test("function foo(){}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction1
#[test]
fn test_remove_recursive_function1() {
    let mut f = Fixture::new();
    f.test("function f(){f()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction2
#[test]
fn test_remove_recursive_function2() {
    let mut f = Fixture::new();
    f.test("var f = function (){f()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction2a
#[test]
fn test_remove_recursive_function2a() {
    let mut f = Fixture::new();
    f.test("var f = function g(){g()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction3
#[test]
fn test_remove_recursive_function3() {
    let mut f = Fixture::new();
    f.test("var f;f = function (){f()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction4
#[test]
fn test_remove_recursive_function4() {
    let mut f = Fixture::new();
    // don't remove redefinition of an external variable
    f.test_same("doThing1 = function (){doThing1()}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction5
#[test]
fn test_remove_recursive_function5() {
    let mut f = Fixture::new();
    f.test("function g(){f()}function f(){g()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction6
#[test]
fn test_remove_recursive_function6() {
    let mut f = Fixture::new();
    f.test("var f=function(){g()};function g(){f()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction7
#[test]
fn test_remove_recursive_function7() {
    let mut f = Fixture::new();
    f.test("var g = function(){f()};var f = function(){g()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction8
#[test]
fn test_remove_recursive_function8() {
    let mut f = Fixture::new();
    f.test("var o = {};o.f = function(){o.f()}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveRecursiveFunction9
#[test]
fn test_remove_recursive_function9() {
    let mut f = Fixture::new();
    f.test_same("var o = {};o.f = function(){o.f()};o.f()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification1
#[test]
fn test_side_effect_classification1() {
    let mut f = Fixture::new();
    f.test_same("doThing1();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification2
#[test]
fn test_side_effect_classification2() {
    let mut f = Fixture::new();
    f.test("var a = doThing1();", "doThing1();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification3
#[test]
fn test_side_effect_classification3() {
    let mut f = Fixture::new();
    f.test_same("var a = doThing1();window['b']=a;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification4
#[test]
fn test_side_effect_classification4() {
    let mut f = Fixture::new();
    f.test_same("function sef(){} sef();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification5
#[test]
fn test_side_effect_classification5() {
    let mut f = Fixture::new();
    f.test_same("function nsef(){} var a = nsef();window['b']=a;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification6
#[test]
fn test_side_effect_classification6() {
    let mut f = Fixture::new();
    f.test_same("function sef(){} sef();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSideEffectClassification7
#[test]
fn test_side_effect_classification7() {
    let mut f = Fixture::new();
    f.test_same("function sef(){} var a = sef();window['b']=a;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation1
#[test]
fn test_no_side_effect_annotation1() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("function f(){}")),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f()")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("function f(){}")),
        TestPart::Sources(CompilerTestCase::srcs("let a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f()")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("function f(){}")),
        TestPart::Sources(CompilerTestCase::srcs("const a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f()")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation2
#[test]
fn test_no_side_effect_annotation2() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "/** @nosideeffects */ function f(){}",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation3
#[test]
fn test_no_side_effect_annotation3() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("var f = function(){};")),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f()")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation4
#[test]
fn test_no_side_effect_annotation4() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "var f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "/** @nosideeffects */ var f = function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation5
#[test]
fn test_no_side_effect_annotation5() {
    let mut f = Fixture::new();
    f.test(
        "var f; f = function(){alert('a')}; var a = f();",
        "var f; f = function(){alert('a')}; f();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation6
#[test]
fn test_no_side_effect_annotation6() {
    let mut f = Fixture::new();
    //
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "var f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation7
#[test]
fn test_no_side_effect_annotation7() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "var f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("f = function(){};var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f = function(){};")),
    ]);
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function sideEffect() {}; var f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "f = function(){sideEffect()};var a = f();",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "f = function(){sideEffect()};f()",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation8
#[test]
fn test_no_side_effect_annotation8() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "var f = function(){}; f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("f();")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation_whenUsedOnDuplicateDefinitions_eliminatesSideEffects
#[test]
fn test_no_side_effect_annotation_when_used_on_duplicate_definitions_eliminates_side_effects() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs_strings(&[
            JsString::from("var f = /** @nosideeffects */ function(){};"),
            JsString::from("var f = /** @nosideeffects */ function(){};"),
        ])),
        TestPart::Sources(CompilerTestCase::srcs("var a = f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation10
#[test]
fn test_no_side_effect_annotation10() {
    let mut f = Fixture::new();
    //
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("var o = {}; o.f = function(){};")),
        TestPart::Sources(CompilerTestCase::srcs("var a = o.f();")),
        TestPart::Expected(CompilerTestCase::expected("o.f();")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation11
#[test]
fn test_no_side_effect_annotation11() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "var o = {}; o.f = /** @nosideeffects */ function(){};",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = o.f();")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation12
#[test]
fn test_no_side_effect_annotation12() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs("function c(){}")),
        TestPart::Sources(CompilerTestCase::srcs("var a = new c()")),
        TestPart::Expected(CompilerTestCase::expected("new c()")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation13
#[test]
fn test_no_side_effect_annotation13() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "/** @nosideeffects */ function c(){}",
        )),
        TestPart::Sources(CompilerTestCase::srcs("var a = new c")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation14
#[test]
fn test_no_side_effect_annotation14() {
    let mut f = Fixture::new();
    let externs = r#"function c(){};
c.prototype.f = /**@nosideeffects*/function(){};
"#;
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(externs)),
        TestPart::Sources(CompilerTestCase::srcs("var o = new c; var a = o.f()")),
        TestPart::Expected(CompilerTestCase::expected("new c")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation15
#[test]
fn test_no_side_effect_annotation15() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs_strings(&[
            JsString::from("/** @nosideeffects */ function c(){}"),
            JsString::from("c.prototype.f = function(){};"),
        ])),
        TestPart::Sources(CompilerTestCase::srcs("var a = (new c).f()")),
        TestPart::Expected(CompilerTestCase::expected("(new c).f()")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoSideEffectAnnotation16
#[test]
fn test_no_side_effect_annotation16() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs_strings(&[
            JsString::from("/** @nosideeffects */ function c(){}"),
            JsString::from("c.prototype.f = /** @nosideeffects */ function(){};"),
        ])),
        TestPart::Sources(CompilerTestCase::srcs("var a = (new c).f()")),
        TestPart::Expected(CompilerTestCase::expected("")),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testFunctionPrototype
#[test]
fn test_function_prototype() {
    let mut f = Fixture::new();
    f.test(
        "var a = 5; Function.prototype.foo = function() {return a;}",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass1
#[test]
fn test_top_level_class1() {
    let mut f = Fixture::new();
    f.test(
        "var Point = function() {}; Point.prototype.foo = function() {}",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass2
#[test]
fn test_top_level_class2() {
    let mut f = Fixture::new();
    f.test(
        "var Point = {}; Point.prototype.foo = function() {};externfoo = new Point()",
        "var Point = {};                                     externfoo = new Point()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass3
#[test]
fn test_top_level_class3() {
    let mut f = Fixture::new();
    f.test("function Point() {this.me_ = Point}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass4
#[test]
fn test_top_level_class4() {
    let mut f = Fixture::new();
    f.test(
        "function f(){} function A(){} A.prototype = {x: function() {}}; f();",
        "function f(){} f();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass5
#[test]
fn test_top_level_class5() {
    let mut f = Fixture::new();
    f.test(
        "function f(){} function A(){} A.prototype = {x: function() { f(); }}; new A();",
        "               function A(){} A.prototype = {                      }; new A();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass6
#[test]
fn test_top_level_class6() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(){} function A(){}
A.prototype = {x: function() { f(); }}; new A().x();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTopLevelClass7
#[test]
fn test_top_level_class7() {
    let mut f = Fixture::new();
    f.test("A.prototype.foo = function(){}; function A() {}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNamespacedClass1
#[test]
fn test_namespaced_class1() {
    let mut f = Fixture::new();
    f.test("var foo = {};foo.bar = {};foo.bar.prototype.baz = {}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNamespacedClass2
#[test]
fn test_namespaced_class2() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {}; foo.bar = {}; foo.bar.prototype.baz = {}; window.z = new foo.bar()",
        "var foo = {}; foo.bar = {};                             window.z = new foo.bar()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNamespacedClass4
#[test]
fn test_namespaced_class4() {
    let mut f = Fixture::new();
    f.test(
        r#"function f(){}
var a = {};
a.b = function() {};
a.b.prototype = {x: function() { f(); }};
new a.b();
"#,
        r#"var a = {};
a.b = function() {};
a.b.prototype = {                      };
new a.b();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNamespacedClass5
#[test]
fn test_namespaced_class5() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(){} var a = {}; a.b = function() {};
a.b.prototype = {x: function() { f(); }}; new a.b().x();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testEs6Class
#[test]
fn test_es6_class() {
    let mut f = Fixture::new();
    f.test("class C {}", "");
    f.test("class C {constructor() {} }", "");
    f.test_same("class C {} var c = new C(); use(c);");
    f.test_same(
        r#"class C {
  constructor() {
    this.x = 1;
  }
  add() {
    this.x++
  }
}
var c = new C;
c.add();
use(c.x);
"#,
    );
    f.test("class C{} class D{} var d = new D;", "");
    f.test("{class C{} }", "{}");
    f.test_same("{class C{} var c = new C; use(c)}");
    // preserve newline
    f.test(
        "function f() { return class C {} } doThing1();",
        "                                   doThing1();",
    );
    f.test_same("export class C {}");
    // class expressions
    f.test("var c = class{}", "");
    f.test_same("var c = class{}; use(c);");
    f.test("var c = class C {}", "");
    // preserve newline
    f.test("var c = class C {}; use(c);", "var c = class   {}; use(c);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInnerClassNameInstanceofCheck
#[test]
fn test_inner_class_name_instanceof_check() {
    let mut f = Fixture::new();
    f.test_same(
        r#"window.Class = class MyClass {
  constructor() {
    if (this instanceof MyClass) {
      console.log("test");
    }
  }
};
new window.Class();
"#,
    );
    f.test_same(
        r#"/** @constructor */
window.Class = function MyClass() {
  if (this instanceof MyClass) {
    console.log("test");
  }
};
new window.Class();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testEs6ClassExtends
#[test]
fn test_es6_class_extends() {
    let mut f = Fixture::new();
    f.test_same("class D {} class C extends D {} var c = new C; c.g();");
    f.test("class D {} class C extends D {}", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testEs6ClassExtendsQualifiedName1
#[test]
fn test_es6_class_extends_qualified_name1() {
    let mut f = Fixture::new();
    f.test_same(
        "var ns = {}; ns.Class1 = class {}; class Class2 extends ns.Class1 {}; use(Class2);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testEs6ClassExtendsQualifiedName2
#[test]
fn test_es6_class_extends_qualified_name2() {
    let mut f = Fixture::new();
    f.test(
        "var ns = {}; ns.Class1 = class {}; use(ns.Class1); class Class2 extends ns.Class1 {}",
        "var ns = {}; ns.Class1 = class {}; use(ns.Class1);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentToThisPrototype
#[test]
fn test_assignment_to_this_prototype() {
    let mut f = Fixture::new();
    f.test_same(
        r#"Function.prototype.inherits = function(parentCtor) {
  function tempCtor() {};
  tempCtor.prototype = parentCtor.prototype;
  this.superClass_ = parentCtor.prototype;
  this.prototype = new tempCtor();
  this.prototype.constructor = this;
};
/** @constructor */ function A() {}
/** @constructor */ function B() {}
B.inherits(A);
use(B.superClass_);
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentToCallResultPrototype
#[test]
fn test_assignment_to_call_result_prototype() {
    let mut f = Fixture::new();
    f.test_same("function f() { return function(){}; } f().prototype = {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentToExternPrototype
#[test]
fn test_assignment_to_extern_prototype() {
    let mut f = Fixture::new();
    f.test_same("externfoo.prototype = {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentToUnknownPrototype
#[test]
fn test_assignment_to_unknown_prototype() {
    let mut f = Fixture::new();
    f.h.disable_compare_js_doc().unwrap();
    // multistage compilation simplifies suppressions
    f.test_same(
        r#"/** @suppress {duplicate} */ var window;
window['a'].prototype = {};
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug2099540
#[test]
fn test_bug2099540() {
    let mut f = Fixture::new();
    f.h.disable_compare_js_doc().unwrap();
    // multistage compilation simplifies suppressions
    f.test_same(
        r#"/** @suppress {duplicate} */ var document;
/** @suppress {duplicate} */ var window;
var klass;
window[klass].prototype = document.createElement('p')['__proto__'];
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testOtherGlobal
#[test]
fn test_other_global() {
    let mut f = Fixture::new();
    f.test_same("goog.global.foo = bar(); function bar(){}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExternName1
#[test]
fn test_extern_name1() {
    let mut f = Fixture::new();
    f.test_same("top.z = bar(); function bar(){}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExternName2
#[test]
fn test_extern_name2() {
    let mut f = Fixture::new();
    f.test_same("top['z'] = bar(); function bar(){}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInherits1
#[test]
fn test_inherits1() {
    let mut f = Fixture::new();
    f.test("var a = {}; var b = {}; goog.inherits(b, a)", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInherits2
#[test]
fn test_inherits2() {
    let mut f = Fixture::new();
    f.test_same("var a = {}; externfoo.inherits(a);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInherits3
#[test]
fn test_inherits3() {
    let mut f = Fixture::new();
    f.test_same("var a = {}; goog.inherits(externfoo, a);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInherits4
#[test]
fn test_inherits4() {
    let mut f = Fixture::new();
    f.test("var b = {}; goog.inherits(b, externfoo);", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testInherits5
#[test]
fn test_inherits5() {
    let mut f = Fixture::new();
    f.test_same("var a = {}; goog.inherits(externfoo, a);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testConstants1
#[test]
fn test_constants1() {
    let mut f = Fixture::new();
    f.test_same("var bar = function(){}; var EXP_FOO = true; if (EXP_FOO) bar();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testConstants2
#[test]
fn test_constants2() {
    let mut f = Fixture::new();
    f.test(
        r#"var bar = function(){}; var EXP_FOO = true; var EXP_BAR = true;
if (EXP_FOO) bar();
"#,
        "var bar = function(){}; var EXP_FOO = true; if (EXP_FOO) bar();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExpressions1
#[test]
fn test_expressions1() {
    let mut f = Fixture::new();
    f.test(
        "var foo={}; foo.A='A'; foo.AB=foo.A+'B'; foo.ABC=foo.AB+'C'",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExpressions2
#[test]
fn test_expressions2() {
    let mut f = Fixture::new();
    f.test(
        "var foo={}; foo.A='A'; foo.AB=foo.A+'B'; this.ABC=foo.AB+'C'",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExpressions3
#[test]
fn test_expressions3() {
    let mut f = Fixture::new();
    f.test_same("var foo = 2; window.bar(foo + 3)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetCreatingReference
#[test]
fn test_set_creating_reference() {
    let mut f = Fixture::new();
    f.test(
        "var foo; var bar = function(){foo=6;}; bar();",
        "         var bar = function(){      }; bar();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous1
#[test]
fn test_anonymous1() {
    let mut f = Fixture::new();
    f.test_same("function foo() {}; function bar() {}; foo(function() {bar()})");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous2
#[test]
fn test_anonymous2() {
    let mut f = Fixture::new();
    f.test("var foo;(function(){foo=6;})()", "(function(){})()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous3
#[test]
fn test_anonymous3() {
    let mut f = Fixture::new();
    f.test_same("var foo; (function(){ if(!foo)foo=6; })()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous4
#[test]
fn test_anonymous4() {
    let mut f = Fixture::new();
    f.test_same("var foo; (function(){ foo=6; })(); externfoo=foo;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous5
#[test]
fn test_anonymous5() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo;
(function(){ foo=function(){ bar() }; function bar(){} })();
foo();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous6
#[test]
fn test_anonymous6() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function foo(){}
function bar(){}
foo(function(){externfoo = bar});
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous7
#[test]
fn test_anonymous7() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo;
(function (){ function bar(){ externfoo = foo; } bar(); })();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous8
#[test]
fn test_anonymous8() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo;
(function (){ var g=function(){ externfoo = foo; }; g(); })();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAnonymous9
#[test]
fn test_anonymous9() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function foo(){}
function bar(){}
foo(function(){ function baz(){ externfoo = bar; } baz(); });
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testFunctions1
#[test]
fn test_functions1() {
    let mut f = Fixture::new();
    f.test(
        "var foo = null; function baz() {sideEffect()} function bar() {foo=baz();} bar();",
        "                function baz() {sideEffect()} function bar() {    baz();} bar();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testFunctions2
#[test]
fn test_functions2() {
    let mut f = Fixture::new();
    f.test(
        "var foo; foo = function() {var a = bar()}; var bar = function(){sideEffect()}; foo();",
        "var foo; foo = function() {        bar()}; var bar = function(){sideEffect()}; foo();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testGetElem1
#[test]
fn test_get_elem1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo = {}; foo.bar = {}; foo.bar.baz = {a: 5, b: 10};
var fn = function() {window[foo.bar.baz.a] = 5;}; fn()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testGetElem2
#[test]
fn test_get_elem2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo = {}; foo.bar = {}; foo.bar.baz = {a: 5, b: 10};
var fn = function() {this[foo.bar.baz.a] = 5;}; fn()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testGetElem3
#[test]
fn test_get_elem3() {
    let mut f = Fixture::new();
    f.test_same("var foo = {'i': 0, 'j': 1}; foo['k'] = 2; top.foo = foo;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf1
#[test]
fn test_if1() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {};if(externfoo)foo.bar=function(){};",
        "if(externfoo);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf2
#[test]
fn test_if2() {
    let mut f = Fixture::new();
    f.test(
        "var e = false;var foo = {};if(e)foo.bar=function(){};",
        "var e = false;if(e);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf3
#[test]
fn test_if3() {
    let mut f = Fixture::new();
    f.test(
        "var e = false;var foo = {};if(e + 1)foo.bar=function(){};",
        "var e = false;if(e + 1);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf4
#[test]
fn test_if4() {
    let mut f = Fixture::new();
    f.test(
        "var e = false, f;var foo = {};if(f=e)foo.bar=function(){};",
        "var e = false;if(e);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf4a
#[test]
fn test_if4a() {
    let mut f = Fixture::new();
    // TODO(johnlenz): fix this.
    f.test_same("var e = [], f;if(f=e);f[0] = 1;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf4b
#[test]
fn test_if4b() {
    let mut f = Fixture::new();
    // TODO(johnlenz): fix this.
    f.test("var e = [], f;if(e=f);f[0] = 1;", "var f;if(f);f[0] = 1;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf4c
#[test]
fn test_if4c() {
    let mut f = Fixture::new();
    f.test(
        "var e = [], f;if(f=e);e[0] = 1;",
        "var e = [];if(e);e[0] = 1;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIf5
#[test]
fn test_if5() {
    let mut f = Fixture::new();
    f.test(
        "var e = false, f;var foo = {};if(f = e + 1)foo.bar=function(){};",
        "var e = false;if(e + 1);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIfElse
#[test]
fn test_if_else() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {}; if(externfoo) foo.bar = function(){}; else foo.bar = function(){};",
        "              if(externfoo)                       ; else                       ;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testWhile
#[test]
fn test_while() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {}; while(doThing1()) foo.bar=function(){};",
        "              while(doThing1())                     ;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testForIn
#[test]
fn test_for_in() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {};for(var e in externfoo)foo.bar=function(){};",
        "             for(var e in externfoo)                    ;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testForOf
#[test]
fn test_for_of() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {};for(var e of externfoo)foo.bar=function(){};",
        "             for(var e of externfoo)                    ;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDo
#[test]
fn test_do() {
    let mut f = Fixture::new();
    f.test(
        "var cond = false;do {var a = 1} while (cond)",
        "var cond = false;do {} while (cond)",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct1
#[test]
fn test_setter_in_for_struct1() {
    let mut f = Fixture::new();
    f.test(
        "var j = 0; for (var i = 1; i = 0; j++);",
        "var j = 0; for (; 0; j++);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct2
#[test]
fn test_setter_in_for_struct2() {
    let mut f = Fixture::new();
    f.test(
        r#"var Class = function() {};
for (var i = 1; Class.prototype.property_ = 0; i++);
"#,
        "for (var i = 1; 0; i++);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct3
#[test]
fn test_setter_in_for_struct3() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "var j = 0;                      for (var i = 1 + f() + g() + h(); i = 0; j++);",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "var j = 0; 1 + f() + g() + h(); for (                           ;     0; j++);",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct4
#[test]
fn test_setter_in_for_struct4() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            r#"var i = 0; var j = 0;                      for (i = 1 + f() + g() + h(); i = 0;
 j++);
"#,
        )),
        TestPart::Expected(CompilerTestCase::expected(
            r#"          var j = 0; 1 + f() + g() + h(); for (                       ;     0;
j++);
"#,
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct5
#[test]
fn test_setter_in_for_struct5() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "var i = 0, j = 0; for (i = f(), j = g(); 0;);",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "                  for (    f(),     g(); 0;);",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct6
#[test]
fn test_setter_in_for_struct6() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "var i = 0, j = 0, k = 0; for (i = f(), j = g(), k = h(); i = 0;);",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "                         for (    f(),     g(),     h();     0;);",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct7
#[test]
fn test_setter_in_for_struct7() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "var i = 0, j = 0, k = 0; for (i = 1, j = 2, k = 3; i = 0;);",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "                         for (                   ;     0;);",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct8
#[test]
fn test_setter_in_for_struct8() {
    let mut f = Fixture::new();
    f.test_parts(vec![
        TestPart::Externs(CompilerTestCase::externs(
            "function f(){} function g() {} function h() {}",
        )),
        TestPart::Sources(CompilerTestCase::srcs(
            "var i = 0, j = 0, k = 0; for (i = 1, j = i, k = 2; i = 0;);",
        )),
        TestPart::Expected(CompilerTestCase::expected(
            "                         for (                   ;     0;);",
        )),
    ]);
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct9
#[test]
fn test_setter_in_for_struct9() {
    let mut f = Fixture::new();
    f.test(
        r#"var Class = function() {};
for (var i = 1; Class.property_ = 0; i++);
"#,
        "for (var i = 1; 0; i++);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct10
#[test]
fn test_setter_in_for_struct10() {
    let mut f = Fixture::new();
    f.test(
        "var Class = function() {}; for (var i = 1; Class.property_ = 0; i = 2);",
        "                           for (         ; 0                  ; 0    );",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct11
#[test]
fn test_setter_in_for_struct11() {
    let mut f = Fixture::new();
    f.test(
        r#"var Class = function() {};
for (;Class.property_ = 0;);
"#,
        "for (;0;);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct12
#[test]
fn test_setter_in_for_struct12() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = 1; var Class = function() {};
for (;Class.property_ = a;);
"#,
        "var a = 1; for (; a;);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct13
#[test]
fn test_setter_in_for_struct13() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = 1; var Class = function() {};
for (Class.property_ = a; 0 ;);
"#,
        "for (; 0;);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct14
#[test]
fn test_setter_in_for_struct14() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var Class = function() {}; for (; 0; Class.property_ = a);",
        "                                      for (; 0; 0                  );",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct15
#[test]
fn test_setter_in_for_struct15() {
    let mut f = Fixture::new();
    f.test(
        r#"var Class = function() {};
for (var i = 1; 0; Class.prototype.property_ = 0);
"#,
        "for (; 0; 0);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForStruct16
#[test]
fn test_setter_in_for_struct16() {
    let mut f = Fixture::new();
    f.test(
        r#"var Class = function() {};
for (var i = 1; i = 0; Class.prototype.property_ = 0);
"#,
        "for (; 0; 0);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn1
#[test]
fn test_setter_in_for_in1() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {}; var bar; for(var e in bar = foo.a);",
        "var foo = {};          for(var e in       foo.a);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn2
#[test]
fn test_setter_in_for_in2() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for(var e in bar = foo.a); bar");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn3
#[test]
fn test_setter_in_for_in3() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for(var e in bar = foo.a); bar.b = 3");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn4
#[test]
fn test_setter_in_for_in4() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for (var e in bar = foo.a); bar.b = 3; foo.a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn5
#[test]
fn test_setter_in_for_in5() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for (var e in foo.a) { bar = e } bar.b = 3; foo.a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForIn6
#[test]
fn test_setter_in_for_in6() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; for(var e in foo);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf1
#[test]
fn test_setter_in_for_of1() {
    let mut f = Fixture::new();
    f.test(
        "var foo = {}; var bar; for(var e of bar = foo.a);",
        "var foo = {};          for(var e of       foo.a);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf2
#[test]
fn test_setter_in_for_of2() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for(var e of bar = foo.a); bar");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf3
#[test]
fn test_setter_in_for_of3() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for(var e of bar = foo.a); bar.b = 3");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf4
#[test]
fn test_setter_in_for_of4() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for (var e of bar = foo.a); bar.b = 3; foo.a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf5
#[test]
fn test_setter_in_for_of5() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; var bar; for (var e of foo.a) { bar = e } bar.b = 3; foo.a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInForOf6
#[test]
fn test_setter_in_for_of6() {
    let mut f = Fixture::new();
    f.test_same("var foo = {}; for(var e of foo);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInIfPredicate
#[test]
fn test_setter_in_if_predicate() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var Class = function() {}; if (Class.property_ = a);",
        "var a = 1;                            if (                  a);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInWhilePredicate
#[test]
fn test_setter_in_while_predicate() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = 1;
var Class = function() {};
while (Class.property_ = a);
"#,
        "var a = 1; for (;a;) {}",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInDoWhilePredicate
#[test]
fn test_setter_in_do_while_predicate() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var Class = function() {}; do {} while (Class.property_ = a);",
        "var a = 1;                            do ;  while (                  a);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSetterInSwitchInput
#[test]
fn test_setter_in_switch_input() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1;var Class = function() {}; switch (Class.property_ = a) {default:}",
        "var a = 1;                           switch (                  a) {default:}",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testComplexAssigns
#[test]
fn test_complex_assigns() {
    let mut f = Fixture::new();
    f.test("var x = 0; x += 3; x *= 5;", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssigns1
#[test]
fn test_nested_assigns1() {
    let mut f = Fixture::new();
    f.test(
        "var x = 0; var y = x = 3; window.alert(y);",
        "var y = 3; window.alert(y);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssigns2
#[test]
fn test_nested_assigns2() {
    let mut f = Fixture::new();
    f.test_same("var x = 0; var y = x = {}; x.b = 3; window.alert(y);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testComplexNestedAssigns1
#[test]
fn test_complex_nested_assigns1() {
    let mut f = Fixture::new();
    // TODO(bradfordcsmith): Make RemoveUnusedCode smarter, so that we can eliminate y.
    f.test_same("var x = 0; var y = 2; y += x = 3; window.alert(x);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testComplexNestedAssigns2
#[test]
fn test_complex_nested_assigns2() {
    let mut f = Fixture::new();
    f.test(
        "var x = 0; var y = 2; y += x = 3; window.alert(y);",
        "var y = 2; y += 3; window.alert(y);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testComplexNestedAssigns3
#[test]
fn test_complex_nested_assigns3() {
    let mut f = Fixture::new();
    f.test(
        "var x = 0; var y = x += 3; window.alert(x);",
        "var x = 0; x += 3; window.alert(x);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testComplexNestedAssigns4
#[test]
fn test_complex_nested_assigns4() {
    let mut f = Fixture::new();
    f.test_same("var x = 0; var y = x += 3; window.alert(y);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope1
#[test]
fn test_prototype_property_set_in_local_scope1() {
    let mut f = Fixture::new();
    f.test(
        "(function() { var x = function(){}; x.prototype.bar = 3; })();",
        "(function() {                                            })();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope2
#[test]
fn test_prototype_property_set_in_local_scope2() {
    let mut f = Fixture::new();
    f.test(
        "var x = function(){}; (function() { x.prototype.bar = 3; })();",
        "                      (function() {                      })();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope3
#[test]
fn test_prototype_property_set_in_local_scope3() {
    let mut f = Fixture::new();
    f.test("var x = function(){ x.prototype.bar = 3; };", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope4
#[test]
fn test_prototype_property_set_in_local_scope4() {
    let mut f = Fixture::new();
    f.test(
        "var x = {}; x.foo = function(){ x.foo.prototype.bar = 3; };",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope5
#[test]
fn test_prototype_property_set_in_local_scope5() {
    let mut f = Fixture::new();
    f.test("var x = {}; x.prototype.foo = 3;", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope6
#[test]
fn test_prototype_property_set_in_local_scope6() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.prototype.foo = 3; use(x.prototype.foo)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPrototypePropertySetInLocalScope7
#[test]
fn test_prototype_property_set_in_local_scope7() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.foo = 3; use(x.foo)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference1
#[test]
fn test_r_value_reference1() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference2
#[test]
fn test_r_value_reference2() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; 1+a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference3
#[test]
fn test_r_value_reference3() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.prototype.foo = 3; var a = x.prototype.foo; 1+a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference4
#[test]
fn test_r_value_reference4() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.prototype.foo = 3; use(x.prototype.foo);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference5
#[test]
fn test_r_value_reference5() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; x.prototype.foo = 3; 1+x.prototype.foo");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRValueReference6
#[test]
fn test_r_value_reference6() {
    let mut f = Fixture::new();
    f.test_same("var x = {}; var idx = 2; x[idx]");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testUnhandledTopNode
#[test]
fn test_unhandled_top_node() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}; Foo.prototype.isBar = function() {};
function Bar() {}; Bar.prototype.isFoo = function() {};
var foo = new Foo(); var bar = new Bar();
// The boolean AND here is currently unhandled by this pass, but it
// it should not cause it to blow up.
var cond = foo.isBar() && bar.isFoo();
if (cond) {window.alert('hello');}"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPropertyDefinedInGlobalScope
#[test]
fn test_property_defined_in_global_scope() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}; var x = new Foo(); x.cssClass = 'bar';
window.alert(x);
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testConditionallyDefinedFunction1
#[test]
fn test_conditionally_defined_function1() {
    let mut f = Fixture::new();
    f.test_same("var g; externfoo.x || (externfoo.x = function() { g; })");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testConditionallyDefinedFunction2
#[test]
fn test_conditionally_defined_function2() {
    let mut f = Fixture::new();
    f.test_same("var g; 1 || (externfoo.x = function() { g; })");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testConditionallyDefinedFunction3
#[test]
fn test_conditionally_defined_function3() {
    let mut f = Fixture::new();
    f.test(
        "var a = {}; doThing1() || (a.f = function() { externfoo = 1; } || doThing2());",
        "            doThing1() || (      function() { externfoo = 1; } || doThing2());",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testGetElemOnThis
#[test]
fn test_get_elem_on_this() {
    let mut f = Fixture::new();
    f.test_same("var a = 3; this['foo'] = a;");
    f.test_same("this['foo'] = 3;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveInstanceOfOnly
#[test]
fn test_remove_instance_of_only() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
Foo.prototype.isBar = function() {};
var x;
if (x instanceof Foo) { window.alert(x); }
"#,
        r#"var x;
if (false           ) { window.alert(x); }
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLocalScopedInstanceOfOnly
#[test]
fn test_remove_local_scoped_instance_of_only() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
function Bar(x) { use(x instanceof Foo); }
externfoo.x = new Bar({});
"#,
        r#"function Bar(x) { use(false           ); }
externfoo.x = new Bar({});
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveInstanceOfWithReferencedMethod
#[test]
fn test_remove_instance_of_with_referenced_method() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
Foo.prototype.isBar = function() {};
var x;
if (x instanceof Foo) { window.alert(x.isBar()); }
"#,
        r#"var x;
if (false           ) { window.alert(x.isBar()); }
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeReferencedInstanceOf
#[test]
fn test_do_not_change_referenced_instance_of() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
var x = new Foo();
if (x instanceof Foo) { window.alert(x); }
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeReferencedLocalScopedInstanceOf
#[test]
fn test_do_not_change_referenced_local_scoped_instance_of() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {};
externfoo.x = new Foo();
function Bar() {
  var x;
  if (x instanceof Foo) { window.alert(x); }
}
externfoo.y = new Bar();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeLocalScopeReferencedInstanceOf
#[test]
fn test_do_not_change_local_scope_referenced_instance_of() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
function Bar() { use(new Foo()); }
externfoo.x = new Bar();
var x;
if (x instanceof Foo) { window.alert(x); }
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeLocalScopeReferencedLocalScopedInstanceOf
#[test]
fn test_do_not_change_local_scope_referenced_local_scoped_instance_of() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
function Bar() { new Foo(); }
Bar.prototype.func = function(x) {
  if (x instanceof Foo) { window.alert(x); }
};
new Bar().func();
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeLocalScopeReferencedLocalScopedInstanceOf2
#[test]
fn test_do_not_change_local_scope_referenced_local_scoped_instance_of2() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
var createAxis = function(f) { return window.passThru(f); };
var axis = createAxis(function(test) {
  return test instanceof Foo;
});
"#,
        r#"var createAxis = function(f) { return window.passThru(f); };
createAxis(function(test) {
  return false;
});
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeInstanceOfGetElem
#[test]
fn test_do_not_change_instance_of_get_elem() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var goog = {};
function f(obj, name) {
  if (obj instanceof goog[name]) {
    return name;
  }
}
window['f'] = f;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue2822
#[test]
fn test_issue2822() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var C = function C() {
  if (!(this instanceof C)) {
    throw new Error('not an instance');
  }
}
use(new C());
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testWeirdnessOnLeftSideOfPrototype
#[test]
fn test_weirdness_on_left_side_of_prototype() {
    let mut f = Fixture::new();
    // This checks a bug where 'x' was removed, but the function referencing
    // it was not, causing problems.
    f.test_same("var x = 3; (function() {}).z = function() { return x; };");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testDoNotChangeInstanceOfGetprop
#[test]
fn test_do_not_change_instance_of_getprop() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(obj) {
  if (obj instanceof window.MouseEvent) obj.preventDefault();
}
window['f'] = f;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit2
#[test]
fn test_short_circuit2() {
    let mut f = Fixture::new();
    f.test("var a = 1 || doThing1()", "1 || doThing1()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit3
#[test]
fn test_short_circuit3() {
    let mut f = Fixture::new();
    f.test(
        "var a = doThing1() || doThing2()",
        "doThing1() || doThing2()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit4
#[test]
fn test_short_circuit4() {
    let mut f = Fixture::new();
    f.test(
        "var a = doThing1() || 3 || doThing2()",
        "doThing1() || 3 || doThing2()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit6
#[test]
fn test_short_circuit6() {
    let mut f = Fixture::new();
    f.test("var a = 1 && doThing1()", "1 && doThing1()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit7
#[test]
fn test_short_circuit7() {
    let mut f = Fixture::new();
    f.test(
        "var a = doThing1() && doThing2()",
        "doThing1() && doThing2()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testShortCircuit8
#[test]
fn test_short_circuit8() {
    let mut f = Fixture::new();
    f.test(
        "var a = doThing1() && 3 && doThing2()",
        "doThing1() && 3 && doThing2()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsReference1
#[test]
fn test_rhs_reference1() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsReference2
#[test]
fn test_rhs_reference2() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; a || doThing1()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsReference3
#[test]
fn test_rhs_reference3() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; 1 || a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsReference4
#[test]
fn test_rhs_reference4() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var b = a || doThing1()",
        "var a = 1; a || doThing1()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsReference5
#[test]
fn test_rhs_reference5() {
    let mut f = Fixture::new();
    f.test_same("var a = 1, b = 5; a; use(b)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign1
#[test]
fn test_rhs_assign1() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var foo, bar; foo || (bar = 1)",
        "var foo;      foo || 0        ",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign2
#[test]
fn test_rhs_assign2() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var foo, bar, baz; foo || (baz = bar = 1)",
        "var foo;           foo || 0              ",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign3
#[test]
fn test_rhs_assign3() {
    let mut f = Fixture::new();
    f.test_same("var foo = null; foo || (foo = 1)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign4
#[test]
fn test_rhs_assign4() {
    let mut f = Fixture::new();
    f.test("var foo = null; foo = (foo || 1)", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign5
#[test]
fn test_rhs_assign5() {
    let mut f = Fixture::new();
    f.test(
        "var a = 3, foo, bar; foo || (bar = a)",
        "var        foo     ; foo || 0        ",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign6
#[test]
fn test_rhs_assign6() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null;
var f = function () {foo || (foo = new Foo()); return foo}
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign7
#[test]
fn test_rhs_assign7() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo(){} var foo = null;
var f = function () {foo || (foo = new Foo())}; f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign8
#[test]
fn test_rhs_assign8() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){}
var foo = null;
var f = function () {
  (foo = new Foo()) || doThing1();
};
f()
"#,
        r#"function Foo(){}

var f = function () {
         new Foo()  || doThing1();
};
f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRhsAssign9
#[test]
fn test_rhs_assign9() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null;
var f = function () {1 + (foo = new Foo()); return foo}
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithOr1
#[test]
fn test_assign_with_or1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo = null;
var f = window.a || function () {return foo}; f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithOr2
#[test]
fn test_assign_with_or2() {
    let mut f = Fixture::new();
    f.test(
        "var foo = null; var f = window.a || function () {return foo};",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithAnd1
#[test]
fn test_assign_with_and1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var foo = null;
var f = window.a && function () {return foo}; f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithAnd2
#[test]
fn test_assign_with_and2() {
    let mut f = Fixture::new();
    f.test(
        "var foo = null; var f = window.a && function () {return foo};",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook1
#[test]
fn test_assign_with_hook1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo(){} var foo = null;
var f = window.a ?
    function () {return new Foo()} : function () {return foo}; f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook2
#[test]
fn test_assign_with_hook2() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null;
var f = window.a ?
    function () {return new Foo()} : function () {return foo};
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook2a
#[test]
fn test_assign_with_hook2a() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null;
var f; f = window.a ?
    function () {return new Foo()} : function () {return foo};
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook3
#[test]
fn test_assign_with_hook3() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window.a ?
    function () {return new Foo()} : function () {return foo}; f.b()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook4
#[test]
fn test_assign_with_hook4() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window.a ?
    function () {return new Foo()} : function () {return foo};
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook5
#[test]
fn test_assign_with_hook5() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window.a ? function () {return new Foo()} :
    window.b ? function () {return foo} :
    function() { return Foo }; f.b()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook6
#[test]
fn test_assign_with_hook6() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window.a ? function () {return new Foo()} :
    window.b ? function () {return foo} :
    function() { return Foo };
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithHook7
#[test]
fn test_assign_with_hook7() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo(){} var foo = null;
var f = window.a ? new Foo() : foo;
f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssign1
#[test]
fn test_assign1() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window.a;
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssign2
#[test]
fn test_assign2() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null; var f = {};
f.b = window;
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssign3
#[test]
fn test_assign3() {
    let mut f = Fixture::new();
    f.test(
        r#"var f = {};
f.b = window;
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssign4
#[test]
fn test_assign4() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){sideEffect()} var foo = null; var f = {};
f.b = new Foo();
"#,
        "function Foo(){sideEffect()} new Foo()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssign5
#[test]
fn test_assign5() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo(){} var foo = null; var f = {};
f.b = foo;
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithCall
#[test]
fn test_assign_with_call() {
    let mut f = Fixture::new();
    f.test(
        "var fun, x; (fun = function(){ x; })();",
        "var x; (function(){ x; })();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignWithCall2
#[test]
fn test_assign_with_call2() {
    let mut f = Fixture::new();
    f.test(
        "var fun, x; (123, fun = function(){ x; })();",
        "var      x; (123,       function(){ x; })();",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign1
#[test]
fn test_nested_assign1() {
    let mut f = Fixture::new();
    f.test("var a, b = a = 1, c = 2", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign2
#[test]
fn test_nested_assign2() {
    let mut f = Fixture::new();
    // preserve newline
    f.test("var a, b = a = 1; use(b)", "var b = 1;        use(b)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign3
#[test]
fn test_nested_assign3() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a, b = a = 1; a = b = 2; use(b)",
        "var    b =     1;     b = 2; use(b)",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign4
#[test]
fn test_nested_assign4() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a, b = a = 1; b = a = 2; use(b)",
        "var    b     = 1; b =     2; use(b)",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign5
#[test]
fn test_nested_assign5() {
    let mut f = Fixture::new();
    f.test("var a, b = a = 1; b = a = 2", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign15
#[test]
fn test_nested_assign15() {
    let mut f = Fixture::new();
    f.test("var a, b, c; c = b = a = 2", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign6
#[test]
fn test_nested_assign6() {
    let mut f = Fixture::new();
    f.test_same("var a, b, c; a = b = c = 1; use(a, b, c)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign7
#[test]
fn test_nested_assign7() {
    let mut f = Fixture::new();
    f.test_same("var a = 0, j = 0, i = []; a = i[j] = 1; use(a, i[j])");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNestedAssign8
#[test]
fn test_nested_assign8() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(){
  this.externProp1 = this.externProp2 = use(this.hiddenInput_, externfoo);
}
f()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain1
#[test]
fn test_ref_chain1() {
    let mut f = Fixture::new();
    f.test("var a = 1; var b = a; var c = b; var d = c", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain2
#[test]
fn test_ref_chain2() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var b = a; var c = b; var d = c || doThing1()",
        "var a = 1; var b = a; var c = b;         c || doThing1()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain6
#[test]
fn test_ref_chain6() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var b = a; var c = b; var d = c ? doThing1() : doThing2()",
        "var a = 1; var b = a; var c = b;         c ? doThing1() : doThing2()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain12
#[test]
fn test_ref_chain12() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; var b = a; doThing1()[b] ? doThing2() : 0");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain14
#[test]
fn test_ref_chain14() {
    let mut f = Fixture::new();
    f.test_same("function f(){}var a = 1; var b = a; f()[b] ? doThing1() : 0");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain16
#[test]
fn test_ref_chain16() {
    let mut f = Fixture::new();
    f.test_same("function f(){}var a = 1; var b = a; var c = f(); c[b] ? doThing1() : 0");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain17
#[test]
fn test_ref_chain17() {
    let mut f = Fixture::new();
    f.test(
        "function f(){sideEffect()} var a = 1; var b = a; var c = f(); var d = c[b]",
        "function f(){sideEffect()}                               f()              ",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain18
#[test]
fn test_ref_chain18() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; doThing1()[a] && doThing2()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain19
#[test]
fn test_ref_chain19() {
    let mut f = Fixture::new();
    f.test(
        "var a = 1; var b = [a]; var c = b; b[doThing1()] ? doThing2() : 0",
        "var a = 1; var b = [a];            b[doThing1()] ? doThing2() : 0",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain21
#[test]
fn test_ref_chain21() {
    let mut f = Fixture::new();
    f.test_same("var a = 1; var b = 2; var c = a + b; use(c)");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain22
#[test]
fn test_ref_chain22() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a = 2; var b = a = 4; use(a)",
        "var a = 2;         a = 4; use(a)",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRefChain23
#[test]
fn test_ref_chain23() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var a = {}; var b = a[1] || doThing1()",
        "var a = {};         a[1] || doThing1()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentWithComplexLhs
#[test]
fn test_assignment_with_complex_lhs() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f() { return this; }
var o = {'key': 'val'};
f().x_ = o['key'];
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentWithComplexLhs2
#[test]
fn test_assignment_with_complex_lhs2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f() { return this; }
var o = {'key': 'val'};
f().foo = function() {
  o
};
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentWithComplexLhs3
#[test]
fn test_assignment_with_complex_lhs3() {
    let mut f = Fixture::new();
    let source = r#"var o = {'key': 'val'}; // preserve newline
function init_() {
  use(o['key'])
}
"#;
    f.test(source, "");
    f.test_same(&cat(&[source, ";init_()"]));
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAssignmentWithComplexLhs4
#[test]
fn test_assignment_with_complex_lhs4() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f() { return this; }
var o = {'key': 'val'};
f().foo = function() {
  use(o['key']);
};
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemovePrototypeDefinitionsOutsideGlobalScope1
#[test]
fn test_no_remove_prototype_definitions_outside_global_scope1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(arg){ use(arg); }
(function(){
  function O() {}
  O.prototype = { constructor: O };
  f(O);
})()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemovePrototypeDefinitionsOutsideGlobalScope2
#[test]
fn test_no_remove_prototype_definitions_outside_global_scope2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(arg){ use(arg); }
(function h(){
  function L() {}
  L.prototype = { constructor: L };
  f(L);
})()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemovePrototypeDefinitionsOutsideGlobalScope4
#[test]
fn test_no_remove_prototype_definitions_outside_global_scope4() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function f(arg){ use(arg); }
function g(){
  function N() {}
  N.prototype = { constructor: N };
  f(N);
}
g()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemovePrototypeDefinitionsOutsideGlobalScope5
#[test]
fn test_no_remove_prototype_definitions_outside_global_scope5() {
    let mut f = Fixture::new();
    f.test(
        "function g(){ function R() {} R.prototype = { constructor: R }; } g()",
        "function g(){                                               } g()",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemovePrototypeDefinitionsInGlobalScope1
#[test]
fn test_remove_prototype_definitions_in_global_scope1() {
    let mut f = Fixture::new();
    f.test_same("function M() {} M.prototype = { constructor: M }; use(M);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemovePrototypeDefinitionsInGlobalScope2
#[test]
fn test_remove_prototype_definitions_in_global_scope2() {
    let mut f = Fixture::new();
    f.test("function Q() {} Q.prototype = { constructor: Q };", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLabeledStatement
#[test]
fn test_remove_labeled_statement() {
    let mut f = Fixture::new();
    f.test("LBL: var x = 1;", "LBL: {}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLabeledStatement3
#[test]
fn test_remove_labeled_statement3() {
    let mut f = Fixture::new();
    f.test("var x; LBL: x = 1;", "LBL: {}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveLabeledStatement4
#[test]
fn test_remove_labeled_statement4() {
    let mut f = Fixture::new();
    // preserve newline
    f.test("var a; LBL: a = doThing1()", "       LBL:     doThing1()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias1
#[test]
fn test_preserve_property_mutations_to_alias1() {
    let mut f = Fixture::new();
    // Test for issue b/2316773 - property get case
    // Since a is referenced, property mutations via a's alias b must
    // be preserved.
    f.test_same("var a = {}; var b = a; b.x = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias2
#[test]
fn test_preserve_property_mutations_to_alias2() {
    let mut f = Fixture::new();
    // Test for issue b/2316773 - property get case, don't keep 'c'
    // preserve newline
    f.test(
        "var a = {}; var b = a; var c = a; b.x = 1; a",
        "var a = {}; var b = a;            b.x = 1; a",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias3
#[test]
fn test_preserve_property_mutations_to_alias3() {
    let mut f = Fixture::new();
    // Test for issue b/2316773 - property get case, chain
    f.test_same("var a = {}; var b = a; var c = b; c.x = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias4
#[test]
fn test_preserve_property_mutations_to_alias4() {
    let mut f = Fixture::new();
    // Test for issue b/2316773 - element get case
    f.test_same("var a = {}; var b = a; b['x'] = 1; a");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias5
#[test]
fn test_preserve_property_mutations_to_alias5() {
    let mut f = Fixture::new();
    // From issue b/2316773 description
    f.test_same(
        r#"function testCall(o){ use(o); }
var DATA = {'prop': 'foo','attr': {}};
var SUBDATA = DATA['attr'];
SUBDATA['subprop'] = 'bar';
testCall(DATA);
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias6
#[test]
fn test_preserve_property_mutations_to_alias6() {
    let mut f = Fixture::new();
    // Longer GETELEM chain
    f.test_same(
        r#"function testCall(o){ use(o); }
var DATA = {'prop': 'foo','attr': {}};
var SUBDATA = DATA['attr'];
var SUBSUBDATA = SUBDATA['subprop'];
SUBSUBDATA['subsubprop'] = 'bar';
testCall(DATA);
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias7
#[test]
fn test_preserve_property_mutations_to_alias7() {
    let mut f = Fixture::new();
    // Make sure that the base class does not depend on the derived class.
    f.test(
        "var a = {}; var b = {}; b.x = 0; goog.inherits(b, a); use(a);",
        "var a = {};                                           use(a);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias8
#[test]
fn test_preserve_property_mutations_to_alias8() {
    let mut f = Fixture::new();
    // Make sure that the derived classes don't end up depending on each other.
    f.test(
        r#"var a = {}; // preserve newline
var b = {}; b.x = 0;
var c = {}; c.y = 0;
goog.inherits(b, a);
goog.inherits(c, a);
c
"#,
        r#"var a = {}; // preserve newline

var c = {}; c.y = 0;

goog.inherits(c, a);
c
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPreservePropertyMutationsToAlias9
#[test]
fn test_preserve_property_mutations_to_alias9() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = {b: {}}; // preserve newline
var c = a.b; c.d = 3;
a.d = 3; a.d;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemoveAlias
#[test]
fn test_remove_alias() {
    let mut f = Fixture::new();
    f.test(
        "var a = {b: {}}; var c = a.b; a.d = 3; a.d;",
        "var a = {b: {}};              a.d = 3; a.d;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSingletonGetter1
#[test]
fn test_singleton_getter1() {
    let mut f = Fixture::new();
    f.test("function Foo() {} goog.addSingletonGetter(Foo);", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSingletonGetter2
#[test]
fn test_singleton_getter2() {
    let mut f = Fixture::new();
    f.test("function Foo() {} goog$addSingletonGetter(Foo);", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSingletonGetter3
#[test]
fn test_singleton_getter3() {
    let mut f = Fixture::new();
    // addSingletonGetter adds a getInstance method to a class.
    f.test_same("function Foo() {} goog$addSingletonGetter(Foo); Foo.getInstance();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefineProperty
#[test]
fn test_object_define_property() {
    let mut f = Fixture::new();
    // TODO(bradfordcsmith): Remove Object.defineProperty() like we do Object.defineProperties().
    f.test_same("var a = {}; Object.defineProperty(a, 'prop', {value: 5});");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespaceThatEscapes
#[test]
fn test_object_define_properties_on_namespace_that_escapes() {
    let mut f = Fixture::new();
    f.test_same("var a = doThing1(); Object.defineProperties(a, {'prop': {value: 5}});");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnConstructorThatEscapes
#[test]
fn test_object_define_properties_on_constructor_that_escapes() {
    let mut f = Fixture::new();
    f.test_same(
        "var Foo = doThing1(); Object.defineProperties(Foo.prototype, {'prop': {value: 5}});",
    );
    f.test_same("Object.defineProperties(doThing1(), {'prop': {value: 5}});");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRegularAssignPropOnNamespaceThatEscapes
#[test]
fn test_regular_assign_prop_on_namespace_that_escapes() {
    let mut f = Fixture::new();
    f.test_same("var a = doThing1(); a.prop = 5;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRegularAssignPropOnPropFromAVar
#[test]
fn test_regular_assign_prop_on_prop_from_a_var() {
    let mut f = Fixture::new();
    f.test_same("var b = 5; var a = {}; a.prop = b; use(a.prop);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testUnanalyzableObjectDefineProperties
#[test]
fn test_unanalyzable_object_define_properties() {
    let mut f = Fixture::new();
    f.test("var a = {}; Object.defineProperties(a, externfoo);", "");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespace1
#[test]
fn test_object_define_properties_on_namespace1() {
    let mut f = Fixture::new();
    f.test_same("var a = {}; Object.defineProperties(a, {prop: {value: 5}}); use(a.prop);");
    f.test(
        "var a = {}; Object.defineProperties(a, {prop: {value: 5}});",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespace2
#[test]
fn test_object_define_properties_on_namespace2() {
    let mut f = Fixture::new();
    f.test(
        r#"var a = {};
Object.defineProperties(a, {p1: {value: 5}, p2: {value: 3} });
use(a.p1);
"#,
        r#"var a = {};
Object.defineProperties(a, {p1: {value: 5}                 });
use(a.p1);
"#,
    );
    f.test(
        r#"var a = {}; // preserve newline
Object.defineProperties(a, {p1: {value: 5}, p2: {value: 3} });
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNonAnalyzableObjectDefinePropertiesCall
#[test]
fn test_non_analyzable_object_define_properties_call() {
    let mut f = Fixture::new();
    f.test_same("var a = {}; var z = Object.defineProperties(a, {'prop': {value: 5}}); use(z);");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespace3
#[test]
fn test_object_define_properties_on_namespace3() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var b = 5;
var a = {};
Object.defineProperties(a, {prop: {value: b}});
use(a.prop);
"#,
    );
    f.test(
        r#"var b = 5;
var a = {};
Object.defineProperties(a, {prop: {value: b}});
use(b);
"#,
        "var b = 5; use(b);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespace4
#[test]
fn test_object_define_properties_on_namespace4() {
    let mut f = Fixture::new();
    f.test(
        r#"function b() { alert('hello'); };
var a = {};
Object.defineProperties(a, {prop: {value: b()}});
"#,
        "function b() { alert('hello'); }; ({prop: {value: b()}});",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnNamespace5
#[test]
fn test_object_define_properties_on_namespace5() {
    let mut f = Fixture::new();
    f.test(
        r#"function b() { alert('hello'); }; // preserve newline
function c() { alert('world'); };
var a = {};
Object.defineProperties(a, {p1: {value: b()}, p2: {value: c()}});
"#,
        r#"function b() { alert('hello'); }; // preserve newline
function c() { alert('world'); };

                          ({p1: {value: b()}, p2: {value: c()}});
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnConstructor
#[test]
fn test_object_define_properties_on_constructor() {
    let mut f = Fixture::new();
    f.test_same(
        "function Foo() {} Object.defineProperties(Foo, {prop: {value: 5}}); use(Foo.prop);",
    );
    f.test(
        "function Foo() {} Object.defineProperties(Foo, {prop: {value: 5}});",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnPrototype1
#[test]
fn test_object_define_properties_on_prototype1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
Object.defineProperties(Foo.prototype, {prop: {value: 5}});
use((new Foo).prop);
"#,
    );
    f.test(
        "function Foo() {} Object.defineProperties(Foo.prototype, {prop: {value: 5}});",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnPrototype2
#[test]
fn test_object_define_properties_on_prototype2() {
    let mut f = Fixture::new();
    f.test(
        r#"var b = 5;
function Foo() {}
Object.defineProperties(Foo.prototype, {prop: {value: b}});
use(b)
"#,
        "var b = 5; use(b);",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefinePropertiesOnPrototype3
#[test]
fn test_object_define_properties_on_prototype3() {
    let mut f = Fixture::new();
    f.test(
        r#"var b = function() {sideEffect()};
function Foo() {}
Object.defineProperties(Foo.prototype, {prop: {value: b()}});
"#,
        "var b = function() {sideEffect()}; ({prop: {value: b()}});",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefineGetters
#[test]
fn test_object_define_getters() {
    let mut f = Fixture::new();
    f.test(
        "function Foo() {} Object.defineProperties(Foo, {prop: {get: function() {}}});",
        "",
    );
    f.test(
        "function Foo() {} Object.defineProperties(Foo.prototype, {prop: {get: function() {}}});",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefineSetters
#[test]
fn test_object_define_setters() {
    let mut f = Fixture::new();
    f.test(
        "function Foo() {} Object.defineProperties(Foo, {prop: {set: function() {}}});",
        "",
    );
    f.test(
        "function Foo() {} Object.defineProperties(Foo.prototype, {prop: {set: function() {}}});",
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testObjectDefineSetters_global
#[test]
fn test_object_define_setters_global() {
    let mut f = Fixture::new();
    f.test(
        r#"function Foo() {}
$jscomp.global.Object.defineProperties(Foo, {prop: {set: function() {}}});
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias1
#[test]
fn test_no_remove_window_property_alias1() {
    let mut f = Fixture::new();
    f.test_same("var self_ = window.gbar; self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias2
#[test]
fn test_no_remove_window_property_alias2() {
    let mut f = Fixture::new();
    f.test_same("var self_ = window; self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias3
#[test]
fn test_no_remove_window_property_alias3() {
    let mut f = Fixture::new();
    f.test_same("var self_ = window; self_['qs'] = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias4
#[test]
fn test_no_remove_window_property_alias4() {
    let mut f = Fixture::new();
    f.test_same("var self_ = window['gbar'] || {}; self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias4a
#[test]
fn test_no_remove_window_property_alias4a() {
    let mut f = Fixture::new();
    f.test_same("var self_; self_ = window.gbar || {}; self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias5
#[test]
fn test_no_remove_window_property_alias5() {
    let mut f = Fixture::new();
    f.test_same("var self_ = window || {}; self_['qs'] = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias5a
#[test]
fn test_no_remove_window_property_alias5a() {
    let mut f = Fixture::new();
    f.test_same("var self_; self_ = window || {}; self_['qs'] = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias6
#[test]
fn test_no_remove_window_property_alias6() {
    let mut f = Fixture::new();
    f.test_same("var self_ = (window.gbar = window.gbar || {}); self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias6a
#[test]
fn test_no_remove_window_property_alias6a() {
    let mut f = Fixture::new();
    f.test_same("var self_; self_ = (window.gbar = window.gbar || {}); self_.qs = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias7
#[test]
fn test_no_remove_window_property_alias7() {
    let mut f = Fixture::new();
    f.test_same("var self_ = (window = window || {}); self_['qs'] = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveWindowPropertyAlias7a
#[test]
fn test_no_remove_window_property_alias7a() {
    let mut f = Fixture::new();
    f.test_same("var self_; self_ = (window = window || {}); self_['qs'] = function() {};");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAlias0
#[test]
fn test_no_remove_alias0() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var x = {}; function f() { return x; };
f().style.display = 'block';
alert(x.style)
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAlias1
#[test]
fn test_no_remove_alias1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var x = {}; function f() { return x; };
var map = f();
map.style.display = 'block';
alert(x.style)
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAlias2
#[test]
fn test_no_remove_alias2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var x = {};
var map = (function () { return x; })();
map.style = 'block';
alert(x.style)
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAlias3
#[test]
fn test_no_remove_alias3() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var x = {}; function f() { return x; };
var map = {};
map[1] = f();
map[1].style.display = 'block';
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAliasOfExternal0
#[test]
fn test_no_remove_alias_of_external0() {
    let mut f = Fixture::new();
    f.test_same("document.getElementById('foo').style.display = 'block';");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAliasOfExternal1
#[test]
fn test_no_remove_alias_of_external1() {
    let mut f = Fixture::new();
    f.test_same("var map = document.getElementById('foo'); map.style.display = 'block';");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveAliasOfExternal2
#[test]
fn test_no_remove_alias_of_external2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var map = {} // preserve newline
map[1] = document.getElementById('foo');
map[1].style.display = 'block';
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveThrowReference1
#[test]
fn test_no_remove_throw_reference1() {
    let mut f = Fixture::new();
    f.test_same("var e = {}; throw e;");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testNoRemoveThrowReference2
#[test]
fn test_no_remove_throw_reference2() {
    let mut f = Fixture::new();
    f.test_same("function e() {} throw new e();");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testVarReferencedInClassDefinedInObjectLit1
#[test]
fn test_var_referenced_in_class_defined_in_object_lit1() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var ref = 3; // preserve newline
var data = {Foo: function() { use(ref); }};
window.Foo = data.Foo;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testVarReferencedInClassDefinedInObjectLit2
#[test]
fn test_var_referenced_in_class_defined_in_object_lit2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var ref = 3;
var data = {
  Foo: function() { use(ref); },
  Bar: function() {}
};
window.Bar = data.Bar;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testArrayExt
#[test]
fn test_array_ext() {
    let mut f = Fixture::new();
    f.test_same(
        r#"Array.prototype.foo = function() { return 1 };
var y = [];
switch (y.foo()) {
}
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testArrayAliasExt
#[test]
fn test_array_alias_ext() {
    let mut f = Fixture::new();
    f.test_same(
        r#"Array$X = Array;
Array$X.prototype.foo = function() { return 1 };
function Array$X() {}
var y = [];
switch (y.foo()) {
}
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExternalAliasInstanceof1
#[test]
fn test_external_alias_instanceof1() {
    let mut f = Fixture::new();
    f.test(
        r#"Array$X = Array; // preserve newline
function Array$X() {}
var y = [];
if (y instanceof Array) {}
"#,
        r#"var y = []; // preserve newline
if (y instanceof Array) {}
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExternalAliasInstanceof2
#[test]
fn test_external_alias_instanceof2() {
    let mut f = Fixture::new();
    f.test_same(
        r#"Array$X = Array;
function Array$X() {}
var y = [];
if (y instanceof Array$X) {}
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testExternalAliasInstanceof3
#[test]
fn test_external_alias_instanceof3() {
    let mut f = Fixture::new();
    f.test_same("var b = Array; var y = []; if (y instanceof b) {}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAliasInstanceof4
#[test]
fn test_alias_instanceof4() {
    let mut f = Fixture::new();
    f.test_same("function Foo() {}; var b = Foo; var y = new Foo(); if (y instanceof b) {}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testAliasInstanceof5
#[test]
fn test_alias_instanceof5() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var x;
function Foo() {}
function Bar() {}
var b = x ? Foo : Bar;
var y = new Foo();
if (y instanceof b) {}
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testRemovePrototypeAliases
#[test]
fn test_remove_prototype_aliases() {
    let mut f = Fixture::new();
    f.test(
        "function g() {} function F() {} F.prototype.bar = g; window.g = g;",
        "function g() {}                                      window.g = g;",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue838a
#[test]
fn test_issue838a() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var z = window['z'] || (window['z'] = {}); // preserve newline
z['hello'] = 'Hello';
z['world'] = 'World';
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue838b
#[test]
fn test_issue838b() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var z; // preserve newline
window['z'] = z || (z = {});
z['hello'] = 'Hello';
z['world'] = 'World';
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue874a
#[test]
fn test_issue874a() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a = a || {};
var b = a;
b.View = b.View || {}
var c = b.View;
c.Editor = function f(d, e) {
  return d + e
};
window.ImageEditor.View.Editor = a.View.Editor;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue874b
#[test]
fn test_issue874b() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var b;
var c = b = {};
c.Editor = function f(d, e) {
  return d + e
};
window['Editor'] = b.Editor;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue874c
#[test]
fn test_issue874c() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var b, c;
c = b = {};
c.Editor = function f(d, e) {
  return d + e
};
window['Editor'] = b.Editor;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue874d
#[test]
fn test_issue874d() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var b = {}, c;
c = b;
c.Editor = function f(d, e) {
  return d + e
};
window['Editor'] = b.Editor;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testIssue874e
#[test]
fn test_issue874e() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var a;
var b = a || (a = {});
var c = b.View || (b.View = {});
c.Editor = function f(d, e) {
  return d + e
};
window.ImageEditor.View.Editor = a.View.Editor;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug6575051
#[test]
fn test_bug6575051() {
    let mut f = Fixture::new();
    f.test_same(
        r#"var hackhack = window['__o_o_o__'] = window['__o_o_o__'] || {};
window['__o_o_o__']['va'] = 1;
hackhack['Vb'] = 1;
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug37975351a
#[test]
fn test_bug37975351a() {
    let mut f = Fixture::new();
    // The original repro case from the bug.
    f.test_same(
        r#"function noop() {}
var x = window['magic'];
var FormData = window['FormData'] || noop;
function f() { return x instanceof FormData; }
console.log(f());
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug37975351b
#[test]
fn test_bug37975351b() {
    let mut f = Fixture::new();
    // The simplified repro that still repro'd the problem.
    f.test_same(
        r#"var FormData = window['FormData'] || function() {};
function f() { return window['magic'] instanceof FormData; }
console.log(f());
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug37975351c
#[test]
fn test_bug37975351c() {
    let mut f = Fixture::new();
    // This simpliification did not reproduce the problematic behavior.
    f.test_same(
        r#"var FormData = window['FormData'];
function f() { return window['magic'] instanceof FormData; }
console.log(f());
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBug30868041
#[test]
fn test_bug30868041() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Base() {};
/** @nosideeffects */
Base.prototype.foo =  function() {
}
var x = new Base();
x.foo()
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testGenerators
#[test]
fn test_generators() {
    let mut f = Fixture::new();
    f.test("function* g() {yield 1}", "");
    f.test_same("function* g() {yield 1} var g = g(); g.next().value()");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testSpread
#[test]
fn test_spread() {
    let mut f = Fixture::new();
    f.test(
        r#"const ns = {};
const X = [];
ns.Y = [{}, ...X];
"#,
        r#"const X = [];
       [{}, ...X];
"#,
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testArrayDestructuring
#[test]
fn test_array_destructuring() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "var [a, b = 3, ...c] = [1, 2, 3]",
        "var [ ,      , ...c] = [1, 2, 3]",
    );
    // preserve newline
    f.test(
        "var [a, b = 3, ...c] = [1, 2, 3]; use(b);",
        "var [ , b = 3  ...c] = [1, 2, 3]; use(b);",
    );
    // preserve newline
    f.test(
        "var [a, b = 3, ...c] = [1, 2, 3]; use(c);",
        "var [ ,      , ...c] = [1, 2, 3]; use(c);",
    );
    // preserve newline
    f.test(
        "var a, b, c; [a, b, ...c] = [1, 2, 3]",
        "var c      ; [ ,  , ...c] = [1, 2, 3]",
    );
    // preserve newline
    f.test(
        "var [a, [b, [c, d]]] = [1, [2, [[[3, 4], 5], 6]]];",
        "var [ , [ , [    ]]] = [1, [2, [[[3, 4], 5], 6]]];",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testBlock
#[test]
fn test_block() {
    let mut f = Fixture::new();
    // Currently after normalization this becomes {var f = function f() {}}
    // Will no longer be able to be removed after that normalize change
    f.test("{function g() {}}", "{}");
    f.test_same("{function g() {} g()}");
    f.test_same("function g() {} {let a = g(); use(a)}");
}

// port: RemoveUnusedCodeNameAnalyzerTest#testTemplateLit
#[test]
fn test_template_lit() {
    let mut f = Fixture::new();
    f.test("let a = `hello`", "");
    f.test("var name = 'foo'; let a = `hello ${name}`", "");
    f.test(
        r#"function Base() {} // preserve newline
Base.prototype.foo =  `hello`;
"#,
        "",
    );
    f.test(
        r#"var bar = 'foo';
function Base() {} // preserve newline
Base.prototype.foo =  `foo ${bar}`;
"#,
        "",
    );
}

// port: RemoveUnusedCodeNameAnalyzerTest#testPureOrBreakMyCode
#[test]
fn test_pure_or_break_my_code() {
    let mut f = Fixture::new();
    f.test("const a = /** @pureOrBreakMyCode */(alert());", "");
    f.test("let a = /** @pureOrBreakMyCode */(alert());", "");
    f.test("var a = /** @pureOrBreakMyCode */(alert());", "");
}
