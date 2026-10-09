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
//   src/com/google/javascript/jscomp/parsing/parser/Identifiers.java.

#![allow(clippy::manual_range_contains)] // Preserve Java's non-short-circuit ASCII fast path.

use closure_rhino::java_lang::{is_digit, is_letter};

/// JS identifier parsing utilities.
pub struct Identifiers;
impl Identifiers {
    // Intentional to minimize branches in this code
    // port: Identifiers#isIdentifierStart
    pub fn is_identifier_start(ch: u16) -> bool {
        // Most code is written in pure ASCII, so create a fast path here.
        if ch <= 127 {
            // Intentionally avoiding short circuiting behavior of "||" and "&&".
            // This minimizes branches in this code which minimizes branch prediction misses.
            return ((ch >= u16::from(b'A')) & (ch <= u16::from(b'Z')))
                | ((ch >= u16::from(b'a')) & (ch <= u16::from(b'z')))
                | ((ch == u16::from(b'_')) | (ch == u16::from(b'$')));
        }
        // Handle non-ASCII characters.
        // TODO(tjgq): This should include all characters with the ID_Start property.
        is_letter(ch)
    }
    // Intentional to minimize branches in this code
    // port: Identifiers#isIdentifierPart
    pub fn is_identifier_part(ch: u16) -> bool {
        // Most code is written in pure ASCII, so create a fast path here.
        if ch <= 127 {
            return ((ch >= u16::from(b'A')) & (ch <= u16::from(b'Z')))
                | ((ch >= u16::from(b'a')) & (ch <= u16::from(b'z')))
                | ((ch >= u16::from(b'0')) & (ch <= u16::from(b'9')))
                | ((ch == u16::from(b'_')) | (ch == u16::from(b'$')));
        }
        // Handle non-ASCII characters.
        // TODO(tjgq): This should include all characters with the ID_Continue property, plus
        // Zero Width Non-Joiner and Zero Width Joiner.
        Self::is_identifier_start(ch) || is_digit(ch)
    }
    // port: Identifiers#<init>
    pub fn new() -> Self {
        Self
    }
}
impl Default for Identifiers {
    fn default() -> Self {
        Self::new()
    }
}
