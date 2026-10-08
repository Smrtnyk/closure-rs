/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckUnusedPrivatePropertiesTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_unused_private_properties::{
    CheckUnusedPrivateProperties, UNUSED_PRIVATE_PROPERTY,
};
use closure_testing::compiler_test_case::CompilerTestCase;

// port: CheckUnusedPrivatePropertiesTest#EXTERNS
fn externs() -> String {
    CompilerTestCase::default_externs()
        .unwrap_or_else(|e| panic!("{e:?}"))
        .to_string_lossy()
        + r#"/** @const */ goog.reflect = {};
goog.reflect.object;
/** @constructor */
function Window() {}
Window.prototype.x;
Window.prototype.a;
Window.prototype.ext;
/** @type {!Window} */ var window;
function alert(a) {}
var EXT = {};
EXT.ext;
"#
}

// port: CheckUnusedPrivatePropertiesTest#CheckUnusedPrivatePropertiesTest
// port: CheckUnusedPrivatePropertiesTest#setUp
// port: CheckUnusedPrivatePropertiesTest#getProcessor
// port: CheckUnusedPrivatePropertiesTest#getOptions
fn case() -> LintTestCase {
    // enableGatherExternProperties(): the pass reads no extern properties; the fixture only
    // parses the externs.
    let mut t = LintTestCase::new(|c| Box::new(CheckUnusedPrivateProperties::new(c)))
        .with_lint_checks_warning();
    t.externs = externs();
    t
}

// port: CheckUnusedPrivatePropertiesTest#unused
fn unused(t: &LintTestCase, code: &str) {
    t.test_warning(code, &UNUSED_PRIVATE_PROPERTY);
}

// port: CheckUnusedPrivatePropertiesTest#used
fn used(t: &LintTestCase, code: &str) {
    t.test_same(code);
}

// port: CheckUnusedPrivatePropertiesTest#testSimpleUnused1
#[test]
fn test_simple_unused1() {
    let t = case();
    unused(&t, "/** @private */ this.a = 2");
    used(&t, "/** @private */ this.a = 2; alert(this.a);");
}

// port: CheckUnusedPrivatePropertiesTest#testConstructorPropertyUsed1
#[test]
fn test_constructor_property_used1() {
    let t = case();
    unused(
        &t,
        "/** @constructor */ function C() {} /** @private */ C.prop = 1;",
    );
    used(
        &t,
        "/** @constructor */ function C() {} /** @private */ C.prop = 1; alert(C.prop);",
    );
    used(
        &t,
        "/** @constructor */ function C() {} /** @private */ C.prop = 1; function f(a) {a.prop}",
    );
}

// port: CheckUnusedPrivatePropertiesTest#testClassConstructorPropertyUnused1
#[test]
fn test_class_constructor_property_unused1() {
    let t = case();
    // Don't ever warn about unused private constructors
    used(&t, "class C { /** @private */ constructor() {} }; ");
}

// port: CheckUnusedPrivatePropertiesTest#testClassPropUnused1
#[test]
fn test_class_prop_unused1() {
    let t = case();
    // A property defined on "this" can be removed
    unused(
        &t,
        "class C { constructor() { /** @private */ this.a = 2 } }",
    );
}

// port: CheckUnusedPrivatePropertiesTest#testClassPropUnused2
#[test]
fn test_class_prop_unused2() {
    let t = case();
    unused(
        &t,
        "/** @constructor */ function C() { /** @private */ this.prop = 1; }",
    );
    used(
        &t,
        "/** @constructor */ function C() {/** @private */ this.prop = 1;} alert(some.prop);",
    );
}

// port: CheckUnusedPrivatePropertiesTest#testClassMethodUnused1
#[test]
fn test_class_method_unused1() {
    let t = case();
    unused(
        &t,
        "class C { constructor() {}  /** @private */ method() {} }",
    );
    used(
        &t,
        r#"class C { constructor() {}  /** @private */ method() {} }
 new C().method();"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testClassQuotedMethodUnused_noWarning
#[test]
fn test_class_quoted_method_unused_no_warning() {
    let t = case();
    used(
        &t,
        r#"class C { constructor() {}
  /** @private */ ['method']() {} }"#,
    );
    used(
        &t,
        r#"class C { constructor() {}
  /** @private */ ['method']() {} }
 new C()['method']();"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testSimple2
#[test]
fn test_simple2() {
    let t = case();
    // A property defined on "this" can be removed, even when defined
    // as part of an expression
    unused(&t, "/** @private */ this.a = 1; this.a = 2, f()");
    unused(&t, "/** @private */ this.a = 1; x = (this.a = 2, f())");
    unused(&t, "/** @private */ this.a = 1; x = (f(), this.a = 2)");
}

// port: CheckUnusedPrivatePropertiesTest#testSimple3
#[test]
fn test_simple3() {
    let t = case();
    // A property defined on an object other than "this" can not be removed.
    used(&t, "y.a = 2");
    // and is seen as a use.
    used(&t, "y.a = 2; /** @private */ this.a = 2");
    // Some use of the property "a" appears as a use.
    used(&t, "y.a = 2; /** @private */ this.a = 1; alert(x.a)");
}

// port: CheckUnusedPrivatePropertiesTest#testObjLit
#[test]
fn test_obj_lit() {
    let t = case();
    // A property defined on an object other than "this" is considered a use.
    used(&t, "({a:2})");
    // and is seen as a use on 'this'.
    used(&t, "({a:0}); /** @private */ this.a = 1;");
    // Some use of the property "a" anywhere is considered a use
    used(&t, "x = ({a:0}); /** @private */ this.a = 1; alert(x.a)");
}

// port: CheckUnusedPrivatePropertiesTest#testExtern
#[test]
fn test_extern() {
    let t = case();
    // A property defined in the externs and isn't a warning.
    t.test_same("this.ext = 2");
}

// port: CheckUnusedPrivatePropertiesTest#testExport
#[test]
fn test_export() {
    let t = case();
    // An exported property can not be removed.
    t.test_same("this.ext = 2; window['export'] = this.ext;");
    t.test_same("function f() { this.ext = 2; } window['export'] = this.ext;");
}

// port: CheckUnusedPrivatePropertiesTest#testAssignOp1
#[test]
fn test_assign_op1() {
    let t = case();
    // Properties defined using a compound assignment can be removed if the
    // result of the assignment expression is not immediately used.
    unused(&t, "/** @private */ this.x; this.x += 2");
    used(&t, "/** @private */ this.x; x = (this.x += 2)");
    used(&t, "/** @private */ this.x; this.x += 2; x = this.x;");
    // But, of course, a later use prevents its removal.
    used(&t, "/** @private */ this.x; this.x += 2; x.x;");
}

// port: CheckUnusedPrivatePropertiesTest#testAssignOp2
#[test]
fn test_assign_op2() {
    let t = case();
    // Properties defined using a compound assignment can be removed if the
    // result of the assignment expression is not immediately used.
    unused(&t, "/** @private */ this.a; this.a += 2, f()");
    unused(&t, "/** @private */ this.a; x = (this.a += 2, f())");
    used(&t, "/** @private */ this.a; x = (f(), this.a += 2)");
}

// port: CheckUnusedPrivatePropertiesTest#testInc1
#[test]
fn test_inc1() {
    let t = case();
    // Increments and Decrements are handled similarly to compound assignments
    // but need a placeholder value when replaced.
    unused(&t, "/** @private */ this.x; this.x++");
    used(&t, "/** @private */ this.x; x = (this.x++)");
    used(&t, "/** @private */ this.x; this.x++; x = this.x;");
    unused(&t, "/** @private */ this.x; --this.x");
    used(&t, "/** @private */ this.x; x = (--this.x)");
    used(&t, "/** @private */ this.x; --this.x; x = this.x;");
}

// port: CheckUnusedPrivatePropertiesTest#testInc2
#[test]
fn test_inc2() {
    let t = case();
    // Increments and Decrements are handled similarly to compound assignments
    unused(&t, "/** @private */ this.a; this.a++, f()");
    unused(&t, "/** @private */ this.a;x = (this.a++, f())");
    used(&t, "/** @private */ this.a;x = (f(), this.a++)");
    unused(&t, "/** @private */ this.a; --this.a, f()");
    unused(&t, "/** @private */ this.a; x = (--this.a, f())");
    used(&t, "/** @private */ this.a; x = (f(), --this.a)");
}

// port: CheckUnusedPrivatePropertiesTest#testJSCompiler_renameProperty
#[test]
fn test_js_compiler_rename_property() {
    let t = case();
    // JSCompiler_renameProperty introduces a use of the property
    used(
        &t,
        "/** @private */ this.a = 2; x[JSCompiler_renameProperty('a')]",
    );
    used(
        &t,
        "/** @private */ this.a = 2; JSCompiler_renameProperty('a')",
    );
}

// port: CheckUnusedPrivatePropertiesTest#testForIn
#[test]
fn test_for_in() {
    let t = case();
    // This is the basic assumption that this pass makes:
    // that it can warn even if it is used indirectly in a for-in loop
    unused(
        &t,
        r#"/** @constructor */ var X = function() {
  /** @private */ this.y = 1;
}
for (var a in new X()) { alert(x[a]) }
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testObjectReflection1
#[test]
fn test_object_reflection1() {
    let t = case();
    // Verify reflection prevents warning.
    used(
        &t,
        r#"/** @constructor */ function A() {/** @private */ this.foo = 1;}
use(goog.reflect.object(A, {foo: 'foo'}));
"#,
    );
    // Verify reflection prevents warning.
    used(
        &t,
        r#"/** @const */ var $jscomp = {};
/** @const */ $jscomp.scope = {};
/**
 * @param {!Function} type
 * @param {Object} object
 * @return {Object}
 */
$jscomp.reflectObject = function (type, object) { return object; };
/** @constructor */ function A() {/** @private */ this.foo = 1;}
use($jscomp.reflectObject(A, {foo: 'foo'}));
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testObjectReflection2
#[test]
fn test_object_reflection2() {
    let t = case();
    // Any object literal definition prevents warning.
    used(
        &t,
        r#"/** @constructor */ function A() {/** @private */  this.foo = 1;}
use({foo: 'foo'});
"#,
    );
    // member functions prevent renaming (since we allow them in goog.reflect.object)
    used(
        &t,
        r#"/** @constructor */ function A() {/** @private */  this.foo = 1;}
use({foo() {}});
"#,
    );
    // computed property doesn't prevent warning
    unused(
        &t,
        r#"/** @constructor */ function A() {/** @private */  this.foo = 1;}
use({['foo']: 'foo'});
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testPrototypeProps1
#[test]
fn test_prototype_props1() {
    let t = case();
    unused(
        &t,
        r#"/** @constructor */ function A() {this.foo = 1;}
/** @private */ A.prototype.foo = 0;
A.prototype.method = function() {this.foo++};
new A().method()
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testPrototypeProps2
#[test]
fn test_prototype_props2() {
    let t = case();
    // warn about all private properties
    unused(
        &t,
        r#"/** @constructor */ function A() {this._foo = 1;}
/** @private */ A.prototype._foo = 0;
A.prototype.method = function() {this._foo++};
new A().method()
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testTypedef
#[test]
fn test_typedef() {
    let t = case();
    used(
        &t,
        r#"/** @constructor */ function A() {}
/** @private @typedef {string} */ A.typedef_;
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testInterface
#[test]
fn test_interface() {
    let t = case();
    used(
        &t,
        r#"/** @constructor */ function A() {}
/**
 * @interface
 * @private
 */
A.Interface = function() {};
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testConstructorProperty1
#[test]
fn test_constructor_property1() {
    let t = case();
    unused(
        &t,
        r#"/** @constructor */
function C() {}
/** @private */ C.prop = 1;
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testConstructorProperty2
#[test]
fn test_constructor_property2() {
    let t = case();
    used(
        &t,
        r#"/** @constructor */
function C() {}
/** @private */ C.prop = 1;
function use(a) {
  alert(a.prop)
};
use(C)
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testInterfaceProperty
#[test]
fn test_interface_property() {
    let t = case();
    unused(
        &t,
        r#"/** @interface */
function C() {}
/** @private */ C.prop = 1;
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testConstructorProperty_nonQnameClass
#[test]
fn test_constructor_property_non_qname_class() {
    let t = case();
    t.test_same(
        r#"const obj = {};
/** @constructor */ obj['ctor'] = function () {};
/** @private */ obj['ctor'].prop = 1;
"#,
    );
}

// port: CheckUnusedPrivatePropertiesTest#testEs6ClassProperty
#[test]
fn test_es6_class_property() {
    let t = case();
    unused(&t, "class C {} /** @private */ C.prop = 1;");
}
