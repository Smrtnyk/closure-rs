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
//   src/com/google/javascript/jscomp/parsing/parser/util/SourcePosition.java.

use crate::jscomp_parsing_parser::source_file::SourceFile;
use std::{
    fmt,
    sync::{Arc, Mutex},
};

/// Rust-only, not in Java (D-025): the identity of the `SourceFile` a position belongs to. Java
/// positions hold the SourceFile reference; the parser makes two positions per token, so the
/// reference-count traffic of an `Arc` here is a measurable part of parsing. The handle is a
/// plain number, unique per registered SourceFile object (Java's `==` on the reference), and
/// gives back the file name, the only thing positions read from their source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFileId(u32);

static SOURCE_FILE_NAMES: Mutex<Vec<Arc<str>>> = Mutex::new(Vec::new());

impl SourceFileId {
    /// A new handle for `file` (one per SourceFile object).
    pub fn register(file: &SourceFile) -> Self {
        let mut names = SOURCE_FILE_NAMES.lock().unwrap();
        names.push(Arc::from(file.name.as_str()));
        Self(u32::try_from(names.len() - 1).unwrap())
    }
    /// The name of the SourceFile.
    pub fn name(self) -> Arc<str> {
        Arc::clone(&SOURCE_FILE_NAMES.lock().unwrap()[self.0 as usize])
    }
}

/// A position in a source string - includes offset, line and column.
#[derive(Debug, Clone)]
pub struct SourcePosition {
    pub source: Option<SourceFileId>,
    pub offset: i32,
    pub line: i32,
    pub column: i32,
}
impl SourcePosition {
    // port: SourcePosition#getOffset
    pub fn get_offset(&self) -> i32 {
        self.offset
    }
    // port: SourcePosition#SourcePosition
    pub fn new(source: Option<SourceFileId>, offset: i32, line: i32, column: i32) -> Self {
        Self {
            source,
            offset,
            line,
            column,
        }
    }
    // port: SourcePosition#shortSourceName
    fn short_source_name(&self) -> String {
        self.source.map_or_else(String::new, |s| {
            s.name().rsplit('/').next().unwrap().to_owned()
        })
    }
}
impl fmt::Display for SourcePosition {
    // port: SourcePosition#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}({}, {})",
            self.short_source_name(),
            self.line.wrapping_add(1),
            self.column.wrapping_add(1)
        )
    }
}
