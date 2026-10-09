/*
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
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java.

//! CompilerTypeTestCase setup and warning correspondence. Type registry methods await jstype.
use crate::{
    compiler_test_case::contains_exactly_by,
    jscomp_api::{Compiler, CompilerOptions},
    replay::replay_dsl::CompilerHandle,
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};
// port: CompilerTypeTestCase#DEFAULT_EXTERNS
pub use crate::compiler_test_case::DEFAULT_EXTERNS;
#[derive(Default)]
pub struct CompilerTypeTestCase {
    pub registry: Option<crate::replay::replay_dsl::DslValue>,
    pub error_reporter: closure_rhino::testing::test_error_reporter::TestErrorReporter,
    pub compiler: Option<CompilerHandle>,
}
impl CompilerTypeTestCase {
    // port: CompilerTypeTestCase#defaultOptions
    pub fn default_options() -> Result<CompilerOptions, Throwable> {
        let mut o = CompilerOptions::new();
        use crate::jscomp_api::CheckLevel;
        o.set_coding_convention(crate::harness_passes::decode_coding_convention(
            &crate::harness_passes::google_coding_convention()?,
        )?);
        o.set_language(crate::jscomp_api::LanguageMode::UNSUPPORTED);
        for group in [
            "MISSING_PROPERTIES",
            "MISPLACED_TYPE_ANNOTATION",
            "INVALID_CASTS",
            "LINT_CHECKS",
            "JSDOC_MISSING_TYPE",
            "BOUNDED_GENERICS",
        ] {
            o.set_warning_level(
                crate::harness_passes::diagnostic_group(group)?,
                CheckLevel::WARNING,
            );
        }
        Ok(o)
    }
    // port: CompilerTypeTestCase#getDefaultOptions
    pub fn get_default_options(&self) -> Result<CompilerOptions, Throwable> {
        Self::default_options()
    }
    // port: CompilerTypeTestCase#getCodingConvention
    pub fn get_coding_convention(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        crate::harness_passes::google_coding_convention()
    }
    // port: CompilerTypeTestCase#initializeNewCompiler
    pub fn initialize_new_compiler(options: CompilerOptions) -> Result<CompilerHandle, Throwable> {
        let c = Self::create_compiler()?;
        c.borrow_mut().init_options(options);
        c.borrow_mut()
            .mark_feature_not_allowed(closure_parsing::parser::feature_set::Feature::MODULES);
        Ok(c)
    }
    // port: CompilerTypeTestCase#initializeNewCompiler (dependency guard)
    pub fn create_compiler() -> Result<CompilerHandle, Throwable> {
        Ok(Rc::new(RefCell::new(Compiler::new())))
    }
    // port: CompilerTypeTestCase#setUp
    pub fn set_up(&mut self) -> Result<(), Throwable> {
        self.error_reporter = closure_rhino::testing::test_error_reporter::TestErrorReporter::new();
        self.initialize_new_compiler_on(self.get_default_options()?)?;
        Ok(())
    }
    // port: CompilerTypeTestCase#checkReportedWarningsHelper
    pub fn check_reported_warnings_helper(
        &self,
        expected: Option<&[String]>,
    ) -> Result<(), Throwable> {
        let c = self
            .compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?;
        contains_exactly_by(
            &c.borrow().get_warnings(),
            expected.unwrap_or(&[]),
            true,
            |a, b| a.description() == b,
            "Regarding warnings:",
        )
    }
    // port: CompilerTypeTestCase#validateWarningsAndErrors
    pub fn validate_warnings_and_errors(&self) -> Result<(), Throwable> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.error_reporter
                .verify_has_encountered_all_warnings_and_errors()
        }))
        .map_err(|e| Throwable::Assertion {
            message: e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).into()))
                .unwrap_or_else(|| "warnings or errors differ".into()),
        })
    }

    // port: CompilerTypeTestCase#initializeNewCompiler (registry binding)
    pub fn initialize_new_compiler_on(
        &mut self,
        options: CompilerOptions,
    ) -> Result<(), Throwable> {
        let c = Self::initialize_new_compiler(options)?;
        self.registry = Some(crate::harness_passes::compiler_type_registry(&c)?);
        self.compiler = Some(c);
        Ok(())
    }
    // port: CompilerTypeTestCase#createUnionType(JSType...)
    pub fn create_union_type(
        &self,
        variants: &[crate::replay::replay_dsl::DslValue],
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call("createUnionType", variants.to_vec())
    }
    // port: CompilerTypeTestCase#createRecordTypeBuilder
    pub fn create_record_type_builder(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        crate::harness_passes::record_type_builder(
            self.registry
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no type registry".into()))?,
        )
    }
    // port: CompilerTypeTestCase#createNullableType
    pub fn create_nullable_type(
        &self,
        type_: crate::replay::replay_dsl::DslValue,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call("createNullableType", vec![type_])
    }
    // port: CompilerTypeTestCase#createOptionalType
    pub fn create_optional_type(
        &self,
        type_: crate::replay::replay_dsl::DslValue,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call("createOptionalType", vec![type_])
    }
    // port: CompilerTypeTestCase#createTemplatizedType(ObjectType,ImmutableList<JSType>) / createTemplatizedType(ObjectType,JSType...)
    pub fn create_templatized_type(
        &self,
        base: crate::replay::replay_dsl::DslValue,
        variants: &[crate::replay::replay_dsl::DslValue],
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call(
            "createTemplatizedType",
            vec![
                base,
                crate::replay::replay_dsl::DslValue::List(variants.to_vec()),
            ],
        )
    }
    // port: CompilerTypeTestCase#assertTypeEquals(JSType,JSType)
    pub fn assert_type_equals(
        &self,
        a: &crate::replay::replay_dsl::DslValue,
        b: &crate::replay::replay_dsl::DslValue,
    ) -> Result<(), Throwable> {
        crate::harness_passes::assert_types_equal(None, a, b)
    }
    // port: CompilerTypeTestCase#assertTypeEquals(String,JSType,JSType)
    pub fn assert_type_equals_message(
        &self,
        message: &str,
        a: &crate::replay::replay_dsl::DslValue,
        b: &crate::replay::replay_dsl::DslValue,
    ) -> Result<(), Throwable> {
        crate::harness_passes::assert_types_equal(Some(message), a, b)
    }
    // port: CompilerTypeTestCase#assertTypeEquals(JSType,Node)
    pub fn assert_type_equals_node(
        &mut self,
        expected: &crate::replay::replay_dsl::DslValue,
        actual: closure_rhino::node::NodeId,
    ) -> Result<(), Throwable> {
        let expression =
            crate::harness_passes::js_type_expression(actual, "<BaseJSTypeTestCase.java>")?;
        self.assert_type_equals_expression(expected, &expression)
    }
    // port: CompilerTypeTestCase#assertTypeEquals(JSType,JSTypeExpression)
    pub fn assert_type_equals_expression(
        &mut self,
        expected: &crate::replay::replay_dsl::DslValue,
        expression: &crate::replay::replay_dsl::DslValue,
    ) -> Result<(), Throwable> {
        let actual = self.resolve(expression, &[])?;
        self.assert_type_equals(expected, &actual)
    }
    // port: CompilerTypeTestCase#resolve
    pub fn resolve(
        &mut self,
        expression: &crate::replay::replay_dsl::DslValue,
        warnings: &[&str],
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.error_reporter.expect_all_warnings(warnings);
        crate::harness_passes::evaluate_type_expression(
            expression,
            self.registry
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no type registry".into()))?,
        )
    }
    // port: CompilerTypeTestCase#initializeNewCompiler (JSTypeRegistry method dispatch)
    fn registry_call(
        &self,
        method: &str,
        args: Vec<crate::replay::replay_dsl::DslValue>,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        crate::harness_passes::type_registry_call(
            self.registry
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no type registry".into()))?,
            method,
            args,
        )
    }
    // port: CompilerTypeTestCase#getNativeObjectType(JSTypeNative)
    pub fn get_native_object_type_of(
        &self,
        native: &str,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.native_type("getNativeObjectType", native)
    }
    // port: CompilerTypeTestCase#getNativeFunctionType(JSTypeNative)
    pub fn get_native_function_type_of(
        &self,
        native: &str,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.native_type("getNativeFunctionType", native)
    }
    // port: CompilerTypeTestCase#getNativeType(JSTypeNative)
    pub fn get_native_type(
        &self,
        native: &str,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.native_type("getNativeType", native)
    }
    // port: CompilerTypeTestCase#getNativeType (enum argument)
    fn native_type(
        &self,
        method: &str,
        native: &str,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call(
            method,
            vec![crate::replay::replay_dsl::DslValue::Enum {
                class: "com.google.javascript.rhino.jstype.JSTypeNative".into(),
                name: native.into(),
            }],
        )
    }
    // port: CompilerTypeTestCase#getNativeNoObjectType
    pub fn get_native_no_object_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("NO_OBJECT_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeArrayType
    pub fn get_native_array_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("ARRAY_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeReadonlyArrayType
    pub fn get_native_readonly_array_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("READONLY_ARRAY_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeStringObjectType
    pub fn get_native_string_object_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("STRING_OBJECT_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNumberObjectType
    pub fn get_native_number_object_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("NUMBER_OBJECT_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeBooleanObjectType
    pub fn get_native_boolean_object_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("BOOLEAN_OBJECT_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNoType
    pub fn get_native_no_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("NO_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeUnknownType
    pub fn get_native_unknown_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("UNKNOWN_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeCheckedUnknownType
    pub fn get_native_checked_unknown_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("CHECKED_UNKNOWN_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeObjectType
    pub fn get_native_object_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_object_type_of("OBJECT_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeObjectConstructorType
    pub fn get_native_object_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("OBJECT_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeArrayConstructorType
    pub fn get_native_array_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("ARRAY_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeBooleanObjectConstructorType
    pub fn get_native_boolean_object_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("BOOLEAN_OBJECT_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNumberObjectConstructorType
    pub fn get_native_number_object_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("NUMBER_OBJECT_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeStringObjectConstructorType
    pub fn get_native_string_object_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("STRING_OBJECT_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeDateConstructorType
    pub fn get_native_date_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("DATE_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeRegexpConstructorType
    pub fn get_native_regexp_constructor_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("REGEXP_FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeFunctionType
    pub fn get_native_function_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_function_type_of("FUNCTION_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeVoidType
    pub fn get_native_void_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("VOID_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNullType
    pub fn get_native_null_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("NULL_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNullVoidType
    pub fn get_native_null_void_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("NULL_VOID")
    }
    // port: CompilerTypeTestCase#getNativeNumberType
    pub fn get_native_number_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("NUMBER_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeBooleanType
    pub fn get_native_boolean_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("BOOLEAN_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeStringType
    pub fn get_native_string_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("STRING_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeNumberStringBooleanType
    pub fn get_native_number_string_boolean_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("NUMBER_STRING_BOOLEAN")
    }
    // port: CompilerTypeTestCase#getNativeValueTypes
    pub fn get_native_value_types(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("VALUE_TYPES")
    }
    // port: CompilerTypeTestCase#getNativeAllType
    pub fn get_native_all_type(&self) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.get_native_type("ALL_TYPE")
    }
    // port: CompilerTypeTestCase#getNativeObjectNumberStringBooleanType
    pub fn get_native_object_number_string_boolean_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call(
            "createUnionType",
            vec!["OBJECT_TYPE", "NUMBER_TYPE", "STRING_TYPE", "BOOLEAN_TYPE"]
                .into_iter()
                .map(|name| crate::replay::replay_dsl::DslValue::Enum {
                    class: "com.google.javascript.rhino.jstype.JSTypeNative".into(),
                    name: name.into(),
                })
                .collect(),
        )
    }
    // port: CompilerTypeTestCase#getNativeObjectNumberStringBooleanSymbolType
    pub fn get_native_object_number_string_boolean_symbol_type(
        &self,
    ) -> Result<crate::replay::replay_dsl::DslValue, Throwable> {
        self.registry_call(
            "createUnionType",
            vec![
                "OBJECT_TYPE",
                "NUMBER_TYPE",
                "STRING_TYPE",
                "BOOLEAN_TYPE",
                "SYMBOL_TYPE",
            ]
            .into_iter()
            .map(|name| crate::replay::replay_dsl::DslValue::Enum {
                class: "com.google.javascript.rhino.jstype.JSTypeNative".into(),
                name: name.into(),
            })
            .collect(),
        )
    }
}

// port: CompilerTypeTestCase#CLOSURE_DEFS
pub const CLOSURE_DEFS: &str = r#"goog.inherits = function(x, y) {};
/** @type {!Function} */ goog.abstractMethod = function() {};
goog.isFunction = function(x) {};
goog.isObject = function(x) {};
/** @const */ goog.array = {};
// simplified ArrayLike definition
/**
 * @typedef {Array|{length: number}}
 */
goog.array.ArrayLike;
/**
 * @param {Array<T>|{length:number}} arr
 * @param {function(this:S, T, number, goog.array.ArrayLike):boolean} f
 * @param {S=} obj
 * @return {!Array<T>}
 * @template T,S
 */
// return empty array to satisfy return type
goog.array.filter = function(arr, f, obj){ return []; };
goog.asserts = {};
/** @return {*} */ goog.asserts.assert = function(obj, msg = undefined) { return obj; };
goog.loadModule = function(mod) {};
"#;
