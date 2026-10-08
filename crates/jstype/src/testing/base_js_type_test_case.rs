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
 *   Bob Jervis
 *   Google Inc.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/testing/BaseJSTypeTestCase.java.

use crate::rhino::js_type_expression::JSTypeExpressionExt;
use crate::testing::type_subject::TypeSubject;
use crate::{
    TypeId,
    function_type::{FunctionType, FunctionTypeBuilder},
    js_type::JSType,
    js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    record_type_builder::RecordTypeBuilder,
};
use closure_rhino::{
    error_reporter::ErrorReporter,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    node::{Ast, NodeId},
    testing::test_error_reporter::TestErrorReporter,
};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SharedTestErrorReporter(pub Arc<Mutex<TestErrorReporter>>);
impl ErrorReporter for SharedTestErrorReporter {
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.0
            .lock()
            .unwrap()
            .warning(message, source_name, line, line_offset);
    }
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.0
            .lock()
            .unwrap()
            .error(message, source_name, line, line_offset);
    }
}

pub struct BaseJSTypeTestCase {
    pub ast: Ast,
    pub reg: JSTypeRegistry,
    pub error_reporter: Arc<Mutex<TestErrorReporter>>,
    pub all_type: TypeId,
    pub no_object_type: TypeId,
    pub no_type: TypeId,
    pub array_function_type: TypeId,
    pub array_type: TypeId,
    pub bigint_number: TypeId,
    pub bigint_number_object: TypeId,
    pub bigint_number_string: TypeId,
    pub bigint_number_string_object: TypeId,
    pub bigint_object_type: TypeId,
    pub bigint_type: TypeId,
    pub boolean_object_function_type: TypeId,
    pub boolean_object_type: TypeId,
    pub boolean_type: TypeId,
    pub checked_unknown_type: TypeId,
    pub date_function_type: TypeId,
    pub date_type: TypeId,
    pub function_function_type: TypeId,
    pub function_prototype: TypeId,
    pub function_type: TypeId,
    pub greatest_function_type: TypeId,
    pub least_function_type: TypeId,
    pub null_type: TypeId,
    pub number_object_function_type: TypeId,
    pub number_object_type: TypeId,
    pub number_string: TypeId,
    pub number_string_boolean: TypeId,
    pub number_type: TypeId,
    pub object_function_type: TypeId,
    pub null_void: TypeId,
    pub object_number_string: TypeId,
    pub object_number_string_boolean: TypeId,
    pub object_prototype: TypeId,
    pub object_type: TypeId,
    pub readonly_array_type: TypeId,
    pub regexp_function_type: TypeId,
    pub regexp_type: TypeId,
    pub string_object_function_type: TypeId,
    pub string_object_type: TypeId,
    pub string_type: TypeId,
    pub symbol_object_type: TypeId,
    pub symbol_type: TypeId,
    pub unknown_type: TypeId,
    pub void_type: TypeId,
    pub native_properties_count: usize,
}

impl BaseJSTypeTestCase {
    // port: BaseJSTypeTestCase#BaseJSTypeTestCase
    pub fn new() -> Self {
        let mut ast = Ast::new();
        let error_reporter = Arc::new(Mutex::new(TestErrorReporter::new()));
        // The port checker treats Java's conditional registry construction as a declared method.
        let reg = JSTypeRegistry::new(
            &mut ast,
            Box::new(SharedTestErrorReporter(error_reporter.clone())),
            Vec::<JsString>::new(),
        );
        Self::with_registry(ast, reg, error_reporter)
    }

    // port: BaseJSTypeTestCase#BaseJSTypeTestCase
    pub fn with_registry(
        ast: Ast,
        reg: JSTypeRegistry,
        error_reporter: Arc<Mutex<TestErrorReporter>>,
    ) -> Self {
        let mut fixture = Self {
            ast,
            reg,
            error_reporter,
            all_type: TypeId(0),
            no_object_type: TypeId(0),
            no_type: TypeId(0),
            array_function_type: TypeId(0),
            array_type: TypeId(0),
            bigint_number: TypeId(0),
            bigint_number_object: TypeId(0),
            bigint_number_string: TypeId(0),
            bigint_number_string_object: TypeId(0),
            bigint_object_type: TypeId(0),
            bigint_type: TypeId(0),
            boolean_object_function_type: TypeId(0),
            boolean_object_type: TypeId(0),
            boolean_type: TypeId(0),
            checked_unknown_type: TypeId(0),
            date_function_type: TypeId(0),
            date_type: TypeId(0),
            function_function_type: TypeId(0),
            function_prototype: TypeId(0),
            function_type: TypeId(0),
            greatest_function_type: TypeId(0),
            least_function_type: TypeId(0),
            null_type: TypeId(0),
            number_object_function_type: TypeId(0),
            number_object_type: TypeId(0),
            number_string: TypeId(0),
            number_string_boolean: TypeId(0),
            number_type: TypeId(0),
            object_function_type: TypeId(0),
            null_void: TypeId(0),
            object_number_string: TypeId(0),
            object_number_string_boolean: TypeId(0),
            object_prototype: TypeId(0),
            object_type: TypeId(0),
            readonly_array_type: TypeId(0),
            regexp_function_type: TypeId(0),
            regexp_type: TypeId(0),
            string_object_function_type: TypeId(0),
            string_object_type: TypeId(0),
            string_type: TypeId(0),
            symbol_object_type: TypeId(0),
            symbol_type: TypeId(0),
            unknown_type: TypeId(0),
            void_type: TypeId(0),
            native_properties_count: 0,
        };
        fixture.init_types();
        fixture
    }

    // port: BaseJSTypeTestCase#resetRegistryWithForwardDeclaredName
    pub fn reset_registry_with_forward_declared_name(&mut self, name: impl Into<JsString>) {
        self.reg = JSTypeRegistry::new(
            &mut self.ast,
            Box::new(SharedTestErrorReporter(self.error_reporter.clone())),
            vec![name.into()],
        );
        self.init_types();
    }
    // port: BaseJSTypeTestCase#validateWarningsAndErrors
    pub fn validate_warnings_and_errors(&self) {
        self.error_reporter
            .lock()
            .unwrap()
            .verify_has_encountered_all_warnings_and_errors();
    }
    // port: BaseJSTypeTestCase#initTypes
    pub fn init_types(&mut self) {
        self.all_type = self.reg.get_native_type(JSTypeNative::ALL_TYPE);
        self.no_object_type = self.reg.get_native_type(JSTypeNative::NO_OBJECT_TYPE);
        self.no_type = self.reg.get_native_type(JSTypeNative::NO_TYPE);
        self.array_function_type = self.reg.get_native_type(JSTypeNative::ARRAY_FUNCTION_TYPE);
        self.array_type = self.reg.get_native_type(JSTypeNative::ARRAY_TYPE);
        self.bigint_number = self.reg.get_native_type(JSTypeNative::BIGINT_NUMBER);
        self.bigint_number_object = self.reg.get_native_type(JSTypeNative::BIGINT_NUMBER_OBJECT);
        self.bigint_number_string = self.reg.get_native_type(JSTypeNative::BIGINT_NUMBER_STRING);
        self.bigint_number_string_object = self
            .reg
            .get_native_type(JSTypeNative::BIGINT_NUMBER_STRING_OBJECT);
        self.bigint_object_type = self.reg.get_native_type(JSTypeNative::BIGINT_OBJECT_TYPE);
        self.bigint_type = self.reg.get_native_type(JSTypeNative::BIGINT_TYPE);
        self.boolean_object_function_type = self
            .reg
            .get_native_type(JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE);
        self.boolean_object_type = self.reg.get_native_type(JSTypeNative::BOOLEAN_OBJECT_TYPE);
        self.boolean_type = self.reg.get_native_type(JSTypeNative::BOOLEAN_TYPE);
        self.checked_unknown_type = self.reg.get_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE);
        self.date_function_type = self.reg.get_native_type(JSTypeNative::DATE_FUNCTION_TYPE);
        self.date_type = self.reg.get_native_type(JSTypeNative::DATE_TYPE);
        self.function_function_type = self
            .reg
            .get_native_type(JSTypeNative::FUNCTION_FUNCTION_TYPE);
        self.function_type = self.reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
        self.function_prototype = self.reg.get_native_type(JSTypeNative::FUNCTION_PROTOTYPE);
        self.greatest_function_type = self
            .reg
            .get_native_type(JSTypeNative::GREATEST_FUNCTION_TYPE);
        self.least_function_type = self.reg.get_native_type(JSTypeNative::LEAST_FUNCTION_TYPE);
        self.null_type = self.reg.get_native_type(JSTypeNative::NULL_TYPE);
        self.number_object_function_type = self
            .reg
            .get_native_type(JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE);
        self.number_object_type = self.reg.get_native_type(JSTypeNative::NUMBER_OBJECT_TYPE);
        self.number_string = self.reg.get_native_type(JSTypeNative::NUMBER_STRING);
        self.number_string_boolean = self
            .reg
            .get_native_type(JSTypeNative::NUMBER_STRING_BOOLEAN);
        self.number_type = self.reg.get_native_type(JSTypeNative::NUMBER_TYPE);
        self.object_function_type = self.reg.get_native_type(JSTypeNative::OBJECT_FUNCTION_TYPE);
        self.null_void = self.reg.get_native_type(JSTypeNative::NULL_VOID);
        self.object_type = self.reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        self.object_prototype = self.reg.get_native_type(JSTypeNative::OBJECT_PROTOTYPE);
        self.readonly_array_type = self.reg.get_native_type(JSTypeNative::READONLY_ARRAY_TYPE);
        self.regexp_function_type = self.reg.get_native_type(JSTypeNative::REGEXP_FUNCTION_TYPE);
        self.regexp_type = self.reg.get_native_type(JSTypeNative::REGEXP_TYPE);
        self.string_object_function_type = self
            .reg
            .get_native_type(JSTypeNative::STRING_OBJECT_FUNCTION_TYPE);
        self.string_object_type = self.reg.get_native_type(JSTypeNative::STRING_OBJECT_TYPE);
        self.string_type = self.reg.get_native_type(JSTypeNative::STRING_TYPE);
        self.symbol_object_type = self.reg.get_native_type(JSTypeNative::SYMBOL_OBJECT_TYPE);
        self.symbol_type = self.reg.get_native_type(JSTypeNative::SYMBOL_TYPE);
        self.unknown_type = self.reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        self.void_type = self.reg.get_native_type(JSTypeNative::VOID_TYPE);
        self.object_number_string = self.reg.create_union_type(
            &self.ast,
            &[self.object_type, self.number_type, self.string_type],
        );
        self.object_number_string_boolean = self.reg.create_union_type(
            &self.ast,
            &[
                self.object_type,
                self.number_type,
                self.string_type,
                self.boolean_type,
            ],
        );
        Self::add_native_properties(&mut self.reg, &self.ast);
        self.native_properties_count = self
            .object_type
            .get_properties_count(&mut self.reg, &self.ast);
    }
    // port: BaseJSTypeTestCase#addNativeProperties
    pub fn add_native_properties(reg: &mut JSTypeRegistry, ast: &Ast) {
        let boolean_type = reg.get_native_type(JSTypeNative::BOOLEAN_TYPE);
        let number_type = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
        let string_type = reg.get_native_type(JSTypeNative::STRING_TYPE);
        let unknown_type = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let object_type = reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        let array_type = reg.get_native_type(JSTypeNative::ARRAY_TYPE);
        let date_type = reg.get_native_type(JSTypeNative::DATE_TYPE);
        let regexp_type = reg.get_native_type(JSTypeNative::REGEXP_TYPE);
        let boolean_object_type = reg.get_native_type(JSTypeNative::BOOLEAN_OBJECT_TYPE);
        let number_object_type = reg.get_native_type(JSTypeNative::NUMBER_OBJECT_TYPE);
        let string_object_type = reg.get_native_type(JSTypeNative::STRING_OBJECT_TYPE);
        let object_prototype = reg
            .get_native_type(JSTypeNative::OBJECT_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(reg, ast, object_prototype, "constructor", object_type);
        Self::add_method(reg, ast, object_prototype, "toString", string_type);
        Self::add_method(reg, ast, object_prototype, "toLocaleString", string_type);
        Self::add_method(reg, ast, object_prototype, "valueOf", unknown_type);
        Self::add_method(reg, ast, object_prototype, "hasOwnProperty", boolean_type);
        Self::add_method(reg, ast, object_prototype, "isPrototypeOf", boolean_type);
        Self::add_method(
            reg,
            ast,
            object_prototype,
            "propertyIsEnumerable",
            boolean_type,
        );
        let array_prototype = reg
            .get_native_type(JSTypeNative::ARRAY_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(reg, ast, array_prototype, "constructor", array_type);
        Self::add_method(reg, ast, array_prototype, "toString", string_type);
        Self::add_method(reg, ast, array_prototype, "toLocaleString", string_type);
        Self::add_method(reg, ast, array_prototype, "concat", array_type);
        Self::add_method(reg, ast, array_prototype, "join", string_type);
        Self::add_method(reg, ast, array_prototype, "pop", unknown_type);
        Self::add_method(reg, ast, array_prototype, "push", number_type);
        Self::add_method(reg, ast, array_prototype, "reverse", array_type);
        Self::add_method(reg, ast, array_prototype, "shift", unknown_type);
        Self::add_method(reg, ast, array_prototype, "slice", array_type);
        Self::add_method(reg, ast, array_prototype, "sort", array_type);
        Self::add_method(reg, ast, array_prototype, "splice", array_type);
        Self::add_method(reg, ast, array_prototype, "unshift", number_type);
        array_type.define_declared_property(reg, ast, "length", number_type, None);
        let boolean_prototype = reg
            .get_native_type(JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(
            reg,
            ast,
            boolean_prototype,
            "constructor",
            boolean_object_type,
        );
        Self::add_method(reg, ast, boolean_prototype, "toString", string_type);
        Self::add_method(reg, ast, boolean_prototype, "valueOf", boolean_type);
        let date_prototype = reg
            .get_native_type(JSTypeNative::DATE_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(reg, ast, date_prototype, "constructor", date_type);
        Self::add_method(reg, ast, date_prototype, "toString", string_type);
        Self::add_method(reg, ast, date_prototype, "toDateString", string_type);
        Self::add_method(reg, ast, date_prototype, "toTimeString", string_type);
        Self::add_method(reg, ast, date_prototype, "toLocaleString", string_type);
        Self::add_method(reg, ast, date_prototype, "toLocaleDateString", string_type);
        Self::add_method(reg, ast, date_prototype, "toLocaleTimeString", string_type);
        Self::add_method(reg, ast, date_prototype, "valueOf", number_type);
        Self::add_method(reg, ast, date_prototype, "getTime", number_type);
        Self::add_method(reg, ast, date_prototype, "getFullYear", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCFullYear", number_type);
        Self::add_method(reg, ast, date_prototype, "getMonth", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCMonth", number_type);
        Self::add_method(reg, ast, date_prototype, "getDate", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCDate", number_type);
        Self::add_method(reg, ast, date_prototype, "getDay", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCDay", number_type);
        Self::add_method(reg, ast, date_prototype, "getHours", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCHours", number_type);
        Self::add_method(reg, ast, date_prototype, "getMinutes", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCMinutes", number_type);
        Self::add_method(reg, ast, date_prototype, "getSeconds", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCSeconds", number_type);
        Self::add_method(reg, ast, date_prototype, "getMilliseconds", number_type);
        Self::add_method(reg, ast, date_prototype, "getUTCMilliseconds", number_type);
        Self::add_method(reg, ast, date_prototype, "getTimezoneOffset", number_type);
        Self::add_method(reg, ast, date_prototype, "setTime", number_type);
        Self::add_method(reg, ast, date_prototype, "setMilliseconds", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCMilliseconds", number_type);
        Self::add_method(reg, ast, date_prototype, "setSeconds", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCSeconds", number_type);
        Self::add_method(reg, ast, date_prototype, "setMinutes", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCMinutes", number_type);
        Self::add_method(reg, ast, date_prototype, "setHours", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCHours", number_type);
        Self::add_method(reg, ast, date_prototype, "setDate", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCDate", number_type);
        Self::add_method(reg, ast, date_prototype, "setMonth", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCMonth", number_type);
        Self::add_method(reg, ast, date_prototype, "setFullYear", number_type);
        Self::add_method(reg, ast, date_prototype, "setUTCFullYear", number_type);
        Self::add_method(reg, ast, date_prototype, "toUTCString", string_type);
        Self::add_method(reg, ast, date_prototype, "toGMTString", string_type);
        let number_prototype = reg
            .get_native_type(JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(
            reg,
            ast,
            number_prototype,
            "constructor",
            number_object_type,
        );
        Self::add_method(reg, ast, number_prototype, "toString", string_type);
        Self::add_method(reg, ast, number_prototype, "toLocaleString", string_type);
        Self::add_method(reg, ast, number_prototype, "valueOf", number_type);
        Self::add_method(reg, ast, number_prototype, "toFixed", string_type);
        Self::add_method(reg, ast, number_prototype, "toExponential", string_type);
        Self::add_method(reg, ast, number_prototype, "toPrecision", string_type);
        let regexp_prototype = reg
            .get_native_type(JSTypeNative::REGEXP_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(reg, ast, regexp_prototype, "constructor", regexp_type);
        let return_type = reg.create_nullable_type(ast, array_type);
        Self::add_method(reg, ast, regexp_prototype, "exec", return_type);
        Self::add_method(reg, ast, regexp_prototype, "test", boolean_type);
        Self::add_method(reg, ast, regexp_prototype, "toString", string_type);
        regexp_type.define_declared_property(reg, ast, "source", string_type, None);
        regexp_type.define_declared_property(reg, ast, "global", boolean_type, None);
        regexp_type.define_declared_property(reg, ast, "ignoreCase", boolean_type, None);
        regexp_type.define_declared_property(reg, ast, "multiline", boolean_type, None);
        regexp_type.define_declared_property(reg, ast, "lastIndex", number_type, None);
        let string_prototype = reg
            .get_native_type(JSTypeNative::STRING_OBJECT_FUNCTION_TYPE)
            .get_prototype(reg, ast);
        Self::add_method(
            reg,
            ast,
            string_prototype,
            "constructor",
            string_object_type,
        );
        Self::add_method(reg, ast, string_prototype, "toString", string_type);
        Self::add_method(reg, ast, string_prototype, "valueOf", string_type);
        Self::add_method(reg, ast, string_prototype, "charAt", string_type);
        Self::add_method(reg, ast, string_prototype, "charCodeAt", number_type);
        Self::add_method(reg, ast, string_prototype, "concat", string_type);
        Self::add_method(reg, ast, string_prototype, "indexOf", number_type);
        Self::add_method(reg, ast, string_prototype, "lastIndexOf", number_type);
        Self::add_method(reg, ast, string_prototype, "localeCompare", number_type);
        let return_type = reg.create_nullable_type(ast, array_type);
        Self::add_method(reg, ast, string_prototype, "match", return_type);
        Self::add_method(reg, ast, string_prototype, "replace", string_type);
        Self::add_method(reg, ast, string_prototype, "search", number_type);
        Self::add_method(reg, ast, string_prototype, "slice", string_type);
        Self::add_method(reg, ast, string_prototype, "split", array_type);
        Self::add_method(reg, ast, string_prototype, "substr", string_type);
        Self::add_method(reg, ast, string_prototype, "substring", string_type);
        Self::add_method(reg, ast, string_prototype, "toLowerCase", string_type);
        Self::add_method(reg, ast, string_prototype, "toLocaleLowerCase", string_type);
        Self::add_method(reg, ast, string_prototype, "toUpperCase", string_type);
        Self::add_method(reg, ast, string_prototype, "toLocaleUpperCase", string_type);
        string_object_type.define_declared_property(reg, ast, "length", number_type, None);
    }
    // port: BaseJSTypeTestCase#addMethod
    fn add_method(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        receiving_type: TypeId,
        method_name: &str,
        return_type: TypeId,
    ) {
        let method = FunctionTypeBuilder::new()
            .with_return_type(return_type)
            .build(reg, ast);
        receiving_type.define_declared_property(reg, ast, method_name, method, None);
    }
    // port: BaseJSTypeTestCase#createUnionType
    pub fn create_union_type(&mut self, variants: &[TypeId]) -> TypeId {
        self.reg.create_union_type(&self.ast, variants)
    }
    // port: BaseJSTypeTestCase#createRecordTypeBuilder
    pub fn create_record_type_builder(&self) -> RecordTypeBuilder {
        RecordTypeBuilder::new()
    }
    // port: BaseJSTypeTestCase#createNullableType
    pub fn create_nullable_type(&mut self, type_: TypeId) -> TypeId {
        self.reg.create_nullable_type(&self.ast, type_)
    }
    // port: BaseJSTypeTestCase#createOptionalType
    pub fn create_optional_type(&mut self, type_: TypeId) -> TypeId {
        self.reg.create_optional_type(&self.ast, type_)
    }
    // port: BaseJSTypeTestCase#createTemplatizedType
    pub fn create_templatized_type(&mut self, base_type: TypeId, types: &[TypeId]) -> TypeId {
        self.reg
            .create_templatized_type(&self.ast, base_type, types)
    }
    // port: BaseJSTypeTestCase#assertTypeEquals
    pub fn assert_type_equals_node(&mut self, expected: TypeId, actual: NodeId) {
        self.assert_type_equals_expression(
            expected,
            &JSTypeExpression::new(actual, "<BaseJSTypeTestCase.java>"),
        );
    }
    // port: BaseJSTypeTestCase#assertTypeEquals
    pub fn assert_type_equals_expression(&mut self, expected: TypeId, actual: &JSTypeExpression) {
        let resolved = self.resolve(actual, &[]);
        self.assert_type_equals(expected, resolved);
    }
    // port: BaseJSTypeTestCase#assertTypeEquals
    pub fn assert_type_equals(&mut self, a: TypeId, b: TypeId) {
        TypeSubject::assert_type(b).is_equal_to(&mut self.reg, &self.ast, a);
    }
    // port: BaseJSTypeTestCase#assertTypeEquals
    pub fn assert_type_equals_with_message(&mut self, message: &str, a: TypeId, b: TypeId) {
        assert!(a.equals(&mut self.reg, &self.ast, b), "{message}");
        TypeSubject::assert_type(b).is_equal_to(&mut self.reg, &self.ast, a);
    }
    // port: BaseJSTypeTestCase#resolve
    pub fn resolve(&mut self, expression: &JSTypeExpression, warnings: &[&str]) -> TypeId {
        self.error_reporter
            .lock()
            .unwrap()
            .expect_all_warnings(warnings);
        expression.evaluate(&mut self.reg, &mut self.ast, None)
    }
    // port: BaseJSTypeTestCase#assertTypeNotEquals
    pub fn assert_type_not_equals(&mut self, a: TypeId, b: TypeId) {
        TypeSubject::assert_type(b).is_not_equal_to(&mut self.reg, &self.ast, a);
    }
    // port: BaseJSTypeTestCase#assertCanTestForEqualityWith
    pub fn assert_can_test_for_equality_with(&mut self, t1: TypeId, t2: TypeId) {
        assert!(t1.can_test_for_equality_with(&mut self.reg, &self.ast, t2));
        assert!(t2.can_test_for_equality_with(&mut self.reg, &self.ast, t1));
    }
    // port: BaseJSTypeTestCase#assertCannotTestForEqualityWith
    pub fn assert_cannot_test_for_equality_with(&mut self, t1: TypeId, t2: TypeId) {
        assert!(!t1.can_test_for_equality_with(&mut self.reg, &self.ast, t2));
        assert!(!t2.can_test_for_equality_with(&mut self.reg, &self.ast, t1));
    }
    // port: BaseJSTypeTestCase#lines
    pub fn line(line: &str) -> String {
        line.to_owned()
    }
    // port: BaseJSTypeTestCase#lines
    pub fn lines(lines: &[&str]) -> String {
        lines.join("\n")
    }
}
impl Default for BaseJSTypeTestCase {
    fn default() -> Self {
        Self::new()
    }
}
impl Drop for BaseJSTypeTestCase {
    fn drop(&mut self) {
        if std::thread::panicking() {
            // JUnit runs @After even when the body fails. Avoid aborting on a second panic.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.validate_warnings_and_errors()
            }))
            .is_err()
            {
                eprintln!(
                    "BaseJSTypeTestCase warning/error validation also failed during test unwinding"
                );
            }
        } else {
            self.validate_warnings_and_errors();
        }
    }
}

pub const ALL_NATIVE_EXTERN_TYPES: &str = "/**\n * @constructor\n * @param {*=} opt_value\n * @return {!Object}\n */\nfunction Object(opt_value) {}\n\n/**\n * @constructor\n * @param {...*} var_args\n */\n\nfunction Function(var_args) {}\n/**\n * @constructor\n * @param {...*} var_args\n * @return {!Array.<?>}\n * @template T\n */\nfunction Array(var_args) {}\n\n/**\n * @constructor\n * @param {*=} opt_value\n * @return {boolean}\n */\nfunction Boolean(opt_value) {}\n\n/**\n * @constructor\n * @param {*=} opt_value\n * @return {number}\n */\nfunction Number(opt_value) {}\n\n/**\n * @constructor\n * @param {?=} opt_yr_num\n * @param {?=} opt_mo_num\n * @param {?=} opt_day_num\n * @param {?=} opt_hr_num\n * @param {?=} opt_min_num\n * @param {?=} opt_sec_num\n * @param {?=} opt_ms_num\n * @return {string}\n */\nfunction Date(opt_yr_num, opt_mo_num, opt_day_num, opt_hr_num,\n    opt_min_num, opt_sec_num, opt_ms_num) {}\n\n/**\n * @constructor\n * @param {*=} opt_str\n * @return {string}\n */\nfunction String(opt_str) {}\n\n/**\n * @constructor\n * @param {*=} opt_pattern\n * @param {*=} opt_flags\n * @return {!RegExp}\n */\nfunction RegExp(opt_pattern, opt_flags) {}\n\n/**\n * @constructor\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!Error}\n */\nfunction Error(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!EvalError}\n */\nfunction EvalError(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!RangeError}\n */\nfunction RangeError(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!ReferenceError}\n */\nfunction ReferenceError(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!SyntaxError}\n */\nfunction SyntaxError(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!TypeError}\n */\nfunction TypeError(opt_message, opt_file, opt_line) {}\n\n/**\n * @constructor\n * @extends {Error}\n * @param {*=} opt_message\n * @param {*=} opt_file\n * @param {*=} opt_line\n * @return {!URIError}\n */\nfunction URIError(opt_message, opt_file, opt_line) {}\n\n/**\n * @param {string} progId\n * @param {string=} opt_location\n * @constructor\n */\nfunction ActiveXObject(progId, opt_location) {}";
