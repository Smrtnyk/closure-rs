/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java,
//   test/com/google/javascript/jscomp/OptimizeCallsIntegrationTest.java.

//! Port of OptimizeCallsIntegrationTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::{
    compiler::Compiler, compiler_pass::CompilerPass, optimize_calls::OptimizeCalls,
    optimize_parameters::OptimizeParameters, optimize_returns::OptimizeReturns,
    pure_function_identifier, remove_unused_code,
};
use closure_rhino::node::NodeId;

/// OptimizeCallsIntegrationTest#getProcessor: the anonymous CompilerPass (Java's captured
/// `compiler` is the `process` argument, DESIGN §6).
struct GetProcessorPass;

impl CompilerPass for GetProcessorPass {
    // port: OptimizeCallsIntegrationTest#getProcessor (anonymous CompilerPass#process)
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        pure_function_identifier::Driver::new().process(compiler, externs, root);
        remove_unused_code::Builder::new(compiler)
            .remove_local_vars(true)
            .remove_globals(true)
            .build()
            .process(compiler, externs, root);

        OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(false)
            .add_pass(Box::new(OptimizeReturns::new()))
            .add_pass(Box::new(OptimizeParameters::new(compiler)))
            .build()
            .process(compiler, externs, root);
    }
}

// port: OptimizeCallsIntegrationTest#OptimizeCallsIntegrationTest (externs)
const EXTERNS: &str = concat!(
    "var window;\n",
    "var goog = {};\n",
    "goog.reflect = {};\n",
    "goog.reflect.object = function(a, b) {};\n",
    "function goog$inherits(a, b) {}\n",
    "var alert;\n",
    "function use(x) {}\n",
);
use closure_testing::testing::test_externs_builder::TestExternsBuilder;
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
    // port: OptimizeCallsIntegrationTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(GetProcessorPass))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: OptimizeCallsIntegrationTest#OptimizeCallsIntegrationTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(default_externs_plus(EXTERNS));
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("OptimizeCallsIntegrationTest"),
            },
        }
    }
}

// port: OptimizeCallsIntegrationTest#testUnusedTaggedTemplateLiteralSubstitutionsAreNotRemoved
#[test]
fn test_unused_tagged_template_literal_substitutions_are_not_removed() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var f = function(strings, x, y) {\n",
            "// x and y are unused\n",
            "  return strings;\n",
            "};\n",
            "f`tagged${1} ${2}`;\n",
            "f();\n"
        ),
        concat!(
            "var f = function(strings) {\n",
            "// x and y are unused\n",
            "  return strings;\n",
            "};\n",
            "f`tagged${1} ${2}`;\n",
            "f();\n"
        ),
    );
    t.test(
        concat!(
            "var f = function(strings, ...rest) {\n",
            "// `rest` is unused\n",
            "  return strings;\n",
            "};\n",
            "f`tagged ${1} ${2}`;\n",
            "f();\n"
        ),
        concat!(
            "var f = function(strings) {\n",
            "// `rest` is unused\n",
            "  return strings;\n",
            "};\n",
            "f`tagged ${1} ${2}`;\n",
            "f();\n"
        ),
    );
    t.test_same(concat!(
        "var f = function(strings, ...rest) {\n",
        "// all arguments are used\n",
        "  return [strings, rest];\n",
        "};\n",
        "f`tagged ${1} ${2}`;\n",
        "f();\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testConvertTaggedTemplateLiteralToANormalCall
#[test]
fn test_convert_tagged_template_literal_to_a_normal_call() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var f = function(strings, x, y) {\n",
            "//  `strings` parameter is unused\n",
            "  return [x, y];\n",
            "};\n",
            "f`tagged${1} ${2}`;\n",
            "f();\n"
        ),
        concat!(
            "var f = function(x$jscomp$1, y) {\n",
            "  return [x$jscomp$1, y];\n",
            "};\n",
            "// Tagged template literal gets converted to a normal call\n",
            "f(1, 2);\n",
            "f();\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testConvertTaggedTemplateLiteralToANormalCallThenOptimized
#[test]
fn test_convert_tagged_template_literal_to_a_normal_call_then_optimized() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var f = function(strings, x, y) {\n",
            "//  all parameters are unused\n",
            "  return '';\n",
            "};\n",
            "f`tagged${1} ${2}`;\n",
            "f();\n"
        ),
        concat!(
            "var f = function() {\n",
            "  return '';\n",
            "};\n",
            "// Tagged template literal gets converted to a normal call\n",
            "f();\n",
            "f();\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testInlineWindow
#[test]
fn test_inline_window() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(window) {\n",
            "  alert(window);\n",
            "}\n",
            "foo(window);\n"
        ),
        concat!(
            "function foo(      ) {\n",
            "  var window$jscomp$1 = window;\n",
            "  alert(window$jscomp$1);\n",
            "}\n",
            "foo(      );\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testRemoveUnusedConstructorArgumentWithDefaultValues
#[test]
fn test_remove_unused_constructor_argument_with_default_values() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs(
            &TestExternsBuilder::new()
                .add_console()
                .build()
                .to_string_lossy(),
        ),
        srcs(concat!(
            "class C {\n",
            "  constructor(unusedValue = () => {}, usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
        expected(concat!(
            "class C {\n",
            "  constructor(                                 ) {\n",
            "    var usedValue = 25;\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "console.log(new C(            ).getValue());\n"
        )),
    ]);
    t.test_parts(vec![
        externs(
            &TestExternsBuilder::new()
                .add_console()
                .build()
                .to_string_lossy(),
        ),
        srcs(concat!(
            "class C {\n",
            "  constructor(unusedValue = () => {}, usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "// 2 calls to the constructor with different used values,\n",
            "// so the second parameter cannot actually be removed.\n",
            "console.log(new C(() => {}, 15).getValue());\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
        expected(concat!(
            "class C {\n",
            "  constructor(                        usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "console.log(new C(          15).getValue());\n",
            "console.log(new C(          25).getValue());\n"
        )),
    ]);
}

// port: OptimizeCallsIntegrationTest#testRemoveUnusedConstructorArgumentWithSubClassWithoutConstructor
#[test]
fn test_remove_unused_constructor_argument_with_sub_class_without_constructor() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs(
            &TestExternsBuilder::new()
                .add_console()
                .build()
                .to_string_lossy(),
        ),
        srcs(concat!(
            "class C {\n",
            "  constructor(unusedValue = () => {}, usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "class SubC extends C {} // no declared constructor\n",
            "console.log(new SubC(() => {}, 25).getValue());\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
        expected(concat!(
            "class C {\n",
            "// default value removed, but no arguments removed or inlined\n",
            "// due to subclass.\n",
            "  constructor(unusedValue           , usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "class SubC extends C {}\n",
            "console.log(new SubC(() => {}, 25).getValue());\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
    ]);
}

// port: OptimizeCallsIntegrationTest#testRemoveUnusedConstructorArgumentWithSubClassWithConstructor
#[test]
fn test_remove_unused_constructor_argument_with_sub_class_with_constructor() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs(
            &TestExternsBuilder::new()
                .add_console()
                .build()
                .to_string_lossy(),
        ),
        srcs(concat!(
            "class C {\n",
            "  constructor(unusedValue = () => {}, usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "class SubC extends C {\n",
            "  constructor(usedValue) {\n",
            "    super(0, usedValue); // calls super constructor\n",
            "  }\n",
            "}\n",
            "console.log(new SubC(25).getValue());\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
        expected(concat!(
            "class C {\n",
            "// First parameter removed because it was never used.\n",
            "  constructor(                        usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "class SubC extends C {\n",
            "// Parameter was inlined with the only value ever passed to it.\n",
            "  constructor(     ) {\n",
            "    var usedValue = 25;\n",
            "// first parameter to super() was removed to match its removal in the\n",
            "// definition above.\n",
            "    super(   usedValue);\n",
            "  }\n",
            "}\n",
            "console.log(new SubC(  ).getValue());\n",
            "// unused parameter removed.\n",
            "console.log(new C(          25).getValue());\n"
        )),
    ]);
    // Same test as above, but with class expressions instead of declarations.
    t.test_parts(vec![
        externs(
            &TestExternsBuilder::new()
                .add_console()
                .build()
                .to_string_lossy(),
        ),
        srcs(concat!(
            "const C = class { // class expression instead of declaration\n",
            "  constructor(unusedValue = () => {}, usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "const SubC = class extends C { // class expression instead of declaration\n",
            "  constructor(usedValue) {\n",
            "    super(0, usedValue); // calls super constructor\n",
            "  }\n",
            "}\n",
            "console.log(new SubC(25).getValue());\n",
            "console.log(new C(() => {}, 25).getValue());\n"
        )),
        expected(concat!(
            "const C = class {\n",
            "// First parameter removed because it was never used.\n",
            "  constructor(                        usedValue = 3) {\n",
            "    this.value = usedValue;\n",
            "  }\n",
            "  getValue() {\n",
            "    return this.value;\n",
            "  }\n",
            "}\n",
            "const SubC = class extends C {\n",
            "// Parameter was inlined with the only value ever passed to it.\n",
            "  constructor(     ) {\n",
            "    var usedValue = 25;\n",
            "// first parameter to super() was removed to match its removal in the\n",
            "// definition above.\n",
            "    super(   usedValue);\n",
            "  }\n",
            "}\n",
            "console.log(new SubC(  ).getValue());\n",
            "// unused parameter removed.\n",
            "console.log(new C(          25).getValue());\n"
        )),
    ]);
}

// port: OptimizeCallsIntegrationTest#testAliasOfAFunction
#[test]
fn test_alias_of_a_function() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function foo(arg1) {\n",
        "  return arg1\n",
        "}\n",
        "\n",
        "// first definition of bar\n",
        "let bar = foo;\n",
        "// really calls foo(1)\n",
        "// the `1` cannot be inlined because bar is an alias of\n",
        "// `foo`\n",
        "bar(1); // return value unused\n",
        "// redefinition of bar with a function literal\n",
        "bar = function(arg1) {\n",
        "  return arg1 + 1;\n",
        "};\n",
        "bar(1) // return value unused & same argument\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testAliasingAssignment
#[test]
fn test_aliasing_assignment() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function MyClass() {\n",
        "  this.myField = null;\n",
        "}\n",
        "\n",
        "// This assignment creates an alias, so we can't know all of the callers and cannot\n",
        "// safely optimize away `myArgument`.\n",
        "MyClass.prototype[\"myMethod\"] =\n",
        "    MyClass.prototype.myMethod = function (myArgument) {\n",
        "  if (undefined === myArgument) {\n",
        "      myArgument = this.myField;\n",
        "  }\n",
        "  return \"myMethod with argument: \" + myArgument;\n",
        "};\n",
        "\n",
        "function globalMyMethod(oMyClass) {\n",
        "// One call to `myMethod` exists, and it doesn't use the optional argument.\n",
        "  return oMyClass.myMethod();\n",
        "}\n",
        "\n",
        "// These both escape, so they won't be removed as unused.\n",
        "window[\"MyClass\"] = MyClass;\n",
        "window[\"globalMyMethod\"] = globalMyMethod;\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testSimpleRemoval
#[test]
fn test_simple_removal() {
    let mut t = Test::new();
    // unused parameter value
    t.test(
        "var foo = (p1)=>{}; foo(1); foo(2)",
        "var foo = (  )=>{}; foo( ); foo( )",
    );
    t.test(
        "let foo = (p1)=>{}; foo(1); foo(2)",
        "let foo = (  )=>{}; foo( ); foo( )",
    );
    t.test(
        "const foo = (p1)=>{}; foo(1); foo(2)",
        "const foo = (  )=>{}; foo( ); foo( )",
    );
}

// port: OptimizeCallsIntegrationTest#testRemovingReturnCallToFunctionWithUnusedParams
#[test]
fn test_removing_return_call_to_function_with_unused_params() {
    let mut t = Test::new();
    t.test(
        "function foo() {var x; return x = bar(1)} foo(); function bar(x) {}",
        "function foo() {bar();return} foo(); function bar() {1;}",
    );
    t.test(
        "function foo() {return} foo(); function bar() {1;}",
        "function foo() {return} foo()",
    );
}

// port: OptimizeCallsIntegrationTest#testNestingFunctionCallWithUnsedParams
#[test]
fn test_nesting_function_call_with_unsed_params() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function f1(x) { }\n",
            "function f2(x) { }\n",
            "function f3(x) { }\n",
            "function f4(x) { }\n",
            "f3(f1(f2()));\n"
        ),
        concat!(
            "function f1(){f2()}\n",
            "function f2(){}\n",
            "function f3(){f1()}f3()\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testUnusedAssignOnFunctionWithUnusedParams
#[test]
fn test_unused_assign_on_function_with_unused_params() {
    let mut t = Test::new();
    t.test(
        "var foo = function(a){  }; function bar(){var x;x = foo} bar(); foo(1)",
        "var foo = function( ){1;}; function bar(){             } bar(); foo()",
    );
}

// port: OptimizeCallsIntegrationTest#testCallSiteInteraction
#[test]
fn test_call_site_interaction() {
    let mut t = Test::new();
    t.test_same("var b=function(){return};b()");
    t.test(
        "var b=function(c){              return c}; b(1)",
        "var b=function( ){var c = 1; c; return  }; b( )",
    );
    t.test(
        "var b=function(c){};b.call(null, 1); b(2);",
        "var b=function( ){};b.call(null   ); b( );",
    );
    t.test(
        "var b=function(c){};b.apply(null, []);",
        "var b=function( ){};b.apply(null, []);",
    );
    t.test(
        "var b=function(c){return};b(1);b(2)",
        "var b=function(){return};b();b();",
    );
    t.test(
        "var b=function(c){return};b(1,2);b();",
        "var b=function(){return};b();b();",
    );
    t.test(
        "var b=function(c){return};b(1,2);b(3,4)",
        "var b=function(){return};b();b()",
    );
    // Here there is a unknown reference to the function so we can't
    // change the signature.
    // TODO(johnlenz): replace unused parameter values, even
    // if we don't know all the uses.
    t.test_same("var b=function(c,d){return d};b(1,2);b(3,4);b.f()");
    t.test(
        "var b=function(c){return};b(1,2);b(3,new use())",
        "var b=function( ){return};b(   );b(  new use())",
    );
    t.test(
        "var b=function(c){return};b(1,2);b(new use(),4)",
        "var b=function( ){return};b(   );b(new use()  )",
    );
    t.test(
        "var b=function(c,d){return d};b(1,2);use(b(new use(),4))",
        "var b=function(c,d){return d};b(0,2);use(b(new use(),4))",
    );
    t.test(
        "var b=function(c,d,e){return d};b(1,2,3);use(b(new use(),4,new use()))",
        "var b=function(c,d  ){return d};b(0,2  );use(b(new use(),4,new use()))",
    );
    t.test(
        "var a=0; var b=function(c,d){return d};b((a++, 1),2);use(b(new use(),4));use(a)",
        "var a=0;var b=function(c,d){return d};b((a++, 0),2);use(b(new use(),4));use(a)",
    );
    t.test(
        "var a=0;var b=function(c,d){return d};b((a++,a++,1),2);use(b(new use(),4));use(a)",
        "var a=0;var b=function(c,d){return d};b((a++,a++,0),2);use(b(new use(),4));use(a)",
    );
    t.test_same(
        "var a=0; var b=function(c,d){return d};b((a++, 1, a++),2);use(b(new use(),4));use(a)",
    );
    // Recursive calls are OK.
    t.test(
        "var b=function(c,d){b(1,2);return d};b(3,4);use(b(5,6))",
        "var b=function(d){b(2);return d};b(4);use(b(6))",
    );
    t.test_same("var b=function(c){return arguments};b(1,2);use(b(3,4))");
    // remove all function arguments
    t.test(
        "var b=function(c,d){return};b();b(1);b(1,2);b(1,2,3)",
        "var b=function(){return};b();b();b();b();",
    );
    // remove no function arguments
    t.test_same("var b=function(c,d){use(c+d)};b(2,3);b(1,2)");
    // remove some function arguments
    t.test(
        "var b=function(e,f,c,d){use(c+d)};b(1,2,3,4);b(3,4,5,6)",
        "var b=function(c,d){use(c+d)};b(3,4);b(5,6)",
    );
    t.test(
        "var b=function(c,d,e,f){use(c+d)};b(1,2);b(3,4)",
        "var b=function(c,d){use(c+d)};b(1,2);b(3,4)",
    );
    t.test(
        "var b=function(e,c,f,d,g){use(c+d)};b(1,2);b(3,4)",
        "var b=function(c){var f;var d;use(c+d)};b(2);b(4)",
    );
    t.test(
        "var b=function(c,d){};var b=function(e,f){};b(1,2)",
        "var b=function(){};var b=function(){};b()",
    );
}

// port: OptimizeCallsIntegrationTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = Test::new();
    t.test_same("var b = function(c) { use(c) } ?? function(c) { use(c) }; b(1)");
    t.test_same("var b; b = function(c) { use(c) } ?? function(c) { use(c) }; b(1)");
    t.test_same("var b = function(c) { use(c) } ?? function(c) { use(c) }; b(1); b(2);");
    t.test_same("var b; b = function(c) { use(c) } ?? function(c) { use(c) }; b(1); b(2);");
    t.test(
        "var f = (function(){ return 1; }) ?? (function(...p2){}); f()",
        "var f = (function(){ return  ; }) ?? (function(     ){}); f()",
    );
}

// port: OptimizeCallsIntegrationTest#testComplexDefinition1
#[test]
fn test_complex_definition1() {
    let mut t = Test::new();
    t.test_same("var x; var b = x ? function(c) { use(c) } : function(c) { use(c) }; b(1)");
    t.test(
        "var x; var b = (x, function(c) {            use(c) }); b(1)",
        "var x; var b = (x, function( ) { var c = 1; use(c) }); b()",
    );
    t.test_same("var x; var b; b = x ? function(c) { use(c) } : function(c) { use(c) }; b(1)");
    t.test(
        "var x; var b; b = (x, function(c) {            use(c) }); b(1)",
        "var x; var b; b = (x, function( ) { var c = 1; use(c) }); b( )",
    );
}

// port: OptimizeCallsIntegrationTest#testComplexDefinition2
#[test]
fn test_complex_definition2() {
    let mut t = Test::new();
    t.test_same("var x; var b = x ? function(c) { use(c) } : function(c) { use(c) }; b(1); b(2);");
    t.test_same("var x;var b = (x, function(c) { use(c) }); b(1); b(2);");
    t.test_same(
        "var x; var b; b = x ? function(c) { use(c) } : function(c) { use(c) }; b(1); b(2);",
    );
    t.test_same("var x; var b; b = (x, function(c) { use(c) }); b(1); b(2);");
}

// port: OptimizeCallsIntegrationTest#testCallSiteInteraction_constructors0
#[test]
fn test_call_site_interaction_constructors0() {
    let mut t = Test::new();
    // Unused parmeters to constructors invoked with .call
    // can be removed.
    t.test(
        concat!(
            "var Ctor1=function(a,b){return a}; // preserve newlines\n",
            "Ctor1.call(this, 1, 2);\n",
            "Ctor1(3, 4)\n"
        ),
        concat!(
            "var Ctor1=function(a  ){a; return}; // preserve newlines\n",
            "Ctor1.call(this,1);\n",
            "Ctor1(3)\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testCallSiteInteraction_constructors1
#[test]
fn test_call_site_interaction_constructors1() {
    let mut t = Test::new();
    // NOTE: Ctor1 used trailing parameter is removed by
    // RemoveUnusedCode
    // For now, goog.inherits prevents optimizations
    t.test(
        concat!(
            "var Ctor1=function(a,b){use(a)};\n",
            "var Ctor2=function(x,y){};\n",
            "goog$inherits(Ctor2, Ctor1);\n",
            "new Ctor2(1,2);new Ctor2(3,4);\n"
        ),
        concat!(
            "var Ctor1=function(a){use(a)};\n",
            "var Ctor2=function(){};\n",
            "goog$inherits(Ctor2, Ctor1);\n",
            "new Ctor2(1,2);new Ctor2(3,4);\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testCallSiteInteraction_constructors2
#[test]
fn test_call_site_interaction_constructors2() {
    let mut t = Test::new();
    // For now, goog.inherits prevents call site optimizations
    let code = concat!(
        "var Ctor1=function(a,b){return a};\n",
        "var Ctor2=function(x,y){Ctor1.call(this,x,y)};\n",
        "goog$inherits(Ctor2, Ctor1);\n",
        "new Ctor2(1,2);new Ctor2(3,4)\n"
    );
    let expected = concat!(
        "var Ctor1=function(a){return a};\n",
        "var Ctor2=function(x,y){Ctor1.call(this,x,y)};\n",
        "goog$inherits(Ctor2, Ctor1);\n",
        "new Ctor2(1,2);new Ctor2(3,4)\n"
    );
    t.test(code, expected);
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalCausingInconsistency
#[test]
fn test_function_arg_removal_causing_inconsistency() {
    let mut t = Test::new();
    // Test the case where an unused argument is removed and the argument
    // contains a call site in its subtree (will cause the call site's parent
    // pointer to be null).
    t.test(
        concat!(
            "var a=function(x,y){};\n",
            "var b=function(z){};\n",
            "a(new b, b)\n"
        ),
        concat!(
            "var a=function(){new b;b};\n",
            "var b=function(){};\n",
            "a()\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testRemoveUnusedVarsPossibleNpeCase
#[test]
fn test_remove_unused_vars_possible_npe_case() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var a = [];\n",
            "var register = function(callback) {a[0] = callback};\n",
            "register(function(transformer) {});\n",
            "register(function(transformer) {});\n"
        ),
        "var register=function(){};register();register()",
    );
}

// port: OptimizeCallsIntegrationTest#testDoNotOptimizeJSCompiler_renameProperty
#[test]
fn test_do_not_optimize_js_compiler_rename_property() {
    let mut t = Test::new();
    // Only the function definition can be modified, none of the call sites.
    t.test(
        concat!(
            "function JSCompiler_renameProperty(a) {};\n",
            "JSCompiler_renameProperty('a');\n"
        ),
        concat!(
            "function JSCompiler_renameProperty() {};\n",
            "JSCompiler_renameProperty('a');\n"
        ),
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSites
#[test]
fn test_function_arg_removal_from_call_sites() {
    let mut t = Test::new();
    // remove all function arguments
    t.test(
        "var b=function(c,d){return};b(1,2);b(3,4)",
        "var b=function(){return};b();b()",
    );
    // remove no function arguments
    t.test_same("var b=function(c,d){return c+d};b(1,2);use(b(3,4))");
    t.test(
        "var b=function(e,f,c,d){return c+d};b(1,2,3,4);use(b(4,3,2,1))",
        "var b=function(c,d){return c+d};b(3,4);use(b(2,1))",
    );
    // remove some function arguments
    t.test(
        "var b=function(c,d,e,f){use(c+d)};b(1,2);b();",
        "var b=function(c,d){use(c+d)};b(1,2);b();",
    );
    t.test(
        "var b=function(e,c,f,d,g){use(c+d)};b(1,2);b(3,4,5,6)",
        "var b=function(c,d){use(c+d)};b(2);b(4,6)",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesSpread1
#[test]
fn test_function_arg_removal_from_call_sites_spread1() {
    let mut t = Test::new();
    t.test(
        "function f(a,b,c,d){};f(...[1,2,3,4]);f(4,3,2,1)",
        "function f(){};f();f()",
    );
    t.test(
        "function f(a,b,c,d){};f(...[1,2,3,4], alert());f(4,3,2,1)",
        "function f(){};f(alert());f()",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(...[1,2,3,4]);f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)};f(...[1,2,3,4]);f(0,0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,...[2,3,4,5]);f(4,3,2,1)",
        "function f(  b,c,d){use(c+d)};f(  ...[2,3,4,5]);f(  0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,2,...[3,4,5]);f(4,3,2,1)",
        "function f(    c,d){use(c+d)};f(    ...[3,4,5]);f(    2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)}; f(...[],2,3);f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)}; f(...[],2,3);f(0,0,2,1)",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesSpread2
#[test]
fn test_function_arg_removal_from_call_sites_spread2() {
    let mut t = Test::new();
    t.test(
        "function f(a,b,c,d){};f(...[alert()]);f(4,3,2,1)",
        "function f(){};f(...[alert()]);f()",
    );
    t.test(
        "function f(a,b,c,d){};f(...[alert()], alert());f(4,3,2,1)",
        "function f(){};f(...[alert()], alert());f()",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(...[alert()]);f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)};f(...[alert()]);f(0,0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,...[alert()]);f(4,3,2,1)",
        "function f(  b,c,d){use(c+d)};f(  ...[alert()]);f(  0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,2,...[alert()]);f(4,3,2,1)",
        "function f(    c,d){use(c+d)};f(    ...[alert()]);f(    2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)}; f(...[alert()],2,3);f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)}; f(...[alert()],2,3);f(0,0,2,1)",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesSpread3
#[test]
fn test_function_arg_removal_from_call_sites_spread3() {
    let mut t = Test::new();
    t.test(
        "function f(a,b,c,d){};f(...alert());f(4,3,2,1)",
        "function f(){};f(...alert());f()",
    );
    t.test(
        "function f(a,b,c,d){};f(...alert(), 1);f(4,3,2,1)",
        "function f(){};f(...alert());f()",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(...alert());f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)};f(...alert());f(0,0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,...alert());f(4,3,2,1)",
        "function f(  b,c,d){use(c+d)};f(  ...alert());f(  0,2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)};f(1,2,...alert());f(4,3,2,1)",
        "function f(    c,d){use(c+d)};f(    ...alert());f(    2,1)",
    );
    t.test(
        "function f(a,b,c,d){use(c+d)}; f(...[alert()],2,3);f(4,3,2,1)",
        "function f(a,b,c,d){use(c+d)}; f(...[alert()],2,3);f(0,0,2,1)",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesRest
#[test]
fn test_function_arg_removal_from_call_sites_rest() {
    let mut t = Test::new();
    // remove all function arguments
    t.test(
        "var b=function(c,...d){return};b(1,2,3);b(4,5,6)",
        "var b=function(      ){return};b(     );b(     )",
    );
    // remove no function arguments
    t.test_same("var b=function(c,...d){return c+d};b(1,2,3);use(b(4,5,6))");
    // remove some function arguments
    t.test(
        "var b=function(e,f,...c){return c};b(1,2,3,4);use(b(4,3,2,1))",
        "var b=function(    ...c){return c};b(    3,4);use(b(    2,1))",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesDefaultValue
#[test]
fn test_function_arg_removal_from_call_sites_default_value() {
    let mut t = Test::new();
    // remove all function arguments
    t.test(
        "function f(c = 1, d = 2){};f(1,2,3);f(4,5,6)",
        "function f(            ){};f(     );f(     )",
    );
    t.test(
        "function f(c = alert()){};f(undefined);f(4)",
        "function f(c = alert()){};f(         );f(4)",
    );
    t.test(
        "function f(c = alert()){};f();f()",
        "function f(){var c = alert();};f();f()",
    );
    // TODO(johnlenz): handle this like the "no value" case above and
    // allow the default value to inlined into the body.
    t.test(
        "function f(c = alert()){};f(undefined);f(undefined)",
        "function f(c = alert()){};f(          );f(        )",
    );
}

// port: OptimizeCallsIntegrationTest#testFunctionArgRemovalFromCallSitesDestructuring
#[test]
fn test_function_arg_removal_from_call_sites_destructuring() {
    let mut t = Test::new();
    // remove all function arguments
    t.test(
        "function f([a] = [1], [b] = [2]){} f(1, 2, 3); f(4, 5, 6)",
        "function f(                    ){} f(       ); f(       )",
    );
    t.test(
        "function f(a, [b] = alert(), [c] = alert(), d){} f(1, 2, 3, 4); f(4, 5, 6, 7)",
        "function f(   [ ] = alert(), [ ] = alert()   ){} f(   2, 3   ); f(   5, 6   )",
    );
    t.test(
        "function f(a, [b = alert()] = [], [c = alert()] = [], d){} f(1, 2, 3, 4);f(4, 5, 6, 7)",
        "function f(   [b = alert()] = [], [c = alert()] = []   ){} f(   2, 3   );f(   5, 6   )",
    );
    t.test(
        "function f(a, [b = alert()], [c = alert()], d){} f(1, 2, 3, 4); f(4, 5, 6, 7);",
        "function f(   [b = alert()], [c = alert()]   ){} f(   2, 3   ); f(   5, 6   );",
    );
}

// port: OptimizeCallsIntegrationTest#testLocalVarReferencesGlobalVar
#[test]
fn test_local_var_references_global_var() {
    let mut t = Test::new();
    t.test(
        "var a=3;function f(b, c){b=a; alert(c);} f(1,2);f()",
        "function f(c) { alert(c); } f(2);f();",
    );
}

// port: OptimizeCallsIntegrationTest#testReflectedMethods
#[test]
fn test_reflected_methods() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function Foo() {}\n",
        "Foo.prototype.handle = function(x, y) { alert(y); };\n",
        "var x = goog.reflect.object(Foo, {handle: 1});\n",
        "for (var i in x) { x[i].call(x); }\n",
        "window['Foo'] = Foo;\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testExistenceOfAGetter_preventsParamOptimization
#[test]
fn test_existence_of_a_getter_prevents_param_optimization() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Bar {\n",
        "// TODO(nickreid): Use `declareAccessor rather than specifying in snippet. We can't do\n",
        "// that currently because `RemoveUnusedCode` unilaterally runs another collection.\n",
        "  get foo() {\n",
        "    return (x) => x;\n",
        "  }\n",
        "}\n",
        "\n",
        "class Foo {\n",
        "  foo() {}\n",
        "}\n",
        "\n",
        "new (Foo || Bar)().foo('onlyUsedByGetter');\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testExistenceOfASetter_preventsParamOptimization
#[test]
fn test_existence_of_a_setter_prevents_param_optimization() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Bar {\n",
        "// TODO(nickreid): Use `declareAccessor rather than specifying in snippet. We can't do\n",
        "// that currently because `RemoveUnusedCode` unilaterally runs another collection.\n",
        "  set foo(f) {\n",
        "    this.bar = f;\n",
        "  }\n",
        "}\n",
        "\n",
        "// This is a defnition for `.bar`. It would be dangerous to optimize it as if it were a\n",
        "// definition of `.foo`\n",
        "var x = new Bar();\n",
        "x.foo = function(param) { return param; };\n",
        "x.foo();\n"
    ));
}

// port: OptimizeCallsIntegrationTest#testUndefinedLiterals_beforeOtherUnused
#[test]
fn test_undefined_literals_before_other_unused() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a, b) { use(a); }\n",
            "// check that this undefined gets removed if it is before another unused param\n",
            "foo(undefined, 3);\n",
            "foo(void 0, 4);\n"
        ),
        concat!("function foo(a) { use(a); }\n", "foo();\n", "foo();\n"),
    );
}
