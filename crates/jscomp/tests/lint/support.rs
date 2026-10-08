/*
 * Copyright 2026 The closure-rs Authors.
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

//! Local test fixture for the lint check tests: the subset of `CompilerTestCase` these Java tests
//! use (`testWarning`, `testError`, `testSame`, `testNoWarning`, `test`), built on the real
//! `Compiler`. The corpus records exercise the full harness (crates/testing); this fixture only
//! lets the ported JUnit methods run natively.

use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    code_printer::Builder,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    diagnostic_group::DiagnosticGroup,
    diagnostic_groups,
    diagnostic_type::DiagnosticType,
    google_coding_convention::GoogleCodingConvention,
    js_error::JSError,
    source_file::SourceFile,
};
use closure_parsing::config::JsDocParsing;
use std::sync::Arc;

pub type ProcessorFactory = Box<dyn Fn(&mut Compiler) -> Box<dyn CompilerPass>>;
pub type OptionsHook = Box<dyn Fn(&mut CompilerOptions)>;

pub struct LintTestCase {
    processor: ProcessorFactory,
    options_hook: Option<OptionsHook>,
    pub accepted_language: LanguageMode,
    pub parse_js_doc_documentation: JsDocParsing,
    pub externs: String,
    pub expect_parse_warnings: bool,
    pub ignored_warnings: Vec<&'static DiagnosticType>,
    /// `CompilerTestCase#ignoreWarnings(DiagnosticGroup...)`
    pub ignored_groups: Vec<Arc<DiagnosticGroup>>,
}

/// Result of one compile through the fixture.
pub struct Run {
    pub compiler: Compiler,
    pub errors: Vec<JSError>,
    pub warnings: Vec<JSError>,
}

impl LintTestCase {
    pub fn new(processor: impl Fn(&mut Compiler) -> Box<dyn CompilerPass> + 'static) -> Self {
        Self {
            processor: Box::new(processor),
            options_hook: None,
            accepted_language: LanguageMode::UNSUPPORTED,
            parse_js_doc_documentation: JsDocParsing::TYPES_ONLY,
            externs: String::new(),
            expect_parse_warnings: false,
            ignored_warnings: Vec::new(),
            ignored_groups: Vec::new(),
        }
    }

    /// The test class's `getOptions()` override, applied after CompilerTestCase's defaults.
    pub fn with_options(mut self, hook: impl Fn(&mut CompilerOptions) + 'static) -> Self {
        self.options_hook = Some(Box::new(hook));
        self
    }

    /// Lint test classes commonly set `DiagnosticGroups.LINT_CHECKS` to WARNING.
    pub fn with_lint_checks_warning(self) -> Self {
        self.with_options(|o| {
            o.set_warning_level(diagnostic_groups::LINT_CHECKS.clone(), CheckLevel::WARNING)
        })
    }

    pub fn options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        options.set_language_in(self.accepted_language);
        options.set_emit_use_strict(false);
        options.set_language_out(LanguageMode::NO_TRANSPILE);
        options.set_parse_js_doc_documentation(self.parse_js_doc_documentation);
        options.set_preserve_type_annotations(true);
        options.set_assume_getters_are_pure(false);
        options.set_check_symbols(true);
        options.set_warning_level(
            diagnostic_groups::INVALID_CASTS.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISPLACED_MSG_ANNOTATION.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISSING_PROPERTIES.clone(),
            CheckLevel::WARNING,
        );
        if !self.ignored_warnings.is_empty() {
            options.set_warning_level(
                Arc::new(DiagnosticGroup::new(&self.ignored_warnings)),
                CheckLevel::OFF,
            );
        }
        for group in &self.ignored_groups {
            options.set_warning_level(group.clone(), CheckLevel::OFF);
        }
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        options.set_polymer_version(Some(1));
        if let Some(hook) = &self.options_hook {
            hook(&mut options);
        }
        options
    }

    pub fn run(&self, inputs: &[&str]) -> Run {
        let names: Vec<String> = (0..inputs.len()).map(|i| format!("input{i}")).collect();
        let files: Vec<(&str, &str)> = names
            .iter()
            .zip(inputs)
            .map(|(name, js)| (name.as_str(), *js))
            .collect();
        self.run_files(&files)
    }

    /// Compiles `SourceFile.fromCode(name, code)` inputs.
    pub fn run_files(&self, inputs: &[(&str, &str)]) -> Run {
        self.run_files_with_externs(self.externs.as_str(), inputs)
    }

    /// `test(externs(code), srcs(...))`: compiles with the given externs instead of the default.
    pub fn run_with_externs(&self, externs: &str, inputs: &[&str]) -> Run {
        let names: Vec<String> = (0..inputs.len()).map(|i| format!("input{i}")).collect();
        let files: Vec<(&str, &str)> = names
            .iter()
            .zip(inputs)
            .map(|(name, js)| (name.as_str(), *js))
            .collect();
        self.run_files_with_externs(externs, &files)
    }

    fn run_files_with_externs(&self, externs: &str, inputs: &[(&str, &str)]) -> Run {
        let mut compiler = Compiler::new();
        compiler.set_allowable_features(self.accepted_language.to_feature_set());
        let externs = vec![Arc::new(SourceFile::from_code("externs", externs))];
        let sources: Vec<_> = inputs
            .iter()
            .map(|(name, js)| Arc::new(SourceFile::from_code(name, *js)))
            .collect();
        compiler.init(&externs, &sources, self.options());
        compiler.parse_inputs();
        let parse_errors = compiler.get_errors();
        assert!(
            parse_errors.is_empty(),
            "Unexpected parse error(s): {:?}",
            parse_errors
                .iter()
                .map(|e| e.description().to_string())
                .collect::<Vec<_>>()
        );
        if !self.expect_parse_warnings {
            let parse_warnings = compiler.get_warnings();
            assert!(
                parse_warnings.is_empty(),
                "Unexpected parse warning(s): {:?}",
                parse_warnings
                    .iter()
                    .map(|e| e.description().to_string())
                    .collect::<Vec<_>>()
            );
        }
        let externs_root = compiler.get_externs_root().unwrap();
        let main_root = compiler.get_js_root().unwrap();
        let mut pass = (self.processor)(&mut compiler);
        pass.process(&mut compiler, externs_root, main_root);
        let errors = compiler.get_errors();
        let warnings = compiler.get_warnings();
        Run {
            compiler,
            errors,
            warnings,
        }
    }

    fn print_inputs(&self, inputs: &[&str]) -> String {
        let mut compiler = Compiler::new();
        compiler.set_allowable_features(self.accepted_language.to_feature_set());
        let sources: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(i, js)| Arc::new(SourceFile::from_code(&format!("expected{i}"), *js)))
            .collect();
        compiler.init(&[], &sources, self.options());
        compiler.parse_inputs();
        let root = compiler.get_js_root().unwrap();
        Builder::new(root).build(&compiler).to_string_lossy()
    }

    fn types(list: &[JSError]) -> Vec<&'static str> {
        list.iter().map(|e| e.get_type().key).collect()
    }

    fn check_diagnostics(
        run: &Run,
        errors: &[&'static DiagnosticType],
        warnings: &[&'static DiagnosticType],
    ) {
        let want_errors: Vec<_> = errors.iter().map(|d| d.key).collect();
        let want_warnings: Vec<_> = warnings.iter().map(|d| d.key).collect();
        assert_eq!(
            Self::types(&run.errors),
            want_errors,
            "errors: {:?}",
            run.errors
                .iter()
                .map(|e| e.description().to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            Self::types(&run.warnings),
            want_warnings,
            "warnings: {:?}",
            run.warnings
                .iter()
                .map(|e| e.description().to_string())
                .collect::<Vec<_>>()
        );
    }

    /// `CompilerTestCase#testWarning(String, DiagnosticType)`
    pub fn test_warning(&self, js: &str, d: &'static DiagnosticType) {
        self.test_warning_sources(&[js], d);
    }

    /// `CompilerTestCase#testWarning(Sources, Diagnostic)`
    pub fn test_warning_sources(&self, js: &[&str], d: &'static DiagnosticType) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], &[d]);
    }

    /// `CompilerTestCase#testWarning(String, DiagnosticType, String)`
    pub fn test_warning_message(&self, js: &str, d: &'static DiagnosticType, message: &str) {
        let run = self.run(&[js]);
        Self::check_diagnostics(&run, &[], &[d]);
        assert_eq!(run.warnings[0].description(), message);
    }

    /// `test(srcs(js), warning(d), warning(d)...)`: several expected warnings in order.
    pub fn test_warnings(&self, js: &str, ds: &[&'static DiagnosticType]) {
        let run = self.run(&[js]);
        Self::check_diagnostics(&run, &[], ds);
    }

    /// `test(srcs(js...), warning(d)...)` / `testWarning(srcs(js...), warning(d))`: the expected
    /// warnings in order.
    pub fn test_warnings_sources(&self, js: &[&str], ds: &[&'static DiagnosticType]) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], ds);
    }

    /// `test(srcs(js...), warning(d).withMessageContaining(text))`
    pub fn test_warning_containing(&self, js: &[&str], d: &'static DiagnosticType, text: &str) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], &[d]);
        let description = run.warnings[0].description();
        assert!(
            description.contains(text),
            "{description:?} does not contain {text:?}"
        );
    }

    /// `testSame(srcs(js...), warning(d).withMessageContaining(text))`
    pub fn test_same_warning_containing(
        &self,
        js: &[&str],
        d: &'static DiagnosticType,
        text: &str,
    ) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], &[d]);
        let description = run.warnings[0].description();
        assert!(
            description.contains(text),
            "{description:?} does not contain {text:?}"
        );
        let root = run.compiler.get_js_root().unwrap();
        let actual = Builder::new(root).build(&run.compiler).to_string_lossy();
        assert_eq!(actual, self.print_inputs(js));
    }

    /// `CompilerTestCase#testError(String, DiagnosticType)`
    pub fn test_error(&self, js: &str, d: &'static DiagnosticType) {
        let run = self.run(&[js]);
        Self::check_diagnostics(&run, &[d], &[]);
    }

    /// `CompilerTestCase#testError(Sources, DiagnosticType)`
    pub fn test_error_sources(&self, js: &[&str], d: &'static DiagnosticType) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[d], &[]);
    }

    /// `CompilerTestCase#testError(String, DiagnosticType, String)`
    pub fn test_error_message(&self, js: &str, d: &'static DiagnosticType, message: &str) {
        let run = self.run(&[js]);
        Self::check_diagnostics(&run, &[d], &[]);
        assert_eq!(run.errors[0].description(), message);
    }

    /// `CompilerTestCase#testNoWarning(String)`
    pub fn test_no_warning(&self, js: &str) {
        self.test_no_warning_sources(&[js]);
    }

    /// `CompilerTestCase#testNoWarning(Sources)`
    pub fn test_no_warning_sources(&self, js: &[&str]) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], &[]);
    }

    /// `CompilerTestCase#testNoWarning(srcs(SourceFile.fromCode(name, code)...))`
    pub fn test_no_warning_files(&self, files: &[(&str, &str)]) {
        let run = self.run_files(files);
        Self::check_diagnostics(&run, &[], &[]);
    }

    /// `test(srcs(SourceFile...), warning(d).withMessageContaining(text))`
    pub fn test_warning_files_containing(
        &self,
        files: &[(&str, &str)],
        d: &'static DiagnosticType,
        text: &str,
    ) {
        let run = self.run_files(files);
        Self::check_diagnostics(&run, &[], &[d]);
        let description = run.warnings[0].description();
        assert!(
            description.contains(text),
            "{description:?} does not contain {text:?}"
        );
    }

    /// `test(externs(code), srcs(js...), warning(d)...)`: diagnostics only.
    pub fn test_externs_warnings(
        &self,
        externs: &str,
        js: &[&str],
        ds: &[&'static DiagnosticType],
    ) {
        let run = self.run_with_externs(externs, js);
        Self::check_diagnostics(&run, &[], ds);
    }

    /// `testSame(externs(code), srcs(js...))`
    pub fn test_same_externs(&self, externs: &str, js: &[&str]) {
        let run = self.run_with_externs(externs, js);
        Self::check_diagnostics(&run, &[], &[]);
        let root = run.compiler.get_js_root().unwrap();
        let actual = Builder::new(root).build(&run.compiler).to_string_lossy();
        assert_eq!(actual, self.print_inputs(js));
    }

    /// `CompilerTestCase#testSame(String)`
    pub fn test_same(&self, js: &str) {
        self.test_sources(&[js], &[js]);
    }

    /// `CompilerTestCase#testSame(Sources)`
    pub fn test_same_sources(&self, js: &[&str]) {
        self.test_sources(js, js);
    }

    /// `CompilerTestCase#testSame(String, DiagnosticType)`: same output plus one warning.
    pub fn test_same_warning(&self, js: &str, d: &'static DiagnosticType) {
        let run = self.run(&[js]);
        Self::check_diagnostics(&run, &[], &[d]);
        let root = run.compiler.get_js_root().unwrap();
        let actual = Builder::new(root).build(&run.compiler).to_string_lossy();
        assert_eq!(actual, self.print_inputs(&[js]));
    }

    /// `CompilerTestCase#test(String, String)`
    pub fn test(&self, js: &str, expected: &str) {
        self.test_sources(&[js], &[expected]);
    }

    /// `CompilerTestCase#test(Sources, Expected)`
    pub fn test_sources(&self, js: &[&str], expected: &[&str]) {
        let run = self.run(js);
        Self::check_diagnostics(&run, &[], &[]);
        let root = run.compiler.get_js_root().unwrap();
        let actual = Builder::new(root).build(&run.compiler).to_string_lossy();
        assert_eq!(actual, self.print_inputs(expected));
    }
}
