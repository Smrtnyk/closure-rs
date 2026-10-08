/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/InlineSimpleMethodsTest.java.

//! Port of InlineSimpleMethodsTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::inline_simple_methods::InlineSimpleMethods;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
#[allow(unused_imports)]
use optimize_calls_support::*;

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: InlineSimpleMethodsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(InlineSimpleMethods::new(&compiler.borrow())))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: InlineSimpleMethodsTest#InlineSimpleMethodsTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        harness.enable_gather_extern_properties().unwrap();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("InlineSimpleMethodsTest"),
            },
        }
    }
}

impl Test {
    /// Helper for tests that expects definitions to remain unchanged, such that `definitions+js`
    /// is converted to `definitions+expected`.
    // port: InlineSimpleMethodsTest#testWithPrefix
    fn test_with_prefix(&mut self, definitions: &str, js: &str, expected: &str) {
        self.test(
            &[definitions, js].concat(),
            &[definitions, expected].concat(),
        );
    }
}

// port: InlineSimpleMethodsTest#testDoesNotInlineMethodOnBaseClass
#[test]
fn test_does_not_inline_method_on_base_class() {
    let mut t = Test::new();
    let base_class_js = concat!(
        "class Base {\n",
        "  constructor() {\n",
        "    /** @const */\n",
        "    this.prop_ =\n",
        "        Math.random() > .5;\n",
        "  }\n",
        "  method() {\n",
        "    return this.prop_;\n",
        "  }\n",
        "}\n"
    );
    let derived_class_js = concat!(
        "class Derived extends Base {\n",
        "  constructor() {\n",
        "    super();\n",
        "  }\n",
        "  derivedMethod() {\n",
        "    super.method();\n",
        "  }\n",
        "}\n",
        "\n",
        "(new Derived()).derivedMethod();\n"
    );
    let source = [base_class_js, derived_class_js].concat();
    t.test_same(&source);
}

// port: InlineSimpleMethodsTest#testSimpleInline1
#[test]
fn test_simple_inline1() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n"
        ),
        "var x=(new Foo).bar();var y=(new Foo).bar();",
        "var x=(new Foo).baz;var y=(new Foo).baz",
    );
}

// port: InlineSimpleMethodsTest#testSimpleInline2
#[test]
fn test_simple_inline2() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype={bar:function(){return this.baz}};\n"
        ),
        "var x=(new Foo).bar();var y=(new Foo).bar();",
        "var x=(new Foo).baz;var y=(new Foo).baz",
    );
}

// port: InlineSimpleMethodsTest#testSimpleGetterInline1
#[test]
fn test_simple_getter_inline1() {
    let mut t = Test::new();
    // TODO(johnlenz): Support this case.
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype={get bar(){return this.baz}};\n",
        "var x=(new Foo).bar;var y=(new Foo).bar\n"
    ));
    // Verify we are not confusing calling the result of an ES5 getter
    // with call the getter.
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype={get bar(){return this.baz}};\n",
        "var x=(new Foo).bar();var y=(new Foo).bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testSimpleSetterInline1
#[test]
fn test_simple_setter_inline1() {
    let mut t = Test::new();
    // Verify 'get' and 'set' are not confused.
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype={set bar(a){return this.baz}};\n",
        "var x=(new Foo).bar;var y=(new Foo).bar\n"
    ));
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype={set bar(a){return this.baz}};\n",
        "var x=(new Foo).bar();var y=(new Foo).bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testSelfInline
#[test]
fn test_self_inline() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n"
        ),
        "Foo.prototype.meth=function(){this.bar();}",
        "Foo.prototype.meth=function(){this.baz}",
    );
}

// port: InlineSimpleMethodsTest#testCallWithArgs
#[test]
fn test_call_with_args() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n"
        ),
        "var x=(new Foo).bar(3,new Foo)",
        "var x=(new Foo).bar(3,new Foo)",
    );
}

// port: InlineSimpleMethodsTest#testCallWithConstArgs
#[test]
fn test_call_with_const_args() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(a){return this.baz};\n"
        ),
        "var x=(new Foo).bar(3, 4)",
        "var x=(new Foo).baz",
    );
}

// port: InlineSimpleMethodsTest#testNestedProperties
#[test]
fn test_nested_properties() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz.ooka};\n"
        ),
        "(new Foo).bar()",
        "(new Foo).baz.ooka",
    );
}

// port: InlineSimpleMethodsTest#testSkipComplexMethods
#[test]
fn test_skip_complex_methods() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n",
            "Foo.prototype.condy=function(){return this.baz?this.baz:1};\n"
        ),
        "var x=(new Foo).argy()",
        "var x=(new Foo).argy()",
    );
}

// port: InlineSimpleMethodsTest#testSkipConflictingMethods
#[test]
fn test_skip_conflicting_methods() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n",
            "Foo.prototype.bar=function(){return this.bazz};\n"
        ),
        "var x=(new Foo).bar()",
        "var x=(new Foo).bar()",
    );
}

// port: InlineSimpleMethodsTest#testSameNamesDifferentDefinitions
#[test]
fn test_same_names_different_definitions() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function A(){}\n",
            "A.prototype.g=function(){return this.a};\n",
            "function B(){}\n",
            "B.prototype.g=function(){return this.b};\n"
        ),
        concat!(
            "var x=(new A).g();\n",
            "var y=(new B).g();\n",
            "var a=new A;\n",
            "var ag=a.g();\n"
        ),
        concat!(
            "var x=(new A).g();\n",
            "var y=(new B).g();\n",
            "var a=new A;\n",
            "var ag=a.g()\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testSameNamesSameDefinitions
#[test]
fn test_same_names_same_definitions() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function A(){}\n",
            "A.prototype.g=function(){return this.a};\n",
            "function B(){}\n",
            "B.prototype.g=function(){return this.a};\n"
        ),
        concat!(
            "var x=(new A).g();\n",
            "var y=(new B).g();\n",
            "var a=new A;\n",
            "var ag=a.g();\n"
        ),
        concat!(
            "var x=(new A).a;\n",
            "var y=(new B).a;\n",
            "var a=new A;\n",
            "var ag=a.a\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testConfusingNames
#[test]
fn test_confusing_names() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return this.baz};\n"
        ),
        "function bar(){var bar=function(){};bar()}",
        "function bar(){var bar=function(){};bar()}",
    );
}

// port: InlineSimpleMethodsTest#testConstantInline
#[test]
fn test_constant_inline() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return 3};\n"
        ),
        "var f=new Foo;var x=f.bar()",
        "var f=new Foo;var x=3",
    );
}

// port: InlineSimpleMethodsTest#testConstantArrayInline
#[test]
fn test_constant_array_inline() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return[3,4]};\n"
        ),
        "var f=new Foo;var x=f.bar()",
        "var f=new Foo;var x=[3,4]",
    );
}

// port: InlineSimpleMethodsTest#testConstantInlineWithSideEffects
#[test]
fn test_constant_inline_with_side_effects() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){return 3};\n"
        ),
        "var x=(new Foo).bar()",
        "var x=(new Foo).bar()",
    );
}

// port: InlineSimpleMethodsTest#testEmptyMethodInline
#[test]
fn test_empty_method_inline() {
    let mut t = Test::new();
    t.test_with_prefix(
        "function Foo(){} Foo.prototype.bar=function(a){};",
        "var x = new Foo(); x.bar();",
        "var x = new Foo();;",
    );
}

// port: InlineSimpleMethodsTest#testEmptyMethodInlineWithSideEffects
#[test]
fn test_empty_method_inline_with_side_effects() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!("function Foo(){}\n", "Foo.prototype.bar=function(){};\n"),
        "(new Foo).bar();var y=new Foo;y.bar(new Foo)",
        "(new Foo).bar();var y=new Foo;y.bar(new Foo)",
    );
}

// port: InlineSimpleMethodsTest#testEmptyMethodInlineInAssign1
#[test]
fn test_empty_method_inline_in_assign1() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!("function Foo(){}\n", "Foo.prototype.bar=function(){};\n"),
        "var x=new Foo;var y=x.bar()",
        "var x=new Foo;var y=void 0",
    );
}

// port: InlineSimpleMethodsTest#testEmptyMethodInlineInAssign2
#[test]
fn test_empty_method_inline_in_assign2() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!("function Foo(){}\n", "Foo.prototype.bar=function(){};\n"),
        "var x=new Foo;var y=x.bar().toString()",
        "var x=new Foo;var y=(void 0).toString()",
    );
}

// port: InlineSimpleMethodsTest#testNormalMethod
#[test]
fn test_normal_method() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "function Foo(){}\n",
            "Foo.prototype.bar=function(){var x=1};\n"
        ),
        "var x=new Foo;x.bar()",
        "var x=new Foo;x.bar()",
    );
}

// port: InlineSimpleMethodsTest#testNoInlineOfExternMethods1
#[test]
fn test_no_inline_of_extern_methods1() {
    let mut t = Test::new();
    t.test_same_parts(vec![
        externs("var external={};external.charAt;"),
        srcs("external.charAt()"),
    ]);
}

// port: InlineSimpleMethodsTest#testNoInlineOfExternMethods2
#[test]
fn test_no_inline_of_extern_methods2() {
    let mut t = Test::new();
    t.test_same_parts(vec![
        externs("var external={};external.charAt=function(){};"),
        srcs("external.charAt()"),
    ]);
}

// port: InlineSimpleMethodsTest#testNoInlineOfExternMethods3
#[test]
fn test_no_inline_of_extern_methods3() {
    let mut t = Test::new();
    t.test_same_parts(vec![
        externs("var external={};external.bar=function(){};"),
        srcs("function Foo(){}Foo.prototype.bar=function(){};(new Foo).bar()"),
    ]);
}

// port: InlineSimpleMethodsTest#testNoInlineOfDangerousProperty
#[test]
fn test_no_inline_of_dangerous_property() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function Foo(){this.bar=3}\n",
        "Foo.prototype.bar=function(){};\n",
        "var x=new Foo;var y=x.bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoWarn
#[test]
fn test_no_warn() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype.bar=function(opt_a,b){var x=1};\n",
        "var x=new Foo;x.bar()\n"
    ));
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype.bar=function(var_args,b){var x=1};\n",
        "var x=new Foo;x.bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testObjectLit
#[test]
fn test_object_lit() {
    let mut t = Test::new();
    t.test_same(concat!(
        "Foo.prototype.bar=function(){return this.baz_};\n",
        "var blah={bar:function(){}};\n",
        "(new Foo).bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testObjectLit2
#[test]
fn test_object_lit2() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var blah={bar:function(){}};\n",
        "(new Foo).bar()\n"
    ));
}

// port: InlineSimpleMethodsTest#testObjectLit3
#[test]
fn test_object_lit3() {
    let mut t = Test::new();
    t.test_same(concat!("var blah={bar(){}};\n", "(new Foo).bar()\n"));
}

// port: InlineSimpleMethodsTest#testObjectLit4
#[test]
fn test_object_lit4() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var key='bar';\n",
        "var blah={[key]:a};\n",
        "(new Foo).bar\n"
    ));
}

// port: InlineSimpleMethodsTest#testObjectLitExtern1
#[test]
fn test_object_lit_extern1() {
    let mut t = Test::new();
    let externs_js = "window.bridge={_sip:function(){}};";
    t.test_same_parts(vec![externs(externs_js), srcs("window.bridge._sip()")]);
}

// port: InlineSimpleMethodsTest#testObjectLitExtern2
#[test]
fn test_object_lit_extern2() {
    let mut t = Test::new();
    let externs_js = "window.bridge={_sip(){}};";
    t.test_same_parts(vec![externs(externs_js), srcs("window.bridge._sip()")]);
}

// port: InlineSimpleMethodsTest#testClassExtern
#[test]
fn test_class_extern() {
    let mut t = Test::new();
    let externs_js = "window.bridge= class { _sip() {} };";
    t.test_same_parts(vec![externs(externs_js), srcs("window.bridge._sip()")]);
}

// port: InlineSimpleMethodsTest#testExternFunction
#[test]
fn test_extern_function() {
    let mut t = Test::new();
    let externs_js = "function emptyFunction() {}";
    t.test_same_parts(vec![
        externs(externs_js),
        srcs(concat!(
            "function Foo(){this.empty=emptyFunction}\n",
            "(new Foo).empty()\n"
        )),
    ]);
}

// port: InlineSimpleMethodsTest#testIssue2508576_1
#[test]
fn test_issue2508576_1() {
    let mut t = Test::new();
    // Method defined by an extern should be left alone.
    let externs_js = "function alert(a) {}";
    t.test_same_parts(vec![
        externs(externs_js),
        srcs("({a:alert,b:alert}).a('a')"),
    ]);
}

// port: InlineSimpleMethodsTest#testIssue2508576_2
#[test]
fn test_issue2508576_2() {
    let mut t = Test::new();
    // Anonymous object definition with a side-effect should be left alone.
    t.test_same("({a:function(){},b:x()}).a('a')");
}

// port: InlineSimpleMethodsTest#testIssue2508576_3
#[test]
fn test_issue2508576_3() {
    let mut t = Test::new();
    // Anonymous object definition without side-effect should be removed.
    t.test("({a:function(){},b:alert}).a('a')", ";");
}

// port: InlineSimpleMethodsTest#testEs6Issue1
#[test]
fn test_es6_issue1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function OldClass() {}\n",
        "\n",
        "OldClass.prototype.foo = function() { return this.oldbar; };\n",
        "\n",
        "class NewClass {\n",
        "  foo() { return this.newbar; }\n",
        "}\n",
        "\n",
        "var x = new OldClass;\n",
        "x.foo();\n",
        "x = new NewClass;\n",
        "x.foo();\n"
    ));
}

// port: InlineSimpleMethodsTest#testEs6Issue2
#[test]
fn test_es6_issue2() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function OldClass() {}\n",
        "\n",
        "OldClass.prototype.foo = function() { return this.oldbar; };\n",
        "\n",
        "class NewClass {\n",
        "  foo() { return this.newbar; }\n",
        "}\n",
        "\n",
        "var x = new OldClass;\n",
        "x.foo();\n",
        "var y = new NewClass;\n",
        "y.foo();\n"
    ));
}

// port: InlineSimpleMethodsTest#testArrowFunction
#[test]
fn test_arrow_function() {
    let mut t = Test::new();
    // Arrow functions cannot be trivially inlined.
    // This is only an issue if ES6+ is being targeted,
    // because otherwise the arrow function is removed before this pass.
    t.test_same(concat!(
        "class Holder {\n",
        "    constructor() {\n",
        "        this.val = {internal: true};\n",
        "        this.context =  {getVal: () => this.val}\n",
        "    }\n",
        "}\n",
        "\n",
        "console.log(new Holder().context.getVal().internal);\n"
    ));
}

// port: InlineSimpleMethodsTest#testAnonymousGet
#[test]
fn test_anonymous_get() {
    let mut t = Test::new();
    // Anonymous object definition without side-effect should be removed.
    t.test_same("({get a(){return function(){}},b:alert}).a('a')");
    t.test_same("({get a(){},b:alert}).a('a')");
    t.test_same("({get a(){},b:alert}).a");
}

// port: InlineSimpleMethodsTest#testAnonymousSet
#[test]
fn test_anonymous_set() {
    let mut t = Test::new();
    // Anonymous object definition without side-effect should be removed.
    t.test_same("({set a(b){return function(){}},b:alert}).a('a')");
    t.test_same("({set a(b){},b:alert}).a('a')");
    t.test_same("({set a(b){},b:alert}).a");
}

// port: InlineSimpleMethodsTest#testInlinesEvenIfClassEscapes
#[test]
fn test_inlines_even_if_class_escapes() {
    let mut t = Test::new();
    // The purpose of this test is to record an unsafe assumption made by the
    // pass. In practice, it's usually safe to inline even if the class
    // escapes, because method definitions aren't commonly mutated.
    t.test_parts(vec![
        externs("var esc;"),
        srcs(concat!(
            "/** @constructor */\n",
            "function Foo() {\n",
            "  this.prop = 123;\n",
            "}\n",
            "Foo.prototype.m = function() {\n",
            "  return this.prop;\n",
            "}\n",
            "(new Foo).m();\n",
            "esc(Foo);\n"
        )),
        expected(concat!(
            "/** @constructor */\n",
            "function Foo(){this.prop=123}\n",
            "Foo.prototype.m=function(){return this.prop}\n",
            "(new Foo).m();\n",
            "esc(Foo)\n"
        )),
    ]);
}

// port: InlineSimpleMethodsTest#testClassFieldDoesntCrash
#[test]
fn test_class_field_doesnt_crash() {
    let mut t = Test::new();
    t.test_same("class C { a; };");
    t.test_same("class C { a = 2; };");
    t.test_same("class C { ['a'] = 2; }");
    t.test_same("class C { a = function() { return 4; }; }");
    t.test_same("class C { static a; };");
    t.test_same("class C { static a = 2; };");
    t.test_same("class C { static ['a'] = 2; }");
    t.test_same("class C { static a = function() { return 4; }; }");
}

// port: InlineSimpleMethodsTest#testStaticInitializationBlockDoesntCrash
#[test]
fn test_static_initialization_block_doesnt_crash() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C {\n",
        "  static {\n",
        "    alert('foo');\n",
        "  }\n",
        "}\n"
    ));
}

// port: InlineSimpleMethodsTest#testNonStaticClassFieldArrowFunction
#[test]
fn test_non_static_class_field_arrow_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  a = 5;\n",
        "  b = () => this.a;\n",
        "}\n",
        "new Foo().b()\n"
    ));
}

// port: InlineSimpleMethodsTest#testStaticClassFieldFunctionDoesInline
#[test]
fn test_static_class_field_function_does_inline() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  static a = 5;\n",
            "  static b = function() { return this.a; };\n",
            "}\n",
            "Foo.b();\n"
        ),
        concat!(
            "class Foo {\n",
            "  static a = 5;\n",
            "  static b = function() { return this.a; };\n",
            "}\n",
            "Foo.a;\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testNoInline
#[test]
fn test_no_inline() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "class Foo {\n",
            " /** @noinline */ bar() { return 'hi'; }\n",
            "}\n"
        ),
        "var x=new Foo;x.bar()",
        "var x=new Foo;x.bar()",
    );
}

// port: InlineSimpleMethodsTest#testReflectObjectProperty
#[test]
fn test_reflect_object_property() {
    let mut t = Test::new();
    t.test_with_prefix(
        concat!(
            "class Foo {\n",
            " bar() { return 'hi'; }\n",
            "}\n",
            "const c = goog.reflect.objectProperty('bar', Foo.prototype);\n"
        ),
        "var x=new Foo;x.bar()",
        "var x=new Foo;x.bar()",
    );
}

// port: InlineSimpleMethodsTest#testNoInlineOfAsyncMethod_es5Style
#[test]
fn test_no_inline_of_async_method_es5_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns a Promise wrapping this.baz.
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype.bar = async function(){return this.baz};\n",
        "var x = (new Foo).bar();\n",
        "var y = (new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfAsyncMethod_es6Style
#[test]
fn test_no_inline_of_async_method_es6_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns a Promise wrapping this.baz.
    t.test_same(concat!(
        "class Foo { async bar(){return this.baz} }\n",
        "var x = (new Foo).bar();\n",
        "var y = (new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfGeneratorMethod_es5Style
#[test]
fn test_no_inline_of_generator_method_es5_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns a generator, not this.baz
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype.bar = function*(){return this.baz};\n",
        "var x = (new Foo).bar();\n",
        "var y = (new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfGeneratorMethod_es6Style
#[test]
fn test_no_inline_of_generator_method_es6_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns a generator, not this.baz
    t.test_same(concat!(
        "class Foo { *bar(){return this.baz} }\n",
        "var x = (new Foo).bar();\n",
        "var y = (new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfAsynceneratorMethod_es5Style
#[test]
fn test_no_inline_of_asyncenerator_method_es5_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns an async generator, not this.baz
    t.test_same(concat!(
        "function Foo(){}\n",
        "Foo.prototype.bar = async function*(){return this.baz};\n",
        "var x = (new Foo).bar();\n",
        "var y =(new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfAsyncGeneratorMethod_es6Style
#[test]
fn test_no_inline_of_async_generator_method_es6_style() {
    let mut t = Test::new();
    // Don't inline: the bar() call returns an async generator, not this.baz
    t.test_same(concat!(
        "class Foo { async *bar(){return this.baz} }\n",
        "var x = (new Foo).bar();\n",
        "var y = (new Foo).bar();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfMethodWithSideEffectfulDefaultParameter
#[test]
fn test_no_inline_of_method_with_side_effectful_default_parameter() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  getVal(x = audit()) { return this.val; }\n",
        "}\n",
        "var x = (new Foo).getVal();\n"
    ));
}

// port: InlineSimpleMethodsTest#testInlineOfMethodWithSideEffectFreeDefaultParameter
#[test]
fn test_inline_of_method_with_side_effect_free_default_parameter() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  getVal(x = 1) { return this.val; }\n",
            "}\n",
            "var x = (new Foo).getVal();\n"
        ),
        concat!(
            "class Foo {\n",
            "  getVal(x = 1) { return this.val; }\n",
            "}\n",
            "var x = (new Foo).val;\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testNoInlineOfEmptyMethodWithSideEffectfulDefaultParameter
#[test]
fn test_no_inline_of_empty_method_with_side_effectful_default_parameter() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  record(x = audit()) {}\n",
        "}\n",
        "var f = new Foo();\n",
        "var x = f.record();\n"
    ));
}

// port: InlineSimpleMethodsTest#testInlineOfEmptyMethodWithSideEffectFreeDefaultParameter
#[test]
fn test_inline_of_empty_method_with_side_effect_free_default_parameter() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  record(x = 1) {}\n",
            "}\n",
            "var f = new Foo();\n",
            "var x = f.record();\n"
        ),
        concat!(
            "class Foo {\n",
            "  record(x = 1) {}\n",
            "}\n",
            "var f = new Foo();\n",
            "var x = void 0;\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testNoInlineOfMethodWithDestructuring
#[test]
fn test_no_inline_of_method_with_destructuring() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  getVal({x}) { return this.val; }\n",
        "}\n",
        "var x = (new Foo).getVal({});\n"
    ));
    t.test_same(concat!(
        "class Foo {\n",
        "  getVal([x]) { return this.val; }\n",
        "}\n",
        "var x = (new Foo).getVal([1]);\n"
    ));
    t.test_same(concat!(
        "class Foo {\n",
        "  getVal({x} = {}) { return this.val; }\n",
        "}\n",
        "var x = (new Foo).getVal();\n"
    ));
}

// port: InlineSimpleMethodsTest#testNoInlineOfEmptyMethodWithDestructuring
#[test]
fn test_no_inline_of_empty_method_with_destructuring() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  record({x}) {}\n",
        "}\n",
        "var f = new Foo();\n",
        "var x = f.record({});\n"
    ));
    t.test_same(concat!(
        "class Foo {\n",
        "  record([x]) {}\n",
        "}\n",
        "var f = new Foo();\n",
        "var x = f.record([1]);\n"
    ));
}

// port: InlineSimpleMethodsTest#testInlineOfMethodWithRestParameter
#[test]
fn test_inline_of_method_with_rest_parameter() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  getVal(...x) { return this.val; }\n",
            "}\n",
            "var x = (new Foo).getVal();\n"
        ),
        concat!(
            "class Foo {\n",
            "  getVal(...x) { return this.val; }\n",
            "}\n",
            "var x = (new Foo).val;\n"
        ),
    );
}

// port: InlineSimpleMethodsTest#testInlineOfEmptyMethodWithRestParameter
#[test]
fn test_inline_of_empty_method_with_rest_parameter() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  record(...x) {}\n",
            "}\n",
            "var f = new Foo();\n",
            "var x = f.record();\n"
        ),
        concat!(
            "class Foo {\n",
            "  record(...x) {}\n",
            "}\n",
            "var f = new Foo();\n",
            "var x = void 0;\n"
        ),
    );
}
