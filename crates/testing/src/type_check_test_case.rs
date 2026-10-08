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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/TypeCheckTestCase.java.

//! TypeCheckTestCase.TypeTestBuilder and parseAndTypeCheckWithScope paths.
use crate::replay::replay_dsl::DslValue;
use crate::{
    compiler_test_case::{contains_exactly_by, java_trim},
    harness_passes as passes,
    jscomp_api::{CheckLevel, CompilerOptions, DiagnosticGroup, DiagnosticType, SourceFile},
    replay::replay_dsl::CompilerHandle,
    throwable::{Throwable, assert_that, check_state},
};
use closure_jstype::prelude::*;
use closure_rhino::{input_id::InputId, ir::IR, js_string::JsString, node::NodeId};
use std::sync::Arc;
pub struct TypeTestBuilder {
    pub sources: Vec<Arc<SourceFile>>,
    pub externs: Vec<JsString>,
    pub include_default_externs: bool,
    pub default_externs: JsString,
    pub diagnostic_types: Vec<&'static DiagnosticType>,
    pub diagnostic_descriptions: Vec<String>,
    pub diagnostics_are_errors: bool,
    pub report_unknown_types: bool,
    pub suppress: Vec<Arc<DiagnosticGroup>>,
    pub has_run: bool,
    pub compiler: CompilerHandle,
}
pub struct TypeCheckResult {
    pub root: Option<NodeId>,
    pub scope: crate::replay::replay_dsl::DslValue,
}
impl TypeCheckResult {
    // port: TypeCheckTestCase.TypeCheckResult#TypeCheckResult
    pub fn new(root: Option<NodeId>, scope: DslValue) -> Self {
        Self { root, scope }
    }
}
// port: TypeCheckTestCase#castAny
pub fn cast_any<T>(value: T) -> T {
    // Java erases T to Object here; the cast is an identity operation in this method.
    value
}
impl TypeTestBuilder {
    // port: TypeCheckTestCase.TypeTestBuilder#TypeTestBuilder
    pub fn new(compiler: CompilerHandle, default_externs: JsString) -> Self {
        Self {
            sources: vec![],
            externs: vec![],
            include_default_externs: false,
            default_externs,
            diagnostic_types: vec![],
            diagnostic_descriptions: vec![],
            diagnostics_are_errors: false,
            report_unknown_types: false,
            suppress: vec![],
            has_run: false,
            compiler,
        }
    }
    // port: TypeCheckTestCase.TypeTestBuilder#newTest
    pub fn new_test(default_externs: JsString) -> Result<Self, Throwable> {
        let c = crate::compiler_type_test_case::CompilerTypeTestCase::create_compiler()?;
        let mut options = crate::compiler_type_test_case::CompilerTypeTestCase::default_options()?;
        configure_type_check_options(&mut options)?;
        c.borrow_mut().init_options(options);
        c.borrow_mut()
            .mark_feature_not_allowed(closure_parsing::parser::feature_set::Feature::MODULES);
        Ok(Self::new(c, default_externs))
    }
    // port: TypeCheckTestCase.TypeTestBuilder#addSource(String)
    pub fn add_source(&mut self, code: impl Into<JsString>) -> &mut Self {
        self.add_source_named(format!("testcode{}", self.sources.len()), code)
    }
    // port: TypeCheckTestCase.TypeTestBuilder#addSource(String,String)
    pub fn add_source_named(
        &mut self,
        filename: impl AsRef<str>,
        code: impl Into<JsString>,
    ) -> &mut Self {
        self.sources
            .push(Arc::new(SourceFile::from_code(filename.as_ref(), code)));
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#addExterns
    pub fn add_externs(&mut self, code: impl Into<JsString>) -> &mut Self {
        self.externs.push(code.into());
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#includeDefaultExterns
    pub fn include_default_externs(&mut self) -> &mut Self {
        self.include_default_externs = true;
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#diagnosticsAreErrors
    pub fn diagnostics_are_errors(&mut self) -> &mut Self {
        self.diagnostics_are_errors = true;
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#enableReportUnknownTypes
    pub fn enable_report_unknown_types(&mut self) -> &mut Self {
        self.report_unknown_types = true;
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#suppress
    pub fn suppress(&mut self, group: Arc<DiagnosticGroup>) -> &mut Self {
        self.suppress.push(group);
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#addDiagnostic(DiagnosticType)
    pub fn add_diagnostic(&mut self, d: &'static DiagnosticType) -> &mut Self {
        self.diagnostic_types.push(d);
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#addDiagnostic(String)
    pub fn add_diagnostic_description(&mut self, s: &str) -> &mut Self {
        self.diagnostic_descriptions.push(java_trim(s).into());
        self
    }
    // port: TypeCheckTestCase.TypeTestBuilder#run
    pub fn run(&mut self) -> Result<(), Throwable> {
        check_state(!self.sources.is_empty(), "Must provide a source")?;
        check_state(!self.has_run, "Cannot run the same test twice")?;
        self.has_run = true;
        if self.include_default_externs {
            self.externs.insert(0, self.default_externs.clone());
        }
        let mut units = vec![];
        for (i, e) in self.externs.iter().enumerate() {
            if i > 0 {
                units.push(10);
            }
            units.extend_from_slice(e.as_units());
        }
        for group in &self.suppress {
            self.compiler
                .borrow_mut()
                .get_options_mut()
                .set_warning_level(group.clone(), CheckLevel::OFF);
        }
        parse_and_type_check_with_scope(
            self.compiler.clone(),
            JsString::from_units(units),
            &self.sources,
            self.report_unknown_types,
        )?;
        let (asserted, other) = if self.diagnostics_are_errors {
            (
                self.compiler.borrow().get_errors(),
                self.compiler.borrow().get_warnings(),
            )
        } else {
            (
                self.compiler.borrow().get_warnings(),
                self.compiler.borrow().get_errors(),
            )
        };
        if !self.diagnostic_types.is_empty() {
            contains_exactly_by(
                &asserted,
                &self.diagnostic_types,
                true,
                |a, b| a.get_type() == *b,
                "type diagnostics",
            )?;
        } else {
            contains_exactly_by(
                &asserted,
                &self.diagnostic_descriptions,
                true,
                |a, b| a.description() == b,
                "diagnostic descriptions",
            )?;
        }
        assert_that(other.is_empty(), "Unexpected other diagnostics")
    }
}
// port: TypeCheckTestCase#parseAndTypeCheckWithScope(Compiler,String,List,boolean)
pub fn parse_and_type_check_with_scope(
    c: CompilerHandle,
    externs: JsString,
    sources: &[Arc<SourceFile>],
    report_unknown_types: bool,
) -> Result<TypeCheckResult, Throwable> {
    let options: CompilerOptions = c.borrow().get_options().clone();
    c.borrow_mut().init(
        &[Arc::new(SourceFile::from_code("[externs]", externs))],
        sources,
        options,
    );
    c.borrow_mut().parse();
    let js = c.borrow().get_js_root().unwrap();
    let e = c.borrow().get_externs_root().unwrap();
    passes::gather_module_metadata(&mut c.borrow_mut(), e, js, false, "BROWSER")?;
    passes::module_map_creator(&mut c.borrow_mut(), e, js)?;
    passes::infer_consts(&mut c.borrow_mut(), e, js)?;
    assert_that(c.borrow().get_errors().is_empty(), "Regarding errors:")?;
    let registry = passes::compiler_type_registry(&c)?;
    let scope = TypeCheckTestCase::make_type_check_with_registry(&c, &registry)?
        .report_unknown_types(report_unknown_types)?
        .process_for_testing(&mut c.borrow_mut(), e, js)?;
    passes::ast_validator(&mut c.borrow_mut(), e, js, false, "JSTYPE")?;
    let root = js.get_first_child(&c.borrow());
    Ok(TypeCheckResult::new(root, scope))
}

// port: TypeCheckTestCase#setUp / TypeTestBuilder#newTest
pub fn configure_type_check_options(options: &mut CompilerOptions) -> Result<(), Throwable> {
    for group in [
        "MISSING_OVERRIDE",
        "STRICT_MISSING_PROPERTIES",
        "STRICT_PRIMITIVE_OPERATORS",
    ] {
        options.set_warning_level(passes::diagnostic_group(group)?, CheckLevel::WARNING);
    }
    Ok(())
}
#[derive(Default)]
pub struct TypeCheckTestCase {
    pub base: crate::compiler_type_test_case::CompilerTypeTestCase,
}
impl TypeCheckTestCase {
    // port: TypeCheckTestCase#setUp
    pub fn set_up(&mut self) -> Result<(), Throwable> {
        self.base.set_up()?;
        configure_type_check_options(
            self.base
                .compiler
                .as_ref()
                .unwrap()
                .borrow_mut()
                .get_options_mut(),
        )
    }
    // port: TypeCheckTestCase#newTestLegacy
    pub fn new_test_legacy(&self, default_externs: JsString) -> Result<TypeTestBuilder, Throwable> {
        Ok(TypeTestBuilder::new(
            self.base
                .compiler
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?
                .clone(),
            default_externs,
        ))
    }
    // port: TypeCheckTestCase#parseAndTypeCheck(String)
    pub fn parse_and_type_check(
        &self,
        js: impl Into<JsString>,
    ) -> Result<Option<NodeId>, Throwable> {
        self.parse_and_type_check_with_externs(JsString::from(""), js)
    }
    // port: TypeCheckTestCase#parseAndTypeCheck(String,String)
    pub fn parse_and_type_check_with_externs(
        &self,
        externs: impl Into<JsString>,
        js: impl Into<JsString>,
    ) -> Result<Option<NodeId>, Throwable> {
        self.parse_and_type_check_with_scope_named(externs, js, "")
            .map(|r| r.root)
    }
    // port: TypeCheckTestCase#parseAndTypeCheckWithScope(String)
    pub fn parse_and_type_check_with_scope(
        &self,
        js: impl Into<JsString>,
    ) -> Result<TypeCheckResult, Throwable> {
        self.parse_and_type_check_with_scope_named(JsString::from(""), js, "")
    }
    // port: TypeCheckTestCase#parseAndTypeCheckWithScope(String,String)
    pub fn parse_and_type_check_with_scope_externs(
        &self,
        externs: impl Into<JsString>,
        js: impl Into<JsString>,
    ) -> Result<TypeCheckResult, Throwable> {
        self.parse_and_type_check_with_scope_named(externs, js, "")
    }
    // port: TypeCheckTestCase#parseAndTypeCheckWithScope(String,String,String)
    pub fn parse_and_type_check_with_scope_named(
        &self,
        externs: impl Into<JsString>,
        js: impl Into<JsString>,
        extension: &str,
    ) -> Result<TypeCheckResult, Throwable> {
        parse_and_type_check_with_scope(
            self.base
                .compiler
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?
                .clone(),
            externs.into(),
            &[Arc::new(SourceFile::from_code(
                &format!("[testcode]{extension}"),
                js,
            ))],
            false,
        )
    }
    // port: TypeCheckTestCase#getInstanceType
    pub fn get_instance_type(
        c: &CompilerHandle,
        js1_node: NodeId,
    ) -> Result<Option<TypeId>, Throwable> {
        let type_ = js1_node
            .get_first_child(&c.borrow())
            .unwrap()
            .get_jstype(&c.borrow());
        assert_that(type_.is_some(), "expected a non-null type")?;
        crate::harness_passes::with_type_registry(c, |registry, _ast| {
            let type_ = type_.unwrap();
            assert_that(type_.is_function_type(registry), "expected a FunctionType")?;
            assert_that(type_.is_constructor(registry), "expected a constructor")?;
            Ok(type_.get_instance_type(registry))
        })
    }
    // port: TypeCheckTestCase#checkObjectType
    pub fn check_object_type(
        &self,
        object_type: TypeId,
        property_name: &str,
        expected_type: TypeId,
    ) -> Result<(), Throwable> {
        let c = self
            .base
            .compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?;
        crate::harness_passes::with_type_registry(c, |registry, ast| {
            let reference = object_type
                .get_reference_name(registry)
                .map_or_else(|| "null".into(), |s| s.to_string_lossy());
            assert_that(
                object_type.has_property(registry, ast, property_name),
                format!("Expected {reference} to have property {property_name}"),
            )?;
            let message = format!(
                "Expected {reference}'s property {property_name} to have type {}",
                expected_type.to_string(registry, ast)
            );
            let actual = object_type.get_property_type(registry, ast, property_name);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                closure_jstype::testing::type_subject::TypeSubject::assert_type(actual)
                    .is_equal_to(registry, ast, expected_type);
            }))
            .map_err(|_| Throwable::Assertion { message })
        })
    }
    // port: TypeCheckTestCase#assertHasXMorePropertiesThanNativeObject
    pub fn assert_has_x_more_properties_than_native_object(
        &self,
        instance_type: TypeId,
        num_extra_properties: i32,
    ) -> Result<(), Throwable> {
        let c = self
            .base
            .compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?;
        let actual = crate::harness_passes::with_type_registry(c, |registry, ast| {
            Ok(instance_type.get_properties_count(registry, ast) as i32)
        })?;
        let expected = self
            .get_native_object_properties_count()?
            .wrapping_add(num_extra_properties);
        assert_that(
            actual == expected,
            format!("expected {expected} properties, actual {actual}"),
        )
    }
    // port: TypeCheckTestCase#getNativeObjectPropertiesCount
    fn get_native_object_properties_count(&self) -> Result<i32, Throwable> {
        let c = self
            .base
            .compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?;
        crate::harness_passes::with_type_registry(c, |registry, ast| {
            Ok(registry
                .get_native_object_type(JSTypeNative::OBJECT_TYPE)
                .get_properties_count(registry, ast) as i32)
        })
    }
    // port: TypeCheckTestCase#typeCheck
    pub fn type_check(&self, n: NodeId) -> Result<NodeId, Throwable> {
        let c = self
            .base
            .compiler
            .as_ref()
            .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?;
        let (externs_node, extern_and_js_root) = {
            let mut compiler = c.borrow_mut();
            let js_root = if n.is_root(&compiler) {
                n
            } else if n.is_script(&compiler) {
                IR::root(&mut compiler, &[n])
            } else {
                let script = IR::script_with_children(&mut compiler, &[n]);
                let js_root = IR::root(&mut compiler, &[script]);
                script.set_input_id(&mut compiler, Some(Arc::new(InputId::new("test"))));
                js_root
            };
            let externs_node = IR::root(&mut compiler, &[]);
            let extern_and_js_root = IR::root(&mut compiler, &[externs_node]);
            extern_and_js_root.add_child_to_back(&mut compiler, js_root);
            (externs_node, extern_and_js_root)
        };
        let mut checker = self.make_type_check()?;
        let js_root = extern_and_js_root.get_second_child(&c.borrow()).unwrap();
        checker.process_for_testing(&mut c.borrow_mut(), externs_node, js_root)?;
        Ok(n)
    }
    // port: TypeCheckTestCase#makeTypeCheck()
    pub fn make_type_check(&self) -> Result<crate::jscomp_api::TypeCheck, Throwable> {
        Self::make_type_check_with_registry(
            self.base
                .compiler
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no compiler".into()))?,
            self.base
                .registry
                .as_ref()
                .ok_or_else(|| Throwable::HarnessError("no type registry".into()))?,
        )
    }
    // port: TypeCheckTestCase#makeTypeCheck(Compiler,JSTypeRegistry)
    pub fn make_type_check_with_registry(
        c: &CompilerHandle,
        registry: &DslValue,
    ) -> Result<crate::jscomp_api::TypeCheck, Throwable> {
        let reverse = crate::jscomp_api::new_semantic_reverse_abstract_interpreter(registry)?;
        crate::jscomp_api::TypeCheck::new(c, reverse, registry)
    }
    // port: TypeCheckTestCase#suppressMissingProperty
    pub fn suppress_missing_property(props: &[&str]) -> String {
        let mut result = String::from("function dummy(x) { ");
        for prop in props {
            result.push_str(&format!("x.{prop} = 3;"));
        }
        result.push('}');
        result
    }
    // port: TypeCheckTestCase#suppressMissingPropertyFor
    pub fn suppress_missing_property_for(type_: &str, props: &[&str]) -> String {
        let mut result = String::from("function dummy(x) { ");
        for prop in props {
            result.push_str(&format!("{type_}.prototype.{prop} = 3;"));
        }
        result.push('}');
        result
    }
}
