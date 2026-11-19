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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/parsing/parser/LineNumberScanner.java.

use closure_rhino::jscomp_parsing_parser::util::source_position::SourceFileId;
use std::sync::Arc;

use super::{
    source_file::SourceFile,
    util::{SourcePosition, SourceRange},
};

/// Utility for finding line and column offsets within a source file.
pub struct LineNumberScanner {
    source_file: Arc<SourceFile>,
    /// Rust-only: the copyable identity of `source_file` that positions carry.
    source_file_id: SourceFileId,
    last_line: i32,
    last_line_start: i32,
    next_line_start: i32,
}

impl LineNumberScanner {
    // port: LineNumberScanner#<init>
    pub fn new(source_file: Arc<SourceFile>) -> Self {
        Self {
            source_file_id: SourceFileId::register(&source_file),
            source_file,
            last_line: -1,
            last_line_start: -1,
            next_line_start: 0,
        }
    }

    /// Returns the source position of character offset {@code offset}. This class expects this method
    /// to be called with increasing values of {@code offset}, more or less. {@link #rewindTo} must be
    /// called before backing up to a previous line.
    // port: LineNumberScanner#getSourcePosition
    pub fn get_source_position(&mut self, offset: i32) -> SourcePosition {
        assert!(
            offset >= self.last_line_start,
            "Must call rewindTo before calling getSourcePosition for an earlier line ({} < {})",
            offset,
            self.last_line_start
        );
        while offset >= self.next_line_start {
            self.advance_line();
        }
        SourcePosition::new(
            Some(self.source_file_id),
            offset,
            self.last_line,
            offset - self.last_line_start,
        )
    }

    // port: LineNumberScanner#getSourceRange
    pub fn get_source_range(&mut self, start_offset: i32, end_offset: i32) -> SourceRange {
        SourceRange::new(
            self.get_source_position(start_offset),
            self.get_source_position(end_offset),
        )
    }

    /// Call this method to rewind the scanner to an earlier position in the source file. This is
    /// necessary if backing up to a previous line.
    // port: LineNumberScanner#rewindTo
    pub fn rewind_to(&mut self, position: &SourcePosition) {
        assert!(position.source == Some(self.source_file_id));
        if position.offset < self.last_line_start {
            self.last_line = position.line - 1;
            self.next_line_start = position.offset - position.column;
            self.advance_line();
        }
    }

    // port: LineNumberScanner#advanceLine
    fn advance_line(&mut self) {
        self.last_line += 1;
        self.last_line_start = self.next_line_start;
        // Rust-only (performance divergence): scan the UTF-16 slice once for the next delimiter.
        let rest = &self.source_file.contents.as_units()[self.last_line_start as usize..];
        self.next_line_start = match rest.iter().position(|&ch| Self::is_line_terminator(ch)) {
            Some(end) => {
                let crlf =
                    rest[end] == u16::from(b'\r') && rest.get(end + 1) == Some(&u16::from(b'\n'));
                self.last_line_start + end as i32 + 1 + i32::from(crlf)
            }
            None => i32::MAX,
        };
    }

    // port: LineNumberScanner#isLineTerminator
    fn is_line_terminator(ch: u16) -> bool {
        matches!(
            ch,
            ch if ch == u16::from(b'\n') // Line Feed
                || ch == u16::from(b'\r') // Carriage Return
                || ch == '\u{2028}' as u16 // Line Separator
                || ch == '\u{2029}' as u16 // Paragraph Separator
        )
    }
}
