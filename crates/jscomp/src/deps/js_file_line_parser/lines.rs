/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 030d049:
//   src/com/google/javascript/jscomp/deps/JsFileLineParser.java.

use closure_rhino::js_string::JsString;
use std::io;

/// Comment handling is shared with the host build that examines bundled dependency inputs.
#[derive(Default)]
pub(crate) struct Comments {
    in_multiline_comment: bool,
    in_js_doc_comment: bool,
}

impl Comments {
    // port: JsFileLineParser#doParse
    pub(crate) fn strip(
        &mut self,
        line: JsString,
        mut visit_doc: impl FnMut(&JsString) -> bool,
    ) -> Option<JsString> {
        let mut revised_line = line;
        let mut revised_js_doc_comment_line = JsString::from("");
        if self.in_multiline_comment {
            if let Some(end) = find(&revised_line, "*/", 0) {
                if self.in_js_doc_comment {
                    revised_js_doc_comment_line = revised_line.substring(0, end + 2);
                    self.in_js_doc_comment = false;
                }
                revised_line = revised_line.substring_from(end + 2);
                self.in_multiline_comment = false;
            } else {
                if self.in_js_doc_comment {
                    revised_js_doc_comment_line = revised_line.clone();
                }
                revised_line = JsString::from("");
            }
        }
        if !self.in_multiline_comment {
            loop {
                let start_line = find(&revised_line, "//", 0);
                let start_multi = find(&revised_line, "/*", 0);
                if let Some(start) = start_line
                    && start_multi.is_none_or(|m| start < m)
                {
                    revised_line = revised_line.substring(0, start);
                    break;
                } else if let Some(start) = start_multi {
                    if Self::is_comment_quoted(&revised_line, start, b'\'' as u16)
                        || Self::is_comment_quoted(&revised_line, start, b'"' as u16)
                    {
                        break;
                    }
                    if find(&revised_line, "/**", 0) == Some(start) {
                        self.in_js_doc_comment = true;
                    }
                    if let Some(end) = find(&revised_line, "*/", start + 2) {
                        if self.in_js_doc_comment {
                            if !visit_doc(&revised_line.substring(start, end + 2)) {
                                break;
                            }
                            self.in_js_doc_comment = false;
                        }
                        revised_line = revised_line
                            .substring(0, start)
                            .concat(&revised_line.substring_from(end + 2));
                    } else {
                        if self.in_js_doc_comment {
                            revised_js_doc_comment_line = revised_line.substring_from(start);
                        }
                        revised_line = revised_line.substring(0, start);
                        self.in_multiline_comment = true;
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        if !revised_js_doc_comment_line.is_empty() && !visit_doc(&revised_js_doc_comment_line) {
            return None;
        }
        Some(revised_line)
    }
    // port: JsFileLineParser#isCommentQuoted
    fn is_comment_quoted(
        line: &JsString,
        start_of_multiline_comment: usize,
        quote_char: u16,
    ) -> bool {
        let quote = JsString::from_units(vec![quote_char]);
        let mut start_quote = usize::try_from(line.index_of(&quote)).ok();
        while let Some(start) = start_quote
            && start < start_of_multiline_comment
        {
            let mut closing = usize::try_from(line.index_of_from(&quote, (start + 1) as i32)).ok();
            while let Some(end) = closing
                && Self::is_escaped(line, end)
            {
                closing = usize::try_from(line.index_of_from(&quote, (end + 1) as i32)).ok();
            }
            let Some(end) = closing else {
                return false;
            };
            if end > start_of_multiline_comment {
                return true;
            }
            start_quote = usize::try_from(line.index_of_from(&quote, (end + 1) as i32)).ok();
        }
        false
    }
    // port: JsFileLineParser#isEscaped
    fn is_escaped(line: &JsString, closing_quote_index: usize) -> bool {
        let escape_char_count = line.as_units()[..closing_quote_index]
            .iter()
            .rev()
            .take_while(|c| **c == b'\\' as u16)
            .count();
        escape_char_count % 2 == 1
    }
}
fn find(s: &JsString, needle: &str, from: usize) -> Option<usize> {
    usize::try_from(s.index_of_from(needle, from as i32)).ok()
}
pub(crate) struct StringLines {
    input: JsString,
    position: usize,
}
impl StringLines {
    // port: StringReader#StringReader
    pub(crate) fn new(input: JsString) -> Self {
        Self { input, position: 0 }
    }
}
impl Iterator for StringLines {
    type Item = io::Result<JsString>;
    // port: BufferedReader#readLine (StringReader source, preserving UTF-16)
    fn next(&mut self) -> Option<Self::Item> {
        if self.position == self.input.length() {
            return None;
        }
        let start = self.position;
        while self.position < self.input.length() {
            let c = self.input.char_at(self.position);
            self.position += 1;
            if c == 10 || c == 13 {
                let end = self.position - 1;
                if c == 13
                    && self.position < self.input.length()
                    && self.input.char_at(self.position) == 10
                {
                    self.position += 1;
                }
                return Some(Ok(self.input.substring(start, end)));
            }
        }
        Some(Ok(self.input.substring_from(start)))
    }
}
