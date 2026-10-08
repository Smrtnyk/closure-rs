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
 *   Norris Boyd
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
//   src/com/google/javascript/rhino/ErrorReporter.java.

pub trait ErrorReporter {
    // port: ErrorReporter#warning
    fn warning(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32);
    // port: ErrorReporter#error
    fn error(&mut self, message: &str, source_name: &str, line: i32, line_offset: i32);
    // port: ErrorReporter#warning (Java String can contain unpaired UTF-16 surrogates)
    fn warning_js_string(
        &mut self,
        message: &crate::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        // UTF-8-only reporters accept scalar messages; WTF-16 reporters override this overload.
        let message = String::from_utf16(message.as_units())
            .expect("WTF-16 warning requires warning_js_string override");
        self.warning(&message, source_name, line, line_offset);
    }
    // port: ErrorReporter#error (Java String can contain unpaired UTF-16 surrogates)
    fn error_js_string(
        &mut self,
        message: &crate::js_string::JsString,
        source_name: &str,
        line: i32,
        line_offset: i32,
    ) {
        // UTF-8-only reporters accept scalar messages; WTF-16 reporters override this overload.
        let message = String::from_utf16(message.as_units())
            .expect("WTF-16 error requires error_js_string override");
        self.error(&message, source_name, line, line_offset);
    }
}
pub struct NullErrorReporter;
pub struct AlwaysThrowsErrorReporter;
pub const NULL_INSTANCE: NullErrorReporter = NullErrorReporter;
pub const ALWAYS_THROWS_INSTANCE: AlwaysThrowsErrorReporter = AlwaysThrowsErrorReporter;
impl ErrorReporter for NullErrorReporter {
    // port: ErrorReporter#warning
    fn warning_js_string(
        &mut self,
        _message: &crate::js_string::JsString,
        _source_name: &str,
        _line: i32,
        _line_offset: i32,
    ) {
    }
    // port: ErrorReporter#error
    fn error_js_string(
        &mut self,
        _message: &crate::js_string::JsString,
        _source_name: &str,
        _line: i32,
        _line_offset: i32,
    ) {
    }
    // port: ErrorReporter#warning
    fn warning(&mut self, _message: &str, _source_name: &str, _line: i32, _line_offset: i32) {}
    // port: ErrorReporter#error
    fn error(&mut self, _message: &str, _source_name: &str, _line: i32, _line_offset: i32) {}
}
impl ErrorReporter for AlwaysThrowsErrorReporter {
    // port: ErrorReporter#warning
    fn warning_js_string(
        &mut self,
        message: &crate::js_string::JsString,
        _source_name: &str,
        _line: i32,
        _line_offset: i32,
    ) {
        std::panic::panic_any(message.clone());
    }
    // port: ErrorReporter#error
    fn error_js_string(
        &mut self,
        message: &crate::js_string::JsString,
        _source_name: &str,
        _line: i32,
        _line_offset: i32,
    ) {
        std::panic::panic_any(message.clone());
    }
    // port: ErrorReporter#warning
    fn warning(&mut self, message: &str, _source_name: &str, _line: i32, _line_offset: i32) {
        panic!("{message}");
    }
    // port: ErrorReporter#error
    fn error(&mut self, message: &str, _source_name: &str, _line: i32, _line_offset: i32) {
        panic!("{message}");
    }
}
