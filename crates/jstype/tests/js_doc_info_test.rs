/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Nick Santos
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/rhino/JSDocInfoTest.java.

use closure_jstype::{
    TypeId,
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    rhino::js_type_expression::JSTypeExpressionExt,
    testing::{base_js_type_test_case::SharedTestErrorReporter, type_subject::TypeSubject},
};
use closure_rhino::{
    js_type_expression::JSTypeExpression,
    jsdoc_info::{
        JSDocInfo,
        Visibility::{INHERITED, PROTECTED},
    },
    node::Ast,
    testing::test_error_reporter::TestErrorReporter,
    token::Token,
};
use std::sync::{Arc, Mutex};

struct Fixture {
    ast: Ast,
    registry: JSTypeRegistry,
    error_reporter: Arc<Mutex<TestErrorReporter>>,
}
impl Fixture {
    fn new() -> Self {
        let mut ast = Ast::new();
        let error_reporter = Arc::new(Mutex::new(TestErrorReporter::new()));
        let registry = JSTypeRegistry::new(
            &mut ast,
            Box::new(SharedTestErrorReporter(error_reporter.clone())),
            Vec::new(),
        );
        Self {
            ast,
            registry,
            error_reporter,
        }
    }
    // port: JSDocInfoTest#getNativeType
    fn get_native_type(&self, type_id: JSTypeNative) -> TypeId {
        self.registry.get_native_type(type_id)
    }
    // port: JSDocInfoTest#validateWarningsAndErrors
    fn validate_warnings_and_errors(&self) {
        self.error_reporter
            .lock()
            .unwrap()
            .verify_has_encountered_all_warnings_and_errors();
    }
    // port: JSDocInfoTest#resolve
    fn resolve(&mut self, n: Option<Arc<JSTypeExpression>>, warnings: &[&str]) -> TypeId {
        self.error_reporter
            .lock()
            .unwrap()
            .expect_all_warnings(warnings);
        n.unwrap().evaluate(&mut self.registry, &mut self.ast, None)
    }
    fn assert_native_expression(&mut self, n: Option<Arc<JSTypeExpression>>, native: JSTypeNative) {
        let actual = self.resolve(n, &[]);
        let expected = self.get_native_type(native);
        TypeSubject::assert_type(actual).is_equal_to(&mut self.registry, &self.ast, expected);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Err(error) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.validate_warnings_and_errors()
            })) {
                eprintln!(
                    "JSDocInfoTest warning/error validation failed during test unwinding: {error:?}"
                );
            }
        } else {
            self.validate_warnings_and_errors();
        }
    }
}
// port: JSDocInfoTest#fromString
fn from_string(ast: &mut Ast, s: &str) -> Option<Arc<JSTypeExpression>> {
    Some(Arc::new(JSTypeExpression::new(ast.new_string(s), "")))
}
fn non_null_number(ast: &mut Ast) -> Option<Arc<JSTypeExpression>> {
    let name = ast.new_string("Number");
    let bang = ast.new_node_with_child(Token::BANG, name);
    Some(Arc::new(JSTypeExpression::new(bang, "")))
}

// port: JSDocInfoTest#testSetType
#[test]
fn test_set_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_type(from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    assert!(info.get_base_type().is_none());
    assert!(info.get_description().is_none());
    assert!(info.get_enum_parameter_type().is_none());
    assert_eq!(info.get_parameter_count(), 0);
    assert!(info.get_return_type().is_none());
    f.assert_native_expression(info.get_type(), JSTypeNative::STRING_TYPE);
    assert_eq!(info.get_visibility(), INHERITED);
    assert!(info.has_type());
    assert!(!info.is_constant());
    assert!(!info.is_constructor());
}
// port: JSDocInfoTest#testSetTypeAndVisibility
#[test]
fn test_set_type_and_visibility() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_type(from_string(&mut f.ast, "string"));
    builder.record_visibility(PROTECTED);
    let info = builder.build().unwrap();
    assert!(info.get_base_type().is_none());
    assert!(info.get_description().is_none());
    assert!(info.get_enum_parameter_type().is_none());
    assert_eq!(info.get_parameter_count(), 0);
    assert!(info.get_return_type().is_none());
    f.assert_native_expression(info.get_type(), JSTypeNative::STRING_TYPE);
    assert_eq!(info.get_visibility(), PROTECTED);
    assert!(info.has_type());
    assert!(!info.is_constant());
    assert!(!info.is_constructor());
}
// port: JSDocInfoTest#testSetReturnType
#[test]
fn test_set_return_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_return_type(from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    assert!(info.get_base_type().is_none());
    assert!(info.get_description().is_none());
    assert!(info.get_enum_parameter_type().is_none());
    assert_eq!(info.get_parameter_count(), 0);
    f.assert_native_expression(info.get_return_type(), JSTypeNative::STRING_TYPE);
    assert!(info.get_type().is_none());
    assert_eq!(info.get_visibility(), INHERITED);
    assert!(!info.has_type());
    assert!(!info.is_constant());
    assert!(!info.is_constructor());
}
// port: JSDocInfoTest#testSetReturnTypeAndBaseType
#[test]
fn test_set_return_type_and_base_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_base_type(non_null_number(&mut f.ast));
    builder.record_return_type(from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    f.assert_native_expression(info.get_base_type(), JSTypeNative::NUMBER_OBJECT_TYPE);
    assert!(info.get_description().is_none());
    assert!(info.get_enum_parameter_type().is_none());
    assert_eq!(info.get_parameter_count(), 0);
    f.assert_native_expression(info.get_return_type(), JSTypeNative::STRING_TYPE);
    assert!(info.get_type().is_none());
    assert_eq!(info.get_visibility(), INHERITED);
    assert!(!info.has_type());
    assert!(!info.is_constant());
    assert!(!info.is_constructor());
}
// port: JSDocInfoTest#testSetEnumParameterType
#[test]
fn test_set_enum_parameter_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_enum_parameter_type(from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    assert!(info.get_base_type().is_none());
    assert!(info.get_description().is_none());
    f.assert_native_expression(info.get_enum_parameter_type(), JSTypeNative::STRING_TYPE);
    assert_eq!(info.get_parameter_count(), 0);
    assert!(info.get_return_type().is_none());
    assert!(info.get_type().is_none());
    assert_eq!(info.get_visibility(), INHERITED);
    assert!(!info.has_type());
    assert!(!info.is_constant());
    assert!(!info.is_constructor());
}
// port: JSDocInfoTest#testMultipleSetType
#[test]
fn test_multiple_set_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_type(from_string(&mut f.ast, "number"));
    assert!(!builder.record_return_type(from_string(&mut f.ast, "boolean")));
    assert!(!builder.record_enum_parameter_type(from_string(&mut f.ast, "string")));
    assert!(!builder.record_typedef(from_string(&mut f.ast, "string")));
    let info = builder.build().unwrap();
    f.assert_native_expression(info.get_type(), JSTypeNative::NUMBER_TYPE);
    assert!(info.get_return_type().is_none());
    assert!(info.get_enum_parameter_type().is_none());
    assert!(info.get_typedef_type().is_none());
    assert!(info.has_type());
}
// port: JSDocInfoTest#testMultipleSetType2
#[test]
fn test_multiple_set_type2() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_return_type(from_string(&mut f.ast, "boolean"));
    assert!(!builder.record_type(from_string(&mut f.ast, "number")));
    assert!(!builder.record_enum_parameter_type(from_string(&mut f.ast, "string")));
    assert!(!builder.record_typedef(from_string(&mut f.ast, "string")));
    let info = builder.build().unwrap();
    f.assert_native_expression(info.get_return_type(), JSTypeNative::BOOLEAN_TYPE);
    assert!(info.get_enum_parameter_type().is_none());
    assert!(info.get_type().is_none());
    assert!(info.get_typedef_type().is_none());
    assert!(!info.has_type());
}
// port: JSDocInfoTest#testMultipleSetType3
#[test]
fn test_multiple_set_type3() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_enum_parameter_type(from_string(&mut f.ast, "boolean"));
    assert!(!builder.record_type(from_string(&mut f.ast, "number")));
    assert!(!builder.record_return_type(from_string(&mut f.ast, "string")));
    assert!(!builder.record_typedef(from_string(&mut f.ast, "string")));
    let info = builder.build().unwrap();
    assert!(info.get_type().is_none());
    assert!(info.get_typedef_type().is_none());
    assert!(info.get_return_type().is_none());
    f.assert_native_expression(info.get_enum_parameter_type(), JSTypeNative::BOOLEAN_TYPE);
}
// port: JSDocInfoTest#testSetTypedefType
#[test]
fn test_set_typedef_type() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_typedef(from_string(&mut f.ast, "boolean"));
    let info = builder.build().unwrap();
    f.assert_native_expression(info.get_typedef_type(), JSTypeNative::BOOLEAN_TYPE);
    assert!(info.has_typedef_type());
    assert!(!info.has_type());
    assert!(!info.has_enum_parameter_type());
    assert!(!info.has_return_type());
}
// port: JSDocInfoTest#testClone
#[test]
fn test_clone() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_description("The source info");
    builder.record_constancy();
    builder.record_constructor();
    builder.record_no_collapse();
    builder.record_base_type(non_null_number(&mut f.ast));
    builder.record_return_type(from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    let cloned = info.clone_info();
    f.assert_native_expression(cloned.get_base_type(), JSTypeNative::NUMBER_OBJECT_TYPE);
    assert_eq!(cloned.get_description().unwrap(), "The source info");
    f.assert_native_expression(cloned.get_return_type(), JSTypeNative::STRING_TYPE);
    assert!(cloned.is_constant());
    assert!(cloned.is_constructor());
    assert!(cloned.is_no_collapse());
}
// port: JSDocInfoTest#testCloneTypeExpressions1
#[test]
fn test_clone_type_expressions1() {
    let mut f = Fixture::new();
    let mut builder = JSDocInfo::builder();
    builder.record_description("The source info");
    builder.record_constancy();
    builder.record_constructor();
    builder.record_base_type(non_null_number(&mut f.ast));
    builder.record_return_type(from_string(&mut f.ast, "string"));
    builder.record_parameter("a", from_string(&mut f.ast, "string"));
    let info = builder.build().unwrap();
    let cloned = info.clone_with_type_nodes(&mut f.ast, true);
    assert_ne!(
        cloned.get_base_type().unwrap().get_root(),
        info.get_base_type().unwrap().get_root()
    );
    f.assert_native_expression(cloned.get_base_type(), JSTypeNative::NUMBER_OBJECT_TYPE);
    assert_eq!(cloned.get_description().unwrap(), "The source info");
    assert_ne!(
        cloned.get_return_type().unwrap().get_root(),
        info.get_return_type().unwrap().get_root()
    );
    f.assert_native_expression(cloned.get_return_type(), JSTypeNative::STRING_TYPE);
    assert_ne!(
        cloned.get_parameter_type("a").unwrap().get_root(),
        info.get_parameter_type("a").unwrap().get_root()
    );
    f.assert_native_expression(cloned.get_parameter_type("a"), JSTypeNative::STRING_TYPE);
}
