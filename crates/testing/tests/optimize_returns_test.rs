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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/OptimizeReturnsTest.java.

//! Port of OptimizeReturnsTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::optimize_returns::OptimizeReturns;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
use optimize_calls_support::{Fixture, default_externs_plus, native_ctx, pass};

// port: OptimizeReturnsTest#EXTERNAL_SYMBOLS
const EXTERNAL_SYMBOLS: &str = "var extern;\nextern.externalMethod\n";

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: OptimizeReturnsTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(OptimizeReturns::new()))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: OptimizeReturnsTest#OptimizeReturnsTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(default_externs_plus(EXTERNAL_SYMBOLS));
        harness.set_up();
        harness.enable_normalize().unwrap(); // Required for `OptimizeCalls`.
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("OptimizeReturnsTest"),
            },
        }
    }
}

// port: OptimizeReturnsTest#nullishCoalesceReturnRemoved
#[test]
fn nullish_coalesce_return_removed() {
    let mut t = Test::new();
    t.test(
        "var f = (function() {return 1}) ?? (function() {return 2}); f();",
        "var f = function() { return; } ?? function() { return; }; f();",
    );
}

// port: OptimizeReturnsTest#testNoRewriteUsedResult1
#[test]
fn test_no_rewrite_used_result1() {
    let mut t = Test::new();
    let source = concat!("function a(){return 1}\n", "var x = a()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteUsedResult2
#[test]
fn test_no_rewrite_used_result2() {
    let mut t = Test::new();
    let source = concat!("var a = function(){return 1}\n", "a(); var b = a()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteUsedClassMethodResult1
#[test]
fn test_no_rewrite_used_class_method_result1() {
    let mut t = Test::new();
    let source = concat!(
        "class C { method() {return 1} }\n",
        "var x = new C().method()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteUnsedObjectMethodResult1
#[test]
fn test_no_rewrite_unsed_object_method_result1() {
    let mut t = Test::new();
    let source = concat!("var o = { method() {return 1} }\n", "o.method()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredArray1
#[test]
fn test_no_rewrite_destructured_array1() {
    let mut t = Test::new();
    let source = concat!("var x = function() { return 1; };\n", "[x] = []\n", "x()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredArray2
#[test]
fn test_no_rewrite_destructured_array2() {
    let mut t = Test::new();
    let source = concat!(
        "var x = function() { return 1; };\n",
        "[x = function() {}] = []\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredArray3
#[test]
fn test_no_rewrite_destructured_array3() {
    let mut t = Test::new();
    let source = concat!(
        "class C { method() { return 1 }}\n",
        "[x.method] = []\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredArray4
#[test]
fn test_no_rewrite_destructured_array4() {
    let mut t = Test::new();
    let source = concat!(
        "class C { method() { return 1 }}\n",
        "[x.method] = []\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredObject1
#[test]
fn test_no_rewrite_destructured_object1() {
    let mut t = Test::new();
    let source = concat!(
        "var x = function() { return 1; };\n",
        "({a:x} = {})\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredObject2
#[test]
fn test_no_rewrite_destructured_object2() {
    let mut t = Test::new();
    let source = concat!(
        "var x = function() { return 1; };\n",
        "({a:x = function() {}} = {})\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredObject3
#[test]
fn test_no_rewrite_destructured_object3() {
    let mut t = Test::new();
    let source = concat!(
        "class C { method() { return 1 }}\n",
        "({a:x.method} = {})\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteDestructuredObject4
#[test]
fn test_no_rewrite_destructured_object4() {
    let mut t = Test::new();
    let source = concat!(
        "class C { method() { return 1 }}\n",
        "({a:x.method = function() {}} = {})\n",
        "x()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteTagged1
#[test]
fn test_no_rewrite_tagged1() {
    let mut t = Test::new();
    // TODO(johnlenz): support this. Unused return can be removed.
    let source = concat!("var f = function() { return 1; };\n", "f`tagged`\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteTagged2
#[test]
fn test_no_rewrite_tagged2() {
    let mut t = Test::new();
    // Tagged use prevents optimizations
    let source = concat!("var f = function() { return 1; };\n", "var x = f`tagged`\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteTagged3
#[test]
fn test_no_rewrite_tagged3() {
    let mut t = Test::new();
    // Tagged use is not ignored.
    let source = concat!(
        "var f = function() { return 1; };\n",
        "var x = f`tagged`\n",
        "f()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteConstructorProp
#[test]
fn test_no_rewrite_constructor_prop() {
    let mut t = Test::new();
    let source = concat!(
        "class C { constructor() { return 1 } }\n",
        "x.constructor()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult1
#[test]
fn test_rewrite_unused_result1() {
    let mut t = Test::new();
    let source = concat!("function a(){return 1}\n", "a()\n");
    let expected = concat!("function a(){return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult2
#[test]
fn test_rewrite_unused_result2() {
    let mut t = Test::new();
    let source = concat!("var a; a = function(){return 1}\n", "a()\n");
    let expected = concat!("var a; a = function(){return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult3
#[test]
fn test_rewrite_unused_result3() {
    let mut t = Test::new();
    let source = concat!("var a = function(){return 1}\n", "a()\n");
    let expected = concat!("var a = function(){return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult4a
#[test]
fn test_rewrite_unused_result4a() {
    let mut t = Test::new();
    let source = concat!("var a = function(){return a()}\n", "a()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult4b
#[test]
fn test_rewrite_unused_result4b() {
    let mut t = Test::new();
    let source = concat!("var a = function b(){return b()}\n", "a()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteWhenAliasedDuringAssignment
#[test]
fn test_no_rewrite_when_aliased_during_assignment() {
    let mut t = Test::new();
    let source = concat!(
        "var a, b;\n",
        "a = b = function (){return 1}\n",
        "use(a()); // result used\n",
        "b()\n"
    );
    // result unused
    t.test_same(source);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult4c
#[test]
fn test_rewrite_unused_result4c() {
    let mut t = Test::new();
    let source = concat!("function a(){return a()}\n", "a()\n");
    t.test_same(source);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult5
#[test]
fn test_rewrite_unused_result5() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {return args};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    let expected = concat!(
        "function a(){}\n",
        "a.prototype.foo = function(args) {args;return};\n",
        "var o = new a;\n",
        "o.foo()\n"
    );
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult6
#[test]
fn test_rewrite_unused_result6() {
    let mut t = Test::new();
    let source = concat!("function a(){return (g = 1)}\n", "a()\n");
    let expected = concat!("function a(){g = 1;return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult7a
#[test]
fn test_rewrite_unused_result7a() {
    let mut t = Test::new();
    let source = concat!(
        "function a() { return 1 }\n",
        "function b() { return a() }\n",
        "function c() { return b() }\n",
        "c();\n"
    );
    let expected = concat!(
        "function a() { return 1 }\n",
        "function b() { return a() }\n",
        "function c() { b(); return }\n",
        "c();\n"
    );
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult7b
#[test]
fn test_rewrite_unused_result7b() {
    let mut t = Test::new();
    let source = concat!(
        "c();\n",
        "function c() { return b() }\n",
        "function b() { return a() }\n",
        "function a() { return 1 }\n"
    );
    // Iteration 1.
    let expected = concat!(
        "c();\n",
        "function c() { b(); return }\n",
        "function b() { return a() }\n",
        "function a() { return 1 }\n"
    );
    t.test(source, expected);
    // Iteration 2.
    let source = expected;
    let expected = concat!(
        "c();\n",
        "function c() { b(); return }\n",
        "function b() { a(); return }\n",
        "function a() { return 1 }\n"
    );
    t.test(source, expected);
    // Iteration 3.
    let source = expected;
    let expected = concat!(
        "c();\n",
        "function c() { b(); return }\n",
        "function b() { a(); return }\n",
        "function a() { return }\n"
    );
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult8
#[test]
fn test_rewrite_unused_result8() {
    let mut t = Test::new();
    let source = concat!(
        "function a() { return c() }\n",
        "function b() { return a() }\n",
        "function c() { return b() }\n",
        "c();\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testRewriteUnusedResult9
#[test]
fn test_rewrite_unused_result9() {
    let mut t = Test::new();
    // Proves that the deleted function scope is reported.
    let source = concat!("function a(){return function() {};}\n", "a()\n");
    let expected = concat!("function a(){(function() {}); return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUsedResult10
#[test]
fn test_rewrite_used_result10() {
    let mut t = Test::new();
    let source = concat!("class C { method() {return 1} }\n", "new C().method()\n");
    let expected = concat!("class C { method() {return} }\n", "new C().method()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedTemplateLitResult
#[test]
fn test_rewrite_unused_template_lit_result() {
    let mut t = Test::new();
    // Proves that the deleted function scope is reported.
    let source = concat!("function a(){ return `template`; }\n", "a()\n");
    let expected = concat!("function a(){ return; }\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedAsyncResult1
#[test]
fn test_rewrite_unused_async_result1() {
    let mut t = Test::new();
    // Async function returns can be dropped if no-one waits on the returned
    // promise.
    let source = concat!("async function a(){return promise}\n", "a()\n");
    let expected = concat!("async function a(){promise; return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedGeneratorResult1
#[test]
fn test_rewrite_unused_generator_result1() {
    let mut t = Test::new();
    // Generator function returns can be dropped if no-one uses the returned
    // iterator.
    let source = concat!("function *a(){return value}\n", "a()\n");
    let expected = concat!("function *a(){value; return}\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testNoRewriteObjLit1
#[test]
fn test_no_rewrite_obj_lit1() {
    let mut t = Test::new();
    let source = concat!(
        "var a = {b:function(){return 1;}}\n",
        "for(c in a) (a[c])();\n",
        "a.b()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testNoRewriteObjLit2
#[test]
fn test_no_rewrite_obj_lit2() {
    let mut t = Test::new();
    let source = concat!(
        "var a = {b:function fn(){return 1;}}\n",
        "for(c in a) (a[c])();\n",
        "a.b()\n"
    );
    t.test_same(source);
    let source_opt_chain = concat!(
        "var a = {b:function fn(){return 1;}}\n",
        "for(c in a) (a[c])?.();\n",
        "a.b?.()\n"
    );
    t.test_same(source_opt_chain);
}

// port: OptimizeReturnsTest#testNoRewriteArrLit
#[test]
fn test_no_rewrite_arr_lit() {
    let mut t = Test::new();
    let source = concat!("var a = [function(){return 1;}]\n", "(a[0])();\n");
    t.test_same(source);
    let source_opt_chain = concat!("var a = [function(){return 1;}]\n", "(a[0])?.();\n");
    t.test_same(source_opt_chain);
}

// port: OptimizeReturnsTest#testPrototypeMethod1
#[test]
fn test_prototype_method1() {
    let mut t = Test::new();
    let source = concat!(
        "function c(){}\n",
        "c.prototype.a = function(){return 1}\n",
        "var x = new c;\n",
        "x.a()\n"
    );
    let result = concat!(
        "function c(){}\n",
        "c.prototype.a = function(){return}\n",
        "var x = new c;\n",
        "x.a()\n"
    );
    t.test(source, result);
}

// port: OptimizeReturnsTest#testPrototypeMethod2
#[test]
fn test_prototype_method2() {
    let mut t = Test::new();
    let source = concat!(
        "function c(){}\n",
        "c.prototype.a = function(){return 1}\n",
        "goog.reflect.object({a: 'v'})\n",
        "var x = new c;\n",
        "x.a()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testPrototypeMethod3
#[test]
fn test_prototype_method3() {
    let mut t = Test::new();
    let source = concat!(
        "function c(){}\n",
        "c.prototype.a = function(){return 1}\n",
        "var x = new c;\n",
        "for(var key in goog.reflect.object({a: 'v'})){ x[key](); }\n",
        "x.a()\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testPrototypeMethod4
#[test]
fn test_prototype_method4() {
    let mut t = Test::new();
    let source = concat!(
        "function c(){}\n",
        "c.prototype.a = function(){return 1}\n",
        "var x = new c;\n",
        "for(var key in goog.reflect.object({a: 'v'})){ x[key](); }\n"
    );
    t.test_same(source);
}

// port: OptimizeReturnsTest#testCallOrApply
#[test]
fn test_call_or_apply() {
    let mut t = Test::new();
    // TODO(johnlenz): Add support for .apply
    t.test(
        "function a() {return 1}; a.call(new foo);",
        "function a() {return  }; a.call(new foo);",
    );
    t.test_same("function a() {return 1}; a.apply(new foo);");
}

// port: OptimizeReturnsTest#testCallOrApply_optChain
#[test]
fn test_call_or_apply_opt_chain() {
    let mut t = Test::new();
    t.test(
        "function a() {return 1}; a?.call(new foo);",
        "function a() {return  }; a?.call(new foo);",
    );
    t.test_same("function a() {return 1}; a?.apply(new foo);");
}

// port: OptimizeReturnsTest#testRewriteUseSiteRemoval
#[test]
fn test_rewrite_use_site_removal() {
    let mut t = Test::new();
    let source = concat!("function a() { return {\"_id\" : 1} }\n", "a();\n");
    let expected = concat!("function a() { ({\"_id\" : 1}); return }\n", "a();\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testUnknownDefinitionAllowRemoval
#[test]
fn test_unknown_definition_allow_removal() {
    let mut t = Test::new();
    // TODO(johnlenz): allow this to be optimized.
    t.test_same(concat!(
        "let x = functionFactory();\n",
        "x(1, 2);\n",
        "x = function(a,b) { return b; }\n"
    ));
}

// port: OptimizeReturnsTest#testReturnNotRemovedFromRecursiveNamedFunctionExpression
#[test]
fn test_return_not_removed_from_recursive_named_function_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "let x = function innerName(n) {\n",
        "  if (n < 1) {\n",
        "    return 0\n",
        "  } else {\n",
        "    return innerName(n - 1) + n;\n",
        "  }\n",
        "\n",
        "}\n",
        "x(3);\n"
    ));
}

// port: OptimizeReturnsTest#testReturnNotRemovedFromRecursiveNamedFunctionExpression_optChain
#[test]
fn test_return_not_removed_from_recursive_named_function_expression_opt_chain() {
    let mut t = Test::new();
    t.test_same(concat!(
        "let x = function innerName(n) {\n",
        "  if (n < 1) {\n",
        "    return 0\n",
        "  } else {\n",
        "    return innerName(n - 1) + n;\n",
        "  }\n",
        "\n",
        "}\n",
        "x?.(3);\n"
    ));
}

// port: OptimizeReturnsTest#testRewriteUnusedResultWithSafeReference1
#[test]
fn test_rewrite_unused_result_with_safe_reference1() {
    let mut t = Test::new();
    let source = concat!("function a(){return 1}\n", "typeof a\n", "a()\n");
    let expected = concat!("function a(){return}\n", "typeof a\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResultWithSafeReference2
#[test]
fn test_rewrite_unused_result_with_safe_reference2() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){return 1}\n",
        "x instanceof a\n",
        "a instanceof x\n",
        "a()\n"
    );
    let expected = concat!(
        "function a(){return}\n",
        "x instanceof a\n",
        "a instanceof x\n",
        "a()\n"
    );
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResultWithSafeReference3
#[test]
fn test_rewrite_unused_result_with_safe_reference3() {
    let mut t = Test::new();
    let source = concat!("function a(){return 1}\n", "x in a\n", "a in x\n", "a()\n");
    let expected = concat!("function a(){return}\n", "x in a\n", "a in x\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResultWithSafeReference4
#[test]
fn test_rewrite_unused_result_with_safe_reference4() {
    let mut t = Test::new();
    let source = concat!("function a(){return 1}\n", "a.x\n", "a['x']\n", "a()\n");
    let expected = concat!("function a(){return}\n", "a.x\n", "a['x']\n", "a()\n");
    t.test(source, expected);
}

// port: OptimizeReturnsTest#testRewriteUnusedResultWithSafeReference5
#[test]
fn test_rewrite_unused_result_with_safe_reference5() {
    let mut t = Test::new();
    let source = concat!(
        "function a(){return 1}\n",
        "for (x in a) {}\n",
        "for (x of a) {}\n",
        "a()\n"
    );
    let expected = concat!(
        "function a(){return}\n",
        "for (x in a) {}\n",
        "for (x of a) {}\n",
        "a()\n"
    );
    t.test(source, expected);
    // optional versions
    let source_opt_chain_call = concat!(
        "function a(){return 1}\n",
        "for (x in a) {}\n",
        "for (x of a) {}\n",
        "a?.()\n"
    );
    let expected_opt_chain_call = concat!(
        "function a(){return}\n",
        "for (x in a) {}\n",
        "for (x of a) {}\n",
        "a?.()\n"
    );
    t.test(source_opt_chain_call, expected_opt_chain_call);
}

// port: OptimizeReturnsTest#testNoRewriteUnusedResultWithUnsafeReference1
#[test]
fn test_no_rewrite_unused_result_with_unsafe_reference1() {
    let mut t = Test::new();
    // call to 'a.x' escapes 'a' as 'this'
    let source = concat!("function a(){return 1}\n", "a.x()\n", "a()\n");
    t.test_same(source);
    // call to 'a?.x' escapes 'a' as 'this'
    let source_opt_chain_call = concat!("function a(){return 1}\n", "a?.x()\n", "a()\n");
    t.test_same(source_opt_chain_call);
}

// port: OptimizeReturnsTest#testNoRewriteUnusedResultWithUnsafeReference2
#[test]
fn test_no_rewrite_unused_result_with_unsafe_reference2() {
    let mut t = Test::new();
    // call to 'a[x]' escapes 'a' as 'this'
    let source = concat!("function a(){return 1}\n", "a['x']()\n", "a()\n");
    t.test_same(source);
    // call to 'a?.[x]' escapes 'a' as 'this'
    let source_opt_chain_call = concat!("function a(){return 1}\n", "a?.['x']()\n", "a()\n");
    t.test_same(source_opt_chain_call);
}

// port: OptimizeReturnsTest#testNoRewriteUnusedResultWithUnsafeReference4
#[test]
fn test_no_rewrite_unused_result_with_unsafe_reference4() {
    let mut t = Test::new();
    // call to 'a' is assigned an unknown value
    // TODO(johnlenz): optimize this.
    let source = concat!("function a(){return 1}\n", "for (a of x) {}\n", "a()\n");
    t.test_same(source);
}
