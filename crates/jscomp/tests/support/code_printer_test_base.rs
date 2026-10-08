/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CodePrinterTest.java,
//   test/com/google/javascript/jscomp/CodePrinterTestBase.java.

#![allow(dead_code)] // Shared helpers mirror the Java fixture, including helpers used by later ports.
use closure_jscomp::{
    Compiler,
    code_printer::Builder,
    compiler_options::{CompilerOptions, LanguageMode},
    default_pass_config::DefaultPassConfig,
    source_file::SourceFile,
};
use closure_parsing::config::JsDocParsing;
use closure_rhino::{js_string::JsString, node::NodeId};
use std::sync::Arc;
pub struct CodePrinterTestBase {
    pub allow_warnings: bool,
    pub diagnostics_to_ignore: Vec<Arc<closure_jscomp::diagnostic_group::DiagnosticGroup>>,
    pub trusted_strings: bool,
    pub preserve_type_annotations: bool,
    pub preserve_non_jsdoc_comments: bool,
    pub language_mode: LanguageMode,
    pub output_charset: Option<String>,
    pub last_compiler: Compiler,
}
impl CodePrinterTestBase {
    // port: CodePrinterTestBase#setUp
    pub fn new() -> Self {
        Self {
            allow_warnings: false,
            diagnostics_to_ignore: Vec::new(),
            trusted_strings: true,
            preserve_type_annotations: false,
            preserve_non_jsdoc_comments: false,
            language_mode: LanguageMode::UNSUPPORTED,
            output_charset: None,
            last_compiler: Compiler::new(),
        }
    }
    // port: CodePrinterTestBase#parse(String)
    pub fn parse(&mut self, js: impl Into<JsString>) -> NodeId {
        self.parse_type_checked(js, /* typeChecked= */ false)
    }
    // port: CodePrinterTestBase#parse(String,boolean)
    pub fn parse_type_checked(&mut self, js: impl Into<JsString>, type_checked: bool) -> NodeId {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_trusted_strings(self.trusted_strings);
        options.set_preserve_type_annotations(self.preserve_type_annotations);
        if let Some(charset) = &self.output_charset {
            options.set_output_charset(closure_rhino::java_lang::charset::Charset::for_name(
                charset,
            ));
        }
        options.set_language_in(self.language_mode);
        if self.preserve_non_jsdoc_comments {
            options.set_parse_js_doc_documentation(JsDocParsing::INCLUDE_ALL_COMMENTS);
            options.set_preserve_non_jsdoc_comments(true);
        }
        options.set_continue_after_errors(true);
        compiler.init(
            &[Arc::new(SourceFile::from_code(
                "externs",
                include_str!("minimal_externs.js"),
            ))],
            &[Arc::new(SourceFile::from_code("testcode", js))],
            options,
        );
        let externs_and_js = compiler.parse_inputs().unwrap();
        self.check_unexpected_errors_or_warnings(&compiler, 0);
        let root = externs_and_js.get_last_child(&compiler).unwrap();
        let externs = externs_and_js.get_first_child(&compiler).unwrap();

        if type_checked {
            // Java passes null options: the factory does not read them.
            let pass_config = DefaultPassConfig::new(CompilerOptions::new());
            let mut infer_types = pass_config.infer_types.create(&mut compiler);
            infer_types.process(&mut compiler, externs, root);
        }

        self.check_unexpected_errors_or_warnings(&compiler, 0);
        let node = root.get_first_child(&compiler).unwrap();
        self.last_compiler = compiler;
        node
    }
    // port: CodePrinterTestBase#checkUnexpectedErrorsOrWarnings
    fn check_unexpected_errors_or_warnings(&self, compiler: &Compiler, expected: usize) {
        let mut actual = 0;
        let mut msg = String::new();
        for error in compiler.get_errors() {
            if self.should_ignore(&error) {
                continue;
            }
            actual += 1;
            msg.push_str(&format!("Error:{error}\n"));
        }
        if !self.allow_warnings {
            for error in compiler.get_warnings() {
                if self.should_ignore(&error) {
                    continue;
                }
                actual += 1;
                msg.push_str(&format!("Warning:{error}\n"));
            }
        }
        if actual != expected {
            assert_eq!(actual, expected, "Unexpected warnings or errors.\n {msg}");
        }
    }
    // port: CodePrinterTestBase#shouldIgnore
    fn should_ignore(&self, error: &closure_jscomp::js_error::JSError) -> bool {
        for diagnostic_group in &self.diagnostics_to_ignore {
            if diagnostic_group.matches(error) {
                return true;
            }
        }
        false
    }
    // port: CodePrinterTestBase#newCompilerOptions
    pub fn new_compiler_options(&self) -> CompilerOptions {
        let mut options = CompilerOptions::new();
        if let Some(charset) = &self.output_charset {
            options.set_output_charset(closure_rhino::java_lang::charset::Charset::for_name(
                charset,
            ));
        }
        options.set_trusted_strings(self.trusted_strings);
        options.set_preserve_type_annotations(self.preserve_type_annotations);
        options.set_preserve_non_jsdoc_comments(self.preserve_non_jsdoc_comments);
        options.set_language_out(self.language_mode);
        options
    }
    // port: CodePrinterTestBase#parsePrint
    pub fn parse_print(&mut self, js: impl Into<JsString>, options: &CompilerOptions) -> JsString {
        let node = self.parse(js);
        Builder::new(node)
            .set_compiler_options(options)
            .build(&self.last_compiler)
    }
    // port: CodePrinterTestBase#assertPrint
    pub fn assert_print(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        let expected = expected.into();
        self.parse(expected.clone());
        let mut options = self.new_compiler_options();
        options.set_pretty_print(false);
        options.set_line_length_threshold(CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
        let actual = self.parse_print(js, &options);
        assert_eq!(java_trim(&actual), java_trim(&expected));
    }
    // port: CodePrinterTestBase#assertPrintSame
    pub fn assert_print_same(&mut self, js: impl Into<JsString>) {
        let js = js.into();
        self.assert_print(js.clone(), js);
    }
    // port: CodePrinterTest#assertPrettyPrintSame
    pub fn assert_pretty_print_same(&mut self, js: impl Into<JsString>) {
        let js = js.into();
        self.assert_pretty_print(js.clone(), js);
    }
    // port: CodePrinterTest#assertPrettyPrint
    pub fn assert_pretty_print(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        self.assert_pretty_print_options(js, expected, |_| {});
    }
    // port: CodePrinterTest#assertPrettyPrint(String,String,CompilerOptionBuilder)
    pub fn assert_pretty_print_options(
        &mut self,
        js: impl Into<JsString>,
        expected: impl Into<JsString>,
        configure: impl FnOnce(&mut CompilerOptions),
    ) {
        let mut options = self.new_compiler_options();
        options.set_pretty_print(true);
        options.set_preserve_type_annotations(true);
        options.set_line_break(false);
        options.set_line_length_threshold(CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
        configure(&mut options);
        assert_eq!(self.parse_print(js, &options), expected.into());
    }
    // port: CodePrinterTest#assertLineBreak
    pub fn assert_line_break(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        self.assert_line(js, expected, CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
    }
    // port: CodePrinterTest#assertLineLength
    pub fn assert_line_length(&mut self, js: impl Into<JsString>, expected: impl Into<JsString>) {
        self.assert_line(js, expected, 10);
    }
    fn assert_line(
        &mut self,
        js: impl Into<JsString>,
        expected: impl Into<JsString>,
        threshold: i32,
    ) {
        let mut options = self.new_compiler_options();
        options.set_pretty_print(false);
        options.set_line_break(true);
        options.set_line_length_threshold(threshold);
        assert_eq!(self.parse_print(js, &options), expected.into());
    }
    // port: CodePrinterTest#assertPrintNumber(String,double)
    pub fn assert_print_number(&mut self, expected: &str, number: f64) {
        self.assert_print(closure_rhino::java_lang::double_to_string(number), expected);
        let n = self.last_compiler.new_number(number);
        self.assert_print_node(expected, n);
    }
    // port: CodePrinterTest#assertPrintNumber(String,int)
    pub fn assert_print_number_int(&mut self, expected: &str, number: i32) {
        self.assert_print(number.to_string(), expected);
        let n = self.last_compiler.new_number(number as f64);
        self.assert_print_node(expected, n);
    }
    // port: CodePrinterTestBase#printNode
    pub fn print_node(&self, node: NodeId) -> JsString {
        let mut options = CompilerOptions::new();
        options.set_language_out(self.language_mode);
        if let Some(charset) = &self.output_charset {
            options.set_output_charset(closure_rhino::java_lang::charset::Charset::for_name(
                charset,
            ));
        }
        options.set_line_length_threshold(CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
        Builder::new(node)
            .set_compiler_options(&options)
            .build(&self.last_compiler)
    }
    // port: CodePrinterTestBase#prettyPrintNode
    pub fn pretty_print_node(&self, node: NodeId) -> JsString {
        let mut options = CompilerOptions::new();
        options.set_language_out(self.language_mode);
        if let Some(charset) = &self.output_charset {
            options.set_output_charset(closure_rhino::java_lang::charset::Charset::for_name(
                charset,
            ));
        }
        options.set_line_length_threshold(CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
        options.set_pretty_print(true);
        Builder::new(node)
            .set_compiler_options(&options)
            .build(&self.last_compiler)
    }
    // port: CodePrinterTestBase#assertPrintNode
    pub fn assert_print_node(&self, expected: impl Into<JsString>, node: NodeId) {
        assert_eq!(self.print_node(node), expected.into());
    }
    // port: CodePrinterTestBase#assertPrettyPrintNode
    pub fn assert_pretty_print_node(&self, expected: impl Into<JsString>, node: NodeId) {
        assert_eq!(self.pretty_print_node(node), expected.into());
    }
    // port: CodePrinterTest#assertTypeAnnotations
    pub fn assert_type_annotations(
        &mut self,
        js: impl Into<JsString>,
        expected: impl Into<JsString>,
    ) {
        let node = self.parse_type_checked(js, /* typeChecked= */ true);
        let mut options = self.new_compiler_options();
        options.set_pretty_print(true);
        options.set_line_break(false);
        options.set_line_length_threshold(CompilerOptions::DEFAULT_LINE_LENGTH_THRESHOLD);
        let (registry, ast) = self.last_compiler.get_type_registry_and_ast();
        let actual = Builder::new(node)
            .set_compiler_options(&options)
            .set_output_types(true)
            .set_type_registry(registry)
            .build(ast);

        assert_eq!(actual, expected.into());
    }
    /// Java returns the configured Builder; the Rust Builder borrows its options and the
    /// registry, so the caller's further settings come in as `configure` and this builds.
    // port: CodePrinterTest#defaultBuilder
    pub fn default_builder_build(
        &mut self,
        js_root: NodeId,
        configure: impl FnOnce(Builder<'_>) -> Builder<'_>,
    ) -> JsString {
        let mut options = self.new_compiler_options();
        options.set_pretty_print(false);
        options.set_line_break(false);
        options.set_line_length_threshold(0);
        let (registry, ast) = self.last_compiler.get_type_registry_and_ast();
        let builder = Builder::new(js_root)
            .set_compiler_options(&options)
            .set_output_types(false)
            .set_type_registry(registry);
        configure(builder).build(ast)
    }
    // port: CodePrinterTest#testReparse
    pub fn test_reparse(&mut self, code: impl Into<JsString>) {
        let once = self.parse(code);
        let printed = Builder::new(once).build(&self.last_compiler);
        let twice = self.last_compiler.parse_test_code(printed);
        assert!(once.is_equivalent_to(&self.last_compiler, twice));
    }
}
fn java_trim(s: &JsString) -> JsString {
    let units = s.as_units();
    let start = units.iter().position(|c| *c > 32).unwrap_or(units.len());
    let end = units.iter().rposition(|c| *c > 32).map_or(start, |i| i + 1);
    JsString::from_units(units[start..end].to_vec())
}
