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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/parsing/parser/SourceFile.java.

use crate::js_string::JsString;
/// A source file.
///
/// <p>Immutable.
#[derive(Debug)]
pub struct SourceFile {
    pub name: String,
    pub contents: JsString,
}
impl SourceFile {
    // port: SourceFile#SourceFile
    pub fn new(name: impl Into<String>, contents: impl Into<JsString>) -> Self {
        Self {
            name: name.into(),
            contents: contents.into(),
        }
    }
}
