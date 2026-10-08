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
/*
 * Copyright (C) 2008 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/deps/JsFileLineParser.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/base/CharMatcher.java.

use crate::{
    check_level::CheckLevel, diagnostic_type::DiagnosticType, error_manager::ErrorManager,
    js_error::JSError,
};
use closure_rhino::{
    java_lang::{
        charset::Charset,
        regex::{Matcher, Pattern},
    },
    js_string::JsString,
};
use indexmap::IndexMap;
use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    sync::{Arc, Mutex},
};
pub static PARSE_WARNING: DiagnosticType =
    DiagnosticType::warning("DEPS_PARSE_WARNING", "{0}\n{1}");
pub static PARSE_ERROR: DiagnosticType = DiagnosticType::error("DEPS_PARSE_ERROR", "{0}\n{1}");
pub type SharedErrorManager = Arc<Mutex<dyn ErrorManager>>;
#[derive(Debug, Clone)]
pub struct ParseException {
    pub message: String,
    fatal: bool,
}
impl ParseException {
    pub const SERIAL_VERSION_UID: i64 = 1;
    // port: JsFileLineParser.ParseException#ParseException
    pub fn new(message: &str, fatal: bool) -> Self {
        Self {
            message: message.into(),
            fatal,
        }
    }
    // port: JsFileLineParser.ParseException#isFatal
    pub fn is_fatal(&self) -> bool {
        self.fatal
    }
}
impl fmt::Display for ParseException {
    // port: Throwable#getMessage
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ParseException {}
pub struct JsFileLineParser {
    pub shortcut_mode: bool,
    value_matcher: Matcher,
    pub file_path: String,
    pub line_num: i32,
    pub error_manager: SharedErrorManager,
    parse_succeeded: bool,
}
pub trait LineParser {
    // port: JsFileLineParser#JsFileLineParser (base fields)
    fn base(&mut self) -> &mut JsFileLineParser;
    // port: JsFileLineParser#parseLine
    fn parse_line(&mut self, line: &JsString) -> Result<bool, ParseException>;
    // port: JsFileLineParser#parseJsDocCommentLine
    fn parse_js_doc_comment_line(&mut self, _line: &JsString) -> bool {
        true
    }
}
impl JsFileLineParser {
    const STRING_LITERAL_PATTERN: &'static str =
        r#"\s*(?:'((?:\\'|[^'])*?)'|"((?:\\"|[^"])*?)")\s*"#;
    // port: JsFileLineParser#JsFileLineParser
    pub fn new(error_manager: SharedErrorManager) -> Self {
        Self {
            shortcut_mode: false,
            value_matcher: Pattern::compile(Self::STRING_LITERAL_PATTERN).matcher(""),
            file_path: String::new(),
            line_num: 0,
            error_manager,
            parse_succeeded: false,
        }
    }
    // port: JsFileLineParser#setShortcutMode
    pub fn set_shortcut_mode(&mut self, mode: bool) {
        self.shortcut_mode = mode;
    }
    // port: JsFileLineParser#didParseSucceed
    pub fn did_parse_succeed(&self) -> bool {
        self.parse_succeeded
    }
    // port: JsFileLineParser#doParse(String,Reader)
    pub fn do_parse(
        parser: &mut impl LineParser,
        file_path: &str,
        file_contents: impl Into<JsString>,
    ) {
        Self::do_parse_lines(parser, file_path, StringLines::new(file_contents.into()));
    }
    // port: JsFileLineParser#doParse(String,Reader)
    pub fn do_parse_reader(
        parser: &mut impl LineParser,
        file_path: &str,
        file_contents: impl Read,
    ) {
        Self::do_parse_lines(
            parser,
            file_path,
            JavaLines {
                reader: BufReader::new(file_contents),
                after_cr: false,
            },
        );
    }
    // port: JsFileLineParser#doParse(String,Reader) (shared line processing)
    pub(crate) fn do_parse_lines(
        parser: &mut impl LineParser,
        file_path: &str,
        lines: impl IntoIterator<Item = io::Result<JsString>>,
    ) {
        parser.base().file_path = file_path.into();
        parser.base().parse_succeeded = true;
        parser.base().line_num = 0;
        let mut in_multiline_comment = false;
        let mut in_js_doc_comment = false;
        for line in lines {
            let line = match line {
                Ok(line) => line,
                Err(_) => {
                    let base = parser.base();
                    base.error_manager.lock().unwrap().report(
                        CheckLevel::ERROR,
                        JSError::make_with_source_location(
                            file_path,
                            0,
                            0,
                            &PARSE_ERROR,
                            &[&format!("Error reading file: {file_path}")],
                        ),
                    );
                    base.parse_succeeded = false;
                    break;
                }
            };
            parser.base().line_num += 1;
            let result = (|| -> Result<bool, ParseException> {
                let mut revised_line = line.clone();
                let mut revised_js_doc_comment_line = JsString::from("");
                if in_multiline_comment {
                    if let Some(end) = find(&revised_line, "*/", 0) {
                        if in_js_doc_comment {
                            revised_js_doc_comment_line = revised_line.substring(0, end + 2);
                            in_js_doc_comment = false;
                        }
                        revised_line = revised_line.substring_from(end + 2);
                        in_multiline_comment = false;
                    } else {
                        if in_js_doc_comment {
                            revised_js_doc_comment_line = line.clone();
                        }
                        revised_line = JsString::from("");
                    }
                }
                if !in_multiline_comment {
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
                                in_js_doc_comment = true;
                            }
                            if let Some(end) = find(&revised_line, "*/", start + 2) {
                                if in_js_doc_comment {
                                    if !parser.parse_js_doc_comment_line(
                                        &revised_line.substring(start, end + 2),
                                    ) && parser.base().shortcut_mode
                                    {
                                        break;
                                    }
                                    in_js_doc_comment = false;
                                }
                                revised_line = revised_line
                                    .substring(0, start)
                                    .concat(&revised_line.substring_from(end + 2));
                            } else {
                                if in_js_doc_comment {
                                    revised_js_doc_comment_line =
                                        revised_line.substring_from(start);
                                }
                                revised_line = revised_line.substring(0, start);
                                in_multiline_comment = true;
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                if !revised_js_doc_comment_line.is_empty()
                    && !parser.parse_js_doc_comment_line(&revised_js_doc_comment_line)
                    && parser.base().shortcut_mode
                {
                    return Ok(false);
                }
                if !revised_line.is_empty()
                    && !parser.parse_line(&revised_line)?
                    && parser.base().shortcut_mode
                {
                    return Ok(false);
                }
                Ok(true)
            })();
            match result {
                Ok(false) => break,
                Ok(true) => {}
                Err(e) => {
                    let base = parser.base();
                    let fatal = e.is_fatal();
                    base.error_manager.lock().unwrap().report(
                        if fatal {
                            CheckLevel::ERROR
                        } else {
                            CheckLevel::WARNING
                        },
                        JSError::make_with_source_location(
                            file_path,
                            base.line_num,
                            0,
                            if fatal { &PARSE_ERROR } else { &PARSE_WARNING },
                            &[&e.message, &line.to_string()],
                        ),
                    );
                    base.parse_succeeded = base.parse_succeeded && !fatal;
                }
            }
        }
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
    // port: JsFileLineParser#parseJsString
    pub fn parse_js_string(
        &mut self,
        literal: impl Into<JsString>,
    ) -> Result<String, ParseException> {
        self.value_matcher.reset(literal);
        if !self.value_matcher.matches() {
            return Err(ParseException::new(
                "Syntax error in JS String literal",
                true,
            ));
        }
        Ok(self
            .value_matcher
            .group(1)
            .or_else(|| self.value_matcher.group(2))
            .unwrap())
    }
    // port: JsFileLineParser#parseJsStringArray
    pub fn parse_js_string_array(
        &mut self,
        input: impl Into<JsString>,
    ) -> Result<Vec<String>, ParseException> {
        let input = input.into();
        let mut results = vec![];
        let Some(start) = usize::try_from(input.index_of_char(b'[' as u16)).ok() else {
            return Err(ParseException::new(
                "Syntax error when parsing JS array",
                true,
            ));
        };
        let Some(end) = usize::try_from(input.last_index_of_char(b']' as u16)).ok() else {
            return Err(ParseException::new(
                "Syntax error when parsing JS array",
                true,
            ));
        };
        let inner_values = input.substring(start + 1, end);
        if !closure_rhino::java_lang::trim(&inner_values).is_empty() {
            self.value_matcher.reset(inner_values.clone());
            loop {
                if !self.value_matcher.looking_at() {
                    return Err(ParseException::new(
                        "Syntax error in JS String literal",
                        true,
                    ));
                }
                results.push(
                    self.value_matcher
                        .group(1)
                        .or_else(|| self.value_matcher.group(2))
                        .unwrap(),
                );
                if self.value_matcher.hit_end() {
                    break;
                }
                if inner_values.char_at(self.value_matcher.end()) != b',' as u16 {
                    return Err(ParseException::new("Missing comma in string array", true));
                }
                self.value_matcher.region(
                    self.value_matcher.end() + 1,
                    self.value_matcher.region_end(),
                );
            }
        }
        Ok(results)
    }
    // port: JsFileLineParser#parseJsStringMap
    pub fn parse_js_string_map(
        &mut self,
        input: impl Into<JsString>,
    ) -> Result<IndexMap<String, String>, ParseException> {
        let input = input.into();
        let start = input
            .as_units()
            .iter()
            .position(|&c| !guava_whitespace(c))
            .unwrap_or(input.length());
        let end = input
            .as_units()
            .iter()
            .rposition(|&c| !guava_whitespace(c))
            .map_or(start, |i| i + 1);
        let input = input.substring(start, end);
        Self::check(
            !input.is_empty()
                && input.char_at(0) == b'{' as u16
                && input.char_at(input.length() - 1) == b'}' as u16,
            "Syntax error when parsing JS object",
        )?;
        let input = closure_rhino::java_lang::trim(&input.substring(1, input.length() - 1));
        let mut results = IndexMap::new();
        let mut done = input.is_empty();
        self.value_matcher.reset(input.clone());
        while !done {
            Self::check(
                self.value_matcher.looking_at(),
                "Bad key in JS object literal",
            )?;
            let key = self
                .value_matcher
                .group(1)
                .or_else(|| self.value_matcher.group(2))
                .unwrap();
            Self::check(
                !self.value_matcher.hit_end(),
                "Missing value in JS object literal",
            )?;
            Self::check(
                input.char_at(self.value_matcher.end()) == b':' as u16,
                "Missing colon in JS object literal",
            )?;
            self.value_matcher.region(
                self.value_matcher.end() + 1,
                self.value_matcher.region_end(),
            );
            Self::check(
                self.value_matcher.looking_at(),
                "Bad value in JS object literal",
            )?;
            let val = self
                .value_matcher
                .group(1)
                .or_else(|| self.value_matcher.group(2))
                .unwrap();
            results.insert(key, val);
            if !self.value_matcher.hit_end() {
                Self::check(
                    input.char_at(self.value_matcher.end()) == b',' as u16,
                    "Missing comma in JS object literal",
                )?;
                self.value_matcher.region(
                    self.value_matcher.end() + 1,
                    self.value_matcher.region_end(),
                );
            } else {
                done = true;
            }
        }
        Ok(results)
    }
    // port: JsFileLineParser#check
    fn check(condition: bool, message: &str) -> Result<(), ParseException> {
        if condition {
            Ok(())
        } else {
            Err(ParseException::new(message, true))
        }
    }
}
// port: CharMatcher.Whitespace#matches (pinned Guava TABLE/MULTIPLIER/SHIFT)
pub(crate) fn guava_whitespace(c: u16) -> bool {
    const TABLE: [u16; 32] = [
        0x2002, 0x3000, 13, 0x85, 0x200a, 0x2005, 0x2000, 0x3000, 0x2029, 11, 0x3000, 0x2008,
        0x2003, 0x205f, 0x3000, 0x1680, 9, 32, 0x2006, 0x2001, 0x202f, 0xa0, 12, 0x2009, 0x3000,
        0x2004, 0x3000, 0x3000, 0x2028, 10, 0x2007, 0x3000,
    ];
    TABLE[(1682554634u32.wrapping_mul(c as u32) >> 27) as usize] == c
}
// port: String#indexOf(String,int)
fn find(s: &JsString, needle: &str, from: usize) -> Option<usize> {
    usize::try_from(s.index_of_from(&JsString::from(needle), from as i32)).ok()
}
struct StringLines {
    input: JsString,
    position: usize,
}
impl StringLines {
    // port: StringReader#StringReader
    fn new(input: JsString) -> Self {
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
struct JavaLines<R: BufRead> {
    reader: R,
    after_cr: bool,
}
impl<R: BufRead> Iterator for JavaLines<R> {
    type Item = io::Result<JsString>;
    // port: BufferedReader#readLine
    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = vec![];
        loop {
            let mut c = [0];
            match self.reader.read(&mut c) {
                Err(e) => return Some(Err(e)),
                Ok(0) => {
                    return if bytes.is_empty() {
                        None
                    } else {
                        Some(Ok(Charset::UTF_8.decode(&bytes, true).unwrap()))
                    };
                }
                _ => {}
            }
            if self.after_cr {
                self.after_cr = false;
                if c[0] == b'\n' {
                    continue;
                }
            }
            if c[0] == b'\r' || c[0] == b'\n' {
                self.after_cr = c[0] == b'\r';
                return Some(Ok(Charset::UTF_8.decode(&bytes, true).unwrap()));
            }
            bytes.push(c[0]);
        }
    }
}
#[cfg(test)]
mod tests;
