/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/RemoveUnusedCodeClassPropertiesTest.java.

//! Port of RemoveUnusedCodeClassPropertiesTest.java: tests for `RemoveUnusedCode` that cover
//! removal of instance properties and properties defined directly on constructors. The tests run
//! on the native CompilerTestCase port of crates/testing. 15 tests call `enableTypeCheck()` (real
//! TypeCheck, type-check).
use closure_jscomp::{compiler_pass::CompilerPass, remove_unused_code::RemoveUnusedCode};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    jscomp_api::Compiler,
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
        replay_values::object,
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

// port: RemoveUnusedCodeClassPropertiesTest#EXTERNS
const EXTERNS: &str = r#"/**
 * @constructor
 * @param {*=} opt_value
 * @return {!Object}
 */
function Object(opt_value) {}
/**
 * @constructor
 * @param {...*} var_args
 */
function Function(var_args) {}
/**
 * @constructor
 * @param {*=} arg
 * @return {string}
 */
function String(arg) {}
/**
 * @record
 * @template VALUE
 */
/**
 * @template T
 * @constructor
 * @param {...*} var_args
 * @return {!Array<?>}
 */
function Array(var_args) {}
var window;
function alert(a) {}
function use(x) {}
var EXT = {};
EXT.ext;
var externVar;
function externFunction() {}
/** @type {Function} */
Object.defineProperties = function() {};
/** @type {Function} */
Object.prototype.constructor = function() {};
// NOTE: The following are needed to prevent NTI inexistent property warnings.
var $jscomp = {};
$jscomp.global = {}
/** @type {?} */
$jscomp.global.Object
function JSCompiler_renameProperty(p) {}
var goog = {};
goog.reflect = {};
goog.reflect.object = function(a, b) {};
"#;

struct RemoveUnusedCodeClassPropertiesTest {
    ctx: Ctx,
    keep_locals: bool,
    keep_globals: bool,
}

impl CompilerTestCaseHooks for RemoveUnusedCodeClassPropertiesTest {
    // port: RemoveUnusedCodeClassPropertiesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let keep_locals = self.keep_locals;
        let keep_globals = self.keep_globals;
        Ok(DslValue::Pass(Rc::new(RefCell::new(Box::new(
            move |compiler: &mut Compiler, externs: NodeId, root: NodeId| {
                RemoveUnusedCode::builder(compiler)
                    .remove_local_vars(!keep_locals)
                    .remove_globals(!keep_globals)
                    .remove_unused_prototype_properties(true)
                    .remove_unused_this_properties(true)
                    .remove_unused_object_define_properties_definitions(true)
                    .build()
                    .process(compiler, externs, root);
            },
        )))))
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

/// The JUnit instance: the CompilerTestCase harness and this test's fields.
struct Fixture {
    h: CompilerTestCase,
    t: RemoveUnusedCodeClassPropertiesTest,
}

impl Fixture {
    // port: RemoveUnusedCodeClassPropertiesTest#RemoveUnusedCodeClassPropertiesTest
    // port: RemoveUnusedCodeClassPropertiesTest#setUp
    fn new() -> Self {
        let mut h = CompilerTestCase::new(EXTERNS);
        h.set_up();
        // Allow testing of features that aren't fully supported for output yet.
        h.enable_normalize().unwrap();
        // TODO(bradfordcsmith): Stop normalizing the expected output or document why it is necessary.
        h.enable_normalize_expected_output().unwrap();
        h.disable_compare_js_doc().unwrap();
        h.enable_gather_extern_properties().unwrap();
        h.disable_type_check().unwrap();
        let t = RemoveUnusedCodeClassPropertiesTest {
            ctx: Ctx::new(
                "RemoveUnusedCodeClassPropertiesTest".into(),
                object([]),
                IndexMap::<_, _>::default(),
                Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                    .unwrap(),
            ),
            keep_locals: true,
            keep_globals: true,
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
}

// port: RemoveUnusedCodeClassPropertiesTest#testSimple1
#[test]
fn test_simple1() {
    let mut f = Fixture::new();
    // A property defined on "this" can be removed
    f.test("this.a = 2", "");
    f.test("let x = (this.a = 2)", "let x = 2");
    f.test_same("this.a = 2; let x = this.a;");
}

// port: RemoveUnusedCodeClassPropertiesTest#testSimple2
#[test]
fn test_simple2() {
    let mut f = Fixture::new();
    // A property defined on "this" can be removed, even when defined
    // as part of an expression
    f.test("this.a = 2, alert(1);", "alert(1);");
    f.test("const x = (this.a = 2, alert(1));", "const x = alert(1);");
    f.test(
        "const x = (alert(1), this.a = 2);",
        "const x = (alert(1), 2);",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testSimple3
#[test]
fn test_simple3() {
    let mut f = Fixture::new();
    // A property defined on an object other than "this" can not be removed.
    f.test_same("var y = {}; y.a = 2");
    // and prevents the removal of the definition on 'this'.
    f.test_same("var y = {}; y.a = 2; this.a = 2");
    // Some use of the property "a" prevents the removal.
    f.test_same("var x; var y = {}; y.a = 2; this.a = 1; alert(x.a)");
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjLit
#[test]
fn test_obj_lit() {
    let mut f = Fixture::new();
    // A property defined on an object other than "this" can not be removed.
    f.test_same("({a:2})");
    // and prevent the removal of the definition on 'this'.
    f.test_same("({a:0}); this.a = 1;");
    // ... even if it's quoted
    f.test_same("({'a':0}); this.a = 1;");
    // Some use of the property "a" prevents the removal.
    f.test_same("var x = ({a:0}); this.a = 1; alert(x.a)");
}

// port: RemoveUnusedCodeClassPropertiesTest#testExtern
#[test]
fn test_extern() {
    let mut f = Fixture::new();
    // A property defined in the externs is can not be removed.
    f.test_same("this.ext = 2");
}

// port: RemoveUnusedCodeClassPropertiesTest#testExport
#[test]
fn test_export() {
    let mut f = Fixture::new();
    // An exported property can not be removed.
    f.test_same("this.ext = 2; window['export'] = this.ext;");
    f.test_same("function f() { this.ext = 2; } window['export'] = this.ext;");
}

// port: RemoveUnusedCodeClassPropertiesTest#testAssignOp1
#[test]
fn test_assign_op1() {
    let mut f = Fixture::new();
    // Properties defined using a compound assignment can be removed if the
    // result of the assignment expression is not immediately used.
    f.test("this.x += 2", "");
    f.test_same("const x = (this.x += 2)");
    f.test_same("this.x += 2; const x = this.x;");
    // But, of course, a later use prevents its removal.
    f.test_same("this.x += 2; let x = {}; x.x;");
}

// port: RemoveUnusedCodeClassPropertiesTest#testAssignOp2
#[test]
fn test_assign_op2() {
    let mut f = Fixture::new();
    // Properties defined using a compound assignment can be removed if the
    // result of the assignment expression is not immediately used.
    f.test("this.a += 2, alert(1)", "alert(1)");
    f.test("const x = (this.a += 2, alert(1))", "const x = alert(1)");
    f.test_same("const x = (alert(1), this.a += 2)");
}

// port: RemoveUnusedCodeClassPropertiesTest#testInc1
#[test]
fn test_inc1() {
    let mut f = Fixture::new();
    // Increments and Decrements are handled similarly to compound assignments
    // but need a placeholder value when replaced.
    f.test("this.x++", "");
    f.test_same("let x = (this.x++)");
    f.test_same("this.x++; let x = this.x;");
    f.test("--this.x", "");
    f.test_same("let x = (--this.x)");
    f.test_same("--this.x; let x = this.x;");
}

// port: RemoveUnusedCodeClassPropertiesTest#testInc2
#[test]
fn test_inc2() {
    let mut f = Fixture::new();
    // Increments and Decrements are handled similarly to compound assignments
    // but need a placeholder value when replaced.
    f.test("this.a++, alert()", "alert()");
    f.test("let x = (this.a++, alert())", "let x = alert()");
    f.test_same("let x = (alert(), this.a++)");
    f.test("--this.a, alert()", "alert()");
    f.test("let x = (--this.a, alert())", "let x = alert()");
    f.test_same("let x = (alert(), --this.a)");
}

// port: RemoveUnusedCodeClassPropertiesTest#testDestructuringRest
#[test]
fn test_destructuring_rest() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function Foo() {}
Foo.a = function() {};
({ ...Foo.a.b } = 0);
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testExprResult
#[test]
fn test_expr_result() {
    let mut f = Fixture::new();
    f.test("this.x", "");
    f.test("externFunction().prototype.x", "externFunction()");
    // It doesn't make much sense to use optional chaining in these cases, but if you do,
    // it shouldn't prevent unused property removal
    f.test("this?.x", "");
    f.test("externFunction()?.prototype.x", "externFunction()");
}

// port: RemoveUnusedCodeClassPropertiesTest#testJSCompiler_renameProperty
#[test]
fn test_js_compiler_rename_property() {
    let mut f = Fixture::new();
    // JSCompiler_renameProperty introduces a use of the property
    f.test_same("var x; this.a = 2; x[JSCompiler_renameProperty('a')]");
    f.test_same("this.a = 2; JSCompiler_renameProperty('a')");
}

// port: RemoveUnusedCodeClassPropertiesTest#testForIn
#[test]
fn test_for_in() {
    let mut f = Fixture::new();
    // This is the basic assumption that this pass makes:
    // it can remove properties even when the object is used in a FOR-IN loop
    f.test(
        "let x = {}; this.y = 1;for (var a in x) { alert(x[a]) }",
        "let x = {};            for (var a in x) { alert(x[a]) }",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectKeys
#[test]
fn test_object_keys() {
    let mut f = Fixture::new();
    // This is the basic assumption that this pass makes:
    // it can remove properties even when the object are referenced
    // preserve format
    f.test(
        "this.y = 1;alert(Object.keys(this))",
        "           alert(Object.keys(this))",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectReflection1
#[test]
fn test_object_reflection1() {
    let mut f = Fixture::new();
    // Verify reflection prevents removal.
    f.test_same(
        r#"/** @constructor */ // preserve newlines
function A() { this.foo = 1; }
use(goog.reflect.object(A, {foo: 'foo'}));
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectReflection2
#[test]
fn test_object_reflection2() {
    let mut f = Fixture::new();
    // Any object literal definition prevents removal.
    // Type based removal would allow this to be removed.
    f.test_same(
        r#"/** @constructor */ // preserve newlines
function A() {this.foo = 1;}
use({foo: 'foo'});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testIssue730
#[test]
fn test_issue730() {
    let mut f = Fixture::new();
    // Partial removal of properties can causes problems if the object is
    // sealed.
    f.test_same(
        r#"function A() {this.foo = 0;}
function B() {this.a = new A();}
B.prototype.dostuff = function() { this.a.foo++; alert('hi'); }
new B().dostuff();
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testPrototypeProps1
#[test]
fn test_prototype_props1() {
    let mut f = Fixture::new();
    f.test(
        r#"function A() {this.foo = 1;}
A.prototype.foo = 0;
A.prototype.method = function() {this.foo++};
new A().method()
"#,
        r#"function A() {             }

A.prototype.method = function() {          };
new A().method()
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testPrototypeProps2
#[test]
fn test_prototype_props2() {
    let mut f = Fixture::new();
    // don't remove properties that are exported by convention
    f.test_same(
        r#"function A() {this._foo = 1;}
A.prototype._foo = 0;
A.prototype.method = function() {this._foo++};
new A().method()
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testES6StaticProperty
#[test]
fn test_es6_static_property() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "class C { static prop() {} }",
        "class C {                  }",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testES6StaticProperty2
#[test]
fn test_es6_static_property2() {
    let mut f = Fixture::new();
    f.test("class C {} C.prop = 1;", "class C {}");
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6GettersWithoutTranspilation
#[test]
fn test_es6_getters_without_transpilation() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "class C { get value() { return 0; } }",
        "class C {                           }",
    );
    f.test_same("class C { get value() { return 0; } } const x = (new C()).value");
}

// port: RemoveUnusedCodeClassPropertiesTest#testES6ClassComputedProperty
#[test]
fn test_es6_class_computed_property() {
    let mut f = Fixture::new();
    f.test_same("class C { ['test' + 3]() { return 0; } }");
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6SettersWithoutTranspilation
#[test]
fn test_es6_setters_without_transpilation() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "class C { set value(val) { this.internalVal = val; } }",
        "class C {                                            }",
    );
    f.test(
        "class C { set value(val) { this.internalVal = val; } } (new C()).value = 3;",
        "class C { set value(val) {                         } } (new C()).value = 3;",
    );
    f.test_same(
        r#"class C {
  set value(val) {
    this.internalVal = val;
  }
  get value() {
    return this.internalVal;
  }
}
const y = new C();
y.value = 3;
const x = y.value;
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6EnhancedObjLiteralsComputedValuesNotRemoved
#[test]
fn test_es6_enhanced_obj_literals_computed_values_not_removed() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function getCar(make, model, value) {
  return {
    ['make' + make] : true
  };
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6EnhancedObjLiteralsMethodShortHandNotRemoved
#[test]
fn test_es6_enhanced_obj_literals_method_short_hand_not_removed() {
    let mut f = Fixture::new();
    f.test_same(
        r#"function getCar(make, model, value) {
  return {
    getModel() {
      return model;
    }
  };
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6EnhancedObjLiteralsPropertyShorthand
#[test]
fn test_es6_enhanced_obj_literals_property_shorthand() {
    let mut f = Fixture::new();
    f.test_same("function getCar(make, model, value) { return {model}; }");
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6ArrowFunction
#[test]
fn test_es6_arrow_function() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "const arrow = () => this.a = 1;",
        "const arrow = () =>          1;",
    );
    f.test_same("const arrow = () => ({a: 2})");
    f.test_same("var y = {}; const arrow = () => {y.a = 2; this.a = 2;}");
    f.test(
        r#"function A() {
  this.foo = 1;
}
A.prototype.foo = 0;
A.prototype.getIncr = function() {
  return () => { this.foo++; };
};
new A().getIncr()
"#,
        r#"function A() {

}

A.prototype.getIncr = function() {
  return () => {             };
};
new A().getIncr()
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6Generator
#[test]
fn test_es6_generator() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "function* gen() { yield this.a = 1; }",
        "function* gen() { yield          1; }",
    );
    f.test_same("function* gen() { yield this.a = 1; yield this.a; }");
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6Destructuring
#[test]
fn test_es6_destructuring() {
    let mut f = Fixture::new();
    // Test normal destructuring removal
    // preserve newline
    f.test("[this.x, this.y] = [1, 2]", "[              ] = [1, 2]");
    // Test normal destructuring, assignment prevent removal
    f.test(
        r#"[this.x, this.y] = [1, 2] // preserve newline
var p = this.x;
"#,
        r#"[this.x        ] = [1, 2] // preserve newline
var p = this.x;
"#,
    );
    // Test rest destructuring, `this` property
    // preserve newline
    f.test(
        "[this.x, ...this.z] = [1, 2, 3]",
        "[      , ...this.z] = [1, 2, 3]",
    );
    // Test rest destructuring with normal variable
    // preserve newline
    f.test(
        "let z; [this.x, ...z] = [1, 2]",
        "let z; [      , ...z] = [1, 2]",
    );
    // Test rest destructuring, assignment prevent removal
    f.test(
        r#"[this.x, ...this.y] = [1, 2]; // preserve newline
var p = this.y;
"#,
        r#"[      , ...this.y] = [1, 2]; // preserve newline
var p = this.y;
"#,
    );
    // Test destructuring rhs prevent removal
    f.test_same(
        r#"let a;
this.x = 1; // preserve newline
this.y = 2;
[...a] = [this.x, this.y];
"#,
    );
    // Test nested destructuring
    // preserve newline
    f.test(
        "let z; [this.x, [this.y, ...z]] = [1, [2]]",
        "let z; [      , [      , ...z]] = [1, [2]]",
    );
    // Test normal object destructuring full removal
    f.test(
        "({a: this.x, b: this.y} = {a: 1, b: 2})",
        "({} = {a: 1, b: 2})",
    );
    // Test normal object destructuring partial removal
    f.test(
        "let y; ({a: this.x, b: y} = {a: 1, b: 2})",
        "let y; ({           b: y} = {a: 1, b: 2})",
    );
    // Test obj destructuring prevent removal
    f.test(
        r#"({a: this.x, b: this.y} = {a: 1, b: 2});
var p = this.x;
"#,
        r#"({a: this.x} = {a: 1, b: 2});
var p = this.x;
"#,
    );
    // Test obj destructuring with old style class
    f.test_same(
        r#"/** @constructor */ function C () {
  this.a = 1;
}
let x;
({a: x} = new C());
"#,
    );
    // Test obj destructuring with new style class
    f.test_same(
        r#"class C {
  constructor() {
     this.a = 1;
  }
}
let x;
({a: x} = new C());
"#,
    );
    // Test let destructuring
    f.test_same(
        r#"class C {
  constructor() {
     this.a = 1;
  }
}
let {a: x} = new C();
"#,
    );
    // Test obj created at a different location and later used in destructuring
    f.test_same(
        r#"class C {
  constructor() {
     this.a = 1;
  }
}
var obj = new C()
let x;
({a: x} = obj);
"#,
    );
    // Test obj destructuring with default value
    f.test_same(
        r#"class C {
  constructor() {
     this.a = 1;
  }
}
let a;
({a = 2} = new C());
"#,
    );
    // Test obj nested destructuring
    f.test_same(
        r#"class C {
  constructor() {
     this.a = 1;
  }
}
var obj = new C()
let a;
({x: {a}} = {x: obj});
"#,
    );
    // Computed Property string expression doesn't prevent removal.
    // preserve newline
    f.test("({['a']:0}); this.a = 1;", "({['a']:0});            ");
}

// port: RemoveUnusedCodeClassPropertiesTest#testDestrucuturing_assginmentToProperty_consideredUse
#[test]
fn test_destrucuturing_assginment_to_property_considered_use() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class Foo {
  constructor() {
    this.x = 0;
  }
}

({a: new Foo().x} = {a: 0});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs6DefaultParameter
#[test]
fn test_es6_default_parameter() {
    let mut f = Fixture::new();
    // preserve newline
    f.test(
        "function foo(x, y = this.a = 1) {}",
        "function foo(x, y =          1) {}",
    );
    f.test_same("this.a = 1; function foo(x, y = this.a) {}");
}

// port: RemoveUnusedCodeClassPropertiesTest#testEs8AsyncFunction
#[test]
fn test_es8_async_function() {
    let mut f = Fixture::new();
    f.test(
        r#"async function foo(promise) { // preserve newlines
   this.x = 1;
   return await promise;
}
"#,
        r#"async function foo(promise) { // preserve newlines

   return await promise;
}
"#,
    );
    f.test_same(
        r#"async function foo() {
   this.x = 1;
   return await this.x;
}
"#,
    );
    f.test_same(
        r#"this.x = 1;
async function foo() {
   return await this.x;
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testField
#[test]
fn test_field() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  x = 1;
  y;
  z = 'hi';
  static x = 1;
  static y;
  static z = 'hi';
}
"#,
        r#"class C {

}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testStaticFieldReferencingClassName
#[test]
fn test_static_field_referencing_class_name() {
    let mut f = Fixture::new();
    f.t.keep_globals = false;
    f.t.keep_locals = false;
    f.test(
        r#"const C = class {
  constructor() {
  }
  static x = new C();
  static method() {
    return C.x;
  }
}
"#,
        r#"const C = class {
  constructor() {
  }
  static x = new C();
}
"#,
    );
    f.test(
        r#"const C = class {
  constructor() {
  }
  static x = new C();
  static y = alert();
  static method() {
    return C.x;
  }
}
"#,
        r#"const C = class {
  constructor() {
  }
  static x = new C();
  static y = alert();
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testComputedField
#[test]
fn test_computed_field() {
    let mut f = Fixture::new();
    f.test_same(
        r#"class C {
  ['x'] = 1;
  'y';
  1 = 'hi';
  static ['x'] = 1;
  static 'y';
  static 1 = 'hi';
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testMixedField
#[test]
fn test_mixed_field() {
    let mut f = Fixture::new();
    // Computed properties cannot be removed, so only non-computed properties are removed
    f.test(
        r#"class C {
  x = 1;
  y;
  z = 'hi';
  static x = 1;
  static y;
  static z = 'hi';
  ['x'] = 1;
  'y';
  1 = 'hi';
  static ['x'] = 1;
  static 'y';
  static 1 = 'hi';
}
"#,
        r#"class C {
  ['x'] = 1;
  'y';
  1 = 'hi';
  static ['x'] = 1;
  static 'y';
  static 1 = 'hi';
}
"#,
    );
    f.test_same(
        r#"class C {
  [alert()] = 5;
}
"#,
    );
    f.test_same(
        r#"class C {
  static x = alert();
}
"#,
    );
    f.test_same(
        r#"class C {
  x = alert();
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testPureOrBreakMyCode
#[test]
fn test_pure_or_break_my_code() {
    let mut f = Fixture::new();
    f.test(
        r#"class C {
  constructor() {
   /** @const */this.used = /** @pureOrBreakMyCode */(alert());
   /** @const */this.unused = /** @pureOrBreakMyCode */(alert());
  }
};
/** @const */C.used = /** @pureOrBreakMyCode */(alert());
/** @const */C.unused = /** @pureOrBreakMyCode */(alert());
function foo() {
  return C.used;
}
foo();
function bar() {
  return new C().used;
}
bar();
"#,
        r#"class C {
  constructor() {
    /** @const */this.used = /** @pureOrBreakMyCode */(alert());
  }
};
/** @const */C.used = /** @pureOrBreakMyCode */(alert());
function foo() {
  return C.used;
}
foo();
function bar() {
  return new C().used;
}
bar();
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testUnusedCodeInStaticInitializationBlock
#[test]
fn test_unused_code_in_static_initialization_block() {
    let mut f = Fixture::new();
    f.t.keep_locals = false;
    f.test(
        r#"class C {
  static {
    let x = 1;
  }
}
"#,
        r#"class C {
  static {
  }
}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testClassReferencedInFieldInitializer
#[test]
fn test_class_referenced_in_field_initializer() {
    let mut f = Fixture::new();
    f.t.keep_locals = false;
    f.t.keep_globals = false;
    f.test_same(
        r#"class One {
  field = 1;
}
class Two {
  field = new One();
}
const two = new Two();
alert(two);
alert(two.field);
alert(two.field.field);
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testConstructorProperty1
#[test]
fn test_constructor_property1() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        "/** @constructor */ function C() {} C.prop = 1;",
        "/** @constructor */ function C() {}            ",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testConstructorProperty2
#[test]
fn test_constructor_property2() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test_same(
        r#"/** @constructor */ function C() {}
C.prop = 1;
function foo(a) { alert(a.prop) };
foo(C)
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties1
#[test]
fn test_object_define_properties1() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test_same(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{value:1}});
function foo(a) { alert(a.prop) };
foo(C)
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties2
#[test]
fn test_object_define_properties2() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{value:1}});
"#,
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties3
#[test]
fn test_object_define_properties3() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C,
  {prop:{
    get:function(){},
    set:function(a){},
}});
"#,
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties4
#[test]
fn test_object_define_properties4() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() { this.prop = 3; }
Object.defineProperties(C, {prop:alert('')});
"#,
        r#"/** @constructor */ function C() {                }
Object.defineProperties(C, {prop:alert('')});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties5
#[test]
fn test_object_define_properties5() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test_same(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {'prop': {value: 1}});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties6
#[test]
fn test_object_define_properties6() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // an unknown destination object doesn't prevent removal.
    f.test(
        "Object.defineProperties(externVar(), {prop:{value:1}});",
        "Object.defineProperties(externVar(), {              });",
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties7
#[test]
fn test_object_define_properties7() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{get:function () {return new C}}});
"#,
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties8
#[test]
fn test_object_define_properties8() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{set:function (a) {return alert(a)}}});
"#,
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefinePropertiesQuotesPreventRemoval
#[test]
fn test_object_define_properties_quotes_prevent_removal() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test_same(
        r#"/** @constructor */ function C() { this.prop = 1; }
Object.defineProperties(C, {'prop':{set:function (a) {return alert(a.prop)}}});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testObjectDefineProperties_usedSetter_notRemoved
#[test]
fn test_object_define_properties_used_setter_not_removed() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test_same(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{set:function (a) {alert(2)}}});
C.prop = 2;
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testPrototypeMethodDef_notConsideredSetterUse
#[test]
fn test_prototype_method_def_not_considered_setter_use() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    f.test(
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {prop:{set:function (a) {alert(2)}}});
/** @constructor */ function D () {}
D.prototype.prop = function() {};
"#,
        r#"/** @constructor */ function C() {}
Object.defineProperties(C, {});
/** @constructor */ function D () {}
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testTranspiledEs6GettersRemoval
#[test]
fn test_transpiled_es6_getters_removal() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // This is the output of ES6->ES5 class getter converter.
    // See Es6TranspilationIntegrationTest.testEs5GettersAndSettersClasses test method.
    f.test(
        r#"/** @constructor @struct */
var C = function() {};
/** @type {?} */
C.prototype.value = 0;
$jscomp.global.Object.defineProperties(C.prototype, {
  value: {
    configurable: true,
    enumerable: true,
    /** @this {C} */
    get: function() {
      return 0;
    }
  }
});
"#,
        r#"/** @constructor @struct */var C=function(){};
$jscomp.global.Object.defineProperties(C.prototype, {});
"#,
    );
}

// port: RemoveUnusedCodeClassPropertiesTest#testTranspiledEs6SettersRemoval
#[test]
fn test_transpiled_es6_setters_removal() {
    let mut f = Fixture::new();
    f.h.enable_type_check().unwrap();
    // This is the output of ES6->ES5 class setter converter.
    // See Es6TranspilationIntegrationTest.testEs5GettersAndSettersClasses test method.
    f.test(
        r#"/** @constructor @struct */
var C = function() {};
/** @type {?} */
C.prototype.value;
/** @type {?} */
C.prototype.internalVal;
$jscomp.global.Object.defineProperties(C.prototype, {
  value: {
    configurable: true,
    enumerable: true,
    /** @this {C} */
    set: function(val) {
      this.internalVal = val;
    }
  }
});
"#,
        r#"/** @constructor @struct */var C=function(){};
$jscomp.global.Object.defineProperties(C.prototype, {});
"#,
    );
}
