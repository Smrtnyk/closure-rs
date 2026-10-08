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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/CrossChunkMethodMotionTest.java.

//! Port of CrossChunkMethodMotionTest (CompilerTestCase over the crates/testing port).
mod support;

use closure_jscomp::{
    cross_chunk_method_motion::{CrossChunkIdGenerator, CrossChunkMethodMotion},
    id_generator::IdGenerator,
};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks, TestPart},
    jscomp_api::CompilerOptions,
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
use support::cross_chunk_fixture::{
    JSChunkGraphBuilder, check, expected, externs, native_ctx, pass, srcs,
};

// port: CrossChunkMethodMotionTest#STUB_DECLARATIONS
const STUB_DECLARATIONS: &str = CrossChunkMethodMotion::STUB_DECLARATIONS;

struct Hooks {
    ctx: Ctx,
    can_move_externs: bool,
    no_stubs: bool,
}

impl CompilerTestCaseHooks for Hooks {
    // port: CrossChunkMethodMotionTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        Ok(pass(CrossChunkMethodMotion::new(
            &mut compiler.borrow_mut(),
            CrossChunkIdGenerator::Owned(IdGenerator::default()),
            self.can_move_externs,
            self.no_stubs,
        )))
    }
    // port: CrossChunkMethodMotionTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<CompilerOptions, Throwable> {
        let mut options =
            harness.get_options_with_coding_convention(|| self.get_coding_convention())?;
        // pretty printing makes it much easier to read the failure messages.
        options.set_pretty_print(true);
        Ok(options)
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
    // port: CompilerTestCase#setUp + CrossChunkMethodMotionTest#customSetUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        let mut t = Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("CrossChunkMethodMotionTest"),
                can_move_externs: false,
                no_stubs: false,
            },
        };
        t.harness.enable_normalize().unwrap();
        t
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

// port: CrossChunkMethodMotionTest#moveMethodAssignedToPrototype
#[test]
fn move_method_assigned_to_prototype() {
    let mut t = Test::new();
    t.test_same(vec![
        // bar property is defined in externs, so it cannot be moved
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype.bar = function() {};\n")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
    ]);
    t.hooks.can_move_externs = true;
    t.test(vec![
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype.bar = function() {};\n")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
        expected(&[
            &(String::new()
                + STUB_DECLARATIONS
                + "function Foo() {}\nFoo.prototype.bar = JSCompiler_stubMethod(0);\n"),
            // Chunk 2
            &"Foo.prototype.bar = JSCompiler_unstubMethod(0, function() {});\n(new Foo).bar()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#moveMethodDefinedInPrototypeLiteralWithStubs
#[test]
fn move_method_defined_in_prototype_literal_with_stubs() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype = { method: function() {} };\n").add_chunk("(new Foo).method()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype = { method: JSCompiler_stubMethod(0) };\n"),
// Chunk 2
&"Foo.prototype.method =\n    JSCompiler_unstubMethod(0, function() {});\n(new Foo).method()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveMethodDefinedInPrototypeLiteralWithoutStubs
#[test]
fn move_method_defined_in_prototype_literal_without_stubs() {
    let mut t = Test::new();
    t.hooks.no_stubs = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype = { method: function() {} };\n")
                .add_chunk("(new Foo).method()")
                .build(),
        ),
        expected(&[
            &"function Foo() {}\nFoo.prototype = {};\n",
            // Chunk 2
            &"Foo.prototype.method = function() {};\n(new Foo).method()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#moveMethodDefinedInPrototypeLiteralUsingShorthandSyntaxWithStub
#[test]
fn move_method_defined_in_prototype_literal_using_shorthand_syntax_with_stub() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype = { method() {} };\n").add_chunk("(new Foo).method()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype = { method: JSCompiler_stubMethod(0) };\n"),
// Chunk 2
&"Foo.prototype.method =\n    JSCompiler_unstubMethod(0, function() {});\n(new Foo).method()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveMethodDefinedInPrototypeLiteralUsingShorthandSyntaxWithoutStub
#[test]
fn move_method_defined_in_prototype_literal_using_shorthand_syntax_without_stub() {
    let mut t = Test::new();
    t.hooks.no_stubs = true;
    t.test(vec![
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype = { method() {} };\n")
                .add_chunk("(new Foo).method()")
                .build(),
        ),
        expected(&[
            &"function Foo() {}\nFoo.prototype = {};\n",
            // Chunk 2
            &"Foo.prototype.method = function() {};\n(new Foo).method()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#doNotMoveMethodDefinedInPrototypeLiteralContainingSuper
#[test]
fn do_not_move_method_defined_in_prototype_literal_containing_super() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk(
                "function Foo() {}\nFoo.prototype = { method() { return super.toString(); } };\n",
            )
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveMethodDefinedInPrototypeLiteralAsComputedProp
#[test]
fn do_not_move_method_defined_in_prototype_literal_as_computed_prop() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function Foo() {}\nFoo.prototype = { [1]:  {} };\n")
            .add_chunk("(new Foo)[1]()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#moveClassMethod
#[test]
fn move_class_method() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { method() {} }").add_chunk("(new Foo).method()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.method = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo).method();\n",
]),
]);
    // Same as above, but reference to the method is via an optional chain
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { method() {} }").add_chunk("(new Foo)?.method()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.method = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo)?.method();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodContainingSuper
#[test]
fn do_not_move_class_method_containing_super() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar { method() {} }\nclass Foo extends Bar { method2() { super.method(); } }\n").add_chunk("(new Foo).method2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Bar {}\nBar.prototype.method = JSCompiler_stubMethod(0);\nclass Foo extends Bar { method2() { super.method(); } }\n\n"),
&"Bar.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo).method2();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodContainingSuperInAnArrow
#[test]
fn do_not_move_class_method_containing_super_in_an_arrow() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar { method() {} }\nclass Foo extends Bar { method2() { return () => super.method(); } }\n").add_chunk("(new Foo).method2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Bar {}\nBar.prototype.method = JSCompiler_stubMethod(0);\nclass Foo extends Bar { method2() { return () => super.method(); } }\n\n"),
&"Bar.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo).method2();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodContainingObjLitContainingSuper
#[test]
fn move_class_method_containing_obj_lit_containing_super() {
    let mut t = Test::new();
    // Don't be fooled by `super` that isn't referring to the method's `super`.
    // This `super` isn't really a reference within `method()`
    // It refers to Object.prototype.toString.
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  method() {\n    return {\n      objLitMethod() {\n// This `super` isn't really a reference within `method()`\n// It refers to Object.prototype.toString.\n        super.toString;\n      }\n    };\n  }\n}\n").add_chunk("(new Foo).method();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {\n}\nFoo.prototype.method = JSCompiler_stubMethod(0);\n"),
&"Foo.prototype.method = JSCompiler_unstubMethod(0, function() {\n  return {\n    objLitMethod() {\n      super.toString;\n    }\n  };\n});\n(new Foo).method();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodContainingSuperDefaultParam
#[test]
fn do_not_move_class_method_containing_super_default_param() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar { defaultValue() { return 1; } }\nclass Foo extends Bar { method(x = super.defaultValue()) { return x; } }\n").add_chunk("(new Foo).method2()").build()),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassConstructor
#[test]
fn do_not_move_class_constructor() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { constructor() { } }\n")
            .add_chunk("(new Foo).constructor")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassComputedPropertyMethod
#[test]
fn do_not_move_class_computed_property_method() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("const methodName = 'method';")
            .add_chunk("class Foo { [methodName]() {} }")
            .add_chunk("(new Foo)[methodName]()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodForConstDefinition
#[test]
fn move_class_method_for_const_definition() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("const Foo = class FooInternal { method() {} }").add_chunk("(new Foo).method()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "const Foo = class FooInternal {}\nFoo.prototype.method = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo).method();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveFunctionCall_thatIsSideEffected
#[test]
fn do_not_move_function_call_that_is_side_effected() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    let chunks = JSChunkGraphBuilder::for_chain()
        // m1
        .add_chunk("var a = 0;\nfunction f1(a) { return a + 1 }\nvar b = f1(1);\na += 1;\n")
        // m2
        .add_chunk("var c = b")
        .build();
    t.test_same(vec![srcs(chunks)]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodWithLocalClassNameReference
#[test]
fn do_not_move_class_method_with_local_class_name_reference() {
    let mut t = Test::new();
    // We could probably rewrite the internal reference, but it is unlikely that the added
    // complexity of doing so would be worthwhile.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("const Foo = class FooInternal { method() { FooInternal; } }")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveGetterDefinedInPrototypeLiteral
#[test]
fn do_not_move_getter_defined_in_prototype_literal() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function Foo() {}\nFoo.prototype = { get method() {} };\n")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassGetter
#[test]
fn do_not_move_class_getter() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { get method() {} }")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodWithoutStub
#[test]
fn move_prototype_method_without_stub() {
    let mut t = Test::new();
    t.test_same(vec![
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype.bar = function() {};\n")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
    ]);
    t.hooks.can_move_externs = true;
    t.hooks.no_stubs = true;
    t.test(vec![
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("function Foo() {}\nFoo.prototype.bar = function() {};\n")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
        expected(&[
            &"function Foo() {}",
            // Chunk 2
            &"Foo.prototype.bar = function() {};\n(new Foo).bar()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodImplementingInterfaceWithoutStub
#[test]
fn move_prototype_method_implementing_interface_without_stub() {
    let mut t = Test::new();
    t.harness.disable_compare_js_doc().unwrap();
    // multistage compilation erases the @implements
    t.test_same(vec![
externs(&[
&"/** @interface */\nclass IFoo {\n  ifooMethod() {}\n}\n",
]),
srcs(JSChunkGraphBuilder::for_chain().add_chunk("/**\n * @constructor\n * @implements {IFoo}\n */\nfunction Foo() {}\nFoo.prototype.ifooMethod = function() {};\n").add_chunk("(new Foo).ifooMethod()").build()),
]);
    t.hooks.can_move_externs = true;
    t.hooks.no_stubs = true;
    t.test(vec![
externs(&[
&"/** @interface */\nclass IFoo {\n  ifooMethod() {}\n}\n",
]),
srcs(JSChunkGraphBuilder::for_chain().add_chunk("/**\n * @constructor\n * @implements {IFoo}\n */\nfunction Foo() {}\nFoo.prototype.ifooMethod = function() {};\n").add_chunk("(new Foo).ifooMethod()").build()),
expected(&[
&"/**\n * @constructor\n */\nfunction Foo() {}\n",
// Chunk 2
&"Foo.prototype.ifooMethod = function() {};\n(new Foo).ifooMethod()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodWithoutStub
#[test]
fn move_class_method_without_stub() {
    let mut t = Test::new();
    t.test_same(vec![
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("class Foo { bar() {} }")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
    ]);
    t.hooks.can_move_externs = true;
    t.hooks.no_stubs = true;
    t.test(vec![
        externs(&[&"IFoo.prototype.bar;"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("class Foo { bar() {} }")
                .add_chunk("(new Foo).bar()")
                .build(),
        ),
        expected(&[
            &"class Foo {}",
            // Chunk 2
            &"Foo.prototype.bar = function() {};\n(new Foo).bar()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodImplementingExternsInterfaceWithoutStub
#[test]
fn move_class_method_implementing_externs_interface_without_stub() {
    let mut t = Test::new();
    t.harness.disable_compare_js_doc().unwrap();
    // multistage compilation deletes the @implements
    t.test_same(vec![
        externs(&[&"/** @interface */\nclass IFoo {\n  ifooMethod() {}\n}\n"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("/** @implements {IFoo} */\nclass Foo { ifooMethod() {} }\n")
                .add_chunk("(new Foo).ifooMethod()")
                .build(),
        ),
    ]);
    t.hooks.can_move_externs = true;
    t.hooks.no_stubs = true;
    t.test(vec![
        externs(&[&"/** @interface */\nclass IFoo {\n  ifooMethod() {}\n}\n"]),
        srcs(
            JSChunkGraphBuilder::for_chain()
                .add_chunk("/** @implements {IFoo} */\nclass Foo { ifooMethod() {} }\n")
                .add_chunk("(new Foo).ifooMethod()")
                .build(),
        ),
        expected(&[
            &"class Foo {}",
            // Chunk 2
            &"Foo.prototype.ifooMethod = function() {};\n(new Foo).ifooMethod()\n",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodIfAliasedAndNoStubs
#[test]
fn do_not_move_prototype_method_if_aliased_and_no_stubs() {
    let mut t = Test::new();
    // don't move if noStubs enabled and there's a reference to the method to be moved
    t.hooks.no_stubs = true;
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.m = function() {};\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m()").build()),
]);
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.m = function() {};\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m(), (new Foo).m2()").build()),
]);
    t.hooks.no_stubs = false;
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.m = function() {};\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.m = JSCompiler_stubMethod(0);\nFoo.prototype.m2 = Foo.prototype.m;\n"),
// Chunk 2
&"Foo.prototype.m = JSCompiler_unstubMethod(0, function() {});\n(new Foo).m()\n",
]),
]);
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.m = function() {};\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m(), (new Foo).m2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.m = JSCompiler_stubMethod(0);\nFoo.prototype.m2 = Foo.prototype.m;\n"),
// Chunk 2
&"Foo.prototype.m = JSCompiler_unstubMethod(0, function() {});\n(new Foo).m(), (new Foo).m2()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodIfAliasedAndNoStubs
#[test]
fn do_not_move_class_method_if_aliased_and_no_stubs() {
    let mut t = Test::new();
    // don't move if noStubs enabled and there's a reference to the method to be moved
    t.hooks.no_stubs = true;
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { m() {} }\nFoo.prototype.m2 = Foo.prototype.m;\n")
            .add_chunk("(new Foo).m()")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { m() {} }\nFoo.prototype.m2 = Foo.prototype.m;\n")
            .add_chunk("(new Foo).m(), (new Foo).m2()")
            .build(),
    )]);
    t.hooks.no_stubs = false;
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { m() {} }\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.m = JSCompiler_stubMethod(0);\nFoo.prototype.m2 = Foo.prototype.m;\n"),
// Chunk 2
&"Foo.prototype.m = JSCompiler_unstubMethod(0, function() {});\n(new Foo).m()\n",
]),
]);
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { m() {} }\nFoo.prototype.m2 = Foo.prototype.m;\n").add_chunk("(new Foo).m(), (new Foo).m2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.m = JSCompiler_stubMethod(0);\nFoo.prototype.m2 = Foo.prototype.m;\n"),
// Chunk 2
&"Foo.prototype.m = JSCompiler_unstubMethod(0, function() {});\n(new Foo).m(), (new Foo).m2()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodRedeclaredInSiblingChunk
#[test]
fn do_not_move_prototype_method_redeclared_in_sibling_chunk() {
    let mut t = Test::new();
    // don't move if it can be overwritten when a sibling of the first referencing chunk is loaded.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function Foo() {}\nFoo.prototype.method = function() {};\n")
            .add_chunk("Foo.prototype.method = function() {};")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodRedeclaredInSiblingChunk
#[test]
fn do_not_move_class_method_redeclared_in_sibling_chunk() {
    let mut t = Test::new();
    // don't move if it can be overwritten when a sibling of the first referencing chunk is loaded.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("class Foo { method() {} }")
            .add_chunk("Foo.prototype.method = function() {};")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodRedeclaredInDependentChunk
#[test]
fn do_not_move_prototype_method_redeclared_in_dependent_chunk() {
    let mut t = Test::new();
    // don't move if it can be overwritten by a chunk depending on the first referencing chunk.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function Foo() {}\nFoo.prototype.method = function() {};\n")
            .add_chunk("(new Foo).method()")
            .add_chunk("Foo.prototype.method = function() {};")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodRedeclaredInDependentChunk
#[test]
fn do_not_move_class_method_redeclared_in_dependent_chunk() {
    let mut t = Test::new();
    // don't move if it can be overwritten by a chunk depending on the first referencing chunk.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { method() {} }")
            .add_chunk("(new Foo).method()")
            .add_chunk("Foo.prototype.method = function() {};")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodRedeclaredBeforeFirstReferencingChunk
#[test]
fn do_not_move_prototype_method_redeclared_before_first_referencing_chunk() {
    let mut t = Test::new();
    // Note: it is reasonable to move the method in this case,
    // but it is difficult enough to prove that we don't.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function Foo() {}\nFoo.prototype.method = function() {};\n")
            .add_chunk("Foo.prototype.method = function() {};")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodRedeclaredBeforeFirstReferencingChunk
#[test]
fn do_not_move_class_method_redeclared_before_first_referencing_chunk() {
    let mut t = Test::new();
    // Note: it is reasonable to move the method in this case,
    // but it is difficult enough to prove that we don't.
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo { method() {} }")
            .add_chunk("Foo.prototype.method = function() {};")
            .add_chunk("(new Foo).method()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#movePrototypeRecursiveMethod
#[test]
fn move_prototype_recursive_method() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() { this.baz(); };\n").add_chunk("(new Foo).baz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() { this.baz(); });\n(new Foo).baz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveInstanceRecursiveMethod
#[test]
fn move_instance_recursive_method() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { baz() { this.baz(); } }").add_chunk("(new Foo).baz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() { this.baz(); });\n(new Foo).baz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveNonLiteralFunction
#[test]
fn do_not_move_non_literal_function() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("function Foo() {}\nFoo.prototype.baz = shared;\n")
            .add_chunk("(new Foo).baz()")
            .build(),
    )]);
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("class Foo {}\nFoo.prototype.baz = shared;\n")
            .add_chunk("(new Foo).baz()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#movePrototypeDeclarationsInTheRightOrder
#[test]
fn move_prototype_declarations_in_the_right_order() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() { return 1; };\nFoo.prototype.baz = function() { return 2; };\n").add_chunk("(new Foo).baz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(1);\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(1, function() { return 1; });\nFoo.prototype.baz = JSCompiler_unstubMethod(0, function() { return 2; });\n(new Foo).baz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodAndReclarationInTheRightOrder
#[test]
fn move_class_method_and_reclaration_in_the_right_order() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { baz() { return 1; } }\nFoo.prototype.baz = function() { return 2; };\n").add_chunk("(new Foo).baz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(1);\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz =\nJSCompiler_unstubMethod(1, function() { return 1; });\nFoo.prototype.baz =\nJSCompiler_unstubMethod(0, function() { return 2; });\n(new Foo).baz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodsForDifferentClassesInTheRightOrder
#[test]
fn move_prototype_methods_for_different_classes_in_the_right_order() {
    let mut t = Test::new();
    let m = JSChunkGraphBuilder::for_unordered().add_chunk("function Foo() {}\nFoo.prototype.baz = function() { return 1; };\nfunction Goo() {}\nGoo.prototype.baz = function() { return 2; };\n")
// Chunk 2, depends on 1
.add_chunk("")
// Chunk 3, depends on 2
.add_chunk("(new Foo).baz()")
// Chunk 4, depends on 3
.add_chunk("")
// Chunk 5, depends on 3
.add_chunk("(new Goo).baz()").build();
    m[1].add_dependency(&m[0]);
    m[2].add_dependency(&m[1]);
    m[3].add_dependency(&m[2]);
    m[4].add_dependency(&m[2]);
    t.test(vec![
srcs(m),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(1);\nfunction Goo() {}\nGoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"",
// Chunk 3
&"Foo.prototype.baz = JSCompiler_unstubMethod(1, function() { return 1; });\nGoo.prototype.baz = JSCompiler_unstubMethod(0, function() { return 2; });\n(new Foo).baz()\n",
// Chunk 4
&"",
// Chunk 5
&"(new Goo).baz()",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodsForDifferentClassesInTheRightOrder
#[test]
fn move_class_methods_for_different_classes_in_the_right_order() {
    let mut t = Test::new();
    let m = JSChunkGraphBuilder::for_unordered()
        .add_chunk("class Foo { baz() { return 1; } }\nclass Goo { baz() { return 2; } }\n")
        // Chunk 2, depends on 1
        .add_chunk("")
        // Chunk 3, depends on 2
        .add_chunk("(new Foo).baz()")
        // Chunk 4, depends on 3
        .add_chunk("")
        // Chunk 5, depends on 3
        .add_chunk("(new Goo).baz()")
        .build();
    m[1].add_dependency(&m[0]);
    m[2].add_dependency(&m[1]);
    m[3].add_dependency(&m[2]);
    m[4].add_dependency(&m[2]);
    t.test(vec![
srcs(m),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(1);\nclass Goo {}\nGoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"",
// Chunk 3
&"Foo.prototype.baz = JSCompiler_unstubMethod(1, function() { return 1; });\nGoo.prototype.baz = JSCompiler_unstubMethod(0, function() { return 2; });\n(new Foo).baz()\n",
// Chunk 4
&"",
// Chunk 5
&"(new Goo).baz()",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodUsedInMultiplepDependentChunks
#[test]
fn do_not_move_prototype_method_used_in_multiplep_dependent_chunks() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("function Foo() {}\nFoo.prototype.baz = function() {};\n")
            .add_chunk("(new Foo).baz()")
            .add_chunk("(new Foo).baz()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodUsedInMultiplepDependentChunks
#[test]
fn do_not_move_class_method_used_in_multiplep_dependent_chunks() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_star()
            .add_chunk("class Foo { baz() {} }")
            .add_chunk("(new Foo).baz()")
            .add_chunk("(new Foo).baz()")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodToDeepestCommonDependencyOfReferencingChunks
#[test]
fn move_prototype_method_to_deepest_common_dependency_of_referencing_chunks() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_unordered()
        .add_chunk("function Foo() {}\nFoo.prototype.baz = function() {};\n")
        // Chunk 2
        // a blank chunk in the middle
        .add_chunk("")
        // Chunk 3
        .add_chunk("(new Foo).baz() , 1")
        // Chunk 4
        .add_chunk("(new Foo).baz() , 2")
        .build();
    chunks[1].add_dependency(&chunks[0]);
    chunks[2].add_dependency(&chunks[1]);
    chunks[3].add_dependency(&chunks[1]);
    t.test(vec![
        srcs(chunks),
        expected(&[
            &(String::new()
                + STUB_DECLARATIONS
                + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
            // Chunk 2
            &"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() {});",
            // Chunk 3
            &"(new Foo).baz() , 1",
            // Chunk 4
            &"(new Foo).baz() , 2",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodToDeepestCommonDependencyOfReferencingChunks
#[test]
fn move_class_method_to_deepest_common_dependency_of_referencing_chunks() {
    let mut t = Test::new();
    let chunks = JSChunkGraphBuilder::for_unordered()
        .add_chunk("class Foo { baz() {} }")
        // Chunk 2
        // a blank chunk in the middle
        .add_chunk("")
        // Chunk 3
        .add_chunk("(new Foo).baz() , 1")
        // Chunk 4
        .add_chunk("(new Foo).baz() , 2")
        .build();
    chunks[1].add_dependency(&chunks[0]);
    chunks[2].add_dependency(&chunks[1]);
    chunks[3].add_dependency(&chunks[1]);
    t.test(vec![
        srcs(chunks),
        expected(&[
            &(String::new()
                + STUB_DECLARATIONS
                + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
            // Chunk 2
            &"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() {});",
            // Chunk 3
            &"(new Foo).baz() , 1",
            // Chunk 4
            &"(new Foo).baz() , 2",
        ]),
    ]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodThatRefersToAnotherOnTheSameClass
#[test]
fn move_prototype_method_that_refers_to_another_on_the_same_class() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() {};\n")
// Chunk 2
.add_chunk("Foo.prototype.callBaz = function() { this.baz(); }")
// Chunk 3
.add_chunk("(new Foo).callBaz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.callBaz = JSCompiler_stubMethod(1);",
// Chunk 3
&"Foo.prototype.callBaz =\n  JSCompiler_unstubMethod(1, function() { this.baz(); });\nFoo.prototype.baz = JSCompiler_unstubMethod(0, function() {});\n(new Foo).callBaz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodThatRefersToAnClassMethodOnTheSameClass
#[test]
fn move_prototype_method_that_refers_to_an_class_method_on_the_same_class() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { baz() {} }")
// Chunk 2
.add_chunk("Foo.prototype.callBaz = function() { this.baz(); }")
// Chunk 3
.add_chunk("(new Foo).callBaz()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.callBaz = JSCompiler_stubMethod(1);",
// Chunk 3
&"Foo.prototype.callBaz = JSCompiler_unstubMethod(1, function() { this.baz(); });\nFoo.prototype.baz = JSCompiler_unstubMethod(0, function() {});\n(new Foo).callBaz()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodDefinitionThatFollowsFirstUse
#[test]
fn do_not_move_prototype_method_definition_that_follows_first_use() {
    let mut t = Test::new();
    // if the programmer screws up the chunk order, we don't try to correct
    // the mistake.
    // call before definition
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() {};\n")
// Chunk 2
// call before definition
.add_chunk("(new Foo).callBaz()")
// Chunk 3
.add_chunk("Foo.prototype.callBaz = function() { this.baz(); }").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"(new Foo).callBaz()",
// Chunk 3
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() {});\nFoo.prototype.callBaz = function() { this.baz(); };\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodPastUsageInAGlobalFunction
#[test]
fn move_prototype_method_past_usage_in_a_global_function() {
    let mut t = Test::new();
    // usage here doesn't really happen until x() is called, so
    // it's OK to move the definition of baz().
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() {};\n// usage here doesn't really happen until x() is called, so\n// it's OK to move the definition of baz().\nfunction x() { return (new Foo).baz(); }\n")
// Chunk 2
.add_chunk("x();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\nfunction x() { return (new Foo).baz(); }\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() {});\nx();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodPastUsageInAGlobalFunction
#[test]
fn move_class_method_past_usage_in_a_global_function() {
    let mut t = Test::new();
    // usage here doesn't really happen until x() is called, so
    // it's OK to move the definition of baz().
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  baz() {}\n}\n// usage here doesn't really happen until x() is called, so\n// it's OK to move the definition of baz().\nfunction x() { return (new Foo).baz(); }\n")
// Chunk 2
.add_chunk("x();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\nfunction x() { return (new Foo).baz(); }\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function() {});\nx();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#doNotMovePrototypeMethodThatUsesLocalClosureVariable
#[test]
fn do_not_move_prototype_method_that_uses_local_closure_variable() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\n(function() {\n  var x = 'x';\n  Foo.prototype.baz = function() {x};\n})();\n").add_chunk("var y = new Foo(); y.baz();").build()),
]);
}

// port: CrossChunkMethodMotionTest#doNotMoveClassMethodThatUsesLocalClosureVariable
#[test]
fn do_not_move_class_method_that_uses_local_closure_variable() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("const Foo = (function() {\n  var x = 'x';\n  return class Foo { baz() { return x; } };\n})();\n").add_chunk("var y = new Foo(); y.baz();").build()),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodThatDefinesOtherMethodsOnSameGlobalClass
#[test]
fn move_prototype_method_that_defines_other_methods_on_same_global_class() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.b1 = function() {\n  var x = 1;\n  Foo.prototype.b2 = function() {\n    Foo.prototype.b3 = function() {\n      x;\n    }\n  }\n};\n")
// Chunk 2
.add_chunk("var y = new Foo(); y.b1();")
// Chunk 3
.add_chunk("y = new Foo(); z.b2();")
// Chunk 4
.add_chunk("y = new Foo(); z.b3();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.b1 = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.b1 = JSCompiler_unstubMethod(0, function() {\n  var x = 1;\n  Foo.prototype.b2 = function() {\n    Foo.prototype.b3 = function() {\n      x;\n    }\n  }\n});\nvar y = new Foo(); y.b1();\n",
// Chunk 3
&"y = new Foo(); z.b2();",
// Chunk 4
&"y = new Foo(); z.b3();",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodThatDefinesOtherMethodsOnSameGlobalClass
#[test]
fn move_class_method_that_defines_other_methods_on_same_global_class() {
    let mut t = Test::new();
    // b2 cannot be extracted, because it contains a reference to x
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  b1() {\n    var x = 1;\n// b2 cannot be extracted, because it contains a reference to x\n    Foo.prototype.b2 = function() {\n      Foo.prototype.b3 = function() {\n        x;\n      }\n    }\n  };\n}\n").add_chunk("var y = new Foo(); y.b1();").add_chunk("y = new Foo(); z.b2();").add_chunk("y = new Foo(); z.b3();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.b1 = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.b1 = JSCompiler_unstubMethod(0, function() {\n  var x = 1;\n  Foo.prototype.b2 = function() {\n    Foo.prototype.b3 = function() {\n      x;\n    }\n  }\n});\nvar y = new Foo(); y.b1();\n",
// Chunk 3
&"y = new Foo(); z.b2();",
// Chunk 4
&"y = new Foo(); z.b3();",
]),
]);
}

// port: CrossChunkMethodMotionTest#extractPrototypeMethodDefinedInAnotherMethodWhenNoClosureReferencePreventsIt
#[test]
fn extract_prototype_method_defined_in_another_method_when_no_closure_reference_prevents_it() {
    let mut t = Test::new();
    // definition of b2 can be extracted, because it doesn't refer to any
    // variables
    // defined by b1.
    // definition of b3 cannot be extracted, because it refers to x
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.b1 = function() {\n// definition of b2 can be extracted, because it doesn't refer to any\n// variables\n// defined by b1.\n  Foo.prototype.b2 = function() {\n    var x = 1;\n// definition of b3 cannot be extracted, because it refers to x\n    Foo.prototype.b3 = function() {\n      x;\n    }\n  }\n};\n").add_chunk("var y = new Foo(); y.b1();").add_chunk("y = new Foo(); z.b2();").add_chunk("y = new Foo(); z.b3();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.b1 = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.b1 = JSCompiler_unstubMethod(0, function() {\n  Foo.prototype.b2 = JSCompiler_stubMethod(1);\n});\nvar y = new Foo(); y.b1();\n",
// Chunk 3
&"Foo.prototype.b2 = JSCompiler_unstubMethod(1, function() {\n  var x = 1;\n  Foo.prototype.b3 = function() {\n    x;\n  }\n});\ny = new Foo(); z.b2();\n",
// Chunk 4
&"y = new Foo(); z.b3();",
]),
]);
}

// port: CrossChunkMethodMotionTest#extractClassMethodDefinedInAnotherMethodWhenNoClosureReferencePreventsIt
#[test]
fn extract_class_method_defined_in_another_method_when_no_closure_reference_prevents_it() {
    let mut t = Test::new();
    // definition of b2 can be extracted, because it doesn't refer to any
    // variables
    // defined by b1.
    // definition of b3 cannot be extracted, because it refers to x
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  b1() {\n// definition of b2 can be extracted, because it doesn't refer to any\n// variables\n// defined by b1.\n    Foo.prototype.b2 = function() {\n      var x = 1;\n// definition of b3 cannot be extracted, because it refers to x\n      Foo.prototype.b3 = function() {\n        x;\n      }\n    }\n  }\n}\n")
// Chunk 2
.add_chunk("var y = new Foo(); y.b1();")
// Chunk 3
.add_chunk("y = new Foo(); z.b2();")
// Chunk 4
.add_chunk("y = new Foo(); z.b3();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.b1 = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.b1 =\n    JSCompiler_unstubMethod(\n        0,\n        function() {\n          Foo.prototype.b2 = JSCompiler_stubMethod(1);\n        });\n\n\nvar y = new Foo(); y.b1();\n",
// Chunk 3
&"Foo.prototype.b2 = JSCompiler_unstubMethod(1, function() {\n  var x = 1;\n  Foo.prototype.b3 = function() {\n    x;\n  }\n});\ny = new Foo(); z.b2();\n",
// Chunk 4
&"y = new Foo(); z.b3();",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodThatReadsGlobalVar
#[test]
fn move_prototype_method_that_reads_global_var() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nvar x = 'x';\nFoo.prototype.baz = function(){x};\n").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nvar x = 'x';\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function(){x});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodThatReadsGlobalVar
#[test]
fn move_class_method_that_reads_global_var() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  baz() { x; }\n}\nvar x = 'x';\n").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\nvar x = 'x';\n\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(0, function(){x});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodThatReferencesOnlyLocalVariables
#[test]
fn move_prototype_method_that_references_only_local_variables() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function(){var x = 1;x};\n").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(\n    0, function(){var x = 1; x});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodThatReferencesOnlyLocalVariables
#[test]
fn move_class_method_that_references_only_local_variables() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo { baz() {var x = 1; x; } }").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(\n    0, function(){var x = 1; x});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#movePrototypeMethodContainingClosureOverLocalVariable
#[test]
fn move_prototype_method_containing_closure_over_local_variable() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("function Foo() {}\nFoo.prototype.baz = function() {\n  var x = 1;\n  return function(){x}\n};\n").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "function Foo() {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(\n    0, function(){var x = 1; return function(){x}});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#moveClassMethodContainingClosureOverLocalVariable
#[test]
fn move_class_method_containing_closure_over_local_variable() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Foo {\n  baz() {\n    var x = 1;\n    return function(){x}\n  }\n}\n").add_chunk("var y = new Foo(); y.baz();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Foo {}\nFoo.prototype.baz = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Foo.prototype.baz = JSCompiler_unstubMethod(\n    0, function(){var x = 1; return function(){x}});\nvar y = new Foo(); y.baz();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#staticBlockWithoutMethodReference
#[test]
fn static_block_without_method_reference() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar {\n  method() {\n  }\n  static {\n  }\n}\nclass Foo extends Bar {\n  method2() {\n    return () => super.method();\n  }\n}\n").add_chunk("(new Foo).method2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Bar {\n  static {\n  }\n}\nBar.prototype.method = JSCompiler_stubMethod(0);\nclass Foo extends Bar {\n  method2() {\n    return () => super.method();\n  }\n}\n"),
// Chunk 2
&"Bar.prototype.method = JSCompiler_unstubMethod(0, function() {});\n(new Foo).method2()\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#referenceToMethodInOwnStaticBlock
#[test]
fn reference_to_method_in_own_static_block() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar {\n  method() {\n  }\n  static {\n    this.prototype.method;\n  }\n}\nclass Foo extends Bar {\n  method2() {\n    return () => super.method();\n  }\n}\n").add_chunk("(new Foo).method2()").build()),
]);
}

// port: CrossChunkMethodMotionTest#staticBlockReferenceToMethodInDifferentClassNoMovement
#[test]
fn static_block_reference_to_method_in_different_class_no_movement() {
    let mut t = Test::new();
    // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
    t.harness.enable_normalize_expected_output().unwrap();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar {\n  method() {}\n}\nclass Foo extends Bar {\n  static {\n    (new Bar).method();\n  }\n  method2() {\n    return () => super.method();\n  }\n}\n").add_chunk("(new Foo).method2()").build()),
]);
}

// port: CrossChunkMethodMotionTest#staticBlockReferenceToMethodInDifferentClassWithMovement
#[test]
fn static_block_reference_to_method_in_different_class_with_movement() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("class Bar {\n  method() {}\n}\n").add_chunk("class Foo extends Bar {\n  static {\n    (new Bar()).method();\n  }\n  method2() {\n    return () => { return super.method(); };\n  }\n}\n").add_chunk("(new Foo).method2()").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "class Bar {\n}\nBar.prototype.method = JSCompiler_stubMethod(0);\n"),
// Chunk 2
&"Bar.prototype.method = JSCompiler_unstubMethod(0, function() {});\nclass Foo extends Bar {\n  static {\n    (new Bar()).method();\n  }\n  method2() {\n    return () => { return super.method(); };\n  }\n}\n",
// Chunk 3
&"(new Foo()).method2();",
]),
]);
}

// port: CrossChunkMethodMotionTest#testIssue600
#[test]
fn test_issue600() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("var jQuery1 = (function() {\n  var jQuery2 = function() {};\n  var theLoneliestNumber = 1;\n  jQuery2.prototype = {\n    size: function() {\n      return theLoneliestNumber;\n    }\n  };\n  return jQuery2;\n})();\n").add_chunk("(function() {\n  var div = jQuery1('div');\n  div.size();\n})();\n").build()),
]);
}

// port: CrossChunkMethodMotionTest#testIssue600b
#[test]
fn test_issue600b() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("var jQuery1 = (function() {\n  var jQuery2 = function() {};\n  jQuery2.prototype = {\n    size: function() {\n      return 1;\n    }\n  };\n  return jQuery2;\n})();\n").add_chunk("(function() {\n  var div = jQuery1('div');\n  div.size();\n})();\n").build()),
]);
}

// port: CrossChunkMethodMotionTest#testIssue600c
#[test]
fn test_issue600c() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("var jQuery2 = function() {};\njQuery2.prototype = {\n  size: function() {\n    return 1;\n  }\n};\n").add_chunk("(function() {\n  var div = jQuery2('div');\n  div.size();\n})();\n").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "var jQuery2 = function() {};\njQuery2.prototype = {\n  size: JSCompiler_stubMethod(0)\n};\n"),
// Chunk 2
&"jQuery2.prototype.size=\n    JSCompiler_unstubMethod(0,function(){return 1});\n(function() {\n  var div = jQuery2('div');\n  div.size();\n})();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#testIssue600d
#[test]
fn test_issue600d() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("var jQuery2 = function() {};\n(function() {\n  jQuery2.prototype = {\n    size: function() {\n      return 1;\n    }\n  };\n})();\n").add_chunk("(function() {\n  var div = jQuery2('div');\n  div.size();\n})();\n").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "var jQuery2 = function() {};\n(function() {\n  jQuery2.prototype = {\n    size: JSCompiler_stubMethod(0)\n  };\n})();\n"),
&"jQuery2.prototype.size=\n    JSCompiler_unstubMethod(0,function(){return 1});\n(function() {\n  var div = jQuery2('div');\n  div.size();\n})();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#testIssue600e
#[test]
fn test_issue600e() {
    let mut t = Test::new();
    t.test_same(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("var jQuery2 = function() {};\n(function() {\n  var theLoneliestNumber = 1;\n  jQuery2.prototype = {\n    size: function() {\n      return theLoneliestNumber;\n    }\n  };\n})();\n").add_chunk("(function() {\n  var div = jQuery2('div');\n  div.size();\n})();\n").build()),
]);
}

// port: CrossChunkMethodMotionTest#testPrototypeOfThisAssign
#[test]
fn test_prototype_of_this_assign() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("/** @constructor */\nfunction F() {}\n")
            .add_chunk("this.prototype.foo = function() {};")
            .add_chunk("(new F()).foo();")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#testDestructuring
#[test]
fn test_destructuring() {
    let mut t = Test::new();
    t.test(vec![
srcs(JSChunkGraphBuilder::for_chain().add_chunk("/** @constructor */\nfunction F() {}\nF.prototype.foo = function() {};\n").add_chunk("const {foo} = new F();").build()),
expected(&[
&(String::new() + STUB_DECLARATIONS + "/** @constructor */\nfunction F() {}\nF.prototype.foo = JSCompiler_stubMethod(0);\n"),
&"F.prototype.foo = JSCompiler_unstubMethod(0, function(){});\nconst {foo} = new F();\n",
]),
]);
}

// port: CrossChunkMethodMotionTest#testDestructuringWithQuotedProp
#[test]
fn test_destructuring_with_quoted_prop() {
    let mut t = Test::new();
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk("/** @constructor */\nfunction F() {}\nF.prototype.foo = function() {};\n")
            .add_chunk("const {'foo': foo} = new F();")
            .build(),
    )]);
}

// port: CrossChunkMethodMotionTest#testDestructuringWithComputedProp
#[test]
fn test_destructuring_with_computed_prop() {
    let mut t = Test::new();
    // See https://github.com/google/closure-compiler/issues/3145
    t.test_same(vec![srcs(
        JSChunkGraphBuilder::for_chain()
            .add_chunk(
                "/** @constructor */\nfunction F() {}\nF.prototype['foo'] = function() {};\n",
            )
            .add_chunk("const {['foo']: foo} = new F();")
            .build(),
    )]);
}
