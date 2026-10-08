/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/JSErrorSubject.java.

//! Port of testing/JSErrorSubject.java: a Truth Subject for JSError.
use closure_jscomp::diagnostic_type::DiagnosticType;
use closure_jscomp::js_error::JSError;

// port: JSErrorSubject
pub struct JSErrorSubject<'a> {
    actual: &'a JSError,
}

// port: JSErrorSubject#assertError
pub fn assert_error(error: &JSError) -> JSErrorSubject<'_> {
    JSErrorSubject::assert_error(error)
}

impl<'a> JSErrorSubject<'a> {
    // port: JSErrorSubject#assertError
    pub fn assert_error(error: &'a JSError) -> Self {
        Self::new(error)
    }

    // port: JSErrorSubject#JSErrorSubject
    pub fn new(error: &'a JSError) -> Self {
        Self { actual: error }
    }

    // port: JSErrorSubject#hasType
    pub fn has_type(&self, type_: &DiagnosticType) {
        assert_eq!(
            self.actual.get_type(),
            type_,
            "value of: getType()\nfor: {}",
            self.actual
        );
    }

    // port: JSErrorSubject#hasMessage
    pub fn has_message(&self, msg: &str) {
        assert_eq!(
            self.actual.description(),
            msg,
            "value of: description\nfor: {}",
            self.actual
        );
    }
}
