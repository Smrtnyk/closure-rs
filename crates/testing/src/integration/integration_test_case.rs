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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/integration/IntegrationTestCase.java.

//! IntegrationTestCase's chain compilation and AST/diagnostic assertions.
use crate::{
    compiler_test_case::contains_exactly_by,
    harness_passes as passes,
    jscomp_api::{
        Compiler, CompilerOptions, DiagnosticGroup, ErrorManagerHandle, JSChunk, JSError,
        SourceFile,
    },
    replay::{
        replay_dsl::{CompilerHandle, DslValue},
        replay_values::chunks,
    },
    throwable::{Throwable, assert_that},
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};
pub struct IntegrationTestCase {
    pub externs: Vec<Arc<SourceFile>>,
    pub last_compiler: Option<CompilerHandle>,
    pub input_file_name_prefix: String,
    pub input_file_name_suffix: String,
}
// port: IntegrationTestCase#EMPTY_JOINER (delimiter)
pub const EMPTY_JOINER: &str = "";
// port: IntegrationTestCase#defaultExternsBuilderwithoutClosure (addExtra text block)
const DEFAULT_EXTERNS_EXTRA: &str = r#"/**
 * @const
 */
var Math = {};
/**
 * @param {?} n1
 * @param {?} n2
 * @return {number}
 * @nosideeffects
 */
Math.pow = function(n1, n2) {};
Math.random = function() {}
var isNaN;
var Infinity;
/**
 * @constructor
 * @extends {Array<string>}
 */
var ITemplateArray = function() {};
/** @constructor */
var Set;
/** @constructor */ function Window() {}
/** @type {string} */ Window.prototype.name;
/** @type {string} */ Window.prototype.offsetWidth;
/** @type {Window} */ var window;

/** @nosideeffects */ function noSideEffects() {}

/**
 * @constructor
 * @nosideeffects
 */
function Widget() {}
/** @modifies {this} */ Widget.prototype.go = function() {};
/** @return {string} */ var widgetToken = function() {};

/**
 * @constructor
 * @return {number}
 * @param {*=} opt_n
 */
function Number(opt_n) {}

/**
 * @constructor
 * @return {boolean}
 * @param {*=} opt_b
 */
function Boolean(opt_b) {}

/**
 * @constructor
 * @return {!TypeError}
 * @param {*=} message
 * @param {*=} fileNameOrOptions
 * @param {*=} line
 */
function TypeError(message, fileNameOrOptions, line) {}
/**
 * @constructor
 * @param {*=} message
 * @param {*=} fileNameOrOptions
 * @param {*=} line
 * @return {!Error}
 * @nosideeffects
 */
function Error(message, fileNameOrOptions, line) {}

/** @constructor */
var HTMLElement = function() {};
"#;
// port: IntegrationTestCase#DEFAULT_EXTERNS (static initializer)
pub static DEFAULT_EXTERNS: std::sync::LazyLock<Result<Vec<Arc<SourceFile>>, Throwable>> =
    std::sync::LazyLock::new(IntegrationTestCase::default_externs);
impl IntegrationTestCase {
    // port: IntegrationTestCase#defaultExternsBuilderwithoutClosure
    pub fn default_externs_builderwithout_closure() -> Result<DslValue, Throwable> {
        let mut builder = passes::test_externs_builder()?;
        for method in [
            "addArguments",
            "addString",
            "addObject",
            "addReflect",
            "addFunction",
            "addIterable",
            "addPromise",
            "addArray",
            "addAlert",
            "addMap",
        ] {
            builder = crate::unit_test_utils::externs_builder_call(&builder, method, vec![])?;
        }
        crate::unit_test_utils::externs_builder_call(
            &builder,
            "addExtra",
            vec![DslValue::String(JsString::from(DEFAULT_EXTERNS_EXTRA))],
        )
    }
    // port: IntegrationTestCase#DEFAULT_EXTERNS (initializer expression)
    pub fn default_externs() -> Result<Vec<Arc<SourceFile>>, Throwable> {
        let builder = Self::default_externs_builderwithout_closure()?;
        let builder =
            crate::unit_test_utils::externs_builder_call(&builder, "addClosureExterns", vec![])?;
        match crate::unit_test_utils::externs_builder_call(
            &builder,
            "buildExternsFile",
            vec![DslValue::String(JsString::from("externs"))],
        )? {
            DslValue::SourceFile(f) => Ok(vec![f]),
            _ => Err(Throwable::HarnessError(
                "TestExternsBuilder.buildExternsFile must return a SourceFile".into(),
            )),
        }
    }
    // port: IntegrationTestCase#files(String...)
    pub fn files(values: Vec<JsString>) -> Vec<JsString> {
        values
    }
    // port: IntegrationTestCase#setUp
    pub fn set_up(externs: Vec<Arc<SourceFile>>) -> Self {
        Self {
            externs,
            last_compiler: None,
            input_file_name_prefix: "i".into(),
            input_file_name_suffix: ".js".into(),
        }
    }
    // port: IntegrationTestCase#createCompiler
    pub fn create_compiler(&self) -> Result<CompilerHandle, Throwable> {
        Ok(Rc::new(RefCell::new(Compiler::new())))
    }
    // port: IntegrationTestCase#createCompiler(ErrorManager)
    pub fn create_compiler_with_error_manager(
        &self,
        manager: ErrorManagerHandle,
    ) -> Result<CompilerHandle, Throwable> {
        Ok(Rc::new(RefCell::new(Compiler::new_with_error_manager(
            manager,
        ))))
    }
    // port: IntegrationTestCase#compile(CompilerOptions,String)
    pub fn compile_string(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
    ) -> Result<CompilerHandle, Throwable> {
        self.compile(options, std::slice::from_ref(original))
    }
    // port: IntegrationTestCase#compile(CompilerOptions,String[])
    pub fn compile(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
    ) -> Result<CompilerHandle, Throwable> {
        let records = original
            .iter()
            .enumerate()
            .map(|(i, s)| crate::record::Chunk {
                name: format!("m{i}"),
                deps: if i == 0 {
                    vec![]
                } else {
                    vec![format!("m{}", i - 1)]
                },
                inputs: vec![crate::value::SourceFile {
                    name: format!(
                        "{}{i}{}",
                        self.input_file_name_prefix, self.input_file_name_suffix
                    ),
                    code: crate::json::JsString(s.as_units().to_vec()),
                    kind: crate::value::SourceKind::STRONG,
                }],
            })
            .collect::<Vec<_>>();
        let c =
            self.create_compiler_with_error_manager(crate::jscomp_api::black_hole_error_manager())?;
        let chunks = chunks(&records)?;
        self.compile_chunks_with_compiler(c, options, &chunks)
    }
    // port: IntegrationTestCase#compile(CompilerOptions,ImmutableList<JSChunk>)
    pub fn compile_chunks(
        &mut self,
        options: CompilerOptions,
        chunks: &[JSChunk],
    ) -> Result<CompilerHandle, Throwable> {
        let c =
            self.create_compiler_with_error_manager(crate::jscomp_api::black_hole_error_manager())?;
        self.compile_chunks_with_compiler(c, options, chunks)
    }
    // port: IntegrationTestCase#compile (lastCompiler assignment)
    fn compile_chunks_with_compiler(
        &mut self,
        c: CompilerHandle,
        options: CompilerOptions,
        chunks: &[JSChunk],
    ) -> Result<CompilerHandle, Throwable> {
        self.last_compiler = Some(c.clone());
        passes::default_pass_config(&mut c.borrow_mut(), &self.externs, chunks, options)?;
        Ok(c)
    }
    // port: IntegrationTestCase#test(CompilerOptions,String[],String[])
    pub fn test(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
        compiled: Option<&[JsString]>,
    ) -> Result<(), Throwable> {
        let c = self.compile(options, original)?;
        let root = c.borrow().get_js_root().unwrap();
        assert_that(
            c.borrow().get_errors().len() + c.borrow().get_warnings().len() == 0,
            format!(
                "Expected no warnings or errors\nErrors:\n{}\nWarnings:\n{}\n",
                c.borrow()
                    .get_errors()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
                c.borrow()
                    .get_warnings()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )?;
        if let Some(compiled) = compiled {
            let expected = self.parse_expected_code_after_compile(&c, compiled)?;
            crate::compiler_test_case::tree_equal_across(
                &c.borrow(),
                root,
                &expected.0.borrow(),
                expected.1,
                false,
                &IndexMap::<_, _>::default(),
            )?;
        }
        Ok(())
    }
    // port: IntegrationTestCase#test(CompilerOptions,String[],String[],DiagnosticGroup[])
    pub fn test_warnings(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
        compiled: Option<&[JsString]>,
        groups: &[Arc<DiagnosticGroup>],
    ) -> Result<(), Throwable> {
        let c = self.compile(options, original)?;
        check_unexpected_errors_or_warnings(
            &c.borrow().get_errors(),
            &c.borrow().get_warnings(),
            groups.len(),
        )?;
        if let Some(compiled) = compiled {
            let root = c.borrow().get_js_root().unwrap();
            let expected = self.parse_expected_code_after_compile(&c, compiled)?;
            crate::compiler_test_case::tree_equal_across(
                &c.borrow(),
                root,
                &expected.0.borrow(),
                expected.1,
                false,
                &IndexMap::<_, _>::default(),
            )?;
        }
        let mut errors = c.borrow().get_errors();
        errors.extend(c.borrow().get_warnings());
        contains_exactly_by(
            &errors,
            groups,
            false,
            |e, g| g.matches(e),
            "Error not in expected diagnostic group",
        )
    }
    // port: IntegrationTestCase#test(CompilerOptions,String[],String[],DiagnosticGroup)
    pub fn test_warning(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
        compiled: Option<&[JsString]>,
        group: Arc<DiagnosticGroup>,
    ) -> Result<(), Throwable> {
        let c = self.compile(options, original)?;
        check_unexpected_errors_or_warnings(
            &c.borrow().get_errors(),
            &c.borrow().get_warnings(),
            1,
        )?;
        if let Some(compiled) = compiled {
            let root = c.borrow().get_js_root().unwrap();
            let expected = self.parse_expected_code_after_compile(&c, compiled)?;
            crate::compiler_test_case::tree_equal_across(
                &c.borrow(),
                root,
                &expected.0.borrow(),
                expected.1,
                false,
                &IndexMap::<_, _>::default(),
            )?;
        }
        let errors = c.borrow().get_errors();
        let warnings = c.borrow().get_warnings();
        let diagnostic = if errors.len() == 1 {
            errors[0].get_type()
        } else {
            warnings[0].get_type()
        };
        assert_that(
            group.matches_type(diagnostic),
            format!(
                "Error not in expected diagnostic group. Error: {}\nExpected group: {}",
                diagnostic.key, group,
            ),
        )
    }
    // port: IntegrationTestCase#testSame(CompilerOptions,String[])
    pub fn test_same(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
    ) -> Result<(), Throwable> {
        self.test(options, original, Some(original))
    }
    // port: IntegrationTestCase#testSame(CompilerOptions,String)
    pub fn test_same_string(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
    ) -> Result<(), Throwable> {
        self.test_same(options, std::slice::from_ref(original))
    }
    // port: IntegrationTestCase#test(CompilerOptions,String,String)
    pub fn test_string(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
        compiled: &JsString,
    ) -> Result<(), Throwable> {
        self.test(
            options,
            std::slice::from_ref(original),
            Some(std::slice::from_ref(compiled)),
        )
    }
    // port: IntegrationTestCase#test(CompilerOptions,String[],DiagnosticGroup)
    pub fn test_warning_without_output(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
        warning: Arc<DiagnosticGroup>,
    ) -> Result<(), Throwable> {
        self.test_warning(options, original, None, warning)
    }
    // port: IntegrationTestCase#test(CompilerOptions,String,DiagnosticGroup)
    pub fn test_warning_string_without_output(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
        warning: Arc<DiagnosticGroup>,
    ) -> Result<(), Throwable> {
        self.test_warning_without_output(options, std::slice::from_ref(original), warning)
    }
    // port: IntegrationTestCase#test(CompilerOptions,String,String,DiagnosticGroup)
    pub fn test_warning_string(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
        compiled: &JsString,
        warning: Arc<DiagnosticGroup>,
    ) -> Result<(), Throwable> {
        self.test_warning(
            options,
            std::slice::from_ref(original),
            Some(std::slice::from_ref(compiled)),
            warning,
        )
    }
    // port: IntegrationTestCase#testNoWarnings(CompilerOptions,String)
    pub fn test_no_warnings_string(
        &mut self,
        options: CompilerOptions,
        code: &JsString,
    ) -> Result<(), Throwable> {
        self.test_no_warnings(options, std::slice::from_ref(code))
    }
    // port: IntegrationTestCase#testParseError(CompilerOptions,String)
    pub fn test_parse_error_without_output(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
    ) -> Result<(), Throwable> {
        self.test_parse_error_string(options, original, None)
    }
    // port: IntegrationTestCase#testParseError(CompilerOptions,String,String)
    pub fn test_parse_error_string(
        &mut self,
        options: CompilerOptions,
        original: &JsString,
        compiled: Option<&JsString>,
    ) -> Result<(), Throwable> {
        self.test_parse_error(
            options,
            std::slice::from_ref(original),
            compiled.map(std::slice::from_ref),
        )
    }
    // port: IntegrationTestCase#testNoWarnings(CompilerOptions,String[])
    pub fn test_no_warnings(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
    ) -> Result<(), Throwable> {
        let c = self.compile(options, original)?;
        assert_that(c.borrow().get_errors().is_empty(), "Unexpected errors")?;
        assert_that(c.borrow().get_warnings().is_empty(), "Unexpected warnings")
    }
    // port: IntegrationTestCase#testParseError
    pub fn test_parse_error(
        &mut self,
        options: CompilerOptions,
        original: &[JsString],
        compiled: Option<&[JsString]>,
    ) -> Result<(), Throwable> {
        let c = self.compile(options, original)?;
        let parsing = passes::diagnostic_group("PARSING")?;
        for e in c.borrow().get_errors() {
            assert_that(
                parsing.matches(&e),
                format!("Found unexpected error type {}:\n{e}", e.get_type()),
            )?;
        }
        assert_that(
            c.borrow().get_warnings().is_empty(),
            format!(
                "Unexpected warnings: {}",
                c.borrow()
                    .get_warnings()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )?;
        if let Some(compiled) = compiled {
            let root = c.borrow().get_js_root().unwrap();
            let expected = self.parse_expected_code_after_compile(&c, compiled)?;
            crate::compiler_test_case::tree_equal_across(
                &c.borrow(),
                root,
                &expected.0.borrow(),
                expected.1,
                false,
                &IndexMap::<_, _>::default(),
            )?;
        }
        Ok(())
    }
    // port: IntegrationTestCase#parseExpectedCode (same options object after compile)
    fn parse_expected_code_after_compile(
        &self,
        actual: &CompilerHandle,
        original: &[JsString],
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        let mut options = actual.borrow().get_options().clone();
        let result = self.parse_expected_code(actual, original, &mut options);
        // Compiler owns its options in Rust. Carry mutations back to the same logical object.
        *actual.borrow_mut().get_options_mut() = options;
        result
    }
    // port: IntegrationTestCase#parseExpectedCode
    pub fn parse_expected_code(
        &self,
        _actual: &CompilerHandle,
        original: &[JsString],
        options: &mut CompilerOptions,
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        let old = options.get_process_common_js_modules();
        options.set_process_common_js_modules(false);
        let result = self.parse(_actual, original, options.clone())?;
        *options = result.0.borrow().get_options().clone();
        options.set_process_common_js_modules(old);
        *result.0.borrow_mut().get_options_mut() = options.clone();
        Ok(result)
    }
    // port: IntegrationTestCase#parse
    pub fn parse(
        &self,
        _actual: &CompilerHandle,
        original: &[JsString],
        options: CompilerOptions,
    ) -> Result<(CompilerHandle, NodeId), Throwable> {
        let c = self.create_compiler()?;
        let inputs = original
            .iter()
            .enumerate()
            .map(|(i, s)| {
                Arc::new(SourceFile::from_code(
                    &format!(
                        "{}{i}{}",
                        self.input_file_name_prefix, self.input_file_name_suffix
                    ),
                    s.clone(),
                ))
            })
            .collect::<Vec<_>>();
        c.borrow_mut().init(&self.externs, &inputs, options);
        check_unexpected_errors_or_warnings(
            &c.borrow().get_errors(),
            &c.borrow().get_warnings(),
            0,
        )?;
        c.borrow_mut().parse();
        check_unexpected_errors_or_warnings(
            &c.borrow().get_errors(),
            &c.borrow().get_warnings(),
            0,
        )?;
        let root = c.borrow().get_js_root().unwrap();
        Ok((c, root))
    }
}
// port: IntegrationTestCase#checkUnexpectedErrorsOrWarnings
pub fn check_unexpected_errors_or_warnings(
    errors: &[JSError],
    warnings: &[JSError],
    expected: usize,
) -> Result<(), Throwable> {
    let actual = errors.len() + warnings.len();
    if actual == expected {
        return Ok(());
    }
    let mut msg = String::new();
    for e in errors {
        msg.push_str(&format!("Error:{e}\n"));
    }
    for e in warnings {
        msg.push_str(&format!("Warning:{e}\n"));
    }
    assert_that(false, format!("Unexpected warnings or errors.\n {msg}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    // port: IntegrationTestCase#parseExpectedCode (post-compile shared options regression)
    #[test]
    fn expected_parse_observes_and_restores_post_compile_options() {
        let harness = IntegrationTestCase::set_up(vec![]);
        let actual = harness.create_compiler().unwrap();
        actual.borrow_mut().init_options(CompilerOptions::new());
        // Model a setter invoked by the processor after the initial options were consumed.
        actual
            .borrow_mut()
            .get_options_mut()
            .set_language_in(crate::jscomp_api::LanguageMode::ECMASCRIPT5);
        actual
            .borrow_mut()
            .get_options_mut()
            .set_process_common_js_modules(true);
        actual
            .borrow_mut()
            .get_options_mut()
            .set_use_types_for_local_optimization(true);
        let (expected, _) = harness
            .parse_expected_code_after_compile(&actual, &["var x;".into()])
            .unwrap();
        assert_eq!(
            expected.borrow().get_options().get_language_in(),
            crate::jscomp_api::LanguageMode::ECMASCRIPT5
        );
        assert!(
            actual
                .borrow()
                .get_options()
                .get_process_common_js_modules()
        );
        assert!(
            expected
                .borrow()
                .get_options()
                .get_process_common_js_modules()
        );
        // Expected Compiler#initOptions mutates the same logical options object too.
        assert!(
            !actual
                .borrow()
                .get_options()
                .should_use_types_for_local_optimization()
        );
    }
}
