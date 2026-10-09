/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapParseException.java.

use closure_rhino::js_string::JsString;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMapParseException {
    message: JsString,
}
impl SourceMapParseException {
    // port: SourceMapParseException#SourceMapParseException
    pub fn new(message: impl Into<JsString>) -> Self {
        Self {
            message: message.into(),
        }
    }
    // port: Throwable#getMessage
    pub fn get_message(&self) -> &JsString {
        &self.message
    }
    // Java Throwable.toString needs a lossless value before its Display boundary.
    pub fn java_to_string(&self) -> JsString {
        crate::java_string::throwable_to_string(
            "com.google.debugging.sourcemap.SourceMapParseException",
            Some(&self.message),
        )
    }
}
impl std::fmt::Display for SourceMapParseException {
    // Rust Display dispatch to the lossless inherited Throwable body.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.java_to_string())
    }
}
impl std::error::Error for SourceMapParseException {}
