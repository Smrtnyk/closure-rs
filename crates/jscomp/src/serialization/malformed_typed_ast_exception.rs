/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/MalformedTypedAstException.java.

//! Port of serialization/MalformedTypedAstException.java.
use std::fmt;

/// port: MalformedTypedAstException
///
/// Emitted when deserialization sees a TypedAst with semantic errors. Thrown as a panic whose
/// payload is this value (Java's unchecked `RuntimeException`); tests downcast it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MalformedTypedAstException {
    pub message: String,
    /// Java's `getCause()` text, when the exception wraps one.
    pub cause: Option<String>,
}

impl MalformedTypedAstException {
    // port: MalformedTypedAstException#checkWellFormed(boolean,String)
    pub fn check_well_formed(condition: bool, description: &str) {
        if !condition {
            Self::new(description).throw();
        }
    }

    // port: MalformedTypedAstException#checkWellFormed(boolean,String,Object)
    pub fn check_well_formed_with_param(
        condition: bool,
        description: &str,
        param: &dyn fmt::Display,
    ) {
        if !condition {
            let message = format!("{description}: {param}");
            Self::new(message).throw();
        }
    }

    // port: MalformedTypedAstException#<init>(Object)
    pub fn new(msg: impl fmt::Display) -> Self {
        Self {
            message: msg.to_string(),
            cause: None,
        }
    }

    // port: MalformedTypedAstException#<init>(Object,Throwable)
    pub fn new_with_cause(msg: impl fmt::Display, cause: impl fmt::Display) -> Self {
        Self {
            message: msg.to_string(),
            cause: Some(cause.to_string()),
        }
    }

    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }

    /// Java `throw new MalformedTypedAstException(..)`.
    pub fn throw(self) -> ! {
        std::panic::panic_any(self)
    }
}

impl fmt::Display for MalformedTypedAstException {
    // port: Throwable#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.serialization.MalformedTypedAstException: {}",
            self.message
        )
    }
}
