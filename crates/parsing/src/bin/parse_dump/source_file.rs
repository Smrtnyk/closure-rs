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
//   src/com/google/javascript/jscomp/parsing/parser/SourceFile.java.

// tool-only: parse_dump depends only on closure-parsing, not on the jscomp SourceFile
use closure_rhino::{
    js_string::JsString,
    static_source_file::{SourceKind, StaticSourceFile},
};
use std::fmt;

#[derive(Debug)]
pub struct SourceFile {
    pub name: String,
    pub code: JsString,
    pub kind: SourceKind,
    line_offsets: Vec<i32>,
}
impl SourceFile {
    // port: SourceFile#SourceFile
    // port: SourceFile#setCodeAndDoBookkeeping
    // port: SourceFile#findLineOffsets
    pub fn new(name: String, code: JsString, kind: SourceKind) -> Self {
        let code = if code.as_units().first() == Some(&0xfeff) {
            code.substring_from(1)
        } else {
            code
        };
        let mut line_offsets = vec![0];
        for (i, unit) in code.as_units().iter().enumerate() {
            if *unit == 10 {
                line_offsets.push(i as i32 + 1);
            }
        }
        Self {
            name,
            code,
            kind,
            line_offsets,
        }
    }
    // port: SourceFile#getLineOffset
    pub fn try_get_line_offset(&self, lineno: i32) -> Result<i32, String> {
        if lineno < 1 || lineno as usize > self.line_offsets.len() {
            return Err(format!(
                "Expected line number between 1 and {}\nActual: {}",
                self.line_offsets.len(),
                lineno
            ));
        }
        Ok(self.line_offsets[lineno as usize - 1])
    }
}
impl StaticSourceFile for SourceFile {
    // port: SourceFile#getName
    fn get_name(&self) -> &str {
        &self.name
    }
    // port: SourceFile#getKind
    fn get_kind(&self) -> SourceKind {
        self.kind
    }
    // port: SourceFile#isClosureUnawareCode
    fn is_closure_unaware_code(&self) -> bool {
        false
    }
    // port: SourceFile#getLineOffset
    fn get_line_offset(&self, line_number: i32) -> i32 {
        self.try_get_line_offset(line_number)
            .unwrap_or_else(|message| panic!("{message}"))
    }
    // port: SourceFile#getLineOfOffset
    fn get_line_of_offset(&self, offset: i32) -> i32 {
        match self.line_offsets.binary_search(&offset) {
            Ok(search) => search as i32 + 1,
            Err(insertion_point) => {
                (insertion_point as i32 - 1).min(self.line_offsets.len() as i32 - 1) + 1
            }
        }
    }
    // port: SourceFile#getColumnOfOffset
    fn get_column_of_offset(&self, offset: i32) -> i32 {
        let line = self.get_line_of_offset(offset);
        offset.wrapping_sub(self.line_offsets[line as usize - 1])
    }
}
impl fmt::Display for SourceFile {
    // port: SourceFile#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
