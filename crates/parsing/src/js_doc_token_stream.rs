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
//   src/com/google/javascript/jscomp/parsing/JsDocTokenStream.java.

//! Port of `com.google.javascript.jscomp.parsing.JsDocTokenStream`.

use closure_rhino::js_string::JsString;
use closure_rhino::token_util::TokenUtil;

use crate::js_doc_token::JsDocToken;

/*
 * For chars - because we need something out-of-range
 * to check.  (And checking EOF by exception is annoying.)
 * Note distinction from EOF token type!
 */
const EOF_CHAR: i32 = -1;

/// This class implements the scanner for JsDoc strings.
///
/// It is heavily based on Rhino's TokenStream.
#[derive(Debug, Clone)]
pub struct JsDocTokenStream {
    // Set this to an initial non-null value so that the Parser has
    // something to retrieve even if an error has occurred and no
    // string is found.  Fosters one class of error, but saves lots of
    // code.
    string: JsString,

    string_buffer: Vec<u16>,
    string_buffer_top: usize,

    // Room to backtrace from to < on failed match of the last - in <!--
    unget_buffer: [i32; 3],
    unget_cursor: usize,

    line_start: i32,
    line_end_char: i32,
    lineno: i32,
    charno: i32,
    init_charno: i32,
    init_lineno: i32,

    source_string: JsString,
    source_end: i32,

    // sourceCursor is an index into a small buffer that keeps a
    // sliding window of the source stream.
    source_cursor: i32,

    // cursor is a monotonically increasing index into the original
    // source stream, tracking exactly how far scanning has progressed.
    // Its value is the index of the next character to be scanned.
    cursor: i32,
}

impl JsDocTokenStream {
    // port: JsDocTokenStream#JsDocTokenStream(String)
    pub fn new(source_string: impl Into<JsString>) -> Self {
        Self::new_with_lineno(source_string, 0)
    }

    // port: JsDocTokenStream#JsDocTokenStream(String, int)
    pub fn new_with_lineno(source_string: impl Into<JsString>, lineno: i32) -> Self {
        Self::new_with_lineno_and_charno(source_string, lineno, 0)
    }

    // port: JsDocTokenStream#JsDocTokenStream(String, int, int)
    pub fn new_with_lineno_and_charno(
        source_string: impl Into<JsString>,
        lineno: i32,
        init_charno: i32,
    ) -> Self {
        let source_string: JsString = source_string.into();
        let source_end = source_string.length() as i32;
        Self {
            string: JsString::from(""),
            string_buffer: vec![0; 128],
            string_buffer_top: 0,
            unget_buffer: [0; 3],
            unget_cursor: 0,
            line_start: 0,
            line_end_char: -1,
            lineno,
            charno: -1,
            init_charno,
            init_lineno: lineno,
            source_string,
            source_end,
            source_cursor: 0,
            cursor: 0,
        }
    }

    /// Tokenizes JSDoc comments.
    // port: JsDocTokenStream#getJsDocToken
    pub fn get_js_doc_token(&mut self) -> JsDocToken {
        let mut c: i32;
        self.string_buffer_top = 0;
        // Java's outer `for (;;)` is kept; every path through its body returns.
        #[allow(clippy::never_loop)]
        loop {
            // eat white spaces
            loop {
                self.charno = -1;
                c = self.get_char();
                if c == EOF_CHAR {
                    return JsDocToken::EOF;
                } else if c == '\n' as i32 {
                    return JsDocToken::EOL;
                } else if !TokenUtil::is_js_space(c) {
                    break;
                }
            }

            // Every case returns except '.', which falls through into default (after the match).
            match char_of(c) {
                // annotation, e.g. @type or @constructor
                Some('@') => loop {
                    c = self.get_char();
                    if is_alpha(c) {
                        self.add_to_string(c);
                    } else {
                        self.unget_char(c);
                        self.string = self.get_string_from_buffer();
                        self.string_buffer_top = 0;
                        return JsDocToken::ANNOTATION;
                    }
                },

                Some('*') => {
                    if self.match_char('/' as i32) {
                        return JsDocToken::EOC;
                    } else {
                        return JsDocToken::STAR;
                    }
                }

                Some(',') => return JsDocToken::COMMA,

                Some('>') => return JsDocToken::RIGHT_ANGLE,

                Some('(') => return JsDocToken::LEFT_PAREN,

                Some(')') => return JsDocToken::RIGHT_PAREN,

                Some('{') => return JsDocToken::LEFT_CURLY,

                Some('}') => return JsDocToken::RIGHT_CURLY,

                Some('[') => return JsDocToken::LEFT_SQUARE,

                Some(']') => return JsDocToken::RIGHT_SQUARE,

                Some('?') => return JsDocToken::QMARK,

                Some('!') => return JsDocToken::BANG,

                Some(':') => return JsDocToken::COLON,

                Some('=') => return JsDocToken::EQUALS,

                Some('|') => return JsDocToken::PIPE,

                Some('<') => return JsDocToken::LEFT_ANGLE,

                Some('.') => {
                    c = self.get_char();
                    if c == '<' as i32 {
                        return JsDocToken::LEFT_ANGLE;
                    } else {
                        if c == '.' as i32 {
                            c = self.get_char();
                            if c == '.' as i32 {
                                return JsDocToken::ITER_REST;
                            } else {
                                self.add_to_string('.' as i32);
                            }
                        }
                        // we may backtrack across line boundary
                        self.unget_buffer[self.unget_cursor] = c;
                        self.unget_cursor += 1;
                        c = '.' as i32;
                    }
                    // fall through
                }

                _ => {}
            }

            // default:
            {
                // recognize a JsDoc string but discard last . if it is followed by
                // a non-JsDoc comment char, e.g. Array.<
                let mut c1: i32;
                self.add_to_string(c);
                let mut c2 = self.get_char();
                if !is_js_doc_string(c2) {
                    self.unget_char(c2);
                    self.string = self.get_string_from_buffer();
                    self.string_buffer_top = 0;
                    return JsDocToken::STRING;
                } else {
                    loop {
                        c1 = c2;
                        c2 = self.get_char();
                        if c1 == '.' as i32 && c2 == '<' as i32 {
                            self.unget_char(c2);
                            self.unget_char(c1);
                            self.string = self.get_string_from_buffer();
                            self.string_buffer_top = 0;
                            return JsDocToken::STRING;
                        } else if is_js_doc_string(c2) {
                            self.add_to_string(c1);
                        } else {
                            self.unget_char(c2);
                            self.add_to_string(c1);
                            self.string = self.get_string_from_buffer();
                            self.string_buffer_top = 0;
                            return JsDocToken::STRING;
                        }
                    }
                }
            }
        }
    }

    /// Gets the remaining JSDoc line without the {@link JsDocToken#EOL},
    /// {@link JsDocToken#EOF} or {@link JsDocToken#EOC}.
    // port: JsDocTokenStream#getRemainingJSDocLine
    pub fn get_remaining_js_doc_line(&mut self) -> JsString {
        let mut c: i32;
        loop {
            c = self.get_char();
            match char_of(c) {
                Some('*') => {
                    if self.peek_char() != '/' as i32 {
                        self.add_to_string(c);
                        continue;
                    }
                    // fall through
                }
                Some('\n') => {}
                None if c == EOF_CHAR => {}
                _ => {
                    self.add_to_string(c);
                    continue;
                }
            }
            // case EOF_CHAR: case '\n':
            self.unget_char(c);
            self.string = self.get_string_from_buffer();
            self.string_buffer_top = 0;
            return self.string.clone();
        }
    }

    // port: JsDocTokenStream#getLineno
    pub fn get_lineno(&self) -> i32 {
        self.lineno
    }

    // port: JsDocTokenStream#getCharno
    pub fn get_charno(&self) -> i32 {
        if self.lineno == self.init_lineno {
            self.init_charno + self.charno
        } else {
            self.charno
        }
    }

    // port: JsDocTokenStream#getString
    pub fn get_string(&self) -> JsString {
        self.string.clone()
    }

    // port: JsDocTokenStream#getStringFromBuffer
    fn get_string_from_buffer(&self) -> JsString {
        JsString::from_units(self.string_buffer[0..self.string_buffer_top].to_vec())
    }

    // port: JsDocTokenStream#addToString
    fn add_to_string(&mut self, c: i32) {
        let n = self.string_buffer_top;
        if n == self.string_buffer.len() {
            let mut tmp = vec![0u16; self.string_buffer.len() * 2];
            tmp[..n].copy_from_slice(&self.string_buffer[..n]);
            self.string_buffer = tmp;
        }
        self.string_buffer[n] = c as u16;
        self.string_buffer_top = n + 1;
    }

    // port: JsDocTokenStream#ungetChar
    pub fn unget_char(&mut self, c: i32) {
        // can not unread past across line boundary
        // (a Java `assert`, which is disabled at run time unless -ea is given)
        debug_assert!(
            !(self.unget_cursor != 0 && self.unget_buffer[self.unget_cursor - 1] == '\n' as i32)
        );
        self.unget_buffer[self.unget_cursor] = c;
        self.unget_cursor += 1;
        self.cursor -= 1;
    }

    // port: JsDocTokenStream#matchChar
    fn match_char(&mut self, test: i32) -> bool {
        let c = self.get_char_ignore_line_end();
        if c == test {
            true
        } else {
            self.unget_char_ignore_line_end(c);
            false
        }
    }

    /// Allows the JSDocParser to update the character offset
    /// so that getCharno() returns a valid character position.
    // port: JsDocTokenStream#update
    pub fn update(&mut self) {
        self.charno = self.get_line_offset();
    }

    // port: JsDocTokenStream#peekChar
    fn peek_char(&mut self) -> i32 {
        let c = self.get_char();
        self.unget_char(c);
        c
    }

    // port: JsDocTokenStream#getChar
    pub fn get_char(&mut self) -> i32 {
        if self.unget_cursor != 0 {
            self.cursor += 1;
            self.unget_cursor -= 1;
            if self.charno == -1 {
                self.charno = self.get_line_offset();
            }
            return self.unget_buffer[self.unget_cursor];
        }

        loop {
            let mut c: i32;
            if self.source_cursor == self.source_end {
                if self.charno == -1 {
                    self.charno = self.get_line_offset();
                }
                return EOF_CHAR;
            }
            self.cursor += 1;
            c = self.source_string.char_at(self.source_cursor as usize) as i32;
            self.source_cursor += 1;

            if self.line_end_char >= 0 {
                if self.line_end_char == '\r' as i32 && c == '\n' as i32 {
                    self.line_end_char = '\n' as i32;
                    continue;
                }
                self.line_end_char = -1;
                self.line_start = self.source_cursor - 1;
                self.lineno += 1;
            }

            if c <= 127 {
                if c == '\n' as i32 || c == '\r' as i32 {
                    self.line_end_char = c;
                    c = '\n' as i32;
                }
            } else {
                if TokenUtil::is_js_format_char(c) {
                    continue;
                }
                if is_js_line_terminator(c) {
                    self.line_end_char = c;
                    c = '\n' as i32;
                }
            }

            if self.charno == -1 {
                self.charno = self.get_line_offset();
            }

            return c;
        }
    }

    // port: JsDocTokenStream#getCharIgnoreLineEnd
    fn get_char_ignore_line_end(&mut self) -> i32 {
        if self.unget_cursor != 0 {
            self.cursor += 1;
            self.unget_cursor -= 1;
            if self.charno == -1 {
                self.charno = self.get_line_offset();
            }
            return self.unget_buffer[self.unget_cursor];
        }

        loop {
            let mut c: i32;
            if self.source_cursor == self.source_end {
                if self.charno == -1 {
                    self.charno = self.get_line_offset();
                }
                return EOF_CHAR;
            }
            self.cursor += 1;
            c = self.source_string.char_at(self.source_cursor as usize) as i32;
            self.source_cursor += 1;

            if c <= 127 {
                if c == '\n' as i32 || c == '\r' as i32 {
                    self.line_end_char = c;
                    c = '\n' as i32;
                }
            } else {
                if TokenUtil::is_js_format_char(c) {
                    continue;
                }
                if is_js_line_terminator(c) {
                    self.line_end_char = c;
                    c = '\n' as i32;
                }
            }

            if self.charno == -1 {
                self.charno = self.get_line_offset();
            }

            return c;
        }
    }

    // port: JsDocTokenStream#ungetCharIgnoreLineEnd
    fn unget_char_ignore_line_end(&mut self, c: i32) {
        self.unget_buffer[self.unget_cursor] = c;
        self.unget_cursor += 1;
        self.cursor -= 1;
    }

    /// Returns the offset into the current line.
    // port: JsDocTokenStream#getLineOffset
    fn get_line_offset(&self) -> i32 {
        self.source_cursor - self.line_start - self.unget_cursor as i32 - 1
    }

    // port: JsDocTokenStream#getCursor
    pub fn get_cursor(&self) -> i32 {
        self.cursor
    }
}

/// The ASCII character `c` stands for, for matching Java `switch (c)` cases on char literals.
fn char_of(c: i32) -> Option<char> {
    if (0..=127).contains(&c) {
        Some(c as u8 as char)
    } else {
        None
    }
}

// port: JsDocTokenStream#isAlpha
fn is_alpha(c: i32) -> bool {
    // Use 'Z' < 'a'
    if c <= 'Z' as i32 {
        'A' as i32 <= c
    } else {
        'a' as i32 <= c && c <= 'z' as i32
    }
}

// port: JsDocTokenStream#isJSDocString
fn is_js_doc_string(c: i32) -> bool {
    match c {
        EOF_CHAR => false,
        _ => match char_of(c) {
            Some(
                '@' | '*' | ',' | '<' | '>' | ':' | '(' | ')' | '{' | '}' | '[' | ']' | '?' | '!'
                | '|' | '=' | '\n',
            ) => false,
            _ => !TokenUtil::is_js_space(c),
        },
    }
}

// port: JsDocTokenStream#isJSLineTerminator
fn is_js_line_terminator(c: i32) -> bool {
    // Optimization for faster check for eol character:
    // they do not have 0xDFD0 bits set
    if (c & 0xDFD0) != 0 {
        return false;
    }
    c == '\n' as i32 || c == '\r' as i32 || c == 0x2028 || c == 0x2029
}
