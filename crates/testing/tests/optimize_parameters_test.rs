/*
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
//   test/com/google/javascript/jscomp/OptimizeParametersTest.java.

//! Port of OptimizeParametersTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::optimize_parameters::OptimizeParameters;
use closure_rhino::node::SideEffectFlags;
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
    // port: OptimizeParametersTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(OptimizeParameters::new(&compiler.borrow())))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: OptimizeParametersTest#OptimizeParametersTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(default_externs_plus("var alert;"));
        harness.set_up();
        harness.enable_normalize().unwrap();
        harness.disable_multistage_compilation().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("OptimizeParametersTest"),
            },
        }
    }
}

// port: OptimizeParametersTest#testAliasingAssignment
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
        "}\n"
    ));
}

// port: OptimizeParametersTest#nullishCoalesce
#[test]
fn nullish_coalesce() {
    let mut t = Test::new();
    t.test(
        "var f = (function(...p1){            }) ?? (function(...p2){           }); f()",
        "var f = (function(     ){ var p1 = []}) ?? (function(     ){var p2 = []}); f()",
    );
}

// port: OptimizeParametersTest#testNoRemoval
#[test]
fn test_no_removal() {
    let mut t = Test::new();
    t.test_same("function foo(p1) { } foo(1); foo(2)");
    // required "remove unused vars"
    t.test_same("function foo(p1) { } foo(this);");
    t.test_same("function foo(p1) { } function g() {foo(arguments)}; g();");
    // Can't move a reference to a local.
    t.test_same("function foo(p1) { use(p1); } function g() {var x = 1; foo(x);}; g();");
    // optional versions
    t.test_same("function foo(p1) { } foo?.(1); foo?.(2)");
    // required "remove unused vars"
    t.test_same("function foo(p1) { } foo?.(this);");
    t.test_same("function foo(p1) { } function g() {foo?.(arguments)}; g?.();");
    // Can't move a reference to a local.
    t.test_same("function foo(p1) { use?.(p1); } function g() {var x = 1; foo?.(x);}; g?.();");
}

// port: OptimizeParametersTest#testNoRemoval_arrowFunctions
#[test]
fn test_no_removal_arrow_functions() {
    let mut t = Test::new();
    t.test_same("var foo = (p1)=>{ }; foo(1); foo(2)");
    // required "remove unused vars"
    t.test_same("var foo = (p1)=>{ }; foo(this);");
    t.test_same("var foo = (p1)=>{ }; function g() {foo(arguments)}; g();");
    // Can't move a reference to a local.
    t.test_same("var foo = (p1)=>{ use(p1); }; function g() {var x = 1; foo(x);}; g();");
    // optional versions
    t.test_same("var foo = (p1)=>{ }; foo?.(1); foo?.(2)");
    // required "remove unused vars"
    t.test_same("var foo = (p1)=>{ }; foo?.(this);");
    t.test_same("var foo = (p1)=>{ }; function g() {foo?.(arguments)}; g?.();");
    // Can't move a reference to a local.
    t.test_same("var foo = (p1)=>{ use(p1); }; function g() {var x = 1; foo?.(x);}; g?.();");
}

// port: OptimizeParametersTest#testNoRemovalSpread
#[test]
fn test_no_removal_spread() {
    let mut t = Test::new();
    // TODO(johnlenz): make spread removable
    t.test_same("function f(p1) {} f(...x);");
    t.test_same("function f(...p1) {} f(...x);");
    t.test(
        "function f(p1, ...p2) {} f(1, ...x);",
        "function f(...p2) {var p1 = 1;} f(...x);",
    );
    t.test(
        "function f(p1, ...p2) {} f?.(1, ...x);",
        "function f(...p2) {var p1 = 1;} f?.(...x);",
    );
    t.test(
        "function f(p1, p2) {} f(1, ...x); f(1, 2);",
        "function f(p2) {var p1 = 1;} f(...x); f(2);",
    );
    t.test(
        "function f(p1, p2) {} f?.(1, ...x); f?.(1, 2);",
        "function f(p2) {var p1 = 1;} f?.(...x); f?.(2);",
    );
    t.test_same("function f(p1, p2) {} f(1, ...x); f(2, ...y);");
    t.test_same("function f(p1, p2) {} f(p1, ...args);");
    t.test_same("function f(p1, p2) {} f.call(thisArg, p1, ...args);");
    t.test_same("function f(p1, p2) {} f?.call(thisArg, p1, ...args);");
    // Test spread argument with side effects
    t.test_same(concat!(
        "function foo(x) {sideEffect(); return x;}\n",
        "function f(p1, p2) {}\n",
        "f(foo(0), ...[foo(1)]);\n",
        "f(foo(0), 1);\n"
    ));
    // Test should not remove arguments following spread
    t.test(
        "function f(p1, p2, p3) {} f(1, ...[2], 3); f(1, 2, 3);",
        "function f(p2, p3) {var p1 = 1;} f(...[2], 3); f(2, 3);",
    );
    t.test(
        "function f(p1, p2, p3) {} f?.(1, ...[2], 3); f?.(1, 2, 3);",
        "function f(p2, p3) {var p1 = 1;} f?.(...[2], 3); f?.(2, 3);",
    );
}

// port: OptimizeParametersTest#testRemovalRest_singleParam
#[test]
fn test_removal_rest_single_param() {
    let mut t = Test::new();
    // rest as the first parameter
    t.test(
        "function f(...p1){          } f();",
        "function f(     ){var p1=[];} f()",
    );
    t.test(
        "function f(...p1){           } f(1);",
        "function f(     ){var p1=[1];} f( )",
    );
    t.test(
        "function f(...p1){            use(p1)} f(1);",
        "function f(     ){var p1=[1]; use(p1)} f( )",
    );
    t.test(
        "function f(...p1){                 } f(alert());",
        "function f(     ){var p1=[alert()];} f(       );",
    );
    t.test(
        "function f(...p1){                 } f?.(alert());",
        "function f(     ){var p1=[alert()];} f?.(       );",
    );
}

// port: OptimizeParametersTest#testRemovalRest_secondParam
#[test]
fn test_removal_rest_second_param() {
    let mut t = Test::new();
    // rest as the second parameter
    t.test(
        "function f(p1, ...p2){          } f(x); f(y);",
        "function f(p1,      ){var p2=[];} f(x); f(y);",
    );
    t.test(
        "function f(p1, ...p2){           } f(x, 1); f(y, 1);",
        "function f(p1,      ){var p2=[1];} f(x   ); f(y   );",
    );
    t.test(
        "function f(p1, ...p2){            use(p2)} f(x, 1); f(y, 1);",
        "function f(p1,      ){var p2=[1]; use(p2)} f(x   ); f(y   );",
    );
    t.test(
        "function f(p1, ...p2){            use(p2)} f?.(x, 1); f?.(y, 1);",
        "function f(p1,      ){var p2=[1]; use(p2)} f?.(x   ); f?.(y   );",
    );
    t.test(
        "function f(p1, ...p2){                 } f(x, alert()); f(y, alert());",
        "function f(p1,      ){var p2=[alert()];} f(x         ); f(y         );",
    );
}

// port: OptimizeParametersTest#testRemovalRest3
#[test]
fn test_removal_rest3() {
    let mut t = Test::new();
    // Can't move a reference to a local.
    t.test_same("function f(...p1){} function _g(x) { f(x); f(x); }");
}

// port: OptimizeParametersTest#testRemovalRest_rollbackPreservesTrailingArgs
#[test]
fn test_removal_rest_rollback_preserves_trailing_args() {
    let mut t = Test::new();
    t.test_same("function f(...p) { use(p); } function g(x) { f(1, x, 2); f(1, x, 2); }");
    t.test(
        "function f(a, ...p) { use(a, p); } function g(x) { f(1, 2, x, 3); f(1, 2, x, 3); }",
        "function f(...p) { var a = 1; use(a, p); } function g(x) { f(2, x, 3); f(2, x, 3); }",
    );
}

// port: OptimizeParametersTest#testRemovalRestWithDestructuring1
#[test]
fn test_removal_rest_with_destructuring1() {
    let mut t = Test::new();
    t.test(
        "function f(...[a]){          } f();",
        "function f(      ){var a; [a]=[];} f()",
    );
    t.test(
        "function f(...[a]){          } f(1);",
        "function f(      ){var a; [a]=[1];} f()",
    );
    t.test(
        "function f(...{a}){          } f();",
        "function f(      ){var {a}=[];} f()",
    );
    t.test(
        "function f(...{a}){          } f?.();",
        "function f(      ){var {a}=[];} f?.()",
    );
    t.test(
        "function f(...{a}){            } f(1);",
        "function f(      ){var a; ({a}=[1]);} f( )",
    );
}

// port: OptimizeParametersTest#testRemoveParamWithDefault1
#[test]
fn test_remove_param_with_default1() {
    let mut t = Test::new();
    t.test(
        "function f(a = 1){          } f();",
        "function f(     ){var a = 1;} f()",
    );
    t.test(
        "function f(a = []){           } f();",
        "function f(      ){var a = [];} f()",
    );
    t.test(
        "function f(a = alert()){                } f();",
        "function f(           ){var a = alert();} f()",
    );
    t.test(
        "function f([a] = []){             } f();",
        "function f(        ){var a; [a] = [];} f()",
    );
    t.test(
        "function f([a] = []){             } f?.();",
        "function f(        ){var a; [a] = [];} f?.()",
    );
    t.test(
        "function f([a = 1] = []){                 } f();",
        "function f(            ){var a; [a = 1] = [];} f()",
    );
    t.test(
        "function f([a = 1]){                 } f([]);",
        "function f(       ){var a; [a = 1] = [];} f(  );",
    );
    t.test(
        "function f([a = 1]){                 } f?.([]);",
        "function f(       ){var a; [a = 1] = [];} f?.(  );",
    );
}

// port: OptimizeParametersTest#testRemoveParam_defaultValueOverriden
#[test]
fn test_remove_param_default_value_overriden() {
    let mut t = Test::new();
    t.test(
        "function f(a = 0){          } f(1);",
        "function f(     ){var a = 1;} f( )",
    );
    t.test(
        "function f({a} = 0){           } f([]);",
        "function f(       ){var a; ({a}=[]);} f(  )",
    );
    t.test(
        "function f({a = 1} = 0){               } f([]);",
        "function f(           ){var a; ({a = 1}=[]);} f(  )",
    );
    t.test(
        "function f({a = 1} = 0){               } f?.([]);",
        "function f(           ){var a; ({a = 1}=[]);} f?.(  )",
    );
    t.test(
        "function f({a = 1}){               } f({});",
        "function f(       ){var a; ({a = 1}={});} f(  )",
    );
    t.test(
        "function f({a: a = 1}){               } f({});",
        "function f(          ){var a; ({a = 1}={});} f(  )",
    );
}

// port: OptimizeParametersTest#testRemoveMultipleParams_withDestructuring_withDefaultValue
#[test]
fn test_remove_multiple_params_with_destructuring_with_default_value() {
    let mut t = Test::new();
    // test removing multiple parameters
    t.test(
        "var x = 0; function f(a, {b} = {b: 1}) {} f(1, {b:4});",
        "var x = 0; function f() { var a = 1; var b; ({b} = {b: 4});} f();",
    );
}

// port: OptimizeParametersTest#testInlineParamWithDefault_referencingOutsideVariable
#[test]
fn test_inline_param_with_default_referencing_outside_variable() {
    let mut t = Test::new();
    t.test(
        "var x = 0; function f(a = x) {} f(1);",
        "var x = 0; function f() { var a = 1; } f();",
    );
    t.test(
        "var x = 0; function f(a = x + x) {} f(1);",
        "var x = 0; function f() { var a = 1; } f();",
    );
    t.test(
        "function g(x) { return x; } function f(a = g(0)) {} f(1);",
        "function g() { var x = 0; return x; } function f() { var a = 1; } f();",
    );
}

// port: OptimizeParametersTest#testNoRemoveParamWithDefault_whenArgPossiblyUndefined
#[test]
fn test_no_remove_param_with_default_when_arg_possibly_undefined() {
    let mut t = Test::new();
    // different scopes
    t.test_same("function f(p = 1){} function _g(x) { f(x); f(x); }");
    // TODO(johnlenz): add logic for adding an undefined check to the body of the function
    // so that this can be inlined into the function body
    t.test_same("function f(a = 2){} f(alert);");
    t.test("function f(a = 2){} f(void 0);", "function f(a = 2){} f();");
    // Make sure `sideEffects()` is always evaluated before `x`;
    t.test_same("var x = 0; function f(a = sideEffects(), b = x) {}; f(void 0, something);");
    t.test_same("var x = 0; function f(a = sideEffects(), b = x) {}; f?.(void 0, something);");
}

// port: OptimizeParametersTest#testNoRemoveParam_beingUsedInSubsequentDefault
#[test]
fn test_no_remove_param_being_used_in_subsequent_default() {
    let mut t = Test::new();
    t.test_same("function f(a, b = a) { return b; }; f(x);");
    t.test_same("function f(a, b = foo(a)) { return b; }; f(x);");
    t.test_same("function f(a, b = (c) => a) { return b; }; f(x);");
    t.test_same("function f({a}, {b: [{c = a}]}) { return c; }; f(x);");
}

// port: OptimizeParametersTest#testNoRemoveParam_beingUsedInSubsequentDefault_fromSingleFormal
#[test]
fn test_no_remove_param_being_used_in_subsequent_default_from_single_formal() {
    let mut t = Test::new();
    t.test_same("function f([a, b = a]) { return b; }; f(x);");
    t.test_same("function f({a, [a]: b}) { return b; }; f(x);");
}

// port: OptimizeParametersTest#testNoInlineParam_beingUsedInSubsequentDefault
#[test]
fn test_no_inline_param_being_used_in_subsequent_default() {
    let mut t = Test::new();
    t.test_same("function f(a, b = a) { return a + b; }; f(1, x);");
    t.test_same("function f(a, b = (a = 9)) { return a + b; }; f(1, x);");
    // These would actually be fine to inline, but it's hard to detect that in general.
    t.test_same("function f(a, b = a) { return a + b; }; f(1, 1);");
    t.test_same("function f(a, b = a) { return a + b; }; f(1);");
}

// port: OptimizeParametersTest#testNoInlineYieldExpression
#[test]
fn test_no_inline_yield_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function f(a) { return a; }\n",
        "function *g() { f(yield 1); }\n",
        "use(g().next());\n"
    ));
}

// port: OptimizeParametersTest#testNoRemoveYieldExpression
#[test]
fn test_no_remove_yield_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function f(a) { }\n",
        "function *g() { f(yield 1); }\n",
        "use(g().next());\n"
    ));
}

// port: OptimizeParametersTest#testNoInlineAwaitExpression
#[test]
fn test_no_inline_await_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function f(a) { return a; }\n",
        "async function g() { f(await 1); }\n",
        "use(g().next());\n"
    ));
}

// port: OptimizeParametersTest#testNoRemoveAwaitExpression
#[test]
fn test_no_remove_await_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function f(a) { }\n",
        "async function g() { f(await 1); }\n",
        "use(g().next());\n"
    ));
}

// port: OptimizeParametersTest#testSimpleRemoval0
#[test]
fn test_simple_removal0() {
    let mut t = Test::new();
    t.test(
        "function foo(p1) {       } foo(); foo()",
        "function foo(  ) {var p1;} foo(); foo()",
    );
    t.test(
        "function foo(p1) {           } foo(1); foo(1)",
        "function foo(  ) {var p1 = 1;} foo( ); foo( )",
    );
    t.test(
        "function foo(p1) {           } foo(1,2); foo(1,4)",
        "function foo(  ) {var p1 = 1;} foo(   ); foo(   )",
    );
    t.test(
        "function foo(p1) { } foo(1,2); foo(3,4);",
        "function foo(p1) { } foo(1  ); foo(3  );",
    );
    t.test(
        "function foo(p1) {           } foo(1,x()); foo(1,y())",
        "function foo(  ) {var p1 = 1;} foo(  x()); foo(  y())",
    );
}

// port: OptimizeParametersTest#testSimpleRemoval1
#[test]
fn test_simple_removal1() {
    let mut t = Test::new();
    // parameter never supplied
    t.test(
        "var foo = (p1)=>{       }; foo(); foo()",
        "var foo = (  )=>{var p1;}; foo(); foo()",
    );
    t.test(
        "let foo = (p1)=>{       }; foo(); foo()",
        "let foo = (  )=>{var p1;}; foo(); foo()",
    );
    t.test(
        "const foo = (p1)=>{       }; foo(); foo()",
        "const foo = (  )=>{var p1;}; foo(); foo()",
    );
    t.test_same(
        "class foo { /** @usedViaDotConstructor */ constructor(p1) {}} new foo(); new foo()",
    );
    // constant parameter
    t.test(
        "var foo = (p1)=>{           }; foo(1); foo(1)",
        "var foo = (  )=>{var p1 = 1;}; foo( ); foo( )",
    );
    t.test(
        "let foo = (p1)=>{           }; foo(1); foo(1)",
        "let foo = (  )=>{var p1 = 1;}; foo( ); foo( )",
    );
    t.test(
        "const foo = (p1)=>{           }; foo(1); foo(1)",
        "const foo = (  )=>{var p1 = 1;}; foo( ); foo( )",
    );
    t.test_same(
        "class foo { /** @usedViaDotConstructor */ constructor(p1) {}} new foo(1); new foo(1)",
    );
}

// port: OptimizeParametersTest#testSimpleRemoval2
#[test]
fn test_simple_removal2() {
    let mut t = Test::new();
    t.test(
        "function f(p1) {       } new f(); new f()",
        "function f(  ) {var p1;} new f(); new f()",
    );
    t.test(
        "function f(p1) {           } new f(1); new f(1)",
        "function f(  ) {var p1 = 1;} new f( ); new f( )",
    );
    t.test(
        "function f(p1) {           } new f(1,2); new f(1,4)",
        "function f(  ) {var p1 = 1;} new f(   ); new f(   )",
    );
    t.test(
        "function f(p1) { } new f(1,2); new f(3,4);",
        "function f(p1) { } new f(1  ); new f(3  );",
    );
    t.test(
        "function f(p1) {           } new f(1,x()); new f(1,y())",
        "function f(  ) {var p1 = 1;} new f(  x()); new f(  y())",
    );
    t.test_same("/** @usedViaDotConstructor */ function f(p1) {} new f(); new f()");
}

// port: OptimizeParametersTest#testSimpleRemovalInstanceof
#[test]
fn test_simple_removal_instanceof() {
    let mut t = Test::new();
    t.test(
        "function f(p1) {       } x instanceof f; new f(); new f()",
        "function f(  ) {var p1;} x instanceof f; new f(); new f()",
    );
    t.test(
        "function f(p1) {           } x instanceof f; new f(1); new f(1)",
        "function f(  ) {var p1 = 1;} x instanceof f; new f( ); new f( )",
    );
}

// port: OptimizeParametersTest#testSimpleRemovalTypeof
#[test]
fn test_simple_removal_typeof() {
    let mut t = Test::new();
    t.test(
        "function f(p1) {       } typeof f != 'undefined' && f();",
        "function f(  ) {var p1;} typeof f != 'undefined' && f();",
    );
    t.test(
        "function f(p1) {           } typeof f != 'undefined'; f(1);",
        "function f(  ) {var p1 = 1;} typeof f != 'undefined'; f();",
    );
}

// port: OptimizeParametersTest#testSimpleRemoval4
#[test]
fn test_simple_removal4() {
    let mut t = Test::new();
    t.test(
        "function f(p1) {       } f.prop = 1; new f(); new f()",
        "function f(  ) {var p1;} f.prop = 1; new f(); new f()",
    );
    t.test_same("/** @usedViaDotConstructor */ function f(p1) {} f.prop = 1; new f(); new f()");
    t.test(
        "function f(p1) {           } f.prop = 1; new f(1); new f(1)",
        "function f(  ) {var p1 = 1;} f.prop = 1; new f( ); new f( )",
    );
    t.test_same("/** @usedViaDotConstructor */ function f(p1) {} f.prop = 1; new f(1); new f(1)");
    t.test(
        "function f(p1) {       } f['prop'] = 1; new f(); new f()",
        "function f(  ) {var p1;} f['prop'] = 1; new f(); new f()",
    );
    t.test_same("/** @usedViaDotConstructor */ function f(p1) {} f['prop'] = 1; new f(); new f()");
    t.test(
        "function f(p1) {           } f['prop'] = 1; new f(1); new f(1)",
        "function f(  ) {var p1 = 1;} f['prop'] = 1; new f( ); new f( )",
    );
    t.test_same(
        "/** @usedViaDotConstructor */ function f(p1) {} f['prop'] = 1; new f(1); new f(1)",
    );
}

// port: OptimizeParametersTest#testSimpleRemovalAsync
#[test]
fn test_simple_removal_async() {
    let mut t = Test::new();
    // parameter never supplied
    t.test(
        "var f = async function (p1) {       }; f(); f()",
        "var f = async function (  ) {var p1;}; f(); f()",
    );
    t.test(
        "let f = async function (p1) {       }; f(); f()",
        "let f = async function (  ) {var p1;}; f(); f()",
    );
    t.test(
        "const f = async function (p1) {       }; f(); f()",
        "const f = async function (  ) {var p1;}; f(); f()",
    );
    // constant parameter
    t.test(
        "var f = async function (p1) {          }; f(1); f(1)",
        "var f = async function (  ) {var p1 = 1;}; f( ); f( )",
    );
    t.test(
        "let f = async function (p1) {           }; f(1); f(1)",
        "let f = async function (  ) {var p1 = 1;}; f( ); f( )",
    );
    t.test(
        "const f = async function (p1) {          }; f(1); f(1)",
        "const f = async function (  ) {var p1 = 1;}; f( ); f( )",
    );
}

// port: OptimizeParametersTest#testSimpleRemovalGenerator
#[test]
fn test_simple_removal_generator() {
    let mut t = Test::new();
    // parameter never supplied
    t.test(
        "var f = function * (p1) {       }; f(); f()",
        "var f = function * (  ) {var p1;}; f(); f()",
    );
    t.test(
        "let f = function * (p1) {       }; f(); f()",
        "let f = function * (  ) {var p1;}; f(); f()",
    );
    t.test(
        "const f = function * (p1) {       }; f(); f()",
        "const f = function * (  ) {var p1;}; f(); f()",
    );
    // constant parameter
    t.test(
        "var f = function * (p1) {          }; f(1); f(1)",
        "var f = function * (  ) {var p1 = 1;}; f( ); f( )",
    );
    t.test(
        "let f = function * (p1) {           }; f(1); f(1)",
        "let f = function * (  ) {var p1 = 1;}; f( ); f( )",
    );
    t.test(
        "const f = function * (p1) {          }; f(1); f(1)",
        "const f = function * (  ) {var p1 = 1;}; f( ); f( )",
    );
}

// port: OptimizeParametersTest#testNotAFunction
#[test]
fn test_not_a_function() {
    let mut t = Test::new();
    t.test_same("var x = 1; x; x = 2");
}

// port: OptimizeParametersTest#testRemoveOneOptionalNamedFunction
#[test]
fn test_remove_one_optional_named_function() {
    let mut t = Test::new();
    t.test(
        "function foo(p1) { } foo()",
        "function foo() {var p1} foo()",
    );
}

// port: OptimizeParametersTest#testDifferentScopes
#[test]
fn test_different_scopes() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function f(a, b) {} f(1, 2); f(1, 3);\n",
            "function h() {function g(a) {} g(4); g(5);} f(1, 2);\n"
        ),
        concat!(
            "function f(b) {var a = 1} f(2); f(3);\n",
            "function h() {function g(a) {} g(4); g(5);} f(2);\n"
        ),
    );
}

// port: OptimizeParametersTest#testOptimizeOnlyImmutableValues1
#[test]
fn test_optimize_only_immutable_values1() {
    let mut t = Test::new();
    t.test(
        "function foo(a) {}; foo(undefined);",
        "function foo() {var a = undefined}; foo()",
    );
}

// port: OptimizeParametersTest#testOptimizeOnlyImmutableValues2
#[test]
fn test_optimize_only_immutable_values2() {
    let mut t = Test::new();
    t.test(
        "function foo(a) {}; foo(null);",
        "function foo() {var a = null}; foo()",
    );
    t.test(
        "function foo(a) {}; foo(1);",
        "function foo() {var a = 1}; foo()",
    );
    t.test(
        "function foo(a) {}; foo('abc');",
        "function foo() {var a = 'abc'}; foo()",
    );
    t.test(
        "var foo = function(a) {}; foo(undefined);",
        "var foo = function() {var a = undefined}; foo()",
    );
    t.test(
        "var foo = function(a) {}; foo(null);",
        "var foo = function() {var a = null}; foo()",
    );
    t.test(
        "var foo = function(a) {}; foo(1);",
        "var foo = function() {var a = 1}; foo()",
    );
    t.test(
        "var foo = function(a) {}; foo('abc');",
        "var foo = function() {var a = 'abc'}; foo()",
    );
}

// port: OptimizeParametersTest#testOptimizeOnlyImmutableValues3
#[test]
fn test_optimize_only_immutable_values3() {
    let mut t = Test::new();
    // "var a = null;" gets inserted after the declaration of 'goo' so the tree stays normalized.
    t.test(
        concat!(
            "function foo(a) {\n",
            "  function goo() {}\n",
            "  goo(a);\n",
            "};\n",
            "foo(null);\n"
        ),
        concat!(
            "function foo() {\n",
            "  function goo() {}\n",
            "  var a = null;\n",
            "  goo(a);\n",
            "};\n",
            "foo();\n"
        ),
    );
    t.test(
        concat!(
            "function foo(a) {\n",
            "  function goo() {}\n",
            "  function boo() {}\n",
            "  goo(a);\n",
            "};\n",
            "foo(null);\n"
        ),
        concat!(
            "function foo() {\n",
            "  function goo() {}\n",
            "  function boo() {}\n",
            "  var a = null;\n",
            "  goo(a);\n",
            "};\n",
            "foo();\n"
        ),
    );
}

// port: OptimizeParametersTest#testOptimizeOnlyImmutableValues4
#[test]
fn test_optimize_only_immutable_values4() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a) {\n",
            "  function goo() { return a; }\n",
            "};\n",
            "foo(null);\n"
        ),
        concat!(
            "function foo() {\n",
            "  function goo() { return a; }\n",
            "  var a = null;\n",
            "};\n",
            "foo();\n"
        ),
    );
}

// port: OptimizeParametersTest#testRemoveOneOptionalVarAssignment
#[test]
fn test_remove_one_optional_var_assignment() {
    let mut t = Test::new();
    t.test(
        "var foo = function (p1) { }; foo()",
        "var foo = function () {var p1}; foo()",
    );
}

// port: OptimizeParametersTest#testDoOptimizeCall
#[test]
fn test_do_optimize_call() {
    let mut t = Test::new();
    t.test_same("var foo = function () {}; foo(); foo.call();");
    t.test_same("var foo = function () {}; foo(); foo?.call();");
    // TODO(johnlenz): support removing unused "this" from .call
    t.test_same("var foo = function () {}; foo(); foo.call(this);");
    t.test_same("var foo = function () {}; foo(); foo?.call(this);");
    t.test(
        "var foo = function (a, b) {}; foo(1); foo.call(this, 1);",
        "var foo = function () {var a = 1;var b;}; foo(); foo.call(this);",
    );
    t.test(
        "var foo = function (a, b) {}; foo?.(1); foo?.call(this, 1);",
        "var foo = function () {var a = 1;var b;}; foo?.(); foo?.call(this);",
    );
    t.test_same("var foo = function () {}; foo(); foo.call(null);");
    t.test_same("var foo = function () {}; foo(); foo?.call(null);");
    t.test(
        "var foo = function (a, b) {}; foo(1); foo.call(null, 1);",
        "var foo = function () {var a = 1;var b;}; foo(); foo.call(null);",
    );
    t.test_same("var foo = function () {}; foo.call();");
    t.test_same("var foo = function () {}; foo.call(this);");
    t.test(
        "var foo = function (a) {}; foo.call(this, 1);",
        "var foo = function () {var a = 1;}; foo.call(this);",
    );
    t.test_same("var foo = function () {}; foo.call(null);");
    t.test(
        "var foo = function (a) {}; foo.call(null, 1);",
        "var foo = function () {var a = 1;}; foo.call(null);",
    );
    t.test(
        "var foo = function (a) {}; foo?.call(null, 1);",
        "var foo = function () {var a = 1;}; foo?.call(null);",
    );
    t.test_same("var foo = function (a, b) {}; foo.call(...arr);");
    t.test_same("var foo = function (a, b, c) {}; foo.call(...arr, 1, 2);");
    t.test_same("var foo = function (a, b) {}; foo?.call(...arr);");
    t.test_same("var foo = function (p1, p2) {}; foo(p1, ...args);");
    t.test_same("var foo = function (p1, p2) {}; foo.call(this, p1, ...args);");
    t.test_same("var foo = function (p1, p2) {}; foo?.call(this, p1, ...args);");
}

// port: OptimizeParametersTest#testDoOptimizeApply
#[test]
fn test_do_optimize_apply() {
    let mut t = Test::new();
    t.test_same("var foo = function () {}; foo(); foo.apply();");
    t.test_same("var foo = function () {}; foo(); foo.apply(this);");
    t.test_same("var foo = function (a, b) {}; foo(1); foo.apply(this, 1);");
    t.test_same("var foo = function () {}; foo(); foo.apply(null);");
    t.test_same("var foo = function (a, b) {}; foo(1); foo.apply(null, []);");
    t.test_same("var foo = function () {}; foo.apply();");
    t.test_same("var foo = function () {}; foo.apply(this);");
    t.test_same("var foo = function (a, b) {}; foo.apply(this, 1);");
    t.test_same("var foo = function () {}; foo.apply(null);");
    t.test_same("var foo = function (a, b) {}; foo.apply(null, []);");
    // optional versions of the above tests
    t.test_same("var foo = function () {}; foo(); foo?.apply();");
    t.test_same("var foo = function () {}; foo(); foo?.apply(this);");
    t.test_same("var foo = function (a, b) {}; foo(1); foo?.apply(this, 1);");
    t.test_same("var foo = function () {}; foo(); foo?.apply(null);");
    t.test_same("var foo = function (a, b) {}; foo(1); foo?.apply(null, []);");
    t.test_same("var foo = function () {}; foo?.apply();");
    t.test_same("var foo = function () {}; foo?.apply(this);");
    t.test_same("var foo = function (a, b) {}; foo?.apply(this, 1);");
    t.test_same("var foo = function () {}; foo?.apply(null);");
    t.test_same("var foo = function (a, b) {}; foo?.apply(null, []);");
}

// port: OptimizeParametersTest#testRemoveOneOptionalExpressionAssign
#[test]
fn test_remove_one_optional_expression_assign() {
    let mut t = Test::new();
    t.test(
        "var foo; foo = function (p1) { }; foo()",
        "var foo; foo = function () { var p1; }; foo()",
    );
}

// port: OptimizeParametersTest#testRemoveOneOptionalOneRequired
#[test]
fn test_remove_one_optional_one_required() {
    let mut t = Test::new();
    t.test(
        "function foo(p1, p2) { } foo(1); foo(2)",
        "function foo(p1) {var p2} foo(1); foo(2)",
    );
}

// port: OptimizeParametersTest#testRemoveOneOptionalMultipleCalls
#[test]
fn test_remove_one_optional_multiple_calls() {
    let mut t = Test::new();
    t.test(
        "function foo(p1, p2) { } foo(1); foo(2); foo()",
        "function foo(p1) {var p2} foo(1); foo(2); foo()",
    );
    t.test(
        "function foo(p1, p2) { } foo?.(1); foo(2); foo?.()",
        "function foo(p1) {var p2} foo?.(1); foo(2); foo?.()",
    );
}

// port: OptimizeParametersTest#testRemoveOneOptionalMultiplePossibleDefinition
#[test]
fn test_remove_one_optional_multiple_possible_definition() {
    let mut t = Test::new();
    let src = concat!(
        "var goog = {};\n",
        "goog.foo = function (p1, p2) { };\n",
        "goog.foo = function (q1, q2) { };\n",
        "goog.foo = function (r1, r2) { };\n",
        "goog.foo(1); goog.foo(2); goog.foo()\n"
    );
    let result = concat!(
        "var goog = {};\n",
        "goog.foo = function (p1) { var p2; };\n",
        "goog.foo = function (q1) { var q2; };\n",
        "goog.foo = function (r1) { var r2; };\n",
        "goog.foo(1); goog.foo(2); goog.foo()\n"
    );
    t.test(src, result);
}

// port: OptimizeParametersTest#testRemoveTwoOptionalMultiplePossibleDefinition
#[test]
fn test_remove_two_optional_multiple_possible_definition() {
    let mut t = Test::new();
    let src = concat!(
        "var goog = {};\n",
        "goog.foo = function (p1, p2, p3, p4) { };\n",
        "goog.foo = function (q1, q2, q3, q4) { };\n",
        "goog.foo = function (r1, r2, r3, r4) { };\n",
        "goog.foo(1,0); goog.foo(2,1); goog.foo()\n"
    );
    let result = concat!(
        "var goog = {};\n",
        "goog.foo = function (p1, p2) { var p3; var p4 };\n",
        "goog.foo = function (q1, q2) { var q3; var q4 };\n",
        "goog.foo = function (r1, r2) { var r3; var r4 };\n",
        "goog.foo(1,0); goog.foo(2,1); goog.foo()\n"
    );
    t.test(src, result);
}

// port: OptimizeParametersTest#testMultipleCalls
#[test]
fn test_multiple_calls() {
    let mut t = Test::new();
    let src = concat!("function f(p1, p2, p3, p4) { };\n", "f(1,0); f(2,1); f()\n");
    let result = concat!(
        "function f(p1, p2) { var p3; var p4 };\n",
        "f(1,0); f(2,1); f()\n"
    );
    t.test(src, result);
}

// port: OptimizeParametersTest#testConstructorOptArgsNotRemoved
#[test]
fn test_constructor_opt_args_not_removed() {
    let mut t = Test::new();
    let src = concat!(
        "/** @constructor */\n",
        "var goog = function(){};\n",
        "goog.prototype.foo = function(a,b) {};\n",
        "goog.prototype.bar = function(a) {};\n",
        "goog.bar.inherits(goog.foo);\n",
        "new goog.foo(2,3);\n",
        "new goog.foo(1,2);\n"
    );
    t.test_same(src);
}

// port: OptimizeParametersTest#testMultipleUnknown
#[test]
fn test_multiple_unknown() {
    let mut t = Test::new();
    let src = concat!(
        "var goog1 = {};\n",
        "goog1.foo = function () { };\n",
        "var goog2 = {};\n",
        "goog2.foo = function (p1) { };\n",
        "var x = getGoog();\n",
        "x.foo()\n"
    );
    let result = concat!(
        "var goog1 = {};\n",
        "goog1.foo = function () { };\n",
        "var goog2 = {};\n",
        "goog2.foo = function () { var p1; };\n",
        "var x = getGoog();\n",
        "x.foo()\n"
    );
    t.test(src, result);
}

// port: OptimizeParametersTest#testSingleUnknown
#[test]
fn test_single_unknown() {
    let mut t = Test::new();
    let src = concat!(
        "var goog2 = {};\n",
        "goog2.foo = function (p1) { };\n",
        "var x = getGoog();x.foo()\n"
    );
    let expected = concat!(
        "var goog2 = {};\n",
        "goog2.foo = function () { var p1 };\n",
        "var x = getGoog();x.foo()\n"
    );
    t.test(src, expected);
}

// port: OptimizeParametersTest#testRemoveVarArg
#[test]
fn test_remove_var_arg() {
    let mut t = Test::new();
    t.test(
        "function foo(p1, var_args) { } foo(1); foo(2)",
        "function foo(p1) { var var_args } foo(1); foo(2)",
    );
}

// port: OptimizeParametersTest#testAliasMethodsDontGetOptimize
#[test]
fn test_alias_methods_dont_get_optimize() {
    let mut t = Test::new();
    let src = concat!(
        "var foo = function(a, b) {};\n",
        "var goog = {};\n",
        "goog.foo = foo;\n",
        "goog.prototype.bar = goog.foo;\n",
        "new goog().bar(1,2);\n",
        "foo(2);\n"
    );
    t.test_same(src);
}

// port: OptimizeParametersTest#testAliasMethodsDontGetOptimize2
#[test]
fn test_alias_methods_dont_get_optimize2() {
    let mut t = Test::new();
    let src = concat!(
        "var foo = function(a, b) {};\n",
        "var bar = foo;foo(1);bar(2,3);\n"
    );
    t.test_same(src);
}

// port: OptimizeParametersTest#testAliasMethodsDontGetOptimize3
#[test]
fn test_alias_methods_dont_get_optimize3() {
    let mut t = Test::new();
    let src = concat!(
        "var array = {};\n",
        "array[0] = function(a, b) {};\n",
        "var foo = array[0]; // foo should be marked as aliased.\n",
        "foo(1);\n"
    );
    t.test_same(src);
    let src_opt_chain = concat!(
        "var array = {};\n",
        "array[0] = function(a, b) {};\n",
        "var foo = array[0]; // foo should be marked as aliased.\n",
        "foo?.(1);\n"
    );
    t.test_same(src_opt_chain);
}

// port: OptimizeParametersTest#testAliasMethodsDontGetOptimize4
#[test]
fn test_alias_methods_dont_get_optimize4() {
    let mut t = Test::new();
    // Don't change the call to baz as it has been aliased.
    t.test(
        concat!(
            "function foo(bar) {};\n",
            "var baz = function(a) {};\n",
            "baz(1);\n",
            "foo(baz);\n"
        ),
        concat!(
            "function foo() {var bar = baz};\n",
            "var baz = function(a) {};\n",
            "baz(1);\n",
            "foo();\n"
        ),
    );
}

// port: OptimizeParametersTest#testMethodsDefinedInArraysDontGetOptimized
#[test]
fn test_methods_defined_in_arrays_dont_get_optimized() {
    let mut t = Test::new();
    let src = concat!("var array = [true, function (a) {}];\n", "array[1](1)\n");
    t.test_same(src);
    let src_opt_chain = concat!("var array = [true, function (a) {}];\n", "array[1]?.(1)\n");
    t.test_same(src_opt_chain);
}

// port: OptimizeParametersTest#testMethodsDefinedInObjectDontGetOptimized
#[test]
fn test_methods_defined_in_object_dont_get_optimized() {
    let mut t = Test::new();
    let src = concat!(
        "var object = { foo: function bar() {} };\n",
        "object.foo(1)\n"
    );
    t.test_same(src);
    let src = concat!(
        "var object = { foo: function bar() {} };\n",
        "object['foo'](1)\n"
    );
    t.test_same(src);
    let src = concat!(
        "var object = { foo: function bar() {} };\n",
        "object['foo']?.(1)\n"
    );
    t.test_same(src);
}

// port: OptimizeParametersTest#testRemoveConstantArgument
#[test]
fn test_remove_constant_argument() {
    let mut t = Test::new();
    // Remove only one parameter
    t.test(
        "function foo(p1, p2) {}; foo(1,2); foo(2,2);",
        "function foo(p1) {var p2 = 2}; foo(1); foo(2)",
    );
    // @noinline prevents constant inlining
    t.test_same("/** @noinline */ function foo(p1, p2) {}; foo(1,2); foo(2,2);");
    // Remove nothing
    t.test_same("function foo(p1, p2) {}; foo(1); foo(2,3);");
    // Remove middle parameter
    t.test(
        "function foo(a,b,c){}; foo(1, 2, 3); foo(1, 2, 4); foo(2, 2, 3)",
        "function foo(a,c){var b=2}; foo(1, 3); foo(1, 4); foo(2, 3)",
    );
    // Number are equals
    t.test(
        "function foo(a) {}; foo(1); foo(1.0);",
        "function foo() {var a = 1;}; foo(); foo();",
    );
    // A more OO test
    t.test(
        concat!(
            "/** @constructor */\n",
            "function Person() {}; Person.prototype.run = function(a, b) {};\n",
            "Person.run(1, 'a'); Person.run(2, 'a');\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function Person() {}; Person.prototype.run = function(a) {var b = 'a'};\n",
            "Person.run(1); Person.run(2);\n"
        ),
    );
}

// port: OptimizeParametersTest#testCanDeleteArgumentsAtAnyPosition
#[test]
fn test_can_delete_arguments_at_any_position() {
    let mut t = Test::new();
    // Argument removed in middle and end
    let src = concat!(
        "function foo(a,b,c,d,e) {};\n",
        "foo(1,2,3,4,5);\n",
        "foo(2,2,4,4,5);\n"
    );
    let expected = concat!(
        "function foo(a,c) {var b=2; var d=4; var e=5;};\n",
        "foo(1,3);\n",
        "foo(2,4);\n"
    );
    t.test(src, expected);
}

// port: OptimizeParametersTest#testNoOptimizationForExternsFunctions
#[test]
fn test_no_optimization_for_externs_functions() {
    let mut t = Test::new();
    t.test_same("function _foo(x, y, z){}; _foo(1);");
}

// port: OptimizeParametersTest#testNoOptimizationForGoogExportSymbol
#[test]
fn test_no_optimization_for_goog_export_symbol() {
    let mut t = Test::new();
    t.test_same(concat!(
        "goog.exportSymbol('foo', foo);\n",
        "function foo(x, y, z){}; foo(1);\n"
    ));
}

// port: OptimizeParametersTest#testNoArgumentRemovalNonEqualNodes
#[test]
fn test_no_argument_removal_non_equal_nodes() {
    let mut t = Test::new();
    t.test_same("function foo(a){}; foo('bar'); foo('baz');");
    t.test_same("function foo(a){}; foo(1.0); foo(2.0);");
    t.test_same("function foo(a){}; foo(true); foo(false);");
    t.test_same("var a = 1, b = 2; function foo(a){}; foo(a); foo(b);");
    t.test_same("function foo(a){}; foo(/&/g); foo(/</g);");
}

// port: OptimizeParametersTest#testFunctionPassedAsParam
#[test]
fn test_function_passed_as_param() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */ function person() {};\n",
            "person.prototype.run = function(a, b) {};\n",
            "person.prototype.walk = function() {};\n",
            "person.prototype.foo = function() { this.run(this.walk, 0.1); };\n",
            "person.foo();\n"
        ),
        concat!(
            "/** @constructor */ function person() {};\n",
            "person.prototype.run = function(a) { var b = 0.1; };\n",
            "person.prototype.walk = function() {};\n",
            "person.prototype.foo = function() { this.run(this.walk); };\n",
            "person.foo();\n"
        ),
    );
}

// port: OptimizeParametersTest#testCallIsIgnore
#[test]
fn test_call_is_ignore() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var goog;\n",
            "goog.foo = function(a, opt) {};\n",
            "var bar = function(){goog.foo.call(this, 1)};\n",
            "goog.foo(1);\n"
        ),
        concat!(
            "var goog;\n",
            "goog.foo = function() {var a = 1;var opt;};\n",
            "var bar = function(){goog.foo.call(this)};\n",
            "goog.foo();\n"
        ),
    );
}

// port: OptimizeParametersTest#testApplyIsIgnore
#[test]
fn test_apply_is_ignore() {
    let mut t = Test::new();
    t.test_same(concat!("var goog;\n", "goog.foo = function(a, opt) {};var bar = function(){goog.foo.apply(this, 1)};goog.foo(1);\n"));
}

// port: OptimizeParametersTest#testFunctionWithReferenceToArgumentsShouldNotBeOptimized
#[test]
fn test_function_with_reference_to_arguments_should_not_be_optimized() {
    let mut t = Test::new();
    t.test_same("function foo(a,b,c) { return arguments.size; }; foo(1);");
    t.test_same("var foo = function(a,b,c) { return arguments.size }; foo(1);");
    t.test_same("var foo = function bar(a,b,c) { return arguments.size }; foo(2);");
}

// port: OptimizeParametersTest#testClassMemberWithReferenceToArgumentsShouldNotBeOptimize
#[test]
fn test_class_member_with_reference_to_arguments_should_not_be_optimize() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C {\n",
        "  constructor() {\n",
        "  }\n",
        "  setValue(value) {\n",
        "    if (!arguments.length) {\n",
        "      return 0;\n",
        "    }\n",
        "    return value;\n",
        "  }\n",
        "}\n",
        "var c = new C();\n",
        "alert(c.setValue(42));\n"
    ));
}

// port: OptimizeParametersTest#testFunctionWithTwoNames
#[test]
fn test_function_with_two_names() {
    let mut t = Test::new();
    t.test_same("var foo = function bar(a,b) {};");
    t.test_same("var foo = function bar(a,b) {}; foo(1)");
    t.test_same("var foo = function bar(a,b) {}; foo(1); foo(2)");
}

// port: OptimizeParametersTest#testRecursion
#[test]
fn test_recursion() {
    let mut t = Test::new();
    t.test(
        "var foo = function (a,b) {foo(1, b)}; foo(1, 2)",
        "var foo = function (b) {var a=1; foo(b)}; foo(2)",
    );
}

// port: OptimizeParametersTest#testConstantArgumentsToConstructorCanBeOptimized
#[test]
fn test_constant_arguments_to_constructor_can_be_optimized() {
    let mut t = Test::new();
    let src = concat!("function foo(a) {};\n", "var bar = new foo(1);\n");
    let expected = concat!("function foo() {var a=1;};\n", "var bar = new foo();\n");
    t.test(src, expected);
}

// port: OptimizeParametersTest#testOptionalArgumentsToConstructorCanBeOptimized
#[test]
fn test_optional_arguments_to_constructor_can_be_optimized() {
    let mut t = Test::new();
    let src = concat!("function foo(a) {};\n", "var bar = new foo();\n");
    let expected = concat!("function foo() {var a;};\n", "var bar = new foo();\n");
    t.test(src, expected);
}

// port: OptimizeParametersTest#testRegexesCanBeInlined
#[test]
fn test_regexes_can_be_inlined() {
    let mut t = Test::new();
    t.test(
        "function foo(a) {}; foo(/abc/);",
        "function foo() {var a = /abc/}; foo();",
    );
}

// port: OptimizeParametersTest#testConstructorUsedAsFunctionCanBeOptimized
#[test]
fn test_constructor_used_as_function_can_be_optimized() {
    let mut t = Test::new();
    let src = concat!(
        "function foo(a) {};\n",
        "var bar = new foo(1);\n",
        "foo(1);\n"
    );
    let expected = concat!(
        "function foo() {var a=1;};\n",
        "var bar = new foo();\n",
        "foo();\n"
    );
    t.test(src, expected);
}

// port: OptimizeParametersTest#testDoNotOptimizeConstructorWhenArgumentsAreNotEqual
#[test]
fn test_do_not_optimize_constructor_when_arguments_are_not_equal() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function Foo(a) {};\n",
        "var bar = new Foo(1);\n",
        "var baz = new Foo(2);\n"
    ));
}

// port: OptimizeParametersTest#testDoNotOptimizeArrayElements
#[test]
fn test_do_not_optimize_array_elements() {
    let mut t = Test::new();
    t.test_same("var array = [function (a, b) {}];");
    t.test_same("var array = [function f(a, b) {}]");
    t.test_same(concat!(
        "var array = [function (a, b) {}];\n",
        "array[0](1, 2);\n",
        "array[0](1);\n"
    ));
    t.test_same(concat!(
        "var array = [];\n",
        "function foo(a, b) {};\n",
        "array[0] = foo;\n"
    ));
}

// port: OptimizeParametersTest#testOptimizeThis1
#[test]
fn test_optimize_this1() {
    let mut t = Test::new();
    let src = concat!(
        "var bar = function (a, b) {};\n",
        "function foo() {\n",
        "  this.bar = function (a, b) {};\n",
        "  this.bar(3);\n",
        "  bar(2);\n",
        "}\n"
    );
    let expected = concat!(
        "var bar = function () {var a = 2;var b;};\n",
        "function foo() {\n",
        "  this.bar = function () {var a = 3;var b;};\n",
        "  this.bar();\n",
        "  bar();\n",
        "}\n"
    );
    t.test(src, expected);
}

// port: OptimizeParametersTest#testOptimizeThis2
#[test]
fn test_optimize_this2() {
    let mut t = Test::new();
    let src = concat!(
        "function foo() {\n",
        "  var bar = function (a, b) {};\n",
        "  this.bar = function (a, b) {};\n",
        "  this.bar(3);\n",
        "  bar(2);\n",
        "}\n"
    );
    let expected = concat!(
        "function foo() {\n",
        "  var bar = function (a, b) {};\n",
        "  this.bar = function () {var a = 3;var b;};\n",
        "  this.bar();\n",
        "  bar(2);\n",
        "}\n"
    );
    t.test(src, expected);
}

// port: OptimizeParametersTest#testDoNotOptimizeWhenArgumentsPassedAsParameter
#[test]
fn test_do_not_optimize_when_arguments_passed_as_parameter() {
    let mut t = Test::new();
    t.test_same("function foo(a) {}; foo(arguments)");
    t.test_same("function foo(a) {}; foo(arguments[0])");
    t.test(
        "function foo(a, b) {}; foo(arguments, 1)",
        "function foo(a) {var b = 1}; foo(arguments)",
    );
    t.test(
        "function foo(a, b) {}; foo(arguments)",
        "function foo(a) {var b}; foo(arguments)",
    );
}

// port: OptimizeParametersTest#testDoNotOptimizeGoogExportFunctions
#[test]
fn test_do_not_optimize_goog_export_functions() {
    let mut t = Test::new();
    t.test_same("function foo(a, b) {}; foo(); goog.export_function(foo);");
}

// port: OptimizeParametersTest#testDoNotOptimizeJSCompiler_renameProperty
#[test]
fn test_do_not_optimize_js_compiler_rename_property() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function JSCompiler_renameProperty(a) {return a};\n",
        "JSCompiler_renameProperty('a');\n"
    ));
}

// port: OptimizeParametersTest#testMutableValues1
#[test]
fn test_mutable_values1() {
    let mut t = Test::new();
    t.test("function foo(p1) {} foo()", "function foo() {var p1} foo()");
    t.test(
        "function foo(p1) {} foo(1)",
        "function foo() {var p1=1} foo()",
    );
    t.test(
        "function foo(p1) {} foo([])",
        "function foo() {var p1=[]} foo()",
    );
    t.test(
        "function foo(p1) {} foo({})",
        "function foo() {var p1={}} foo()",
    );
    t.test(
        "var x;function foo(p1) {} foo(x)",
        "var x;function foo() {var p1=x} foo()",
    );
    t.test(
        "var x;function foo(p1) {} foo(x())",
        "var x;function foo() {var p1=x()} foo()",
    );
    t.test(
        "var x;function foo(p1) {} foo(new x())",
        "var x;function foo() {var p1=new x()} foo()",
    );
    t.test(
        "var x;function foo(p1) {} foo('' + x)",
        "var x;function foo() {var p1='' + x} foo()",
    );
    t.test_same("function foo(p1) {} foo(this)");
    t.test_same("function foo(p1) {} foo(arguments)");
    t.test_same("function foo(p1) {} foo(function(){})");
    t.test_same("function foo(p1) {} (function () {var x;foo(x)})()");
}

// port: OptimizeParametersTest#testMutableValues2
#[test]
fn test_mutable_values2() {
    let mut t = Test::new();
    t.test(
        "function foo(p1, p2) {} foo(1, 2)",
        "function foo() {var p1=1; var p2 = 2} foo()",
    );
    t.test(
        "var x; var y; function foo(p1, p2) {} foo(x(), y())",
        "var x; var y; function foo() {var p1=x(); var p2 = y()} foo()",
    );
}

// port: OptimizeParametersTest#testMutableValues3
#[test]
fn test_mutable_values3() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var x; var y; var z;\n",
            "function foo(p1, p2) {}\n",
            "foo(x(), y()); foo(x(),y())\n"
        ),
        concat!(
            "var x; var y; var z;\n",
            "function foo() {var p1=x(); var p2=y()}\n",
            "foo(); foo()\n"
        ),
    );
}

// port: OptimizeParametersTest#testMutableValues4
#[test]
fn test_mutable_values4() {
    let mut t = Test::new();
    // Preserve the ordering of side-effects.
    // If z(), can't be moved into the function then z() may change the value
    // of x and y.
    t.test_same(concat!(
        "var x; var y; var z;\n",
        "function foo(p1, p2, p3) {}\n",
        "foo(x(), y(), z()); foo(x(),y(),3)\n"
    ));
    // If z(), can't be moved into the function then z() may change the value
    // of x and y.
    t.test_same(concat!(
        "var x; var y; var z;\n",
        "function foo(p1, p2, p3) {}\n",
        "foo(x, y(), z()); foo(x,y(),3)\n"
    ));
    // Mutable object that can not be effect by side-effects are movable,
    // however.
    t.test(
        concat!(
            "var x; var y; var z;\n",
            "function foo(p1, p2, p3) {}\n",
            "foo([], y(), z()); foo([],y(),3)\n"
        ),
        concat!(
            "var x; var y; var z;\n",
            "function foo(p2, p3) {var p1=[]}\n",
            "foo(y(), z()); foo(y(),3)\n"
        ),
    );
    // Pure literal at the first call site and side-effectful call at the second call site.
    t.test_same(concat!(
        "var x; var y; var z;\n",
        "function foo(p1, p2, p3) { alert(p1); }\n",
        "foo(x, y(), 3);\n",
        "foo(x, y(), z());\n"
    ));
    // Mutable variable passed along with an argument that mutates it at the second call site.
    t.test_same(concat!(
        "var out = [];\n",
        "var policy = 'STRICT';\n",
        "function mutateAndReturn() { policy = 'LENIENT'; return {}; }\n",
        "function render(p, opts) { out.push(p); }\n",
        "render(policy, {});\n",
        "render(policy, mutateAndReturn());\n"
    ));
}

// port: OptimizeParametersTest#testMutableValues5
#[test]
fn test_mutable_values5() {
    let mut t = Test::new();
    t.test(
        concat!(
            "var x; var y; var z;\n",
            "function foo(p1, p2) {}\n",
            "new foo(new x(), y()); new foo(new x(),y())\n"
        ),
        concat!(
            "var x; var y; var z;\n",
            "function foo() {var p1=new x(); var p2=y()}\n",
            "new foo(); new foo()\n"
        ),
    );
    t.test(
        concat!(
            "var x; var y; var z;\n",
            "function foo(p1, p2) {}\n",
            "new foo(x(), y()); new foo(x(),y())\n"
        ),
        concat!(
            "var x; var y; var z;\n",
            "function foo() {var p1=x(); var p2=y()}\n",
            "new foo(); new foo()\n"
        ),
    );
    t.test_same(concat!(
        "var x; var y; var z;\n",
        "function foo(p1, p2, p3) {}\n",
        "new foo(x(), y(), z()); new foo(x(),y(),3)\n"
    ));
    t.test_same(concat!(
        "var x; var y; var z;\n",
        "function foo(p1, p2, p3) {}\n",
        "new foo(x, y(), z()); new foo(x,y(),3)\n"
    ));
    t.test(
        concat!(
            "var x; var y; var z;\n",
            "function foo(p1, p2, p3) {}\n",
            "new foo([], y(), z()); new foo([],y(),3)\n"
        ),
        concat!(
            "var x; var y; var z;\n",
            "function foo(p2, p3) {var p1=[]}\n",
            "new foo(y(), z()); new foo(y(),3)\n"
        ),
    );
}

// port: OptimizeParametersTest#testMutableValuesDoNotMoveSuper
#[test]
fn test_mutable_values_do_not_move_super() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var A;\n",
        "function fn(p1) {}\n",
        "class B extends A { constructor() { fn(super.x); } }\n"
    ));
}

// port: OptimizeParametersTest#testShadows
#[test]
fn test_shadows() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function foo(a) {}\n",
        "var x;\n",
        "function f() {\n",
        "  var x;\n",
        "  function g() {\n",
        "    foo(x());\n",
        "  }\n",
        "};\n",
        "foo(x())\n"
    ));
}

// port: OptimizeParametersTest#testNoCrash
#[test]
fn test_no_crash() {
    let mut t = Test::new();
    t.test(
        concat!("function foo(a) {}\n", "foo({o:1});\n", "foo({o:1})\n"),
        concat!("function foo() {var a = {o:1}}\n", "foo();\n", "foo()\n"),
    );
}

// port: OptimizeParametersTest#testGlobalCatch
#[test]
fn test_global_catch() {
    let mut t = Test::new();
    t.test_same("function foo(a) {} try {} catch (e) {foo(e)}");
}

// port: OptimizeParametersTest#testNamelessParameter1
#[test]
fn test_nameless_parameter1() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs("var g;"),
        srcs("f(g()); function f(){}"),
        expected("f(); function f(){g()}"),
    ]);
}

// port: OptimizeParametersTest#testNamelessParameter2
#[test]
fn test_nameless_parameter2() {
    let mut t = Test::new();
    t.test_parts(vec![
        externs("var g, h;"),
        srcs("f(g(),h()); function f(){}"),
        expected("f(); function f(){g();h()}"),
    ]);
}

// port: OptimizeParametersTest#testRewriteUsedClassConstructor1
#[test]
fn test_rewrite_used_class_constructor1() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  constructor(a) {\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n"
        ),
        concat!(
            "class C {\n",
            "  constructor( ) {\n",
            "    var a; // moved from parameter list\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n"
        ),
    );
}

// port: OptimizeParametersTest#testConstructorEscapesThroughThis
#[test]
fn test_constructor_escapes_through_this() {
    let mut t = Test::new();
    // Demonstrate b/174875103
    t.test(
        concat!(
            "class C {\n",
            "  constructor(a) {\n",
            "    use(a);\n",
            "  }\n",
            "  static create() { return new this(2); };\n",
            "}\n",
            "class D extends C {\n",
            "  constructor(b) { super(1); }\n",
            "\n",
            "}\n",
            "var c = new C(1);\n",
            "var d = new D(1)\n",
            "var e = D.create();\n"
        ),
        concat!(
            "class C {\n",
            "  constructor() {\n",
            "    var a = 1; // <-- bad optimization\n",
            "    use(a);\n",
            "  }\n",
            "  static create() { return new this(2); }; // <-- also a call here\n",
            "}\n",
            "class D extends C {\n",
            "  constructor(b) { super(); }\n",
            "\n",
            "}\n",
            "var c = new C();\n",
            "var d = new D(1)\n",
            "var e = D.create();\n"
        ),
    );
}

// port: OptimizeParametersTest#testRewriteUsedES5Constructor1
#[test]
fn test_rewrite_used_es5_constructor1() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C(a) {\n",
            "  use(a);\n",
            "}\n",
            "var c = new C();\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C( ) {\n",
            "  var a; // moved from parameter list\n",
            "  use(a);\n",
            "}\n",
            "var c = new C();\n"
        ),
    );
}

// port: OptimizeParametersTest#testRewriteUsedClassConstructor2
#[test]
fn test_rewrite_used_class_constructor2() {
    let mut t = Test::new();
    // `constructor` aliases the class constructor
    t.test(
        concat!(
            "class C {\n",
            "  constructor(a) {\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n",
            "new c.constructor(1);\n"
        ),
        concat!(
            "class C {\n",
            "  constructor( ) {\n",
            "    var a; // moved from parameter list\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n",
            "// TODO(bradfordcsmith): This call is now broken, since the parameter is ignored.\n",
            "//     For now we consider the code size savings worth the risk of breaking this\n",
            "//     coding pattern that we consider bad practice anyway.\n",
            "new c.constructor(1);\n"
        ),
    );
}

// port: OptimizeParametersTest#testRewriteUsedES5Constructor2
#[test]
fn test_rewrite_used_es5_constructor2() {
    let mut t = Test::new();
    // `constructor` aliases the class constructor
    t.test(
        concat!(
            "/** @constructor */\n",
            " function C(a) {\n",
            "  use(a);\n",
            "}\n",
            "var c = new C();\n",
            "new c.constructor(1);\n"
        ),
        concat!(
            "/** @constructor */\n",
            " function C( ) {\n",
            "  var a; // moved from parameter list\n",
            "  use(a);\n",
            "}\n",
            "var c = new C();\n",
            "// TODO(bradfordcsmith): This call is now broken, since the parameter is ignored.\n",
            "//     For now we consider the code size savings worth the risk of breaking this\n",
            "//     coding pattern that we consider bad practice anyway.\n",
            "new c.constructor(1);\n"
        ),
    );
}

// port: OptimizeParametersTest#testNoRewriteUsedClassConstructor3
#[test]
fn test_no_rewrite_used_class_constructor3() {
    let mut t = Test::new();
    // `super` aliases the super type constructor
    t.test_same(concat!(
        "class C { constructor(a) { use(a); } }\n",
        "class D extends C { constructor() { super(1); } }\n",
        "var d = new D(); new C();\n"
    ));
}

// port: OptimizeParametersTest#testRewriteUsedClassConstructor4
#[test]
fn test_rewrite_used_class_constructor4() {
    let mut t = Test::new();
    // `new.target` aliases self and subtype constructors
    t.test(
        concat!(
            "class C {\n",
            "  constructor() {\n",
            "    var x = new new.target(1);\n",
            "  }\n",
            "}\n",
            "class D extends C {\n",
            "  constructor(a) {\n",
            "    super();\n",
            "  }\n",
            "}\n",
            "var d = new D(); new C();\n"
        ),
        concat!(
            "class C {\n",
            "  constructor() {\n",
            "// TODO(bradfordcsmith): This call is now broken, since the parameter is ignored.\n",
            "//     For now we consider the code size savings worth the risk of breaking this\n",
            "//     coding pattern that we consider bad practice anyway.\n",
            "    var x = new new.target(1);\n",
            "  }\n",
            "}\n",
            "class D extends C {\n",
            "  constructor( ) {\n",
            "    var a; // moved from parameter list\n",
            "    super();\n",
            "  }\n",
            "}\n",
            "var d = new D(); new C();\n"
        ),
    );
}

// port: OptimizeParametersTest#testNoRewriteUsedClassConstructor5
#[test]
fn test_no_rewrite_used_class_constructor5() {
    let mut t = Test::new();
    // Static class methods "this" values can alias constructors.
    t.test_same(concat!(
        "class C { constructor(a) { use(a); }; static create(a) { new this(1); } }\n",
        "var c = new C();\n",
        "C.create();\n"
    ));
}

// port: OptimizeParametersTest#testRewriteUsedClassConstructorWithClassStaticField
#[test]
fn test_rewrite_used_class_constructor_with_class_static_field() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  static field2 = alert();\n",
            "  constructor(a) {\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C(1);\n"
        ),
        concat!(
            "class C {\n",
            "  static field2 = alert();\n",
            "  constructor( ) {\n",
            "    var a = 1; // moved from parameter list\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n"
        ),
    );
    t.test(
        concat!(
            "class C {\n",
            "  static field2 = alert();\n",
            "  constructor(a) {\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C(alert());\n"
        ),
        concat!(
            "class C {\n",
            "  static field2 = alert();\n",
            "  constructor() {\n",
            "    var a = alert();\n",
            "    use(a);\n",
            "  }\n",
            "}\n",
            "var c = new C();\n"
        ),
    );
}

// port: OptimizeParametersTest#testRewriteClassStaticBlock_removeOptional
#[test]
fn test_rewrite_class_static_block_remove_optional() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a,b=1){\n",
            "  return a * b;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo(1));\n",
            "    use(foo(2));\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function foo(a){\n",
            "  var b = 1;\n",
            "  return a * b;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo(1));\n",
            "    use(foo(2));\n",
            "  }\n",
            "}\n"
        ),
    );
    // TODO(b/240443227): Function parameters inside class static blocks not optimized
    t.test_same(concat!(
        "class C {\n",
        "  static {\n",
        "    function foo(a,b=1){\n",
        "      return(a * b);\n",
        "    }\n",
        "    use(foo(1));\n",
        "    use(foo(2));\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeParametersTest#testRewriteClassStaticBlock_trailingUndefinedLiterals
#[test]
fn test_rewrite_class_static_block_trailing_undefined_literals() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a,b){\n",
            "  return a;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo(1, undefined, 2));\n",
            "    use(foo(2));\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function foo(a,b){\n",
            "  return a;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo(1));\n",
            "    use(foo(2));\n",
            "  }\n",
            "}\n"
        ),
    );
    // TODO(b/240443227): Function parameters inside class static blocks not optimized
    t.test_same(concat!(
        "class C {\n",
        "  static {\n",
        "    function foo(a,b){\n",
        "      return a;\n",
        "    }\n",
        "    use(foo(1, undefined, 2));\n",
        "    use(foo(2));\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeParametersTest#testRewriteClassStaticBlock_inlineParameter
#[test]
fn test_rewrite_class_static_block_inline_parameter() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a){\n",
            "  return a;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo(1));\n",
            "    use(foo(1));\n",
            "    use(foo(1));\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "function foo(){\n",
            "  var a = 1;\n",
            "  return a;\n",
            "}\n",
            "class C {\n",
            "  static {\n",
            "    use(foo());\n",
            "    use(foo());\n",
            "    use(foo());\n",
            "  }\n",
            "}\n"
        ),
    );
    // TODO(b/240443227): Function parameters inside class static blocks not optimized
    t.test_same(concat!(
        "class C {\n",
        "  static {\n",
        "    function foo(a){\n",
        "      return(a);\n",
        "    }\n",
        "    use(foo(1));\n",
        "    use(foo(1));\n",
        "    use(foo(1));\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteUsedClassConstructorWithClassNonstaticField
#[test]
fn test_no_rewrite_used_class_constructor_with_class_nonstatic_field() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C {\n",
        "  field2 = alert();\n",
        "  constructor(a) {\n",
        "    use(a);\n",
        "  }\n",
        "}\n",
        "var c = new C(1);\n"
    ));
    t.test_same(concat!(
        "class C {\n",
        "  field2 = alert();\n",
        "  constructor(a) {\n",
        "    use(a);\n",
        "  }\n",
        "}\n",
        "var c = new C(alert());\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteUsedClassMethodParam1
#[test]
fn test_no_rewrite_used_class_method_param1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C { method(a) { use(a); } }\n",
        "var c = new C(); c.method(1); c.method(2)\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteUnusedClassComputedMethodParam1
#[test]
fn test_no_rewrite_unused_class_computed_method_param1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C { [method](a) { } }\n",
        "var c = new C(); c[method](1); c[method](2)\n"
    ));
}

// port: OptimizeParametersTest#testRewriteUsedClassMethodParam1
#[test]
fn test_rewrite_used_class_method_param1() {
    let mut t = Test::new();
    t.test(
        "class C { method(a) {          }} new C().method(1)",
        "class C { method( ) {var a = 1;}} new C().method( )",
    );
}

// port: OptimizeParametersTest#testNoRewriteUnsedObjectMethodParam
#[test]
fn test_no_rewrite_unsed_object_method_param() {
    let mut t = Test::new();
    t.test_same("var o = { method(a) {          }}; o.method(1)");
}

// port: OptimizeParametersTest#testNoRewriteDestructured1
#[test]
fn test_no_rewrite_destructured1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C { m(a) {}};\n",
        "var c = new C();\n",
        "({m} = c);\n",
        "c.m(1)\n"
    ));
    t.test_same(concat!(
        "class C { m(a) {}};\n",
        "var c = new C();\n",
        "({m:x} = c);\n",
        "c.m(1)\n"
    ));
    t.test_same(concat!(
        "class C { m(a) {}};\n",
        "var c = new C();\n",
        "({xx:C.prototype.m} = {xx:function(a) {}});\n",
        "c.m(1)\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteDestructured2
#[test]
fn test_no_rewrite_destructured2() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var x = function(a) {};\n",
        "({x} = {})\n",
        "x(1)\n"
    ));
    t.test_same(concat!(
        "var x = function(a) {};\n",
        "({x:x} = {})\n",
        "x(1)\n"
    ));
    t.test_same(concat!(
        "var x = function(a) {};\n",
        "({x:x = function(a) {}} = {})\n",
        "x(1)\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteDestructured3
#[test]
fn test_no_rewrite_destructured3() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var x = function(a) {};\n",
        "[x = function() {}] = []\n",
        "x(1)\n"
    ));
    t.test_same(concat!(
        "class C { method() { return 1 }}\n",
        "var c = new C();\n",
        "var y = C.prototype;\n",
        "[y.method] = []\n",
        "c.method()\n"
    ));
    t.test_same(concat!(
        "class C { method() { return 1 }}\n",
        "[x.method] = []\n",
        "y.method()\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteTagged1
#[test]
fn test_no_rewrite_tagged1() {
    let mut t = Test::new();
    // Optimizing methods called though tagged template literal requires
    // specific knowledge of how tagged templated is supplied to the method.
    t.test_same(concat!("var f = function(a, b, c) {};\n", "f`tagged`\n"));
    t.test_same(concat!(
        "var f = function(a, b, c) {};\n",
        "f`tagged`\n",
        "f()\n"
    ));
}

// port: OptimizeParametersTest#testArrow
#[test]
fn test_arrow() {
    let mut t = Test::new();
    // Optimizing methods called though tagged template literal requires
    // specific knowledge of how tagged templated is supplied to the method.
    t.test_same(concat!("var f = (a)=>{};\n", "f`tagged`\n"));
    t.test_same(concat!(
        "var f = function(a, b, c) {};\n",
        "f`tagged`\n",
        "f()\n"
    ));
}

// port: OptimizeParametersTest#testNoRewriteTagged_methodCall
#[test]
fn test_no_rewrite_tagged_method_call() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var obj = {\n",
        "  f: function(a, b, c) {}\n",
        "};\n",
        "obj.f`tagged`;\n"
    ));
}

// port: OptimizeParametersTest#testSuperInvocation_preventsParamInlining_whenImplicit
#[test]
fn test_super_invocation_prevents_param_inlining_when_implicit() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  constructor(x) {\n",
        "    this.x = x;\n",
        "  }\n",
        "}\n",
        "\n",
        "// lack of explicit constructor prevents optimizing calls to both classes.\n",
        "class Bar extends Foo { }\n",
        "\n",
        "new Foo(4);\n",
        "new Bar(4);\n"
    ));
}

// port: OptimizeParametersTest#testSuperInvocationCanBeInlined
#[test]
fn test_super_invocation_can_be_inlined() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  constructor(x) {\n",
            "    this.x = x;\n",
            "  }\n",
            "}\n",
            "\n",
            "class Bar extends Foo {\n",
            "  constructor() {\n",
            "    super(4);\n",
            "  }\n",
            "}\n",
            "\n",
            "new Foo(4);\n",
            "new Bar();\n"
        ),
        concat!(
            "class Foo {\n",
            "  constructor( ) {\n",
            "    var x = 4; // moved from the parameter list\n",
            "    this.x = x;\n",
            "  }\n",
            "}\n",
            "\n",
            "class Bar extends Foo {\n",
            "  constructor() {\n",
            "    super( );\n",
            "  }\n",
            "}\n",
            "\n",
            "new Foo( );\n",
            "new Bar();\n"
        ),
    );
}

// port: OptimizeParametersTest#testSuperInvocationCannotBeInlinedWhenExtendingExpression
#[test]
fn test_super_invocation_cannot_be_inlined_when_extending_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Foo {\n",
        "  constructor(x) {\n",
        "    this.x = x;\n",
        "  }\n",
        "}\n",
        "\n",
        "// This bit of indirection prevents us from recognizing what\n",
        "// is being extended, so `new Foo()` cannot be optimized.\n",
        "class Bar extends (() => Foo)() {\n",
        "  constructor() {\n",
        "    super(4);\n",
        "  }\n",
        "}\n",
        "\n",
        "new Foo(4);\n",
        "new Bar();\n"
    ));
}

// port: OptimizeParametersTest#testRemoveOptionalDestructuringParam
#[test]
fn test_remove_optional_destructuring_param() {
    let mut t = Test::new();
    t.test(
        "function f({x}) {} f();",
        "function f() { var x; ({x} = void 0); } f();",
    );
    t.test(
        "function f({x} = {}) {} f();",
        "function f() { var x; ({x} = {}); } f();",
    );
}

// port: OptimizeParametersTest#testClassField
#[test]
fn test_class_field() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C {\n",
        "  x = function(a,b) { return a + b };\n",
        "}\n",
        "new C().x(1,2)\n"
    ));
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals
#[test]
fn test_trailing_undefined_literals() {
    let mut t = Test::new();
    t.test(
        "function foo(a) { use(a);}; foo(undefined); foo(2);",
        "function foo(a) { use(a);}; foo(         ); foo(2);",
    );
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals_multiple
#[test]
fn test_trailing_undefined_literals_multiple() {
    let mut t = Test::new();
    t.test(
        concat!(
            "function foo(a, b, c) { use(a, b, c); }\n",
            "foo(undefined);\n",
            "foo(undefined, void 0);\n",
            "foo(undefined, void 0, undefined);\n",
            "foo(2);\n"
        ),
        concat!(
            "function foo(a, b, c) { use(a, b, c); }\n",
            "foo();\n",
            "foo();\n",
            "foo();\n",
            "foo(2);\n"
        ),
    );
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals_functionRefsArguments
#[test]
fn test_trailing_undefined_literals_function_refs_arguments() {
    let mut t = Test::new();
    t.test_same("function foo(a) { use(arguments);}; foo(undefined); foo(2);");
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals_afterASpread
#[test]
fn test_trailing_undefined_literals_after_a_spread() {
    let mut t = Test::new();
    t.test_same("function foo(a,b) { use(a)}; foo(...[1], undefined, undefined);");
    t.test_same("function foo(a,b) { use(a)}; foo(undefined, ...[1], undefined); foo(2);");
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals_afterAllFormalParameters
#[test]
fn test_trailing_undefined_literals_after_all_formal_parameters() {
    let mut t = Test::new();
    t.test(
        "function foo(a, b) { use(a)}; foo('used', undefined, undefined, 2, 'a'); foo(2);",
        "function foo(a, b) { use(a)}; foo('used');                               foo(2);",
    );
}

// port: OptimizeParametersTest#testTrailingUndefinedLiterals_afterAllFormalParameters_sideEffects
#[test]
fn test_trailing_undefined_literals_after_all_formal_parameters_side_effects() {
    let mut t = Test::new();
    t.test_same("function foo(a, b) { use(a)}; foo('used', undefined, sideEffects()); foo(2);");
}

// port: OptimizeParametersTest#testInliningSideEffectfulArg_updatesInvocationSideEffects
#[test]
fn test_inlining_side_effectful_arg_updates_invocation_side_effects() {
    let mut t = Test::new();
    t.harness.enable_compute_side_effects().unwrap();
    t.test_parts(vec![
        externs("function sideEffects() {}"),
        srcs("function foo() {} foo(sideEffects()); foo(sideEffects());"),
        expected("function foo() { sideEffects(); } foo(); foo();"),
    ]);

    // Inspect the AST to verify both calls to `foo()` are marked as mutating global state
    let compiler = t.harness.get_last_compiler().unwrap();
    let compiler = compiler.borrow();
    let ast = &compiler.ast;
    let js_root = compiler.get_js_root().unwrap();
    let calls = find_nodes_non_empty(ast, js_root, |n, ast| {
        n.is_call(ast) && n.get_first_child(ast).unwrap().matches_name(ast, "foo")
    });
    for call in calls {
        assert_eq!(
            call.get_side_effect_flags(ast),
            SideEffectFlags::with_value(SideEffectFlags::NO_SIDE_EFFECTS)
                .set_mutates_global_state()
                .value_of()
        );
    }
}

// port: OptimizeParametersTest#testNoHoistNewTarget
#[test]
fn test_no_hoist_new_target() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function check(x) { if (x) throw 1; }\n",
        "class C { constructor() { check(!new.target); } }\n",
        "new C();\n"
    ));
}

// port: OptimizeParametersTest#testNoHoistImportMeta
#[test]
fn test_no_hoist_import_meta() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function check(x) { if (x) throw 1; }\n",
        "function f() { check(import.meta.url); }\n",
        "f();\n"
    ));
    t.test_same(concat!(
        "function check(x) { if (x) throw 1; }\n",
        "function f() { check(import.meta); }\n",
        "f();\n"
    ));
}
