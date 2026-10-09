/*
 * Copyright 2012 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/InlinePropertiesTest.java.

//! Port of InlinePropertiesTest (CompilerTestCase over the crates/testing port).
mod optimize_calls_support;

use closure_jscomp::{
    Compiler, compiler_pass::CompilerPass, remove_unused_code, remove_unused_code::RemoveUnusedCode,
};
use closure_jscomp::{
    check_level::CheckLevel, diagnostic_group::DiagnosticGroup,
    inline_properties::InlineProperties, type_check, type_validator,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::replay_dsl::{CompilerHandle, Ctx, DslValue},
    throwable::Throwable,
};
#[allow(unused_imports)]
use optimize_calls_support::*;

struct Hooks {
    ctx: Ctx,
    run_smart_name_removal: bool,
}

/// InlinePropertiesTest$1: the anonymous CompilerPass of getProcessor (runSmartNameRemoval).
struct RemovalThenInline {
    pass: InlineProperties,
    removal_pass: RemoveUnusedCode,
}

impl CompilerPass for RemovalThenInline {
    // port: InlinePropertiesTest$1#process
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        self.removal_pass.process(compiler, externs, root);
        self.pass.process(compiler, externs, root);
    }
}

impl CompilerTestCaseHooks for Hooks {
    // port: InlinePropertiesTest#getProcessor
    fn get_processor(&mut self, compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let compiler = compiler.borrow();
        let inline_pass = InlineProperties::new(&compiler);
        if self.run_smart_name_removal {
            let removal_pass = remove_unused_code::Builder::new(&compiler)
                .remove_local_vars(true)
                .remove_globals(true)
                .preserve_function_expression_names(true)
                .remove_unused_prototype_properties(true)
                .remove_unused_this_properties(true)
                .remove_unused_object_define_properties_definitions(true)
                .build();
            return Ok(pass(RemovalThenInline {
                pass: inline_pass,
                removal_pass,
            }));
        }
        Ok(pass(inline_pass))
    }
    // port: InlinePropertiesTest#getOptions
    fn get_options(
        &mut self,
        harness: &mut CompilerTestCase,
    ) -> Result<closure_jscomp::compiler_options::CompilerOptions, Throwable> {
        let mut options = harness.get_options()?;
        // Ignore a few type warnings: we intentionally trigger these warnings
        // to make sure that the pass still operates correctly with bad code.
        let ignored = std::sync::Arc::new(DiagnosticGroup::new(&[
            &type_check::INEXISTENT_PROPERTY,
            &type_validator::TYPE_MISMATCH_WARNING,
        ]));
        options.set_warning_level(ignored, CheckLevel::OFF);
        Ok(options)
    }
    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

type Test = Fixture<Hooks>;

impl Test {
    // port: InlinePropertiesTest#InlinePropertiesTest + #setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new(externs_text());
        harness.set_up();
        harness.enable_type_check().unwrap();
        harness.enable_normalize().unwrap();
        harness.enable_closure_pass().unwrap();
        harness.enable_rewrite_closure_provides().unwrap();
        harness.enable_gather_extern_properties().unwrap();
        harness.replace_types_with_colors().unwrap();
        harness.disable_compare_js_doc().unwrap();
        Self {
            harness,
            hooks: Hooks {
                ctx: native_ctx("InlinePropertiesTest"),
                run_smart_name_removal: false,
            },
        }
    }
}

// port: InlinePropertiesTest#EXTERNS
fn externs_text() -> JsString {
    CompilerTestCase::minimal_externs()
        .unwrap()
        .concat(&JsString::from(concat!(
            "Function.prototype.call=function(){};\n",
            "Function.prototype.inherits=function(){};\n",
            "prop.toString;\n",
            "var google = { gears: { factory: {}, workerPool: {} } };\n",
            "/** @type {?} */ var externUnknownVar;\n",
            "/** @type {!Function} */ var externFn;\n",
        )))
}

// port: InlinePropertiesTest#testConstInstanceProp1
#[test]
fn test_const_instance_prop1() {
    let mut t = Test::new();
    // Replace a reference to known constant property.
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1;\n",
            "}\n",
            "new C().foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1;\n",
            "}\n",
            "new C(), 1;\n"
        ),
    );
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  {\n",
            "    this.foo = 1;\n",
            "  }\n",
            "}\n",
            "new C().foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  {\n",
            "    this.foo = 1;\n",
            "  }\n",
            "}\n",
            "new C(), 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstInstanceProp2
#[test]
fn test_const_instance_prop2() {
    let mut t = Test::new();
    // Replace a constant reference
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1;\n",
            "}\n",
            "var x = new C();\n",
            "x.foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1\n",
            "}\n",
            "var x = new C();\n",
            "1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstInstanceProp3
#[test]
fn test_const_instance_prop3() {
    let mut t = Test::new();
    // Replace a constant reference
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1;\n",
            "}\n",
            "/** @type {C} */\n",
            "var x = new C();\n",
            "x.foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 1\n",
            "}\n",
            "/** @type {C} */\n",
            "var x = new C();\n",
            "1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstInstanceProp4
#[test]
fn test_const_instance_prop4() {
    let mut t = Test::new();
    // This pass replies on DisambiguateProperties to distinguish like named
    // properties so it doesn't handle this case.
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "  this.foo = 1;\n",
        "}\n",
        "/** @constructor */\n",
        "function B() {\n",
        "  this.foo = 1;\n",
        "}\n",
        "new C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstInstanceProp5
#[test]
fn test_const_instance_prop5() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function Foo() {\n",
            "  /** @type {?number} */\n",
            "  this.a = 1;\n",
            "  /** @type {number} */\n",
            "  this.b = 2;\n",
            "}\n",
            "var x = (new Foo).b;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function Foo() {\n",
            "  /** @type {?number} */\n",
            "  this.a = 1;\n",
            "  /** @type {number} */\n",
            "  this.b = 2;\n",
            "}\n",
            "var x = (new Foo, 2);\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstClassProps1
#[test]
fn test_const_class_props1() {
    let mut t = Test::new();
    // Inline constant class properties,
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "}\n",
            "C.bar = 2;\n",
            "C.foo = 1;\n",
            "var z = C.foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "}\n",
            "C.bar = 2;\n",
            "C.foo = 1;\n",
            "var z = 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstClassProps2
#[test]
fn test_const_class_props2() {
    let mut t = Test::new();
    // Don't confuse, class properties with instance properties
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "  this.foo = 1;\n",
        "}\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps3
#[test]
fn test_const_class_props3() {
    let mut t = Test::new();
    // Don't confuse, class properties with prototype properties
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "C.prototype.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps4
#[test]
fn test_const_class_props4() {
    let mut t = Test::new();
    // Don't confuse unique constructors with similiar function types
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "/** @constructor @extends {C} */\n",
        "function D() {}\n",
        "/** @type {function(new:C): undefined} */\n",
        "var x = D;\n",
        "/** @type {number} */ x.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps5
#[test]
fn test_const_class_props5() {
    let mut t = Test::new();
    // Don't confuse subtype constructors properties
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "/** @constructor @extends {C} */\n",
        "function D() {}\n",
        "D.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps6
#[test]
fn test_const_class_props6() {
    let mut t = Test::new();
    // Don't inline to unknowns
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "C.foo = 1;\n",
        "var z = externUnknownVar.foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps7
#[test]
fn test_const_class_props7() {
    let mut t = Test::new();
    // Don't inline to Function prop
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "C.foo = 1;\n",
        "var z = externFn.foo;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstClassProp1
#[test]
fn test_non_const_class_prop1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "}\n",
        "C.foo = 1;\n",
        "alert(C.foo);\n",
        "delete C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstClassProp2
#[test]
fn test_non_const_class_prop2() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "}\n",
        "C.foo = 1;\n",
        "alert(C.foo);\n",
        "C.foo = 2;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstClassProp3
#[test]
fn test_non_const_class_prop3() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "}\n",
        "C.foo = 1;\n",
        "function f(a) {\n",
        " a.foo = 2;\n",
        "}\n",
        "alert(C.foo);\n",
        "f(C);\n"
    ));
}

// port: InlinePropertiesTest#testNonConstInstanceProp1
#[test]
fn test_non_const_instance_prop1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "  this.foo = 1;\n",
        "}\n",
        "var x = new C();\n",
        "alert(x.foo);\n",
        "delete x.foo;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstInstanceProp2
#[test]
fn test_non_const_instance_prop2() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "  this.foo = 1;\n",
        "}\n",
        "var x = new C();\n",
        "alert(x.foo);\n",
        "x.foo = 2;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstructorInstanceProp1
#[test]
fn test_non_constructor_instance_prop1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "function C() {\n",
        "  this.foo = 1;\n",
        "  return this;\n",
        "}\n",
        "C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testConditionalInstanceProp1
#[test]
fn test_conditional_instance_prop1() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {\n",
        "  if (false) this.foo = 1;\n",
        "}\n",
        "new C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstPrototypeProp1
#[test]
fn test_const_prototype_prop1() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "new C().foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "new C(), 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstPrototypeProp2
#[test]
fn test_const_prototype_prop2() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "var x = new C();\n",
            "x.foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "var x = new C();\n",
            "1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstPrototypePropInGlobalBlockScope
#[test]
fn test_const_prototype_prop_in_global_block_scope() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "{\n",
            "  C.prototype.foo = 1;\n",
            "}\n",
            "var x = new C();\n",
            "x.foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "{\n",
            "  C.prototype.foo = 1;\n",
            "}\n",
            "var x = new C();\n",
            "1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testGlobalThisNotInlined
#[test]
fn test_global_this_not_inlined() {
    let mut t = Test::new();
    t.test_same(concat!(
        "this.foo = 1;\n",
        "/** @constructor */\n",
        "function C() {\n",
        "  foo;\n",
        "}\n"
    ));
}

// port: InlinePropertiesTest#testConstPrototypePropFromSuper
#[test]
fn test_const_prototype_prop_from_super() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "/** @constructor @extends {C} */\n",
            "function D() {}\n",
            "(new D).foo;\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "/** @constructor @extends {C} */\n",
            "function D() {}\n",
            "new D, 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testTypedPropInlining
#[test]
fn test_typed_prop_inlining() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "function f(/** !C */ x) { return x.foo; }\n",
            "f(new C);\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {}\n",
            "C.prototype.foo = 1;\n",
            "function f(/** !C */ x) { return 1; }\n",
            "f(new C);\n"
        ),
    );
}

// port: InlinePropertiesTest#testTypeMismatchNoPropInlining
#[test]
fn test_type_mismatch_no_prop_inlining() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "C.prototype.foo = 1;\n",
        "function f(/** !C */ x) { return x.foo; }\n",
        "f([]);\n"
    ));
}

// port: InlinePropertiesTest#testTypeMismatchForPrototypeNoPropInlining
#[test]
fn test_type_mismatch_for_prototype_no_prop_inlining() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @constructor */\n",
        "function C() {}\n",
        "C.prototype.foo = 1;\n",
        "function f(/** number */ x) {}\n",
        "f(C.prototype);\n",
        "new C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testStructuralInterfacesNoPropInlining
#[test]
fn test_structural_interfaces_no_prop_inlining() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @record */ function I() {}\n",
        "/** @type {number|undefined} */ I.prototype.foo;\n",
        "\n",
        "/** @constructor @implements {I} */\n",
        "function C() {}\n",
        "/** @override */\n",
        "C.prototype.foo = 1;\n",
        "\n",
        "function f(/** !I */ x) { return x.foo; }\n",
        "f([]);\n"
    ));
}

// port: InlinePropertiesTest#testStructuralInterfacesNoPropInlining2
#[test]
fn test_structural_interfaces_no_prop_inlining2() {
    let mut t = Test::new();
    t.hooks.run_smart_name_removal = true;

    t.test(
        concat!(
            "/** @record */\n",
            "function I() {\n",
            "  /** @type {number} */ this.foo;\n",
            "}\n",
            "\n",
            "/** @constructor @implements {I} */\n",
            "function C() { /** @type {number} */ this.foo = 1; }\n",
            "\n",
            "function f(/** ? */ x) { return x.foo; }\n",
            "f(new C());\n"
        ),
        concat!(
            "/** @constructor @implements {I} */\n",
            "function C() { /** @type {number} */ this.foo = 1; }\n",
            "\n",
            "function f(/** ? */ x) { return x.foo; }\n",
            "f(new C());\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstInstanceProp_es6Class
#[test]
fn test_const_instance_prop_es6_class() {
    let mut t = Test::new();
    // Replace a reference to known constant property.
    t.test(
        concat!(
            "class C {\n",
            "  constructor() {\n",
            "    this.foo = 1;\n",
            "  }\n",
            "}\n",
            "new C().foo;\n"
        ),
        concat!(
            "class C {\n",
            "  constructor() {\n",
            "    this.foo = 1;\n",
            "  }\n",
            "}\n",
            "new C(), 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testMultipleConstInstanceProp_es6Class
#[test]
fn test_multiple_const_instance_prop_es6_class() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class Foo {\n",
            "  constructor() {\n",
            "    /** @type {?number} */\n",
            "    this.a = 1;\n",
            "    /** @type {number} */\n",
            "    this.b = 2;\n",
            "  }\n",
            "}\n",
            "var x = (new Foo).b;\n"
        ),
        concat!(
            "class Foo {\n",
            "  constructor() {\n",
            "    /** @type {?number} */\n",
            "    this.a = 1;\n",
            "    /** @type {number} */\n",
            "    this.b = 2;\n",
            "  }\n",
            "}\n",
            "var x = (new Foo, 2);\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstInstancePropInArrowFunction_es6Class
#[test]
fn test_const_instance_prop_in_arrow_function_es6_class() {
    let mut t = Test::new();
    // Don't replace a reference to known constant property defined in an arrow function.
    t.test_same(concat!(
        "/** @unrestricted */ // make this not a struct, so we can define this.foo\n",
        "class C {\n",
        "  constructor() {\n",
        "    (() => {\n",
        "      this.foo = 1;\n",
        "    })();\n",
        "  }\n",
        "}\n",
        "new C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstInstancePropInForOf_es6Class
#[test]
fn test_const_instance_prop_in_for_of_es6_class() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @unrestricted */\n",
        "class C {\n",
        "  constructor() {\n",
        "    for (const x of [1, 2]) {\n",
        "      this.foo = 1;\n",
        "    }\n",
        "  }\n",
        "}\n",
        "new C().foo;\n"
    ));
}

// port: InlinePropertiesTest#testConstClassProps_es6Class
#[test]
fn test_const_class_props_es6_class() {
    let mut t = Test::new();
    // Inline constant class properties,
    t.test(
        concat!(
            "class C {}\n",
            "C.bar = 2;\n",
            "C.foo = 1;\n",
            "var z = C.foo;\n"
        ),
        concat!(
            "class C {}\n",
            "C.bar = 2;\n",
            "C.foo = 1;\n",
            "var z = 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstClassPropsInheritedProp_es6Class
#[test]
fn test_const_class_props_inherited_prop_es6_class() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {}\n",
            "class D extends C {}\n",
            "C.foo = 1;\n",
            "var z = D.foo;\n"
        ),
        concat!(
            "class C {}\n",
            "class D extends C {}\n",
            "C.foo = 1;\n",
            "var z = 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstClassPropsInheritedPropChain_es6Class
#[test]
fn test_const_class_props_inherited_prop_chain_es6_class() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {}\n",
            "class D extends C {}\n",
            "class E extends D {}\n",
            "class F extends E {}\n",
            "C.foo = 1;\n",
            "var z = F.foo;\n"
        ),
        concat!(
            "class C {}\n",
            "class D extends C {}\n",
            "class E extends D {}\n",
            "class F extends E {}\n",
            "C.foo = 1;\n",
            "var z = 1;\n"
        ),
    );
}

// port: InlinePropertiesTest#testConstClassPropsNonInheritedProp_es6Class
#[test]
fn test_const_class_props_non_inherited_prop_es6_class() {
    let mut t = Test::new();
    // Test that we don't accidentally treat the superclass as having a subclass prop
    t.test_same(concat!(
        "class C {}\n",
        "class D extends C {}\n",
        "D.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testNonConstClassProp_es6ClassWithStaticMethod
#[test]
fn test_non_const_class_prop_es6_class_with_static_method() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C { static foo() {} }\n",
        "alert(C.foo);\n",
        "C.foo = 1;\n"
    ));
}

// port: InlinePropertiesTest#testConstPrototypeProp_es6Class
#[test]
fn test_const_prototype_prop_es6_class() {
    let mut t = Test::new();
    t.test(
        concat!("class C {}\n", "C.prototype.foo = 1;\n", "new C().foo;\n"),
        concat!("class C {}\n", "C.prototype.foo = 1;\n", "new C(), 1;\n"),
    );
}

// port: InlinePropertiesTest#testNonConstPrototypePropFromMemberFn
#[test]
fn test_non_const_prototype_prop_from_member_fn() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class C {\n",
        "  foo() {}\n",
        "}\n",
        "C.prototype.foo = 4;\n",
        "(new C()).foo\n"
    ));
}

// port: InlinePropertiesTest#testObjectPatternStringKeyDoesntInvalidateProp
#[test]
fn test_object_pattern_string_key_doesnt_invalidate_prop() {
    let mut t = Test::new();
    t.test(
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 3;\n",
            "}\n",
            "(new C()).foo\n",
            "const {foo} = new C();\n"
        ),
        concat!(
            "/** @constructor */\n",
            "function C() {\n",
            "  this.foo = 3;\n",
            "}\n",
            "new C(), 3;\n",
            "const {foo} = new C();\n"
        ),
    );
}

// port: InlinePropertiesTest#testNoInlineOnRecordType
#[test]
fn test_no_inline_on_record_type() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @record */\n",
        "class C {}\n",
        "C.bar = 2;\n",
        "C.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testNoInlineOnInterfaceType
#[test]
fn test_no_inline_on_interface_type() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @interface */\n",
        "class C {}\n",
        "C.bar = 2;\n",
        "C.foo = 1;\n",
        "var z = C.foo;\n"
    ));
}

// port: InlinePropertiesTest#testClassField
#[test]
fn test_class_field() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  a = 1;\n",
            "  b;\n",
            "}\n",
            "(new C()).a;\n",
            "(new C()).b\n"
        ),
        concat!(
            "class C {\n",
            "  a = 1;\n",
            "  b;\n",
            "}\n",
            "new C(), 1;\n",
            "(new C()).b\n"
        ),
    );
}

// port: InlinePropertiesTest#testClassFieldWithInheritance
#[test]
fn test_class_field_with_inheritance() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  a = 1;\n",
            "  b;\n",
            "}\n",
            "class D extends C {};\n",
            "(new D()).a;\n",
            "(new D()).b\n"
        ),
        concat!(
            "class C {\n",
            "  a = 1;\n",
            "  b;\n",
            "}\n",
            "class D extends C {};\n",
            "new D(), 1;\n",
            "(new D()).b;\n"
        ),
    );
}

// port: InlinePropertiesTest#testClassField_static
#[test]
fn test_class_field_static() {
    let mut t = Test::new();
    t.test(
        concat!(
            "class C {\n",
            "  static a = 1;\n",
            "  static b;\n",
            "}\n",
            "C.a;\n",
            "C.b\n"
        ),
        concat!(
            "class C {\n",
            "  static a = 1;\n",
            "  static b;\n",
            "}\n",
            "1;\n",
            "C.b\n"
        ),
    );
}

// port: InlinePropertiesTest#testClassComputedField
#[test]
fn test_class_computed_field() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @dict */\n",
        "class C {\n",
        "  ['a'] = 1;\n",
        "  ['b'];\n",
        "}\n",
        "(new C())['a'];\n",
        "(new C())['b']\n"
    ));
}

// port: InlinePropertiesTest#testClassComputedField_static
#[test]
fn test_class_computed_field_static() {
    let mut t = Test::new();
    t.test_same(concat!(
        "/** @dict */\n",
        "class C {\n",
        "  static ['a'] = 1;\n",
        "  static ['b'];\n",
        "}\n",
        "C['a']\n",
        "C['b']\n"
    ));
}

// port: InlinePropertiesTest#testUninitializedSubclassFieldPreventsInlining
#[test]
fn test_uninitialized_subclass_field_prevents_inlining() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Guard {\n",
        "  constructor() { this.escaped = true; }\n",
        "}\n",
        "class RawGuard extends Guard {\n",
        "  escaped;\n",
        "}\n",
        "(new RawGuard()).escaped;\n"
    ));
}

// port: InlinePropertiesTest#testInitializedSubclassFieldPreventsInlining
#[test]
fn test_initialized_subclass_field_prevents_inlining() {
    let mut t = Test::new();
    t.test_same(concat!(
        "class Guard {\n",
        "  constructor() { this.escaped = true; }\n",
        "}\n",
        "class RawGuard extends Guard {\n",
        "  escaped = false;\n",
        "}\n",
        "(new RawGuard()).escaped;\n"
    ));
}
