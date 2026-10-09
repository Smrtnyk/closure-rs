/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CrossChunkCodeMotionTest.java.

//! Port of CrossChunkCodeMotionTest (CompilerTestCase over the crates/testing port).
mod support;

use closure_jscomp::{
    cross_chunk_code_motion::CrossChunkCodeMotion, js_chunk::JSChunk, source_file::SourceFile,
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
use support::cross_chunk_fixture::{JSChunkGraphBuilder, check, expected, native_ctx, pass, srcs};

// port: CrossChunkCodeMotionTest#EXTERNS
const EXTERNS: &str = "alert";

struct Hooks {
    ctx: Ctx,
    parent_chunk_can_see_symbols_declared_in_children: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: CrossChunkCodeMotionTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(CrossChunkCodeMotion::new(
            self.parent_chunk_can_see_symbols_declared_in_children,
        )))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

struct Test {
    harness: CompilerTestCase,
    hooks: Hooks,
}

impl Test {
    // port: CrossChunkCodeMotionTest#CrossChunkCodeMotionTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(EXTERNS);
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("CrossChunkCodeMotionTest"),
                parent_chunk_can_see_symbols_declared_in_children: false,
            },
        }
    }
    // port: CompilerTestCase#test(TestPart...)
    fn test(&mut self, parts: Vec<TestPart>) {
        check(self.harness.test(&mut self.hooks, parts));
    }
    // port: CompilerTestCase#testSame(TestPart...)
    fn test_same(&mut self, parts: Vec<TestPart>) {
        check(self.harness.test_same(&mut self.hooks, parts));
    }
}

// port: CrossChunkCodeMotionTest#testFunctionMovement1
#[test]
fn test_function_movement1() {
    let mut t = Test::new();
    // This tests lots of things:
    // 1) f1 is declared in m1, and used in m2. Move it to m2
    // 2) f2 is declared in m1, and used in m3 twice. Move it to m3
    // 3) f3 is declared in m1, and used in m2+m3. It stays put
    // 4) g declared in m1 and never used. It stays put
    // 5) h declared in m2 and never used. It stays put
    // 6) f4 declared in m1 and used in m2 as var. It moves to m2
    let chunks = JSChunkGraphBuilder::for_star()
// m1
.add_chunk("function f1(a) { alert(a); }\nfunction f2(a) { alert(a); }\nfunction f3(a) { alert(a); }\nfunction f4() { alert(1); }\nfunction g() { alert('ciao'); }\n")
// m2
.add_chunk("f1('hi'); f3('bye'); var a = f4; function h(a) { alert('h:' + a); }")
// m3
.add_chunk("f2('hi'); f2('hi'); f3('bye');").build();
    t.test(vec![
srcs(chunks),
expected(&[
// m1
&"function f3(a) { alert(a); } function g() { alert('ciao'); }",
// m2
&"function f1(a) { alert(a); }\nfunction f4() { alert(1); }\nf1('hi'); f3('bye'); var a = f4;\nfunction h(a) { alert('h:' + a); }\n",
// m3
&"function f2(a) { alert(a); } f2('hi'); f2('hi'); f3('bye');",
]),
]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement2
#[test]
fn test_function_movement2() {
    let mut t = Test::new();
    // having f declared as a local variable should block the migration to m2
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("function f(a) { alert(a); } function g() {var f = 1; f++}")
        // m2
        .add_chunk("f(1);")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"function g() {var f = 1; f++}",
            // m2
            &"function f(a) { alert(a); } f(1);",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement3
#[test]
fn test_function_movement3() {
    let mut t = Test::new();
    // having f declared as a arg should block the migration to m2
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("function f(a) { alert(a); } function g(f) {f++}")
        // m2
        .add_chunk("f(1);")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"function g(f) {f++}",
            // m2
            &"function f(a) { alert(a); } f(1);",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement4
#[test]
fn test_function_movement4() {
    let mut t = Test::new();
    // Try out moving a function which returns a closure
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("function f(){return function(a){}}")
        // m2
        .add_chunk("var a = f();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function f(){return function(a){}} var a = f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement5
#[test]
fn test_function_movement5() {
    let mut t = Test::new();
    // Try moving a recursive function [using factorials for kicks]
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("function f(n){return (n<1)?1:f(n-1)}")
        // m2
        .add_chunk("var a = f(4);")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function f(n){return (n<1)?1:f(n-1)} var a = f(4);",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement5b
#[test]
fn test_function_movement5b() {
    let mut t = Test::new();
    // Try moving a recursive function declared differently.
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var f = function(n){return (n<1)?1:f(n-1)};")
        // m2
        .add_chunk("var a = f(4);")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = function(n){return (n<1)?1:f(n-1)}; var a = f(4);",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement5c
#[test]
fn test_function_movement5c() {
    let mut t = Test::new();
    // Try moving a recursive function declared differently, in a nested block scope.
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var f = function(n){if(true){if(true){return (n<1)?1:f(n-1)}}};")
        // m2
        .add_chunk("var a = f(4);")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = function(n){if(true){if(true){return (n<1)?1:f(n-1)}}};\nvar a = f(4);\n",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement6
#[test]
fn test_function_movement6() {
    let mut t = Test::new();
    // Try out moving to the common ancestor
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("function f(){return 1}")
        // m2
        .add_chunk("var a = f();")
        // m3
        .add_chunk("var b = f();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function f(){return 1} var a = f();",
            // m3
            &"var b = f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement7
#[test]
fn test_function_movement7() {
    let mut t = Test::new();
    // Try out moving to the common ancestor with deeper ancestry chain
    let chunks = JSChunkGraphBuilder::for_unordered()
        // m1
        .add_chunk("function f(){return 1}")
        // m2
        .add_chunk("")
        // m3
        .add_chunk("var a = f();")
        // m4
        .add_chunk("var b = f();")
        // m5
        .add_chunk("var c = f();")
        .build();
    chunks[1].add_dependency(&chunks[0]);
    chunks[2].add_dependency(&chunks[1]);
    chunks[3].add_dependency(&chunks[1]);
    chunks[4].add_dependency(&chunks[1]);
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function f(){return 1}",
            // m3
            &"var a = f();",
            // m4
            &"var b = f();",
            // m5
            &"var c = f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionMovement8
#[test]
fn test_function_movement8() {
    let mut t = Test::new();
    // Check what happens with named functions
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("var v = function f(){return 1}")
        // m2
        .add_chunk("v();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var v = function f(){return 1}; v();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testFunctionNonMovement1
#[test]
fn test_function_non_movement1() {
    let mut t = Test::new();
    // This tests lots of things:
    // 1) we can't move it if it is a class with non-const attributes accessed
    // 2) if it's in an if statement, we can't move it
    // 3) if it's in an while statement, we can't move it [with some extra
    // block elements]
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_star().add_chunk("function f(){};f.prototype.bar=new f;\nif(a)function f2(){}\n{{while(a)function f3(){}}}\n").add_chunk("var a = new f();f2();f3();").build()),
]);
}

// port: CrossChunkCodeMotionTest#testFunctionNonMovement2
#[test]
fn test_function_non_movement2() {
    let mut t = Test::new();
    // A generic case where 2 chunks depend on the first one. But it's the
    // common ancestor, so we can't move.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f(){return 1}")
            .add_chunk("var a = f();")
            .add_chunk("var b = f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement1
#[test]
fn test_es6_class_movement1() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("class f { bar() {} }")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[&"", &"class f { bar() {} } var a = new f();"]),
    ]);
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("var f = class { bar() {} };")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[&"", &"var f = class { bar() {} }; var a = new f();"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement1
#[test]
fn test_class_movement1() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("function f(){} f.prototype.bar=function (){};")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[
            &"",
            &"function f(){} f.prototype.bar=function (){}; var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement_instanceof
#[test]
fn test_es6_class_movement_instanceof() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("class f { bar(){} } 1 instanceof f;")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[
            &"'function' == typeof f && 1 instanceof f;",
            &"class f { bar(){} } var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof
#[test]
fn test_class_movement_instanceof() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("function f(){} f.prototype.bar=function (){}; 1 instanceof f;")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[
            &"'function' == typeof f && 1 instanceof f;",
            &"function f(){} f.prototype.bar=function (){}; var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement_instanceofTurnedOff
#[test]
fn test_es6_class_movement_instanceof_turned_off() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = false;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("class f { bar(){} } 1 instanceof f;")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceofTurnedOff
#[test]
fn test_class_movement_instanceof_turned_off() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = false;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f(){} f.prototype.bar=function (){}; 1 instanceof f;")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement_instanceof2
#[test]
fn test_es6_class_movement_instanceof2() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk("class f { bar(){} } (true && 1 instanceof f);")
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[
            &"(true && ('function' == typeof f && 1 instanceof f));",
            &"class f { bar(){} } var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof2
#[test]
fn test_class_movement_instanceof2() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_star()
                .add_chunk(
                    "function f(){} f.prototype.bar=function (){}; (true && 1 instanceof f);",
                )
                .add_chunk("var a = new f();")
                .build(),
        ),
        expected(&[
            &"(true && ('function' == typeof f && 1 instanceof f));",
            &"function f(){} f.prototype.bar=function (){}; var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_alreadyGuardedInstanceof
#[test]
fn test_class_movement_already_guarded_instanceof() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
srcs(JSChunkGraphBuilder::for_star().add_chunk("function f(){} f.prototype.bar=function (){};\n(true && ('undefined' != typeof f && 1 instanceof f));\n").add_chunk("var a = new f();").build()),
expected(&[
&"(true && ('undefined' != typeof f && 1 instanceof f));",
&"function f(){} f.prototype.bar=function (){}; var a = new f();",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_alreadyGuardedInstanceof_functionGuard
#[test]
fn test_class_movement_already_guarded_instanceof_function_guard() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test(vec![
srcs(JSChunkGraphBuilder::for_star().add_chunk("function f(){} f.prototype.bar=function (){};\n(true && ('function' == typeof f && 1 instanceof f));\n").add_chunk("var a = new f();").build()),
expected(&[
&"(true && ('function' == typeof f && 1 instanceof f));",
&"function f(){} f.prototype.bar=function (){}; var a = new f();",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof3
#[test]
fn test_class_movement_instanceof3() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f(){} f.prototype.bar=function (){}; f instanceof 1")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof_sideEffectsLhs
#[test]
fn test_class_movement_instanceof_side_effects_lhs() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk(
                String::new()
                    + "class SafeHtml {} var out = raw; (out = escapeHtml(raw)) instanceof"
                    + " SafeHtml;",
            )
            .add_chunk("new SafeHtml();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof_noRewriteRequired
#[test]
fn test_class_movement_instanceof_no_rewrite_required() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f(){} f.prototype.bar=function (){}; 1 instanceof f; new f;")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_instanceof_noRewriteRequired2
#[test]
fn test_class_movement_instanceof_no_rewrite_required2() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function f(){} f.prototype.bar=function (){}; new f;")
            .add_chunk("1 instanceof f;")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement2
#[test]
fn test_es6_class_movement2() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("class f {} f.prototype.bar=3; f.prototype.baz=5;")
// m2
.add_chunk("f.prototype.baq = 7;")
// m3
.add_chunk("f.prototype.baz = 9;")
// m4
.add_chunk("var a = new f();").build()),
expected(&[
// m1
&"",
// m2
&"",
// m3
&"",
// m4
&"class f {}\nf.prototype.bar = 3;\nf.prototype.baz = 5;\nf.prototype.baq = 7;\nf.prototype.baz = 9;\nvar a = new f();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClassMovement2
#[test]
fn test_class_movement2() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("function f(){} f.prototype.bar=3; f.prototype.baz=5;")
// m2
.add_chunk("f.prototype.baq = 7;")
// m3
.add_chunk("f.prototype.baz = 9;")
// m4
.add_chunk("var a = new f();").build()),
expected(&[
// m1
&"",
// m2
&"",
// m3
&"",
// m4
&"function f(){}\nf.prototype.bar=3;\nf.prototype.baz=5;\nf.prototype.baq = 7;\nf.prototype.baz = 9;\nvar a = new f();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClassMovement3
#[test]
fn test_class_movement3() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("var f = function() {}; f.prototype.bar=3; f.prototype.baz=5;")
// m2
.add_chunk("f = 7;")
// m3
.add_chunk("f = 9;")
// m4
.add_chunk("f = 11;").build()),
expected(&[
// m1
&"",
// m2
&"",
// m3
&"",
// m4
&"var f = function() {};\nf.prototype.bar=3;\nf.prototype.baz=5;\nf = 7;\nf = 9;\nf = 11;\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClassMovement4
#[test]
fn test_class_movement4() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f(){} f.prototype.bar=3; f.prototype.baz=5;")
            .add_chunk("f.prototype.baq = 7;")
            .add_chunk("var a = new f();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement5
#[test]
fn test_class_movement5() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_unordered()
        // m1
        .add_chunk("function f(){} f.prototype.bar=3; f.prototype.baz=5;")
        // m2
        .add_chunk("")
        // m3
        .add_chunk("f.prototype.baq = 7;")
        // m4
        .add_chunk("var a = new f();")
        .build();
    chunks[1].add_dependency(&chunks[0]);
    chunks[2].add_dependency(&chunks[1]);
    chunks[3].add_dependency(&chunks[1]);
    // m4 +
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function f(){} f.prototype.bar=3; f.prototype.baz=5;",
            // m3
            &"f.prototype.baq = 7;",
            // m4 +
            &"var a = new f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement6
#[test]
fn test_class_movement6() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("function Foo(){} function Bar(){} goog.inherits(Bar, Foo); new Foo();")
                // m2
                .add_chunk("new Bar();")
                .build(),
        ),
        expected(&[
            // m1
            &"function Foo(){} new Foo();",
            // m2
            &"function Bar(){} goog.inherits(Bar, Foo); new Bar();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement6
#[test]
fn test_es6_class_movement6() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("class Foo{} class Bar extends Foo {} new Foo();")
                // m2
                .add_chunk("new Bar();")
                .build(),
        ),
        expected(&[
            // m1
            &"class Foo {} new Foo();",
            // m2
            &"class Bar extends Foo {} new Bar();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement7
#[test]
fn test_class_movement7() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            // m1
            .add_chunk("function Foo(){} function Bar(){} goog.inherits(Bar, Foo); new Bar();")
            // m2
            .add_chunk("new Foo();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement7
#[test]
fn test_es6_class_movement7() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            // m1
            .add_chunk("class Foo {} class Bar extends Foo {} new Bar();")
            // m2
            .add_chunk("new Foo();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassMovement8
#[test]
fn test_class_movement8() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("function Foo(){}\nObject.defineProperties(Foo.prototype, {a: {get:function(){return 0;}}});\n")
// m2
.add_chunk("new Foo();").build()),
expected(&[
&"",
// m1
&"function Foo(){}\nObject.defineProperties(Foo.prototype, {a: {get:function(){return 0;}}});\nnew Foo();\n",
// m2
]),
]);
}

// port: CrossChunkCodeMotionTest#testEs6ClassMovement8
#[test]
fn test_es6_class_movement8() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("class Foo { get test() { return 0; }}")
                .add_chunk("new Foo();")
                .build(),
        ),
        expected(&[
            &"",
            // m1
            &"class Foo { get test() { return 0; }} new Foo();",
            // m2
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_classFields
#[test]
fn test_class_movement_class_fields() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("class Foo { a; static b = 2; ['c'] = 3; static 'd' = 'hi'; 1 = 2;}")
                .add_chunk("new Foo();")
                .build(),
        ),
        expected(&[
            &"",
            &"class Foo { a; static b = 2; ['c'] = 3; static 'd' = 'hi'; 1 = 2; } new Foo();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_classStaticBlock1
#[test]
fn test_class_movement_class_static_block1() {
    let mut t = Test::new();
    // TODO(bradfordcsmith):Ideally the class would move
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("class Foo { static { } }")
                .add_chunk("new Foo();")
                .build(),
        ),
        expected(&[&"class Foo { static { } }", &"new Foo();"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_classStaticBlock2
#[test]
fn test_class_movement_class_static_block2() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("var x = 1;")
                .add_chunk("class Foo { static { x; } } new Foo();")
                .build(),
        ),
        expected(&[&"", &"var x = 1; class Foo { static { x; } } new Foo();"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_classStaticBlock3
#[test]
fn test_class_movement_class_static_block3() {
    let mut t = Test::new();
    // TODO(bradfordcsmith):Ideally the class and var would move to m3
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("const x = 1; var y = 2;")
        // m2
        .add_chunk("class Foo { static { y = 3; } }")
        // m3
        .add_chunk("new Foo();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"const x = 1;",
            // m2
            &"var y =2; class Foo { static { y = 3; } } ",
            // m3
            &"new Foo();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_classStaticBlock4
#[test]
fn test_class_movement_class_static_block4() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("var x =1;")
        // m2
        .add_chunk("class Foo {\n  static {\n    x = 2;\n  }\n}\nuse(x);\n")
        // m3
        .add_chunk("new Foo();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var x =1;\nclass Foo {\n  static {\n    x = 2\n  }\n}\nuse(x);\n",
            // m3
            &"new Foo();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClassMovement_mixins
#[test]
fn test_class_movement_mixins() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class ValueType {}\nclass Foo {}\nValueType.mixin(Foo, ValueType, 5, goog.reflect.objectProperty('foo', Foo))\n").add_chunk("new Foo();").build()),
expected(&[
&"",
// m1
&"class ValueType {}\nclass Foo {}\nValueType.mixin(Foo, ValueType, 5, goog.reflect.objectProperty('foo', Foo))\nnew Foo();\n",
// m2
]),
]);
}

// port: CrossChunkCodeMotionTest#testPureOrBreakMyCodedStaticClassFieldIsMovable
#[test]
fn test_pure_or_break_my_coded_static_class_field_is_movable() {
    let mut t = Test::new();
    //
    //
    //
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function wrapIt(val) { return [val]; }\nclass ImmovableClass { static x = wrapIt(123); }\nclass MovableClass { static y = /** @pureOrBreakMyCode */ wrapIt(123); }\n").add_chunk("new ImmovableClass(); new MovableClass();").build()),
expected(&[
&"function wrapIt(val) { return [val] }\nclass ImmovableClass { static x = wrapIt(123);}\n",
&"class MovableClass { static y = /** @pureOrBreakMyCode */ wrapIt(123); }\nnew ImmovableClass(); new MovableClass();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testStubMethodMovement
#[test]
fn test_stub_method_movement() {
    let mut t = Test::new();
    // The method stub can move, but the unstub definition cannot, because
    // CrossChunkCodeMotion doesn't know where individual methods are used.
    // CrossChunkMethodMotion is responsible for putting the unstub definitions
    // in the right places.
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m0
.add_chunk("function Foo(){} Foo.prototype.bar = JSCompiler_stubMethod(x);")
// m1
.add_chunk("Foo.prototype.bar = JSCompiler_unstubMethod(x);")
// m2
.add_chunk("new Foo();").build()),
expected(&[
// m0
&"",
// m1
&"function Foo(){} Foo.prototype.bar = JSCompiler_stubMethod(x);\nFoo.prototype.bar = JSCompiler_unstubMethod(x);\n",
// m2
&"new Foo();",
]),
]);
}

// port: CrossChunkCodeMotionTest#testNoMoveSideEffectProperty
#[test]
fn test_no_move_side_effect_property() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            // m1
            .add_chunk("function Foo(){}  Foo.prototype.bar = createSomething();")
            // m2
            .add_chunk("new Foo();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testNoMoveSideEffectDefineProperties
#[test]
fn test_no_move_side_effect_define_properties() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("function Foo(){}\nObject.defineProperties(Foo.prototype, {a: {get: createSomething()}})\n")
// m2
.add_chunk("new Foo();").build()),
]);
}

// port: CrossChunkCodeMotionTest#testNoMoveSideEffectDefinePropertiesComputed
#[test]
fn test_no_move_side_effect_define_properties_computed() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo(){}\nObject.defineProperties(Foo.prototype,{[test()]:{get: function() {return 10;}}})\n").add_chunk("new Foo();").build()),
]);
}

// port: CrossChunkCodeMotionTest#testAssignMovement
#[test]
fn test_assign_movement() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var f = 3; f = 5;")
                // m2
                .add_chunk("var h = f;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = 3; f = 5; var h = f;",
        ]),
    ]);
    // don't move nested assigns
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("var f = 3; var g = f = 5;")
            .add_chunk("var h = f;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testNoClassMovement2
#[test]
fn test_no_class_movement2() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var f = {}; f.h = 5;")
                // m2
                .add_chunk("var h = f;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = {}; f.h = 5; var h = f;",
        ]),
    ]);
    // don't move nested getprop assigns
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("var f = {}; var g = f.h = 5;")
            .add_chunk("var h = f;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testLiteralMovement1
#[test]
fn test_literal_movement1() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var f = {'hi': 'mom', 'bye': function() {}};")
                // m2
                .add_chunk("var h = f;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = {'hi': 'mom', 'bye': function() {}}; var h = f;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLiteralMovement2
#[test]
fn test_literal_movement2() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("var f = {'hi': 'mom', 'bye': shared};")
            .add_chunk("var h = f;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testLiteralMovement3
#[test]
fn test_literal_movement3() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var f = ['hi', function() {}];")
                // m2
                .add_chunk("var h = f;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var f = ['hi', function() {}]; var h = f;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLiteralMovement4
#[test]
fn test_literal_movement4() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("var f = ['hi', shared];")
            .add_chunk("var h = f;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testStringTemplateLiteralMovement1
#[test]
fn test_string_template_literal_movement1() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var s = 'world'; var f = `hi ${s}`;")
                // m2
                .add_chunk("var h = f;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var s = 'world'; var f = `hi ${s}`; var h = f;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testStringTemplateLiteralMovement2
#[test]
fn test_string_template_literal_movement2() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("var f = `hi ${shared()}`;")
            .add_chunk("var h = f;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testVarMovement1
#[test]
fn test_var_movement1() {
    let mut t = Test::new();
    // test moving a variable
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var a = 0;")
        // m2
        .add_chunk("var x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var a = 0; var x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement
#[test]
fn test_let_const_movement() {
    let mut t = Test::new();
    // test moving a variable
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("const a = 0;")
        // m2
        .add_chunk("let x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"const a = 0; let x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement2
#[test]
fn test_var_movement2() {
    let mut t = Test::new();
    // Test moving 1 variable out of the block
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var a = 0; var b = 1; var c = 2;")
        // m2
        .add_chunk("var x = b;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"var a = 0; var c = 2;",
            // m2
            &"var b = 1; var x = b;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement2
#[test]
fn test_let_const_movement2() {
    let mut t = Test::new();
    // Test moving 1 variable out of the block
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("const a = 0; const b = 1; const c = 2;")
        // m2
        .add_chunk("let x = b;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"const a = 0; const c = 2;",
            // m2
            &"const b = 1; let x = b;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement3
#[test]
fn test_var_movement3() {
    let mut t = Test::new();
    // Test moving all variables out of the block
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var a = 0; var b = 1;")
        // m2
        .add_chunk("var x = a + b;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var a = 0; var b = 1; var x = a + b;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement3
#[test]
fn test_let_const_movement3() {
    let mut t = Test::new();
    // Test moving all variables out of the block
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("const a = 0; const b = 1;")
        // m2
        .add_chunk("let x = a + b;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"const a = 0; const b = 1; let x = a + b;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement4
#[test]
fn test_var_movement4() {
    let mut t = Test::new();
    // Test moving a function
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var a = function(){alert(1)};")
        // m2
        .add_chunk("var x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var a = function(){alert(1)}; var x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement4
#[test]
fn test_let_const_movement4() {
    let mut t = Test::new();
    // Test moving a function
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("const a = function(){alert(1)};")
        // m2
        .add_chunk("let x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"const a = function(){alert(1)}; let x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement5
#[test]
fn test_var_movement5() {
    let mut t = Test::new();
    // Don't move a function outside of scope
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("var a = alert;")
            .add_chunk("var x = a;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement5
#[test]
fn test_let_const_movement5() {
    let mut t = Test::new();
    // Don't move a function outside of scope
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("const a = alert;")
            .add_chunk("let x = a;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testVarMovement6
#[test]
fn test_var_movement6() {
    let mut t = Test::new();
    // Test moving a var with no assigned value
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("var a;")
        // m2
        .add_chunk("var x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var a; var x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetMovement6
#[test]
fn test_let_movement6() {
    let mut t = Test::new();
    // Test moving a let with no assigned value
    let chunks = JSChunkGraphBuilder::for_star()
        // m1
        .add_chunk("let a;")
        // m2
        .add_chunk("let x = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"let a; let x = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement7
#[test]
fn test_var_movement7() {
    let mut t = Test::new();
    // Don't move a variable higher in the dependency tree
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function f() {g();} f();")
            .add_chunk("function g(){};")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testVarMovement8
#[test]
fn test_var_movement8() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_bush()
        // m1
        .add_chunk("var a = 0;")
        // m2 -> m1
        .add_chunk("")
        // m3 -> m2
        .add_chunk("var x = a;")
        // m4 -> m2
        .add_chunk("var y = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var a = 0;",
            // m3
            &"var x = a;",
            // m4
            &"var y = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testLetConstMovement8
#[test]
fn test_let_const_movement8() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_bush()
        // m1
        .add_chunk("const a = 0;")
        // m2 -> m1
        .add_chunk("")
        // m3 -> m2
        .add_chunk("let x = a;")
        // m4 -> m2
        .add_chunk("let y = a;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"const a = 0;",
            // m3
            &"let x = a;",
            // m4
            &"let y = a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testVarMovement9
#[test]
fn test_var_movement9() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_tree()
        // m1
        .add_chunk("var a = 0; var b = 1; var c = 3;")
        // m2 -> m1
        .add_chunk("")
        // m3 -> m1
        .add_chunk("")
        // m4 -> m2
        .add_chunk("a;")
        // m5 -> m2
        .add_chunk("a;c;")
        // m6 -> m3
        .add_chunk("b;")
        // m7 -> m4
        .add_chunk("b;c;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"var c = 3;",
            // m2
            &"var a = 0;",
            // m3
            &"var b = 1;",
            // m4
            &"a;",
            // m5
            &"a;c;",
            // m6
            &"b;",
            // m7
            &"b;c;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testConstMovement9
#[test]
fn test_const_movement9() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_tree()
        // m1
        .add_chunk("const a = 0; const b = 1; const c = 3;")
        // m2 -> m1
        .add_chunk("")
        // m3 -> m1
        .add_chunk("")
        // m4 -> m2
        .add_chunk("a;")
        // m5 -> m2
        .add_chunk("a;c;")
        // m6 -> m3
        .add_chunk("b;")
        // m7 -> m4
        .add_chunk("b;c;")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"const c = 3;",
            // m2
            &"const a = 0;",
            // m3
            &"const b = 1;",
            // m4
            &"a;",
            // m5
            &"a;c;",
            // m6
            &"b;",
            // m7
            &"b;c;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClinit1
#[test]
fn test_clinit1() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("function Foo$clinit() { Foo$clinit = function() {}; }")
        // m2
        .add_chunk("Foo$clinit();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"function Foo$clinit() { Foo$clinit = function() {}; } Foo$clinit();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClinit2
#[test]
fn test_clinit2() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("var Foo$clinit = function() { Foo$clinit = function() {}; };")
        // m2
        .add_chunk("Foo$clinit();")
        .build();
    t.test(vec![
        srcs(chunks),
        expected(&[
            // m1
            &"",
            // m2
            &"var Foo$clinit = function() { Foo$clinit = function() {}; }; Foo$clinit();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testClone1
#[test]
fn test_clone1() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("function f(){} f.prototype.clone = function() { return new f };")
// m2
.add_chunk("var a = (new f).clone();").build()),
expected(&[
// m1
&"",
// m2
&"function f(){}\nf.prototype.clone = function() { return new f() };\nvar a = (new f).clone();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testClone2
#[test]
fn test_clone2() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function f(){}\nf.prototype.cloneFun = function() {\n  return function() {new f}\n};\n").add_chunk("var a = (new f).cloneFun();").build()),
expected(&[
// m1
&"",
&"function f(){}\nf.prototype.cloneFun = function() {\n  return function() {new f}\n};\nvar a = (new f).cloneFun();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testBug4118005
#[test]
fn test_bug4118005() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk(
                "var m = 1;\n(function () {\n var x = 1;\n m = function() { return x };\n})();\n",
            )
            .add_chunk("m();")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testEmptyChunk
#[test]
fn test_empty_chunk() {
    let mut t = Test::new();
    // When the dest chunk is empty, it might try to move the code to the
    // one of the chunks that the empty chunk depends on. In some cases
    // this might ended up to be the same chunk as the definition of the code.
    // When that happens, CrossChunkCodeMotion might report a code change
    // while nothing is moved. This should not be a problem if we know all
    // chunks are non-empty.
    let m1 = JSChunk::new("m1");
    m1.add_source_file(SourceFile::from_code("m1", "function x() {}"));
    let empty = JSChunk::new("empty");
    empty.add_dependency(&m1);
    let m2 = JSChunk::new("m2");
    m2.add_source_file(SourceFile::from_code("m2", "x()"));
    m2.add_dependency(&empty);
    let m3 = JSChunk::new("m3");
    m3.add_source_file(SourceFile::from_code("m3", "x()"));
    m3.add_dependency(&empty);
    t.test(vec![
        srcs(vec![m1.clone(), empty.clone(), m2.clone(), m3.clone()]),
        expected(&[&"", &"function x() {}", &"x()", &"x()"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testAbstractMethod
#[test]
fn test_abstract_method() {
    let mut t = Test::new();
    // m2 -> m1
    // m3 -> m1
    t.test(vec![
srcs(JSChunkGraphBuilder::for_star()
// m1
.add_chunk("var abstractMethod = function () {};\nfunction F(){} F.prototype.bar=abstractMethod;\nfunction G(){} G.prototype.bar=abstractMethod;\n")
// m2 -> m1
.add_chunk("var f = new F();")
// m3 -> m1
.add_chunk("var g = new G();").build()),
expected(&[
&"var abstractMethod = function () {};",
&"function F(){} F.prototype.bar=abstractMethod; var f = new F();",
&"function G(){} G.prototype.bar=abstractMethod; var g = new G();",
]),
]);
}

// port: CrossChunkCodeMotionTest#testMovableUseBeforeDeclaration
#[test]
fn test_movable_use_before_declaration() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m0
                .add_chunk("function f() { g(); } function g() {}")
                // m1
                .add_chunk("f();")
                .build(),
        ),
        expected(&[
            // m0
            &"",
            // m1
            &"function g() {} function f() { g(); } f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testImmovableUseBeforeDeclaration
#[test]
fn test_immovable_use_before_declaration() {
    let mut t = Test::new();
    // must recognize this as a reference to the following declaration
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("g(); // must recognize this as a reference to the following declaration\nfunction g() {}\n").add_chunk("g();").build()),
]);
    // f() cannot move, so neither can g()
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m1
.add_chunk("function f() { g(); }\nfunction g() {}\n// f() cannot move, so neither can g()\nf();\n")
// m2
.add_chunk("g();").build()),
]);
}

// port: CrossChunkCodeMotionTest#testSplitDeclaration
#[test]
fn test_split_declaration() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m0
.add_chunk("function a() { b(); }\nfunction b() {}\nfunction c() {}\na.prototype.x = function() { c(); };\n")
// m1
.add_chunk("a();").build()),
expected(&[
// m0
&"",
// m1
&"function b() {}\nfunction c() {}\nfunction a() { b(); }\na.prototype.x = function() { c(); };\na();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testOutOfOrderAfterSplitDeclaration
#[test]
fn test_out_of_order_after_split_declaration() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m0
.add_chunk("function a() { c(); }\nfunction b() {}\na.prototype.x = function() { c(); };\nfunction c() {}\n")
// m1
.add_chunk("a();").build()),
expected(&[
// m0
&"function b() {}",
// m1
&"function c() {}\nfunction a() { c(); }\na.prototype.x = function() { c(); };\na();\n",
]),
]);
}

// port: CrossChunkCodeMotionTest#testOutOfOrderWithInterveningReferrer
#[test]
fn test_out_of_order_with_intervening_referrer() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m0
                .add_chunk("function a() { c(); } function b() { a(); } function c() {}")
                // m1
                .add_chunk("b();")
                .build(),
        ),
        expected(&[
            // m0
            &"",
            // m1
            &"function c() {} function a() { c(); } function b() { a(); } b();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testOutOfOrderWithDifferentReferrers
#[test]
fn test_out_of_order_with_different_referrers() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m0
                .add_chunk("function a() { b(); } function b() {}")
                // m1
                .add_chunk("b();")
                // m2
                .add_chunk("a();")
                .build(),
        ),
        expected(&[
            // m0
            &"",
            // m1
            &"function b() { } b();",
            &"function a() { b(); } a();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testCircularWithDifferentReferrers
#[test]
fn test_circular_with_different_referrers() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m0
                .add_chunk("function a() { b(); } function b() { a(); }")
                // m1
                .add_chunk("b();")
                // m2
                .add_chunk("a();")
                .build(),
        ),
        expected(&[
            // m0
            &"",
            // m1
            &"function a() { b(); } function b() { a(); } b();",
            &"a();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testSmallestCoveringDependencyDoesNotDependOnDeclarationChunk
#[test]
fn test_smallest_covering_dependency_does_not_depend_on_declaration_chunk() {
    let mut t = Test::new();
    //       m0
    //      /  \
    //    m1   m2  // declaration in m1
    //    |    /|  // smallest common dep is m2
    //    m3_ | |  // best place for declaration is m3
    //   / | X  |
    //  /  |/ \ /
    // m4  m5  m6  // references in m5 and m6
    let m = JSChunkGraphBuilder::for_unordered()
        .add_chunk("")
        // m0
        .add_chunk("function f() {}")
        // m1
        .add_chunk("")
        // m2
        .add_chunk("")
        // m3
        .add_chunk("")
        // m4
        .add_chunk("f();")
        // m5
        .add_chunk("f();")
        // m6
        .build();
    m[1].add_dependency(&m[0]);
    m[2].add_dependency(&m[0]);
    m[3].add_dependency(&m[1]);
    m[4].add_dependency(&m[3]);
    m[5].add_dependency(&m[2]);
    m[5].add_dependency(&m[3]);
    m[6].add_dependency(&m[2]);
    m[6].add_dependency(&m[3]);
    t.test(vec![
        srcs(m),
        expected(&[
            // m0
            &"",
            // m1
            &"",
            // m2
            &"",
            // m3
            &"function f() {}",
            // m4
            &"",
            // m5
            &"f();",
            // m6
            &"f();",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testEarlyReferencesPinLateDeclarations
#[test]
fn test_early_references_pin_late_declarations() {
    let mut t = Test::new();
    // globalC.x == 2 - not safe to move declaration
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function C() {} C.prototype.x = 1; var globalC = new C();")
            // globalC.x == 2 - not safe to move declaration
            .add_chunk("C.prototype.x = 2; globalC.x;")
            .add_chunk("new C().x;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testMovedInstanceofIsHandledCorrectly
#[test]
fn test_moved_instanceof_is_handled_correctly() {
    let mut t = Test::new();
    t.hooks.parent_chunk_can_see_symbols_declared_in_children = true;
    // no need to guard instanceof
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m0
.add_chunk("function C() {}\nfunction X() {}\nX.prototype.a = function(x) { return x instanceof C; }\n")
// m1
.add_chunk("new C();")
// m2
.add_chunk("new X();").build()),
expected(&[
// m0
&"",
// m1
&"function C() {} new C();",
// m2
&"function X() {}\n// no need to guard instanceof\nX.prototype.a = function(x) { return x instanceof C; }\nnew X();\n",
]),
]);
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain()
// m0
.add_chunk("function C() {}\nfunction X() {}\nX.prototype.a = function(x) { return x instanceof C; }\n")
// m1
.add_chunk("new X();")
// m2
.add_chunk("new C();").build()),
expected(&[
// m0
&"",
// m1
&"function X() {}\nX.prototype.a = function(x) { return 'function' == typeof C && x instanceof C; }\nnew X();\n",
// m2
&"function C() {} new C();",
]),
]);
}

// port: CrossChunkCodeMotionTest#testValueNotWellDefined
#[test]
fn test_value_not_well_defined() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): This prevents us from guaranteeing that all moves are made in a single
    //     pass. Code movement in the first pass may cause some variables to become "well defined"
    //     that weren't before, unblocking movement of some statements.
    // B is blocked from moving because A is used before it is defined.
    // See ReferenceCollection#isWellDefined and CrossChunkReferenceCollector#canMoveValue
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m0
                .add_chunk("function f() { return A; } var A = 1; var B = A;")
                // m1
                .add_chunk("f(); function f2() { return B; }")
                .build(),
        ),
        expected(&[
            // m0
            &"var A = 1; var B = A;",
            // m1
            &"function f() { return A; } f(); function f2() { return B; }",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testDestructuringDeclarationsNotMovable
#[test]
fn test_destructuring_declarations_not_movable() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("const [a] = [];")
            .add_chunk("a;")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("const {a} = { a: 1 };")
            .add_chunk("a;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testDestructuringAssignmentsAreReferences
#[test]
fn test_destructuring_assignments_are_references() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = 1; [a] = [5];")
            .add_chunk("a;")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = 1; ({x: a} = {x: 5});")
            .add_chunk("a;")
            .build(),
    )]);
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = 1;")
                .add_chunk("[a] = [5];")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"let a = 1; [a] = [5];", &"a;"]),
    ]);
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = 1;")
                .add_chunk("({x: a} = {x: 5});")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"let a = 1; ({x: a} = {x: 5});", &"a;"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testDefaultParamValuesAreReferences
#[test]
fn test_default_param_values_are_references() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = 1; function f(x = a) {} f();")
            .add_chunk("a;")
            .build(),
    )]);
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = 1; function f(x = a) {}")
                .add_chunk("f();")
                .build(),
        ),
        expected(&[&"", &"let a = 1; function f(x = a) {} f();"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testSpreadCountsAsAReference
#[test]
fn test_spread_counts_as_a_reference() {
    let mut t = Test::new();
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = [];")
                .add_chunk("function f(...args) {} f(...a);")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"let a = []; function f(...args) {} f(...a);", &"a;"]),
    ]);
}

// port: CrossChunkCodeMotionTest#testObjectLiteralMethods
#[test]
fn test_object_literal_methods() {
    let mut t = Test::new();
    // Object literal methods, getters, and setters are movable and references within them are
    // handled correctly.
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("const a = 1;")
                .add_chunk("const o = { foo() {return a;}, get x() {}, set x(v) {a = v;} };")
                .add_chunk("o;")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            &"",
            &"",
            &"const a = 1; const o = { foo() {return a;}, get x() {}, set x(v) {a = v;} }; o;",
            &"a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testComputedProperties
#[test]
fn test_computed_properties() {
    let mut t = Test::new();
    // Computed properties are movable if the key expression is a literal.
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = { ['something']: 1};")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"let a = { ['something']: 1}; a;"]),
    ]);
    // Computed properties are movable if the key is a well defined variable
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("const x = 1; let a = { [x]: 1};")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"const x = 1; let a = { [x]: 1}; a;"]),
    ]);
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("const x = 1; let a = class { [x]() {} };")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"const x = 1; let a = class { [x]() {} }; a;"]),
    ]);
    // Computed properties are not movable if the key is an unknown variable
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = { [x]: 1};")
            .add_chunk("a;")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = class { [x]() {} };")
            .add_chunk("a;")
            .build(),
    )]);
    // Computed properties are not movable if the key is a
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = { [x]: 1};")
            .add_chunk("a;")
            .build(),
    )]);
    // references in computed properties are honored
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = 1;")
                .add_chunk("let b = { [a + 1]: 2 };")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[&"", &"let a = 1; let b = { [a + 1]: 2 };", &"a;"]),
    ]);
    //  Well defined variables are moveable
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("var a = function() {}; a.prototype.blah = function() {};")
                // m2
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"var a = function() {};\na.prototype.blah = function() {};\na;\n",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testComputedPropertiesUsingSymbol
#[test]
fn test_computed_properties_using_symbol() {
    let mut t = Test::new();
    // Computed properties are movable if the key is a `Symbol.<any_property_on_symbol>`
    // `Symbol` is a built-in JavaScript object that is guaranteed to be unique. Therefore, it
    // is movable. Any properties on `Symbol` are also movable (e.g `Symbol.iterator`).
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("let a = { [Symbol.iterator]: 1};")
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            &"", // everything has been moved out of this chunk
            &"let a = { [Symbol.iterator]: 1}; a;",
        ]),
    ]);

    // `Symbol` is declared in m1, and used in m2. It should be moved to m2 in expected output.
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("let a = class { [Symbol.anyPropertyOnSymbolIsMovable]() {} };")
                // m2
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"let a = class { [Symbol.anyPropertyOnSymbolIsMovable]() {} }; a;",
        ]),
    ]);

    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            // m1
            .add_chunk("let prop; let a = class { [prop]() {} };")
            // m2
            .add_chunk("a;")
            .build(),
    )]);

    // Now turn on transpilation and rerun the same above 3 unit tests.
    check(t.harness.enable_transpile());

    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("let a = { [Symbol.iterator]: 1};")
                // m2
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            // m1
            // Ideally, we should be moving this code out of m1, but this would require us to move
            // an `ASSIGN` node which we don't want to do.
            &"var $jscomp$compprop98447280$0 = {};\nvar a = ($jscomp$compprop98447280$0[Symbol.iterator] = 1, $jscomp$compprop98447280$0);\n",
            // m2
            &"a;",
        ]),
    ]);

    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("let a = class { [Symbol.anyPropertyOnSymbolIsMovable]() {} };")
                // m2
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            // m1
            &"",
            // m2
            &"/** @constructor */ var a = function() {};\na.prototype[Symbol.anyPropertyOnSymbolIsMovable] = function() {};\na;\n",
        ]),
    ]);

    // Non-Symbol computed properties are currently not movable.
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                // m1
                .add_chunk("let prop; let a = class { [prop]() {} };")
                // m2
                .add_chunk("a;")
                .build(),
        ),
        expected(&[
            // m1
            &"var prop;\nvar $jscomp$compField$98447280$0 = prop;\n/** @constructor */ var a = function() {};\na.prototype[$jscomp$compField$98447280$0] = function() {};\n",
            // m2
            &"a;",
        ]),
    ]);
}

// port: CrossChunkCodeMotionTest#testComputedPropertiesUsingSymbolUnmovable
#[test]
fn test_computed_properties_using_symbol_unmovable() {
    let mut t = Test::new();
    // Computed properties are not movable if the key is not explicitly called `Symbol.`
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("let a = { [Symbol123.iterator]: 1};")
            .add_chunk("a;")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            // m1
            .add_chunk("let a = class { [Symbol123.anyPropertyOnSymbolIsMovable]() {} };")
            // m2
            .add_chunk("a;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testClassComputedFieldUnmovable
#[test]
fn test_class_computed_field_unmovable() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class C { ['x'] = 1; }")
            .add_chunk("a;")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class C { static ['x'] = 1; }")
            .add_chunk("a;")
            .build(),
    )]);
}

// port: CrossChunkCodeMotionTest#testPureOrBreakMyCode
#[test]
fn test_pure_or_break_my_code() {
    let mut t = Test::new();
    // should move a function if its top level invocation was annotated with @pureOrBreakMyCode
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function wrapIt(val) { return [val]; }\nconst value = /** @pureOrBreakMyCode */ wrapIt(123);\n").add_chunk("const unwrappedValue = value[0];").build()),
expected(&[
&"",
&"function wrapIt(val) { return [val]; }\nconst value = /** @pureOrBreakMyCode */ wrapIt(123);\nconst unwrappedValue = value[0];\n",
]),
]);
    // should move a class if its static initializer containing a function call was annotated with
    // @pureOrBreakMyCode
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function wrapIt(val) { return [val]; }\nclass ImmovableClass {}\nImmovableClass.prop = wrapIt(123);\nclass MovableClass {}\nMovableClass.prop = /** @pureOrBreakMyCode */ wrapIt(123);\n").add_chunk("new ImmovableClass(); new MovableClass();").build()),
expected(&[
&"function wrapIt(val) { return [val] }\nclass ImmovableClass {}\nImmovableClass.prop = wrapIt(123);\n",
&"class MovableClass {}\nMovableClass.prop = /** @pureOrBreakMyCode */ wrapIt(123);\nnew ImmovableClass(); new MovableClass();\n",
]),
]);
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class LowerCasePipe {}\n/** @nocollapse */ LowerCasePipe.ɵpipe = /** @pureOrBreakMyCode*/ i0.ɵɵdefinePipe({ name: \"lowercase\", type: LowerCasePipe, pure: true });\n").add_chunk("new LowerCasePipe();").build()),
expected(&[
&"",
&"class LowerCasePipe {}\n/** @nocollapse */ LowerCasePipe.ɵpipe = /** @pureOrBreakMyCode*/ i0.ɵɵdefinePipe({ name: \"lowercase\", type: LowerCasePipe, pure: true });\nnew LowerCasePipe();\n",
]),
]);
}
