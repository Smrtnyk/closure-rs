/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckConstPrivatePropertiesTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_const_private_properties::{
    CheckConstPrivateProperties, MISSING_CONST_PROPERTY,
};

// port: CheckConstPrivatePropertiesTest#getProcessor
// port: CheckConstPrivatePropertiesTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckConstPrivateProperties::new(c))).with_lint_checks_warning()
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified1
#[test]
fn test_constructor_prop_modified1() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() {} /** @private */ C.prop = 1;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("/** @constructor */ function C() {} /** @private */ C.prop = 1; C.prop = 2;");
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_es6Class
#[test]
fn test_constructor_prop_modified_es6_class() {
    let t = case();
    t.test_warning(
        "class C {} /** @private */ C.prop = 1;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C {} /** @private */ C.prop = 1; C.prop = 2;");
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_interface
#[test]
fn test_constructor_prop_modified_interface() {
    let t = case();
    t.test_warning(
        "/** @interface */ function C() {} /** @private */ C.prop = 1;",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("/** @interface */ function C() {} /** @private */ C.prop = 1; C.prop = 2;");
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_function
#[test]
fn test_constructor_prop_modified_function() {
    let t = case();
    t.test_same("/** @constructor */ function C() {} /** @private */ C.prop = function() {};");
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_enum
#[test]
fn test_constructor_prop_modified_enum() {
    let t = case();
    t.test_same(
        "/** @constructor */ function C() {} /** @enum {number} @private */ C.prop = { A: 1 };",
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_const
#[test]
fn test_constructor_prop_modified_const() {
    let t = case();
    t.test_same("/** @constructor */ function C() {} /** @private @const */ C.prop = 1;");
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropUnmodified_nonQnameClass
#[test]
fn test_constructor_prop_unmodified_non_qname_class() {
    let t = case();
    t.test_same(
        r#"const obj = {};
/** @constructor */ obj['ctor'] = function () {};
/** @private */ obj['ctor'].prop = 1;
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified2
#[test]
fn test_constructor_prop_modified2() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this.foo = 1; } ",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same(
        "/** @constructor */ function C() { /** @private */ this.foo = 1; this.foo = 2; } ",
    );
    t.test_same(
        r#"/** @constructor */ function C() { /** @private */ this.foo = 1; }
C.prototype.bar = function() { this.foo = 2; }
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_delete
#[test]
fn test_constructor_prop_modified_delete() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this.foo = 1; } ",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same(
        "/** @constructor */ function C() { /** @private */ this.foo = 1; delete this.foo; } ",
    );
    t.test_same(
        r#"/** @constructor */ function C() { /** @private */ this.foo = 1; }
C.prototype.bar = function() { delete this.foo; }
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_arrayIndexGetProp
#[test]
fn test_constructor_prop_modified_array_index_get_prop() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this.foo = [1]; } ",
        &MISSING_CONST_PROPERTY,
    );
    // Even though the underlying data structure is testSame, the reference this['foo'] points to is
    // not modified.
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this.foo = [1]; this.foo[0] = 2; } ",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        r#"/** @constructor */ function C() { /** @private */ this.foo = [1]; }
C.prototype.bar = function() { this.foo[0] = 2; }
"#,
        &MISSING_CONST_PROPERTY,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_objectElement
#[test]
fn test_constructor_prop_modified_object_element() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this['foo'] = 1; } ",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same(
        "/** @constructor */ function C() { /** @private */ this['foo'] = 1; this['foo'] = 2; } ",
    );
    t.test_same(
        r#"/** @constructor */ function C() { /** @private */ this['foo'] = 1; }
C.prototype.bar = function() { this['foo'] = 2; }
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_arrayIndexGetElem
#[test]
fn test_constructor_prop_modified_array_index_get_elem() {
    let t = case();
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this['foo'] = [1]; } ",
        &MISSING_CONST_PROPERTY,
    );
    // Even though the underlying data structure is testSame, the reference this['foo'] points to is
    // not modified.
    t.test_warning(
        "/** @constructor */ function C() { /** @private */ this['f'] = [1]; this['f'][0] = 2; } ",
        &MISSING_CONST_PROPERTY,
    );
    t.test_warning(
        r#"/** @constructor */ function C() { /** @private */ this['foo'] = [1]; }
C.prototype.bar = function() { this['foo'][0] = 2; }
"#,
        &MISSING_CONST_PROPERTY,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_mixedObjectAccess
#[test]
fn test_constructor_prop_modified_mixed_object_access() {
    let t = case();
    t.test_same(
        "/** @constructor */ function C() { /** @private */ this.foo = 1; this['foo'] = 2; } ",
    );
    t.test_same(
        r#"/** @constructor */ function C() { /** @private */ this['foo'] = 1; }
C.prototype.bar = function() { this.foo = 2; }
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testConstructorPropModified_lambda
#[test]
fn test_constructor_prop_modified_lambda() {
    let t = case();
    t.test_same(
        r#"/** @constructor */
function C() {
  /** @private */
  this.foo_ = 2;

  (() => { this.foo_ = 1; })();
}
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified1
#[test]
fn test_class_prop_modified1() {
    let t = case();
    t.test_warning(
        "class C { constructor() { /** @private */ this.a = 2; } }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C { constructor() { /** @private */ this.a = 2; this.a = 3; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; } foo() { this.a = 3; } }");
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_const
#[test]
fn test_class_prop_modified_const() {
    let t = case();
    t.test_same("class C { constructor() { /** @private @const */ this.a = 2; } }");
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_lambda
#[test]
fn test_class_prop_modified_lambda() {
    let t = case();
    t.test_same(
        r#"class C {
  constructor() {
    /** @private */
    this.foo_ = 2;

    (() => { this.foo_ = 1; })();
  }
}
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_assignModify
#[test]
fn test_class_prop_modified_assign_modify() {
    let t = case();
    t.test_warning(
        "class C { constructor() { /** @private */ this.a = 2; } }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C { constructor() { /** @private */ this.a = 2; this.a += 3; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; } foo() { this.a += 3; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; this.a -= 3; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; } foo() { this.a -= 3; } }");
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_increment
#[test]
fn test_class_prop_modified_increment() {
    let t = case();
    t.test_warning(
        "class C { constructor() { /** @private */ this.a = 2; } }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C { constructor() { /** @private */ this.a = 2; this.a++; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; } foo() { this.a++; } }");
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_decrement
#[test]
fn test_class_prop_modified_decrement() {
    let t = case();
    t.test_warning(
        "class C { constructor() { /** @private */ this.a = 2; } }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C { constructor() { /** @private */ this.a = 2; this.a--; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; } foo() { this.a--; } }");
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_delete
#[test]
fn test_class_prop_modified_delete() {
    let t = case();
    t.test_warning(
        "class C { constructor() { /** @private */ this.a = 2; } }",
        &MISSING_CONST_PROPERTY,
    );
    t.test_same("class C { constructor() { /** @private */ this.a = 2; delete this.a; } }");
    t.test_same(
        "class C { constructor() { /** @private */ this.a = 2; } foo() { delete this.a; } }",
    );
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_multiFile
#[test]
fn test_class_prop_modified_multi_file() {
    let t = case();
    t.test_warning_sources(
        &[
            "class A { constructor() { /** @private */ this.a = 2; } }",
            "class B { constructor() { /** @private */ this.a = 1; this.a = 4; } }",
        ],
        &MISSING_CONST_PROPERTY,
    );
    t.test_same_sources(&[
        "class A { constructor() { /** @private */ this.a = 2; this.a = 3; } }",
        "class B { constructor() { /** @private */ this.a = 2; this.a = 4; } }",
    ]);
}

// port: CheckConstPrivatePropertiesTest#testClassPropModified_destructuring
#[test]
fn test_class_prop_modified_destructuring() {
    let t = case();
    t.test_same("class C { constructor() { /** @private */ this.a = 2; [this.a] = []; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; ({a: this.a} = {}); } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; [{a: this.a}] = [{}]; } }");
    t.test_same("class C { constructor() { /** @private */ this.a = 2; [this.a = 0] = []; } }");
}

// port: CheckConstPrivatePropertiesTest#testPrototype_Property
#[test]
fn test_prototype_property() {
    let t = case();
    t.test_same("/** @constructor */ function C() {} /** @private */ C.prototype.prop = 1;");
    t.test_same(
        r#"/** @constructor */ function C() {} /** @private */ C.prototype.prop = 1;
C.prototype.prop = 2;
"#,
    );
}

// port: CheckConstPrivatePropertiesTest#testPrototype_Method
#[test]
fn test_prototype_method() {
    let t = case();
    t.test_same(
        "/** @constructor */ function C() {} /** @private */ C.prototype.method = function() {};",
    );
}

// port: CheckConstPrivatePropertiesTest#testPropertyAsRvalueOfDeclaration
#[test]
fn test_property_as_rvalue_of_declaration() {
    let t = case();
    // Test bug where the rhs of a @private prop declaration was treated like a declaration.
    // See b/110938671
    // In the repro below, there was a spurious warning that Foo.SOME_PROP was an effectively
    // constant private property, and the JSDoc "/** @private {number} */" should be @const.
    t.test_no_warning(
        r#"class Foo {}
class C {
  constructor() {
    /** @private {number} */
    this.nonConstProp_ = Foo.SOME_PROP;
    this.nonConstProp_ = 4; // change this.nonConstProp_ to avoid a warning
  }
}
"#,
    );
}
