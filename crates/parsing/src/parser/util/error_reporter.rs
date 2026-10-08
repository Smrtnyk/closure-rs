/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/util/ErrorReporter.java.

use std::{cell::RefCell, rc::Rc};

use super::SourcePosition;
use crate::parser::JsString;
use closure_rhino::java_lang::format_message;

/// A conduit for reporting errors and warnings to the user.
// Messages stay UTF-16 because diagnostics can embed source characters, including lone surrogates.
pub trait ErrorReporter {
    // port: ErrorReporter#reportError
    fn report_error(&mut self, location: SourcePosition, message: JsString);
    // port: ErrorReporter#reportWarning
    fn report_warning(&mut self, location: SourcePosition, message: JsString);
}

struct State {
    had_error: bool,
    sink: Box<dyn ErrorReporter>,
}

/// Shared conduit used by both the scanner and the parser.
#[derive(Clone)]
pub struct Reporter(Rc<RefCell<State>>);

impl Reporter {
    // port: ErrorReporter#<init>
    pub fn new(sink: impl ErrorReporter + 'static) -> Self {
        Self(Rc::new(RefCell::new(State {
            had_error: false,
            sink: Box::new(sink),
        })))
    }

    // port: ErrorReporter#reportError
    pub fn report_error(&self, location: SourcePosition, format: &str, arguments: &[JsString]) {
        let mut state = self.0.borrow_mut();
        state.had_error = true;
        state
            .sink
            .report_error(location, format_message(&JsString::from(format), arguments));
    }

    // port: ErrorReporter#reportWarning
    pub fn report_warning(&self, location: SourcePosition, format: &str, arguments: &[JsString]) {
        self.0
            .borrow_mut()
            .sink
            .report_warning(location, format_message(&JsString::from(format), arguments));
    }

    // port: ErrorReporter#hadError
    pub fn had_error(&self) -> bool {
        self.0.borrow().had_error
    }
}
