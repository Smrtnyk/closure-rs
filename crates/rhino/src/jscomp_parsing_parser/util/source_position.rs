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
//   src/com/google/javascript/jscomp/parsing/parser/util/SourcePosition.java.

use crate::jscomp_parsing_parser::source_file::SourceFile;
use std::{fmt, sync::Arc};
/// A position in a source string - includes offset, line and column.
#[derive(Debug, Clone)]
pub struct SourcePosition {
    pub source: Option<Arc<SourceFile>>,
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
    pub fn new(source: Option<Arc<SourceFile>>, offset: i32, line: i32, column: i32) -> Self {
        Self {
            source,
            offset,
            line,
            column,
        }
    }
    // port: SourcePosition#shortSourceName
    fn short_source_name(&self) -> &str {
        self.source
            .as_ref()
            .map_or("", |s| s.name.rsplit('/').next().unwrap())
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
