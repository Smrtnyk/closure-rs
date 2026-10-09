/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/OptimizeConstructorsTest.java.

//! Port of OptimizeConstructorsTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::optimize_constructors::OptimizeConstructors;
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
    // port: OptimizeConstructorsTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(OptimizeConstructors::new(&compiler.borrow())))
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: OptimizeConstructorsTest#OptimizeConstructorsTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(default_externs_plus("var alert;var use;"));
        harness.set_up();
        harness.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        harness.enable_normalize_expected_output().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("OptimizeConstructorsTest"),
            },
        }
    }
}

// port: OptimizeConstructorsTest#testSimple
#[test]
fn test_simple() {
    let mut t = Test::new();
    // As simple a test case as I can come up with...
    t.test(
        concat!(
            "class S { constructor() {} }\n",
            "class C extends S { constructor() { super(); } }\n",
            "let c = new C();\n"
        ),
        concat!(
            "class S { }\n",
            "class C extends S {}\n",
            "let c = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testSimpleClassExpression
#[test]
fn test_simple_class_expression() {
    let mut t = Test::new();
    // As simple a test case as I can come up with...
    t.test(
        concat!(
            "const S = class { constructor() {} }\n",
            "let C = class extends S { constructor() { super(); } };\n",
            "let instance = new C();\n"
        ),
        concat!(
            "const S = class { };\n",
            "let C = class extends S {};\n",
            "let instance = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testClassExpressionWithInComma
#[test]
fn test_class_expression_with_in_comma() {
    let mut t = Test::new();
    // The use of the class being modified doesn't matter so
    // definitions in complex expressions is ok.
    t.test(
        concat!(
            "class S { constructor() {} }\n",
            "let C = use ?? class extends S { constructor() { super(); } };\n",
            "let c = new C();\n"
        ),
        concat!(
            "class S { }\n",
            "let C = use ?? class extends S { };\n",
            "let c = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testClassExpressionDefinedWithComma
#[test]
fn test_class_expression_defined_with_comma() {
    let mut t = Test::new();
    // As simple a test case as I can come up with...
    t.test(
        concat!(
            "const S = (0, class { constructor() {} });\n",
            "let C = class extends S { constructor() { super(); } };\n",
            "let instance = new C();\n"
        ),
        concat!(
            "const S = (0, class { });\n",
            "let C = class extends S {};\n",
            "let instance = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testES5SuperClass
#[test]
fn test_es5_super_class() {
    let mut t = Test::new();
    // NOTE: we can remove subclasses of well defined ES5 classes
    t.test(
        concat!(
            "/** @constructor */ let S = function() {};\n",
            "class C extends S { constructor() { super(); } }\n",
            "let c = new C();\n"
        ),
        concat!(
            "/** @constructor */ let S = function() {};\n",
            "class C extends S { }\n",
            "let c = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testES5SubClass
#[test]
fn test_es5_sub_class() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class S { constructor() {} }\n",
            "class C extends S { constructor() { super(); } }\n",
            "function E() { return Reflect.construct(C); }\n",
            "let c = new C();\n"
        ),
        concat!(
            "class S { }\n",
            "class C extends S { }\n",
            "function E() { return Reflect.construct(C); }\n",
            "let c = new C();\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testParameterMismatch1
#[test]
fn test_parameter_mismatch1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class S { constructor(a=undefined) {use(a);} }\n",
        "class C extends S { constructor() { super(); } }\n",
        "let c = new C(1);\n"
    ));
}

// port: OptimizeConstructorsTest#testParameterMismatch2
#[test]
fn test_parameter_mismatch2() {
    let mut t = Test::new();
    // NOTE: super class asks for "rest" so we don't remove subclass constructors
    // that don't provide all of them.  We can do better here though because
    // we know the constructor is removed here because it isn't doing anything interesting.
    t.test(
        concat!(
            "class S { constructor(...rest) {} }\n",
            "class C extends S { constructor() { super(); } }\n",
            "let c = new C(1);\n"
        ),
        concat!(
            "class S { }\n",
            "class C extends S { constructor() { super(); } }\n",
            "let c = new C(1);\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testParameterMismatch3
#[test]
fn test_parameter_mismatch3() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class S { constructor() { use(arguments); } }\n",
        "class C extends S { constructor() { super(); } }\n",
        "let c = new C(1);\n"
    ));
}

// port: OptimizeConstructorsTest#testOptimize_emptyConstructor
#[test]
fn test_optimize_empty_constructor() {
    let mut t = Test::new();
    t.test("class A {  constructor() {} }", "class A {}");
    t.test("class A {  constructor(a,b,c) {} }", "class A {}");
}

// port: OptimizeConstructorsTest#testOptimize_noArgs
#[test]
fn test_optimize_no_args() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Super {\n",
            "  constructor() {\n",
            "    this.a = 1;\n",
            "  }\n",
            "}\n",
            "class A extends Super {\n",
            "  constructor() {\n",
            "    super();\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "class Super {\n",
            "  constructor() {\n",
            "    this.a = 1;\n",
            "  }\n",
            "}\n",
            "class A extends Super {}\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testOptimize_matchingArgs
#[test]
fn test_optimize_matching_args() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class B {\n",
            "  constructor(a,b,c) {\n",
            "    this.a=a;\n",
            "    this.b=b;\n",
            "    this.c=c;\n",
            "  }\n",
            "}\n",
            "class A extends B {\n",
            "  constructor(a,b,c) {\n",
            "    super(a,b,c);\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "class B{\n",
            "  constructor(a,b,c) {\n",
            "    this.a=a;\n",
            "    this.b=b;\n",
            "    this.c=c;\n",
            "  }\n",
            "}\n",
            "class A extends B {}\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testOptimize_explicitObjectSuper
#[test]
fn test_optimize_explicit_object_super() {
    let mut t = Test::new();
    // NOTE: It would be valid to remove the constructor in these cases.
    t.test_same("class A extends Object { constructor(a) { super(); } }");
    t.test_same("class A extends Object { constructor(a) { super(a); } }");
    t.test_same("class A extends Object { constructor() { super(A); }}");
}

// port: OptimizeConstructorsTest#testOptimize_es5super
#[test]
fn test_optimize_es5super() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */ function B(a,b,c) {\n",
            "  this.a=a;\n",
            "  this.b=b;\n",
            "  this.c=c;\n",
            "}\n",
            "class A extends B {\n",
            "  constructor(a,b,c) {\n",
            "    super(a,b,c);\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "/** @constructor */ function B(a,b,c) {\n",
            "  this.a=a;\n",
            "  this.b=b;\n",
            "  this.c=c;\n",
            "}\n",
            "class A extends B {}\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testOptimize_externSuper
#[test]
fn test_optimize_extern_super() {
    let mut t = Test::new();
    // NOTE: to optimize this we need to assume the external definition is accurately describes
    // the behavior.
    t.test_same_parts(vec![
        externs_files(&[source_file_from_code(
            "externs",
            "/** @constructor */ function Error(x, y, z) {}",
        )]),
        srcs(concat!(
            "class A extends Error {\n",
            "  constructor(a, b, c) {\n",
            "    super(a, b, c);\n",
            "  }\n",
            "}\n"
        )),
    ]);
}

// port: OptimizeConstructorsTest#testOptimize_varArgs
#[test]
fn test_optimize_var_args() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Super {\n",
            "  constructor(...a) {\n",
            "    this.a=a;\n",
            "  }\n",
            "}\n",
            "class A extends Super {\n",
            "  constructor(...a) {\n",
            "    super(...a);\n",
            "  }\n",
            "}\n"
        ),
        concat!(
            "class Super {\n",
            "  constructor(...a) {\n",
            "    this.a=a;\n",
            "  }\n",
            "}\n",
            "class A extends Super {}\n"
        ),
    );
}

// port: OptimizeConstructorsTest#testOptimize_syntheticConstructor
#[test]
fn test_optimize_synthetic_constructor() {
    let mut t = Test::new();
    // NOTE: This test demonstrates the current behavior but
    // the subclass constructor here is safely removable.
    t.test_same(concat!(
        "class Super {\n",
        "  constructor(a, b, c) {\n",
        "    this.a=a;\n",
        "  }\n",
        "}\n",
        "class A extends Super {\n",
        "  constructor() {\n",
        "    super(...arguments);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testOptimize_implicitConstructor
#[test]
fn test_optimize_implicit_constructor() {
    let mut t = Test::new();
    // NOTE: extend the handling to allow for
    // intermediate class with implicit constructors
    t.test_same(concat!(
        "class Top {\n",
        "  constructor(a) {\n",
        "    this.a=1;\n",
        "  }\n",
        "}\n",
        "class Super extends Top {}\n",
        "class A extends Super {\n",
        "  constructor(b) {\n",
        "    super(b);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_wrongParameterOrder
#[test]
fn test_no_optimize_wrong_parameter_order() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class B {\n",
        "  constructor(a,b,c) { this.a = a; }\n",
        "}\n",
        "class A extends B {\n",
        "  constructor(a,b,c) {\n",
        "    super(b,a,c);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_notEveryParameterUsed
#[test]
fn test_no_optimize_not_every_parameter_used() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class B {\n",
        "  constructor(a,b) {\n",
        "    this.a=a;\n",
        "    this.b=b;\n",
        "  }\n",
        "}\n",
        "class A extends B {\n",
        "  constructor(a,b,c) {\n",
        "    super(a,b);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_nonEmptyConstructor
#[test]
fn test_no_optimize_non_empty_constructor() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class A {\n",
        "  constructor() {\n",
        "    A.$clinit();\n",
        "  }\n",
        "  static $clinit() {}\n",
        "}\n",
        "class C {\n",
        "  constructor(a, b, c) {\n",
        "    this.a = a;\n",
        "  }\n",
        "}\n",
        "class B extends C {\n",
        "  constructor(a, b, c) {\n",
        "    B.$clinit();\n",
        "    super(a, b, c);\n",
        "  }\n",
        "  static $clinit() {}\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_superNotMatching_es6
#[test]
fn test_no_optimize_super_not_matching_es6() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class B {\n",
        "  constructor(opt_a,opt_b) {\n",
        "    this.a=opt_a;\n",
        "    this.b=opt_b;\n",
        "  }\n",
        "}\n",
        "class A extends B { constructor() { super(); } }\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_superNotMatching_es5
#[test]
fn test_no_optimize_super_not_matching_es5() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */ function B(opt_a,opt_b) {\n",
        "  this.a=opt_a;\n",
        "  this.b=opt_b;\n",
        "}\n",
        "class A extends B { constructor() { super(); } }\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_superNotMatching_extern
#[test]
fn test_no_optimize_super_not_matching_extern() {
    let mut t = Test::new();
    t.test_same_parts(vec![
        externs_files(&[source_file_from_code(
            "externs",
            "/** @constructor */ function Error(opt_a, opt_b, opt_c) {}",
        )]),
        srcs("class A extends Error { constructor() { super(); } }"),
    ]);
}

// port: OptimizeConstructorsTest#testNoOptimize_superAcceptsRest
#[test]
fn test_no_optimize_super_accepts_rest() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Super {\n",
        "  constructor(...a) {\n",
        "    this.a=a;\n",
        "  }\n",
        "}\n",
        "class A extends Super {\n",
        "  constructor(a) {\n",
        "    super(a);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_restNotSread
#[test]
fn test_no_optimize_rest_not_sread() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Super {\n",
        "  constructor(...a) {\n",
        "    this.a=a;\n",
        "  }\n",
        "}\n",
        "class A extends Super {\n",
        "  constructor(...a) {\n",
        "    super(a);\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_unknownParent
#[test]
fn test_no_optimize_unknown_parent() {
    let mut t = Test::new();
    t.test_same(concat!(
        "var Super;\n",
        "class A extends Super {\n",
        "  constructor() {\n",
        "    super();\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_extendsExpression
#[test]
fn test_no_optimize_extends_expression() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function f() { return undefined; }\n",
        "class A extends f() {\n",
        "  constructor() {\n",
        "    super();\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_implicitConstructor
#[test]
fn test_no_optimize_implicit_constructor() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Top {\n",
        "  constructor(opt_a) {\n",
        "    this.a=1;\n",
        "  }\n",
        "}\n",
        "class Super extends Top {}\n",
        "class A extends Super {\n",
        "  constructor() {\n",
        "    super();\n",
        "  }\n",
        "}\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_sideEffectObjectSuperCall
#[test]
fn test_no_optimize_side_effect_object_super_call() {
    let mut t = Test::new();
    t.test_same("class Top extends Object { constructor(a) { super(a++); }}");
}

// port: OptimizeConstructorsTest#testNoOptimize_reassignedSuperclass
#[test]
fn test_no_optimize_reassigned_superclass() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Super {\n",
        "  constructor() {}\n",
        "}\n",
        "class A extends Super {\n",
        "  constructor() {\n",
        "    super();\n",
        "  }\n",
        "}\n",
        "Super = class {};\n",
        "let a = new A();\n"
    ));
}

// port: OptimizeConstructorsTest#testNoOptimize_reassignedConstructor
#[test]
fn test_no_optimize_reassigned_constructor() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class A {\n",
        "  constructor() {}\n",
        "}\n",
        "A = class {};\n",
        "let a = new A();\n"
    ));
}
