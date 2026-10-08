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
//   src/com/google/debugging/sourcemap/SourceMapSection.java.

use closure_rhino::js_string::JsString;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionType {
    URL,
    MAP,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMapSection {
    pub r#type: SectionType,
    pub value: JsString,
    pub line: i32,
    pub column: i32,
}
impl SourceMapSection {
    // port: SourceMapSection#SourceMapSection
    pub fn new(r#type: SectionType, value: impl Into<JsString>, line: i32, column: i32) -> Self {
        let value = value.into();
        Self {
            r#type,
            value,
            line,
            column,
        }
    }
    // port: SourceMapSection#forMap
    pub fn for_map(value: impl Into<JsString>, line: i32, column: i32) -> Self {
        Self::new(SectionType::MAP, value, line, column)
    }
    // port: SourceMapSection#forURL
    pub fn for_url(value: impl Into<JsString>, line: i32, column: i32) -> Self {
        Self::new(SectionType::URL, value, line, column)
    }
    // port: SourceMapSection#getSectionType
    pub fn get_section_type(&self) -> SectionType {
        self.r#type
    }
    // port: SourceMapSection#getSectionValue
    pub fn get_section_value(&self) -> &JsString {
        &self.value
    }
    // port: SourceMapSection#getLine
    pub fn get_line(&self) -> i32 {
        self.line
    }
    // port: SourceMapSection#getColumn
    pub fn get_column(&self) -> i32 {
        self.column
    }
}

impl std::fmt::Display for SourceMapSection {
    // port: SourceMapSection#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SourceMapSection[type={:?}, value={}, line={}, column={}]",
            self.r#type, self.value, self.line, self.column
        )
    }
}
