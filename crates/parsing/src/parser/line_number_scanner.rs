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
//   src/com/google/javascript/jscomp/parsing/parser/LineNumberScanner.java.

use std::sync::Arc;

use super::{
    source_file::SourceFile,
    util::{SourcePosition, SourceRange},
};

/// Utility for finding line and column offsets within a source file.
pub struct LineNumberScanner {
    source_file: Arc<SourceFile>,
    source_length: i32,
    last_line: i32,
    last_line_start: i32,
    next_line_start: i32,
}

impl LineNumberScanner {
    // port: LineNumberScanner#<init>
    pub fn new(source_file: Arc<SourceFile>) -> Self {
        Self {
            source_length: source_file.contents.length() as i32,
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
            Some(self.source_file.clone()),
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
        assert!(
            position
                .source
                .as_ref()
                .is_some_and(|source| Arc::ptr_eq(source, &self.source_file))
        );
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
        let mut index = self.last_line_start;
        while index < self.source_length {
            let ch = self.source_file.contents.char_at(index as usize);
            if Self::is_line_terminator(ch) {
                if ch == u16::from(b'\r')
                    && index + 1 < self.source_length
                    && self.source_file.contents.char_at((index + 1) as usize) == u16::from(b'\n')
                {
                    index += 1;
                }
                self.next_line_start = index + 1;
                return;
            }
            index += 1;
        }
        self.next_line_start = i32::MAX;
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
