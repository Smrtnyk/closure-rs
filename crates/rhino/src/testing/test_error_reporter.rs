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
//   src/com/google/javascript/rhino/testing/TestErrorReporter.java.

use crate::error_reporter::ErrorReporter;
use crate::fx_hash::IndexMap;
use crate::js_string::JsString;
#[derive(Default, Debug)]
pub struct TestErrorReporter {
    expected_errors: Vec<JsString>,
    expected_warnings: Vec<JsString>,
    seen_errors: IndexMap<JsString, JsString>,
    seen_warnings: IndexMap<JsString, JsString>,
}
impl ErrorReporter for TestErrorReporter {
    // port: TestErrorReporter#error
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.error_js_string(&message.into(), source_name, line, line_offset);
    }
    // port: TestErrorReporter#warning
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32) {
        self.warning_js_string(&message.into(), source_name, line, line_offset);
    }
    // port: TestErrorReporter#error
    fn error_js_string(
        &mut self,
        message: &JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.seen_errors.insert(
            self.fmt_diagnostic_key(message, source_name, line, line_offset),
            message.clone(),
        );
    }
    // port: TestErrorReporter#warning
    fn warning_js_string(
        &mut self,
        message: &JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        self.seen_warnings.insert(
            self.fmt_diagnostic_key(message, source_name, line, line_offset),
            message.clone(),
        );
    }
}
impl TestErrorReporter {
    // port: TestErrorReporter#TestErrorReporter
    pub fn new() -> Self {
        Self::default()
    }
    // port: TestErrorReporter#expectAllErrors
    pub fn expect_all_errors(&mut self, errors: &[&str]) -> &mut Self {
        self.expected_errors
            .extend(errors.iter().map(|s| JsString::from(*s)));
        self
    }
    // port: TestErrorReporter#expectAllWarnings
    pub fn expect_all_warnings(&mut self, warnings: &[&str]) -> &mut Self {
        self.expected_warnings
            .extend(warnings.iter().map(|s| JsString::from(*s)));
        self
    }
    // port: TestErrorReporter#expectAllErrors
    pub fn expect_all_errors_js_strings(&mut self, errors: &[JsString]) -> &mut Self {
        self.expected_errors.extend_from_slice(errors);
        self
    }
    // port: TestErrorReporter#expectAllWarnings
    pub fn expect_all_warnings_js_strings(&mut self, warnings: &[JsString]) -> &mut Self {
        self.expected_warnings.extend_from_slice(warnings);
        self
    }
    // port: TestErrorReporter#verifyHasEncounteredAllWarningsAndErrors
    pub fn verify_has_encountered_all_warnings_and_errors(&self) {
        assert_eq!(
            self.seen_warnings.values().collect::<Vec<_>>(),
            self.expected_warnings.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            self.seen_errors.values().collect::<Vec<_>>(),
            self.expected_errors.iter().collect::<Vec<_>>()
        );
    }
    // port: TestErrorReporter#fmtDiagnosticKey
    fn fmt_diagnostic_key(
        &self,
        message: &JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) -> JsString {
        message.concat(&format!(":{source_name}:{line}:{line_offset}").into())
    }
}
