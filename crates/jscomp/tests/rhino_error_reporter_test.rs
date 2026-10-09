/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/RhinoErrorReporterTest.java.

//! Port of `RhinoErrorReporterTest.java` (error message filtering of the
//! RhinoErrorReporter, seen through Compiler parsing).
use closure_jscomp::{
    Compiler,
    check_level::CheckLevel,
    compiler_options::{CompilerOptions, LanguageMode},
    diagnostic_groups,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    rhino_error_reporter,
    source_file::SourceFile,
};
use closure_parsing::js_doc_info_parser::BAD_TYPE_WIKI_LINK;
use std::sync::Arc;

struct RhinoErrorReporterTest {
    report_lint_warnings: bool,
    language_in: LanguageMode,
}

impl RhinoErrorReporterTest {
    // port: RhinoErrorReporterTest#setUp
    fn set_up() -> Self {
        Self {
            report_lint_warnings: true,
            language_in: LanguageMode::UNSUPPORTED,
        }
    }

    /// Verifies that the compiler emits an error for the given code.
    // port: RhinoErrorReporterTest#assertNoWarningOrError
    fn assert_no_warning_or_error(&self, code: &str) {
        let compiler = self.parse_code(code);
        assert_eq!(compiler.get_error_count(), 0, "Expected error");
        assert_eq!(compiler.get_error_count(), 0, "Expected warning");
    }

    /// Verifies that the compiler emits an error for the given code.
    // port: RhinoErrorReporterTest#assertError
    fn assert_error(
        &self,
        code: &str,
        type_: &'static DiagnosticType,
        description: &str,
    ) -> JSError {
        let compiler = self.parse_code(code);
        assert_eq!(compiler.get_error_count(), 1, "Expected error");

        let errors = compiler.get_errors();
        assert_eq!(errors.len(), 1);
        let error = errors.into_iter().next().unwrap();
        assert_eq!(error.get_type(), type_);
        assert_eq!(error.description(), description);
        error
    }

    /// Verifies that the compiler emits a warning for the given code.
    // port: RhinoErrorReporterTest#assertWarning
    fn assert_warning(
        &self,
        code: &str,
        type_: &'static DiagnosticType,
        description: &str,
    ) -> JSError {
        let compiler = self.parse_code(code);
        assert_eq!(compiler.get_warning_count(), 1, "Expected warning");

        let warnings = compiler.get_warnings();
        assert_eq!(warnings.len(), 1);
        let error = warnings.into_iter().next().unwrap();
        assert_eq!(error.get_type(), type_);
        assert_eq!(error.description(), description);
        error
    }

    // port: RhinoErrorReporterTest#parseCode
    fn parse_code(&self, code: &str) -> Compiler {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();

        if !self.report_lint_warnings {
            options.set_warning_level(diagnostic_groups::LINT_CHECKS.clone(), CheckLevel::OFF);
        } else {
            options.set_warning_level(diagnostic_groups::LINT_CHECKS.clone(), CheckLevel::WARNING);
            options.set_warning_level(
                diagnostic_groups::JSDOC_MISSING_TYPE.clone(),
                CheckLevel::WARNING,
            );
        }

        options.set_language_in(self.language_in);

        let externs: Vec<Arc<SourceFile>> = vec![];
        let inputs = vec![Arc::new(SourceFile::from_code("input", code))];
        compiler.init(&externs, &inputs, options);
        compiler.parse_inputs();
        compiler
    }
}

// port: RhinoErrorReporterTest#testMissingTypeWarnings
#[test]
fn test_missing_type_warnings() {
    let mut t = RhinoErrorReporterTest::set_up();
    t.report_lint_warnings = false;

    t.assert_no_warning_or_error("/** @return */ function f() {}");

    t.report_lint_warnings = true;

    let message = "Missing type declaration.";
    let error = t.assert_warning(
        "/** @return */ function f() {}",
        &rhino_error_reporter::JSDOC_MISSING_TYPE_WARNING,
        message,
    );

    assert_eq!(error.get_line_number(), 1);
    assert_eq!(error.charno(), 4);
}

// port: RhinoErrorReporterTest#testMissingCurlyBraceWarning
#[test]
fn test_missing_curly_brace_warning() {
    let mut t = RhinoErrorReporterTest::set_up();
    t.report_lint_warnings = false;
    t.assert_no_warning_or_error("/** @type string */ var x;");

    t.report_lint_warnings = true;
    t.assert_warning(
        "/** @type string */ var x;",
        &rhino_error_reporter::JSDOC_MISSING_BRACES_WARNING,
        &format!(
            "Bad type annotation. Type annotations should have curly braces.{BAD_TYPE_WIKI_LINK}"
        ),
    );
}

// port: RhinoErrorReporterTest#testLanguageFeatureInHigherLanguageInError
#[test]
fn test_language_feature_in_higher_language_in_error() {
    let mut t = RhinoErrorReporterTest::set_up();
    t.language_in = LanguageMode::ECMASCRIPT_2015;
    t.assert_error(
        "2 ** 3",
        &rhino_error_reporter::LANGUAGE_FEATURE,
        "This language feature is only supported for ECMASCRIPT_2016 mode or better: \
         exponent operator (**).",
    );
}
