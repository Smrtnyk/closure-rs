/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTestCase.java.

#![allow(dead_code)] // Each test binary uses a different subset of the fixture.
//! The part of CompilerTestCase#testInternal that a single-pass test with default settings
//! exercises: getOptions defaults, createCompiler, init/initChunks, parseInputs, the processor run,
//! ChangeVerifier, warning/error checks, validateCodeChangeReporting and the tree or string
//! comparison against parseExpectedJs. closure-testing's full CompilerTestCase also runs
//! AstValidator and SourceInfoCheck, which are stand-ins until port/harness-prereqs merges; the
//! unit records replay through that harness once they land.
use closure_jscomp::{
    Compiler,
    change_verifier::ChangeVerifier,
    check_level::CheckLevel,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    deps::module_loader::ResolutionMode,
    diagnostic_group::DiagnosticGroup,
    diagnostic_groups,
    diagnostic_type::DiagnosticType,
    google_coding_convention::GoogleCodingConvention,
    js::runtime_js_lib_manager::RuntimeLibraryMode,
    js_chunk::JSChunk,
    source_file::SourceFile,
};
use closure_parsing::config::JsDocParsing;
use closure_rhino::{
    js_string::JsString,
    node::{JsDocComparison, NodeId, RecursionMode, SideEffectComparison, TypeComparison},
    testing::node_subject::assert_node,
};
use closure_testing::jscomp_api::CompilerHarnessAccess;
use std::sync::Arc;

pub const GENERATED_SRC_NAME: &str = "testcode";
pub const GENERATED_EXTERNS_NAME: &str = "externs";

pub enum Sources {
    Flat(Vec<Arc<SourceFile>>),
    Chunks(Vec<JSChunk>),
}

pub struct Diagnostic {
    pub level: CheckLevel,
    pub diagnostic: &'static DiagnosticType,
    pub message: Option<String>,
}

impl Diagnostic {
    // port: CompilerTestCase.Diagnostic#withMessage
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = Some(message.to_string());
        self
    }
}

// port: CompilerTestCase#error(DiagnosticType)
pub fn error(diagnostic: &'static DiagnosticType) -> Diagnostic {
    Diagnostic {
        level: CheckLevel::ERROR,
        diagnostic,
        message: None,
    }
}
// port: CompilerTestCase#warning(DiagnosticType)
pub fn warning(diagnostic: &'static DiagnosticType) -> Diagnostic {
    Diagnostic {
        level: CheckLevel::WARNING,
        diagnostic,
        message: None,
    }
}

// port: CompilerTestCase#srcs(String)
pub fn srcs(code: &str) -> Sources {
    Sources::Flat(vec![Arc::new(SourceFile::from_code(
        GENERATED_SRC_NAME,
        code,
    ))])
}
// port: CompilerTestCase#srcs(String[])
pub fn srcs_many(code: &[&str]) -> Sources {
    Sources::Flat(create_sources(GENERATED_SRC_NAME, code))
}
// port: CompilerTestCase#srcs(JSChunk[])
pub fn srcs_chunks(chunks: Vec<JSChunk>) -> Sources {
    Sources::Chunks(chunks)
}
// port: CompilerTestCase#expected(String)
pub fn expected(code: &str) -> Vec<Arc<SourceFile>> {
    vec![Arc::new(SourceFile::from_code(GENERATED_SRC_NAME, code))]
}
// port: CompilerTestCase#expected(String[])
pub fn expected_many(code: &[&str]) -> Vec<Arc<SourceFile>> {
    create_sources(GENERATED_SRC_NAME, code)
}
// port: CompilerTestCase#externs(String)
pub fn externs(code: &str) -> Vec<Arc<SourceFile>> {
    vec![Arc::new(SourceFile::from_code(
        GENERATED_EXTERNS_NAME,
        code,
    ))]
}
// port: CompilerTestCase#expected(JSChunk[])
pub fn expected_chunks(chunks: &[JSChunk]) -> Vec<Arc<SourceFile>> {
    let mut files = Vec::new();
    for chunk in chunks {
        for input in chunk.get_inputs() {
            files.push(input.get_source_file_arc());
        }
    }
    files
}
// port: CompilerTestCase#createSources
fn create_sources(name: &str, sources: &[&str]) -> Vec<Arc<SourceFile>> {
    sources
        .iter()
        .enumerate()
        .map(|(i, s)| Arc::new(SourceFile::from_code(&format!("{name}{i}"), *s)))
        .collect()
}

pub struct PassTestCase {
    pub accepted_language: LanguageMode,
    pub language_out: LanguageMode,
    pub compare_as_tree: bool,
    pub compare_js_doc: bool,
    pub check_ast_change_marking: bool,
    pub allow_externs_changes: bool,
    pub ignored_warnings: Vec<&'static DiagnosticType>,
    pub default_externs_inputs: Vec<Arc<SourceFile>>,
}

impl PassTestCase {
    // port: CompilerTestCase#CompilerTestCase(String) + CompilerTestCase#setUp
    pub fn new(externs: &str) -> Self {
        Self {
            accepted_language: LanguageMode::UNSUPPORTED,
            language_out: LanguageMode::NO_TRANSPILE,
            compare_as_tree: true,
            compare_js_doc: true,
            check_ast_change_marking: true,
            allow_externs_changes: false,
            ignored_warnings: Vec::new(),
            default_externs_inputs: vec![Arc::new(SourceFile::from_code(
                GENERATED_EXTERNS_NAME,
                externs,
            ))],
        }
    }
    // port: CompilerTestCase#disableCompareAsTree
    pub fn disable_compare_as_tree(&mut self) {
        self.compare_as_tree = false;
    }
    // port: CompilerTestCase#ignoreWarnings
    pub fn ignore_warnings(&mut self, types: &[&'static DiagnosticType]) {
        self.ignored_warnings.extend_from_slice(types);
    }
    // port: CompilerTestCase#allowExternsChanges
    pub fn allow_externs_changes(&mut self) {
        self.allow_externs_changes = true;
    }
    // port: CompilerTestCase#getOptions
    pub fn get_options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        options.set_language_in(self.accepted_language);
        options.set_emit_use_strict(false);
        options.set_language_out(self.language_out);
        options.set_module_resolution_mode(ResolutionMode::BROWSER);
        options.set_parse_js_doc_documentation(JsDocParsing::TYPES_ONLY);
        options.set_assume_static_inheritance_is_not_used(true);
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
        options.set_coding_convention(Arc::new(GoogleCodingConvention::new()));
        options.set_polymer_version(Some(1));
        options.set_runtime_library_mode(RuntimeLibraryMode::RECORD_AND_VALIDATE_FIELDS);
        options
    }
    // port: CompilerTestCase#createCompiler
    fn create_compiler(&self) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.set_allowable_features(self.accepted_language.to_feature_set());
        compiler.set_prefer_regex_parser(false);
        compiler
    }
    // port: CompilerTestCase#test(Sources,Expected)
    pub fn test<P: CompilerPass>(
        &self,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        expected: Vec<Arc<SourceFile>>,
    ) {
        self.test_internal(get_processor, inputs, Some(expected), &[]);
    }
    // port: CompilerTestCase#testSame(Sources)
    pub fn test_same<P: CompilerPass>(
        &self,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
    ) {
        let expected = match &inputs {
            Sources::Flat(files) => files.clone(),
            Sources::Chunks(chunks) => expected_chunks(chunks),
        };
        self.test_internal(get_processor, inputs, Some(expected), &[]);
    }
    // port: CompilerTestCase#testError(Sources,Diagnostic)
    pub fn test_error<P: CompilerPass>(
        &self,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        diagnostic: Diagnostic,
    ) {
        assert!(diagnostic.level == CheckLevel::ERROR);
        self.test_internal(get_processor, inputs, None, &[diagnostic]);
    }
    // port: CompilerTestCase#testWarning(Sources,Diagnostic)
    pub fn test_warning<P: CompilerPass>(
        &self,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        diagnostic: Diagnostic,
    ) {
        assert!(diagnostic.level == CheckLevel::WARNING);
        self.test_internal(get_processor, inputs, None, &[diagnostic]);
    }

    // port: CompilerTestCase#test(Externs,Sources,Expected)
    pub fn test_with_externs<P: CompilerPass>(
        &self,
        externs: Vec<Arc<SourceFile>>,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        expected: Vec<Arc<SourceFile>>,
    ) {
        self.test_internal_with_externs(&externs, get_processor, inputs, Some(expected), &[]);
    }
    // port: CompilerTestCase#testSame(Externs,Sources)
    pub fn test_same_with_externs<P: CompilerPass>(
        &self,
        externs: Vec<Arc<SourceFile>>,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
    ) {
        let expected = match &inputs {
            Sources::Flat(files) => files.clone(),
            Sources::Chunks(chunks) => expected_chunks(chunks),
        };
        self.test_internal_with_externs(&externs, get_processor, inputs, Some(expected), &[]);
    }

    // port: CompilerTestCase#testInternal(Externs,Sources,Expected,List,List) (default externs)
    pub fn test_internal<P: CompilerPass>(
        &self,
        get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        expected: Option<Vec<Arc<SourceFile>>>,
        diagnostics: &[Diagnostic],
    ) {
        let externs = self.default_externs_inputs.clone();
        self.test_internal_with_externs(&externs, get_processor, inputs, expected, diagnostics);
    }

    // port: CompilerTestCase#testInternal(Externs,Sources,Expected,List,List)
    pub fn test_internal_with_externs<P: CompilerPass>(
        &self,
        externs: &[Arc<SourceFile>],
        mut get_processor: impl FnMut(&mut Compiler) -> P,
        inputs: Sources,
        expected: Option<Vec<Arc<SourceFile>>>,
        diagnostics: &[Diagnostic],
    ) {
        let mut compiler = self.create_compiler();
        match inputs {
            Sources::Flat(files) => compiler.init(externs, &files, self.get_options()),
            Sources::Chunks(chunks) => compiler.init_chunks(externs, chunks, self.get_options()),
        }
        let expected_errors: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.level == CheckLevel::ERROR)
            .collect();
        let expected_warnings: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|d| d.level == CheckLevel::WARNING)
            .collect();
        compiler.add_recent_change_handler();
        let Some(root) = compiler.parse_inputs() else {
            check_diagnostics(&compiler.get_errors(), &expected_errors, "parse errors");
            return;
        };
        assert!(
            compiler.get_warnings().is_empty(),
            "Unexpected parser warning(s): {:?}",
            descriptions(&compiler.get_warnings())
        );
        let externs_root = root.get_first_child(&compiler).unwrap();
        let main_root = root.get_last_child(&compiler).unwrap();
        let root_clone = root.clone_tree(&mut compiler);
        let externs_root_clone = root_clone.get_first_child(&compiler).unwrap();
        let main_root_clone = root_clone.get_last_child(&compiler).unwrap();

        let mut has_code_changed = false;
        if compiler.get_errors().is_empty() {
            compiler.reset_recent_change();
            let change_verifier = if self.check_ast_change_marking {
                Some(ChangeVerifier::new(&compiler).snapshot(&mut compiler, main_root))
            } else {
                None
            };
            let mut processor = get_processor(&mut compiler);
            processor.process(&mut compiler, externs_root, main_root);
            if let Some(verifier) = change_verifier {
                verifier.check_recorded_changes(&mut compiler, main_root);
            }
            has_code_changed |= compiler.has_code_changed();
        }

        if expected_errors.is_empty() {
            assert!(
                compiler.get_errors().is_empty(),
                "Unexpected compile errors: {:?}",
                descriptions(&compiler.get_errors())
            );
            check_diagnostics(&compiler.get_warnings(), &expected_warnings, "warnings");
            self.validate_code_change_reporting(
                &compiler,
                main_root,
                externs_root,
                main_root_clone,
                externs_root_clone,
                has_code_changed,
            );
            if let Some(expected) = expected {
                if self.compare_as_tree {
                    let (expected_compiler, expected_root) = self.parse_expected_js(&expected);
                    if let Err(message) = assert_node(main_root).check_equal_to_across(
                        &compiler,
                        &expected_compiler,
                        expected_root,
                        self.compare_js_doc,
                    ) {
                        panic!("{message}");
                    }
                } else {
                    let actual = compiler.to_source_for_node_utf16(main_root);
                    let mut want = Vec::new();
                    for file in &expected {
                        want.extend_from_slice(file.get_code().unwrap().as_units());
                    }
                    let actual = replace_spaces_before_newline(&actual);
                    assert_eq!(
                        actual.to_string(),
                        JsString::from_units(want).to_string(),
                        "compiled source differs"
                    );
                }
            }
        } else {
            check_diagnostics(&compiler.get_errors(), &expected_errors, "compile errors");
        }
    }

    // port: CompilerTestCase#validateCodeChangeReporting
    fn validate_code_change_reporting(
        &self,
        compiler: &Compiler,
        main_root: NodeId,
        externs_root: NodeId,
        main_root_clone: NodeId,
        externs_root_clone: NodeId,
        was_code_change_reported: bool,
    ) {
        let code_change = !main_root_clone.is_equivalent_to_with_options(
            compiler,
            main_root,
            RecursionMode::DEEP_NO_SHADOW,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::COMPARE,
        );
        let externs_change = !externs_root_clone.is_equivalent_to_with_options(
            compiler,
            externs_root,
            RecursionMode::DEEP_NO_SHADOW,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::COMPARE,
        );
        if externs_change
            && !self.allow_externs_changes
            && let Err(message) =
                assert_node(externs_root).check_equal_to(compiler, externs_root_clone, false)
        {
            panic!("{message}");
        }
        if self.check_ast_change_marking {
            if !code_change && !externs_change {
                assert!(
                    !was_code_change_reported,
                    "compiler.reportCodeChange() was called even though nothing changed"
                );
            } else {
                assert!(
                    was_code_change_reported,
                    "compiler.reportCodeChange() should have been called."
                );
            }
        }
    }

    // port: CompilerTestCase#parseExpectedJs
    fn parse_expected_js(&self, inputs: &[Arc<SourceFile>]) -> (Compiler, NodeId) {
        let mut compiler = self.create_compiler();
        compiler.init(&self.default_externs_inputs, inputs, self.get_options());
        let root = compiler.parse_inputs();
        let Some(root) = root else {
            panic!(
                "Unexpected parse error(s): {:?}",
                descriptions(&compiler.get_errors())
            );
        };
        let externs_root = root.get_first_child(&compiler).unwrap();
        let main_root = externs_root.get_next(&compiler).unwrap();
        (compiler, main_root)
    }
}

fn descriptions(errors: &[closure_jscomp::js_error::JSError]) -> Vec<String> {
    errors.iter().map(|e| e.description().to_string()).collect()
}

// Java: assertThat(actual).comparingElementsUsing(DIAGNOSTIC_CORRESPONDENCE).containsExactly...
fn check_diagnostics(
    actual: &[closure_jscomp::js_error::JSError],
    expected: &[&Diagnostic],
    what: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: expected {:?}, got {:?}",
        expected
            .iter()
            .map(|d| d.diagnostic.key)
            .collect::<Vec<_>>(),
        descriptions(actual)
    );
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(
            a.get_type().key,
            e.diagnostic.key,
            "{what}: wrong diagnostic"
        );
        if let Some(message) = &e.message {
            assert_eq!(a.description(), message, "{what}: wrong message");
        }
    }
}

// Java: compiler.toSource(mainRoot).replaceAll(" +\n", "\n")
fn replace_spaces_before_newline(source: &JsString) -> JsString {
    let mut out: Vec<u16> = Vec::new();
    for c in source.as_units() {
        if *c == 10 {
            while out.last() == Some(&32) {
                out.pop();
            }
        }
        out.push(*c);
    }
    JsString::from_units(out)
}
