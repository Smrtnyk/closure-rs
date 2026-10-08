/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckInterfacesTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_interfaces::{
    CheckInterfaces, INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY,
    INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS, INTERFACE_DEFINED_WITH_EXTENDS,
    MISSING_JSDOC_IN_DECLARATION_STATEMENT, NON_DECLARATION_STATEMENT_IN_INTERFACE,
    STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS,
};

// port: CheckInterfacesTest#getProcessor
// port: CheckInterfacesTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckInterfaces::new(c))).with_lint_checks_warning()
}

// port: CheckInterfacesTest#testInterfaceArgs
#[test]
fn test_interface_args() {
    let t = case();
    t.test_same_warning(
        "/** @interface */ function A(x) {}",
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
    t.test_same_warning(
        "/** @interface */ class C { constructor(x) {} }",
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
    t.test_same_warning(
        r#"var ns = {};

/** @interface */

ns.SomeInterface = function(x) {};
"#,
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
    t.test_same_warning(
        r#"var ns = {};

/** @interface */

ns.SomeInterface = class { constructor(x) {}};
"#,
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
}

// port: CheckInterfacesTest#testInterfaceArgs_withES6Modules
#[test]
fn test_interface_args_with_es6_modules() {
    let t = case();
    t.test_same_warning(
        "export /** @interface */ function A(x) {}",
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
    t.test_same_warning(
        "export /** @interface */ class C { constructor(x) {} }",
        &INTERFACE_CONSTRUCTOR_SHOULD_NOT_TAKE_ARGS,
    );
}

// port: CheckInterfacesTest#testInterfaceConstructorWithFieldDeclarations_noWarning
#[test]
fn test_interface_constructor_with_field_declarations_no_warning() {
    let t = case();
    t.test_same("/** @interface */ function A() { /** @type {string} */ this.foo; }");
    t.test_same("/** @interface */ class C { constructor() { /** @type {string} */ this.foo; }}");
    t.test_same(
        r#"var ns = {};
/** @interface */
ns.SomeInterface = function() { /** @type {string} */ this.foo; };
"#,
    );
    t.test_same(
        r#"var ns = {};
/** @interface */
ns.SomeInterface = class { constructor() { /** @type {string} */ this.foo; }; }
"#,
    );
}

// port: CheckInterfacesTest#testInterfaceMethodNotEmpty
#[test]
fn test_interface_method_not_empty() {
    let t = case();
    t.test_same_warning(
        r#"/** @interface */
class C {
  constructor() {}
  A() { this.foo; }
}
"#,
        &INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY,
    );
}

// port: CheckInterfacesTest#testInterfaceWithFieldDeclarations_withES6Modules
#[test]
fn test_interface_with_field_declarations_with_es6_modules() {
    let t = case();
    t.test_same("export /** @interface */ function A() { /** @type {string} */ this.foo; }");
    t.test_same(
        "export /** @interface */ class C { constructor() { /** @type {string} */ this.foo; } }",
    );
}

// port: CheckInterfacesTest#testInterfaceClass_callToSuperInConstructorNotAllowed
#[test]
fn test_interface_class_call_to_super_in_constructor_not_allowed() {
    let t = case();
    // disableAstValidation(): the fixture runs no AST validation.
    // silences ASTValidator for `super()` used without `extends`
    t.test_warning(
        "class D {} /** @interface \n @extends {D} */ class C { constructor() { super(); } }",
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
    t.test_warning(
        "class D {} /** @interface \n @extends {D} */ class C { constructor() { super(x); } }",
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
    t.test_warning(
        "class D {} /** @record \n @extends {D} */ class C { constructor() { super(); } }",
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
    t.test_warning(
        "class D {} /** @record \n @extends {D} */ class C { constructor() { super(x); } }",
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
}

// port: CheckInterfacesTest#testInterfaceClass_extendsKeywordNotAllowed
#[test]
fn test_interface_class_extends_keyword_not_allowed() {
    let t = case();
    t.test_warnings_sources(
        &["class D {} /** @interface */ class C extends D { constructor() { super(); } }"],
        &[
            &INTERFACE_DEFINED_WITH_EXTENDS,
            &NON_DECLARATION_STATEMENT_IN_INTERFACE,
        ],
    );
    t.test_warnings_sources(
        &["class D {} /** @interface */ class C extends D { constructor() { super(x); } }"],
        &[
            &INTERFACE_DEFINED_WITH_EXTENDS,
            &NON_DECLARATION_STATEMENT_IN_INTERFACE,
        ],
    );
    t.test_warnings_sources(
        &["class D {} /** @record */ class C extends D { constructor() { super(); } }"],
        &[
            &INTERFACE_DEFINED_WITH_EXTENDS,
            &NON_DECLARATION_STATEMENT_IN_INTERFACE,
        ],
    );
    t.test_warnings_sources(
        &["class D {} /** @record */ class C extends D { constructor() { super(x); } }"],
        &[
            &INTERFACE_DEFINED_WITH_EXTENDS,
            &NON_DECLARATION_STATEMENT_IN_INTERFACE,
        ],
    );
}

// port: CheckInterfacesTest#testInterfaceComputedProperties
#[test]
fn test_interface_computed_properties() {
    let t = case();
    t.test_same_warning(
        "/** @interface */  class C { ['f']() { return 1; }}",
        &INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY,
    );
}

// port: CheckInterfacesTest#testInterfaceGetters
#[test]
fn test_interface_getters() {
    let t = case();
    t.test_same_warning(
        "/** @interface */  class C { get One() { return 1; }}",
        &INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY,
    );
}

// port: CheckInterfacesTest#testInterfaceSetters
#[test]
fn test_interface_setters() {
    let t = case();
    t.test_same_warning(
        r#"/** @interface */
class C {
  set One(x) {
   this.one = x;
  }
}
"#,
        &INTERFACE_CLASS_NONSTATIC_METHOD_NOT_EMPTY,
    );
}

// port: CheckInterfacesTest#testInterfaceEs6ClassDeclaration_havingStaticMethod
#[test]
fn test_interface_es6_class_declaration_having_static_method() {
    let t = case();
    t.test_same_warning_containing(
        &[r#"/** @interface */
class I {
  constructor() {}
  static foo() {}
}
I.foo();
"#],
        &STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS,
        "Consider pulling out the static method into a flat name as I_foo",
    );
}

// port: CheckInterfacesTest#testInterfaceEs6ClassAssignment_havingStaticMethod
#[test]
fn test_interface_es6_class_assignment_having_static_method() {
    let t = case();
    t.test_same_warning_containing(
        &[r#"/** @interface */
let I = class {
  constructor() {}
  static foo() {}
}
I.foo();
"#],
        &STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS,
        "Consider pulling out the static method into a flat name as I_foo",
    );
}

// port: CheckInterfacesTest#testInterfaceEs6ClassAssignment_havingStaticMethod2
#[test]
fn test_interface_es6_class_assignment_having_static_method2() {
    let t = case();
    t.test_same_warning_containing(
        &[r#"let C;
/** @interface */
C.I = class {
  constructor() {}
  static foo() {}
}
C.I.foo();
"#],
        &STATIC_MEMBER_FUNCTION_IN_INTERFACE_CLASS,
        "Consider pulling out the static method into a flat name as C.I_foo",
    );
}

// port: CheckInterfacesTest#testInterfaceWithFieldDeclarations_missingJDoc
#[test]
fn test_interface_with_field_declarations_missing_j_doc() {
    let t = case();
    t.test_same_warning(
        "/** @interface */ function A() { this.foo; }",
        &MISSING_JSDOC_IN_DECLARATION_STATEMENT,
    );
    t.test_same_warning(
        "/** @interface */ class C { constructor() { this.foo; }}",
        &MISSING_JSDOC_IN_DECLARATION_STATEMENT,
    );
    // in ES6 modules
    t.test_same_warning(
        "export /** @interface */ function A() {this.foo; }",
        &MISSING_JSDOC_IN_DECLARATION_STATEMENT,
    );
}

// port: CheckInterfacesTest#testRecordWithFieldDeclarations
#[test]
fn test_record_with_field_declarations() {
    let t = case();
    t.test_same(
        r#"/** @record */
function R() {
  /** @type {string} */
  this.foo;

  /** @type {number} */
  this.bar;
}
"#,
    );
}

// port: CheckInterfacesTest#testRecordWithFieldDeclarationsMissingJSDoc
#[test]
fn test_record_with_field_declarations_missing_js_doc() {
    let t = case();
    t.test_same_warning(
        r#"/** @record */
function R() {
  // This should have a JSDoc.
  this.noJSDoc;

}
"#,
        &MISSING_JSDOC_IN_DECLARATION_STATEMENT,
    );
}

// port: CheckInterfacesTest#testRecordWithOtherContents
#[test]
fn test_record_with_other_contents() {
    let t = case();
    t.test_same_warning(
        r#"/** @record */
function R() {
  /** @type {string} */
  let foo = '';
}
"#,
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
}

// port: CheckInterfacesTest#testInterfaceWithOtherContents
#[test]
fn test_interface_with_other_contents() {
    let t = case();
    t.test_same_warning(
        r#"/** @interface */
function R() {
  /** @type {string} */
  let foo = '';
}
"#,
        &NON_DECLARATION_STATEMENT_IN_INTERFACE,
    );
}
