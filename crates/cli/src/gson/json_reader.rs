/*
 * Copyright (C) 2010 Google Inc.
 * Copyright (C) 2011 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * Copyright (C) 2008 Google Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
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
// Ported from Gson 2.9.1 (https://github.com/google/gson): com/google/gson/Gson.java,
//   com/google/gson/internal/bind/TypeAdapters.java, com/google/gson/stream/JsonReader.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java.

//! Gson 2.9.1 JsonReader subset, checked against its decompiled stream state machine.
use crate::abstract_command_line_runner::JsonFileSpec;
use closure_rhino::js_string::JsString;
mod java_string {
    pub fn to_string(value: &[u16]) -> String {
        String::from_utf16_lossy(value)
    }
    pub fn concat(prefix: &str, value: &[u16]) -> Vec<u16> {
        prefix.encode_utf16().chain(value.iter().copied()).collect()
    }
}
#[derive(Clone, Debug)]
pub struct JsonParseException {
    pub class: String,
    pub message: Vec<u16>,
    pub frames: Vec<String>,
}
impl JsonParseException {
    fn new_utf16(class: &str, message: Vec<u16>) -> Self {
        Self {
            class: class.into(),
            message,
            frames: vec![],
        }
    }
    fn at(mut self, method: &str, line: u32) -> Self {
        self.frames.push(format!(
            "com.google.gson.stream.JsonReader.{method}(JsonReader.java:{line})"
        ));
        self
    }
    fn at_frame(mut self, frame: &str) -> Self {
        self.frames.push(frame.into());
        self
    }
    fn into_runner(self) -> crate::abstract_command_line_runner::RunnerException {
        crate::abstract_command_line_runner::RunnerException::java_exception(
            &self.class,
            String::from_utf16_lossy(&self.message),
            self.frames,
        )
    }
    fn new(class: &str, message: String) -> Self {
        Self::new_utf16(class, message.encode_utf16().collect())
    }
}
#[derive(Clone, Debug)]
pub struct JsonError(
    pub String,
    pub String,
    pub Box<crate::abstract_command_line_runner::RunnerException>,
);
impl From<JsonError> for crate::abstract_command_line_runner::RunnerException {
    fn from(error: JsonError) -> Self {
        *error.2
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonToken {
    BEGIN_OBJECT,
    END_OBJECT,
    BEGIN_ARRAY,
    END_ARRAY,
    NAME(u16),
    STRING(u16),
    NUMBER(String),
    BOOLEAN(bool),
    NULL,
    END_DOCUMENT,
}
pub struct JsonReader {
    content: Vec<u16>,
    pos: usize,
    line_number: usize,
    line_start: usize,
    pub lenient: bool,
    stack: Vec<i32>,
    path_names: Vec<Option<Vec<u16>>>,
    path_indices: Vec<usize>,
    peeked: Option<JsonToken>,
}
impl JsonReader {
    // port: com.google.gson.stream.JsonReader#JsonReader
    pub fn new(s: &str) -> Self {
        Self::new_utf16(s.encode_utf16().collect())
    }
    // port: com.google.gson.stream.JsonReader#JsonReader
    pub fn new_utf16(content: Vec<u16>) -> Self {
        let pos = usize::from(content.first() == Some(&0xfeff));
        Self {
            content,
            pos,
            line_number: 0,
            line_start: pos,
            lenient: false,
            stack: vec![6],
            path_names: vec![None],
            path_indices: vec![0],
            peeked: None,
        }
    }
    // port: com.google.gson.stream.JsonReader#locationString
    pub fn location_string(&self) -> String {
        java_string::to_string(&self.location_string_utf16())
    }
    // port: com.google.gson.stream.JsonReader#locationString
    fn location_string_utf16(&self) -> Vec<u16> {
        java_string::concat(
            &format!(
                " at line {} column {} path ",
                self.line_number + 1,
                self.pos - self.line_start + 1
            ),
            &self.get_path_utf16(),
        )
    }
    // port: com.google.gson.stream.JsonReader#getPath
    pub fn get_path(&self) -> String {
        java_string::to_string(&self.get_path_utf16())
    }
    // port: com.google.gson.stream.JsonReader#getPath
    fn get_path_utf16(&self) -> Vec<u16> {
        let mut out = vec![36];
        for (i, &scope) in self.stack.iter().enumerate() {
            match scope {
                1 | 2 => out.extend(format!("[{}]", self.path_indices[i]).encode_utf16()),
                3..=5 => {
                    out.push(46);
                    if let Some(name) = &self.path_names[i] {
                        out.extend_from_slice(name);
                    }
                }
                _ => {}
            }
        }
        out
    }
    // port: com.google.gson.stream.JsonReader#syntaxError
    fn syntax_error(&self, msg: &str) -> JsonParseException {
        JsonParseException::new_utf16(
            "com.google.gson.stream.MalformedJsonException",
            java_string::concat(msg, &self.location_string_utf16()),
        )
        .at("syntaxError", 1607)
    }
    // port: com.google.gson.stream.JsonReader#checkLenient
    fn check_lenient(&self) -> Result<(), JsonParseException> {
        if !self.lenient {
            Err(self
                .syntax_error("Use JsonReader.setLenient(true) to accept malformed JSON")
                .at("checkLenient", 1414))
        } else {
            Ok(())
        }
    }
    // port: com.google.gson.stream.JsonReader#nextNonWhitespace
    fn next_non_whitespace(&mut self, required: bool) -> Result<Option<u16>, JsonParseException> {
        while self.pos < self.content.len() {
            let c = self.content[self.pos];
            self.pos += 1;
            match c {
                10 => {
                    self.line_number += 1;
                    self.line_start = self.pos;
                }
                32 | 13 | 9 => {}
                47 if self.pos < self.content.len() => {
                    self.check_lenient()
                        .map_err(|e| e.at("nextNonWhitespace", 1365))?;
                    match self.content[self.pos] {
                        42 => {
                            self.pos += 1;
                            if !self.skip_to(&[42, 47]) {
                                return Err(self
                                    .syntax_error("Unterminated comment")
                                    .at("nextNonWhitespace", 1372));
                            }
                            self.pos += 2;
                        }
                        47 => {
                            self.pos += 1;
                            self.skip_to_end_of_line();
                        }
                        _ => return Ok(Some(c)),
                    }
                }
                35 => {
                    self.check_lenient()
                        .map_err(|e| e.at("nextNonWhitespace", 1396))?;
                    self.skip_to_end_of_line();
                }
                _ => return Ok(Some(c)),
            }
        }
        if required {
            Err(JsonParseException::new_utf16(
                "java.io.EOFException",
                java_string::concat("End of input", &self.location_string_utf16()),
            )
            .at("nextNonWhitespace", 1406))
        } else {
            Ok(None)
        }
    }
    // port: com.google.gson.stream.JsonReader#skipToEndOfLine
    fn skip_to_end_of_line(&mut self) {
        while self.pos < self.content.len() {
            let c = self.content[self.pos];
            self.pos += 1;
            if c == 10 {
                self.line_number += 1;
                self.line_start = self.pos;
                break;
            }
            if c == 13 {
                break;
            }
        }
    }
    // port: com.google.gson.stream.JsonReader#skipTo
    fn skip_to(&mut self, target: &[u16]) -> bool {
        while self.pos + target.len() <= self.content.len() {
            if &self.content[self.pos..self.pos + target.len()] == target {
                return true;
            }
            if self.content[self.pos] == 10 {
                self.line_number += 1;
                self.line_start = self.pos + 1;
            }
            self.pos += 1;
        }
        false
    }
    // port: com.google.gson.stream.JsonReader#isLiteral
    fn is_literal(&self, c: u16) -> Result<bool, JsonParseException> {
        match c {
            47 | 92 | 59 | 35 | 61 => {
                self.check_lenient()?;
                Ok(false)
            }
            123 | 125 | 91 | 93 | 58 | 44 | 32 | 9 | 12 | 13 | 10 => Ok(false),
            _ => Ok(true),
        }
    }
    // port: com.google.gson.stream.JsonReader#consumeNonExecutePrefix
    fn consume_non_execute_prefix(&mut self) -> Result<(), JsonParseException> {
        self.next_non_whitespace(true)?;
        self.pos -= 1;
        if self.content.get(self.pos..self.pos + 5) == Some(&[41, 93, 125, 39, 10]) {
            self.pos += 5;
        }
        Ok(())
    }
    // port: com.google.gson.stream.JsonReader#peek
    pub fn peek(&mut self) -> Result<JsonToken, JsonParseException> {
        self.peek_impl().map_err(|e| e.at("peek", 435))
    }
    fn peek_impl(&mut self) -> Result<JsonToken, JsonParseException> {
        if let Some(v) = &self.peeked {
            return Ok(v.clone());
        }
        let v = self.do_peek()?;
        self.peeked = Some(v.clone());
        Ok(v)
    }
    // port: com.google.gson.stream.JsonReader#doPeek
    fn do_peek(&mut self) -> Result<JsonToken, JsonParseException> {
        let index = self.stack.len() - 1;
        let context = self.stack[index];
        match context {
            1 => self.stack[index] = 2,
            2 => match self
                .next_non_whitespace(true)
                .map_err(|e| e.at("doPeek", 477))?
                .unwrap()
            {
                93 => return Ok(JsonToken::END_ARRAY),
                59 => self.check_lenient().map_err(|e| e.at("doPeek", 482))?,
                44 => {}
                _ => return Err(self.syntax_error("Unterminated array").at("doPeek", 486)),
            },
            3 | 5 => {
                self.stack[index] = 4;
                if context == 5 {
                    match self
                        .next_non_whitespace(true)
                        .map_err(|e| e.at("doPeek", 492))?
                        .unwrap()
                    {
                        125 => return Ok(JsonToken::END_OBJECT),
                        59 => self.check_lenient().map_err(|e| e.at("doPeek", 497))?,
                        44 => {}
                        _ => {
                            return Err(self.syntax_error("Unterminated object").at("doPeek", 501));
                        }
                    }
                }
                let c = self
                    .next_non_whitespace(true)
                    .map_err(|e| e.at("doPeek", 504))?
                    .unwrap();
                match c {
                    34 => return Ok(JsonToken::NAME(34)),
                    39 => {
                        self.check_lenient().map_err(|e| e.at("doPeek", 509))?;
                        return Ok(JsonToken::NAME(39));
                    }
                    125 if context != 5 => return Ok(JsonToken::END_OBJECT),
                    125 => {
                        return Err(self
                            .syntax_error("Expected name")
                            .at("doPeek", 523)
                            .at("doPeek", 515));
                    }
                    _ => {
                        self.check_lenient().map_err(|e| e.at("doPeek", 518))?;
                        self.pos -= 1;
                        if self.is_literal(c)? {
                            return Ok(JsonToken::NAME(0));
                        }
                        return Err(self.syntax_error("Expected name"));
                    }
                }
            }
            4 => {
                self.stack[index] = 5;
                match self
                    .next_non_whitespace(true)
                    .map_err(|e| e.at("doPeek", 529))?
                    .unwrap()
                {
                    58 => {}
                    61 => {
                        self.check_lenient().map_err(|e| e.at("doPeek", 534))?;
                        if self.content.get(self.pos) == Some(&62) {
                            self.pos += 1;
                        }
                    }
                    _ => return Err(self.syntax_error("Expected ':'").at("doPeek", 540)),
                }
            }
            6 => {
                if self.lenient {
                    self.consume_non_execute_prefix()?;
                }
                self.stack[index] = 7;
            }
            7 => {
                if self
                    .next_non_whitespace(false)
                    .map_err(|e| e.at("doPeek", 548))?
                    .is_none()
                {
                    return Ok(JsonToken::END_DOCUMENT);
                }
                self.check_lenient().map_err(|e| e.at("doPeek", 552))?;
                self.pos -= 1;
            }
            _ => panic!("java.lang.IllegalStateException: JsonReader is closed"),
        }
        let c = self
            .next_non_whitespace(true)
            .map_err(|e| e.at("doPeek", 559))?
            .unwrap();
        match c {
            93 if context == 1 => return Ok(JsonToken::END_ARRAY),
            93 | 59 | 44 => {
                if context == 1 || context == 2 {
                    self.check_lenient().map_err(|e| e.at("doPeek", 570))?;
                    self.pos -= 1;
                    return Ok(JsonToken::NULL);
                }
                return Err(self.syntax_error("Unexpected value").at("doPeek", 574));
            }
            39 => {
                self.check_lenient().map_err(|e| e.at("doPeek", 577))?;
                return Ok(JsonToken::STRING(39));
            }
            34 => return Ok(JsonToken::STRING(34)),
            91 => return Ok(JsonToken::BEGIN_ARRAY),
            123 => return Ok(JsonToken::BEGIN_OBJECT),
            _ => self.pos -= 1,
        }
        if let Some(token) = self.peek_keyword()? {
            return Ok(token);
        }
        if let Some(token) = self.peek_number()? {
            return Ok(token);
        }
        if !self.is_literal(c)? {
            return Err(self.syntax_error("Expected value").at("doPeek", 600));
        }
        self.check_lenient().map_err(|e| e.at("doPeek", 603))?;
        Ok(JsonToken::STRING(0))
    }
    // port: com.google.gson.stream.JsonReader#peekKeyword
    fn peek_keyword(&mut self) -> Result<Option<JsonToken>, JsonParseException> {
        for (word, token) in [
            ("true", JsonToken::BOOLEAN(true)),
            ("false", JsonToken::BOOLEAN(false)),
            ("null", JsonToken::NULL),
        ] {
            let bytes = word.as_bytes();
            if self
                .content
                .get(self.pos..self.pos + bytes.len())
                .is_some_and(|v| {
                    v.iter()
                        .zip(bytes)
                        .all(|(&a, &b)| a == b as u16 || a == b.to_ascii_uppercase() as u16)
                })
                && (self.pos + bytes.len() == self.content.len()
                    || !self.is_literal(self.content[self.pos + bytes.len()])?)
            {
                self.pos += bytes.len();
                return Ok(Some(token));
            }
        }
        Ok(None)
    }
    // port: com.google.gson.stream.JsonReader#peekNumber
    fn peek_number(&mut self) -> Result<Option<JsonToken>, JsonParseException> {
        let mut end = self.pos;
        while end < self.content.len() && self.is_literal(self.content[end])? {
            end += 1;
        }
        if end - self.pos >= 1024 {
            return Ok(None);
        }
        let s = String::from_utf16_lossy(&self.content[self.pos..end]);
        let b = s.as_bytes();
        let mut i = 0;
        if b.get(i) == Some(&b'-') {
            i += 1;
        }
        if b.get(i) == Some(&b'0') {
            i += 1;
        } else {
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if start == i {
                return Ok(None);
            }
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == start {
                return Ok(None);
            }
        }
        if b.get(i) == Some(&b'e') || b.get(i) == Some(&b'E') {
            i += 1;
            if b.get(i) == Some(&b'+') || b.get(i) == Some(&b'-') {
                i += 1;
            }
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == start {
                return Ok(None);
            }
        }
        if i != b.len() || i == 0 {
            return Ok(None);
        }
        self.pos = end;
        let s = if s != "-0" {
            s.parse::<i64>().map_or(s.clone(), |n| n.to_string())
        } else {
            s
        };
        Ok(Some(JsonToken::NUMBER(s)))
    }
    // port: com.google.gson.stream.JsonReader#nextQuotedValue
    fn next_quoted_value(&mut self, quote: u16) -> Result<Vec<u16>, JsonParseException> {
        let mut value = Vec::new();
        while self.pos < self.content.len() {
            let c = self.content[self.pos];
            self.pos += 1;
            if c == quote {
                return Ok(value);
            }
            if c == 92 {
                value.push(
                    self.read_escape_character()
                        .map_err(|e| e.at("nextQuotedValue", 1024))?,
                );
            } else {
                if c == 10 {
                    self.line_number += 1;
                    self.line_start = self.pos;
                }
                value.push(c);
            }
        }
        Err(self
            .syntax_error("Unterminated string")
            .at("nextQuotedValue", 1041))
    }
    // port: com.google.gson.stream.JsonReader#readEscapeCharacter
    fn read_escape_character(&mut self) -> Result<u16, JsonParseException> {
        if self.pos == self.content.len() {
            return Err(self
                .syntax_error("Unterminated escape sequence")
                .at("readEscapeCharacter", 1551)
                .at("readEscapeCharacter", 1544));
        }
        let c = self.content[self.pos];
        self.pos += 1;
        match c {
            117 => {
                if self.pos + 4 > self.content.len() {
                    return Err(self.syntax_error("Unterminated escape sequence"));
                }
                let units = &self.content[self.pos..self.pos + 4];
                let s = String::from_utf16_lossy(units);
                if !units.iter().all(|&c| matches!(c,48..=57|65..=70|97..=102)) {
                    return Err(JsonParseException::new_utf16(
                        "java.lang.NumberFormatException",
                        java_string::concat("\\u", units),
                    ));
                }
                let n = u16::from_str_radix(&s, 16).unwrap();
                self.pos += 4;
                Ok(n)
            }
            116 => Ok(9),
            98 => Ok(8),
            110 => Ok(10),
            114 => Ok(13),
            102 => Ok(12),
            10 => {
                self.line_number += 1;
                self.line_start = self.pos;
                Ok(c)
            }
            39 | 34 | 92 | 47 => Ok(c),
            _ => Err(self
                .syntax_error("Invalid escape sequence")
                .at("readEscapeCharacter", 1598)),
        }
    }
    // port: com.google.gson.stream.JsonReader#nextUnquotedValue
    fn next_unquoted_value(&mut self) -> Result<Vec<u16>, JsonParseException> {
        let start = self.pos;
        while self.pos < self.content.len() && self.is_literal(self.content[self.pos])? {
            self.pos += 1;
        }
        Ok(self.content[start..self.pos].to_vec())
    }
    // port: com.google.gson.stream.JsonReader#push
    fn push(&mut self, scope: i32) {
        self.stack.push(scope);
        self.path_names.push(None);
        self.path_indices.push(0);
    }
    // port: com.google.gson.stream.JsonReader#endObject
    fn pop(&mut self) {
        self.stack.pop();
        self.path_names.pop();
        self.path_indices.pop();
        *self.path_indices.last_mut().unwrap() += 1;
    }
    // port: JsonReader#beginArray
    pub fn begin_array(&mut self) -> Result<(), JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("beginArray", 353))?;
        if token != JsonToken::BEGIN_ARRAY {
            return Err(self.expected("BEGIN_ARRAY", &token).at("beginArray", 360));
        }
        self.peeked = None;
        self.push(1);
        Ok(())
    }
    // port: JsonReader#endArray
    pub fn end_array(&mut self) -> Result<(), JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("endArray", 373))?;
        if token != JsonToken::END_ARRAY {
            return Err(self.expected("END_ARRAY", &token).at("endArray", 379));
        }
        self.peeked = None;
        self.pop();
        Ok(())
    }
    // port: JsonReader#beginObject
    pub fn begin_object(&mut self) -> Result<(), JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("beginObject", 389))?;
        if token != JsonToken::BEGIN_OBJECT {
            return Err(self.expected("BEGIN_OBJECT", &token).at("beginObject", 395));
        }
        self.peeked = None;
        self.push(3);
        Ok(())
    }
    // port: JsonReader#endObject
    pub fn end_object(&mut self) -> Result<(), JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("endObject", 408))?;
        if token != JsonToken::END_OBJECT {
            return Err(self.expected("END_OBJECT", &token).at("endObject", 415));
        }
        self.peeked = None;
        self.pop();
        Ok(())
    }
    // port: JsonReader#hasNext
    pub fn has_next(&mut self) -> Result<bool, JsonParseException> {
        Ok(!matches!(
            self.peek_impl().map_err(|e| e.at("hasNext", 424))?,
            JsonToken::END_ARRAY | JsonToken::END_OBJECT | JsonToken::END_DOCUMENT
        ))
    }
    // port: JsonReader#nextName
    pub fn next_name(&mut self) -> Result<JsString, JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("nextName", 789))?;
        let JsonToken::NAME(quote) = token else {
            return Err(self.expected("a name", &token).at("nextName", 803));
        };
        self.peeked = None;
        let value = if quote == 0 {
            self.next_unquoted_value()?
        } else {
            self.next_quoted_value(quote)?
        };
        *self.path_names.last_mut().unwrap() = Some(value.clone());
        Ok(JsString::from_units(value))
    }
    // port: JsonReader#nextString / TypeAdapters.STRING#read
    pub fn next_string(&mut self) -> Result<Option<JsString>, JsonParseException> {
        let token = self.peek_impl().map_err(|e| e.at("nextString", 817))?;
        let value = match token {
            JsonToken::STRING(quote) => Some(JsString::from_units(if quote == 0 {
                self.next_unquoted_value()?
            } else {
                self.next_quoted_value(quote)
                    .map_err(|e| e.at("nextString", if quote == 39 { 823 } else { 825 }))?
            })),
            JsonToken::NUMBER(ref number) => Some(JsString::from(number.clone())),
            JsonToken::BOOLEAN(value) => Some(JsString::from(if value { "true" } else { "false" })),
            JsonToken::NULL => None,
            _ => return Err(self.expected("a string", &token).at("nextString", 835)),
        };
        self.peeked = None;
        *self.path_indices.last_mut().unwrap() += 1;
        Ok(value)
    }
    // port: JsonReader#skipValue
    pub fn skip_value(&mut self) -> Result<(), JsonParseException> {
        let token = self.peek()?;
        match token {
            JsonToken::BEGIN_ARRAY => {
                self.begin_array()?;
                while self.has_next()? {
                    self.skip_value()?;
                }
                self.end_array()?;
            }
            JsonToken::BEGIN_OBJECT => {
                self.begin_object()?;
                while self.has_next()? {
                    self.next_name()?;
                    self.skip_value()?;
                }
                self.end_object()?;
            }
            _ => {
                self.next_string()?;
            }
        }
        Ok(())
    }
    fn expected(&self, expected: &str, token: &JsonToken) -> JsonParseException {
        let kind = match token {
            JsonToken::NAME(_) => "NAME",
            JsonToken::STRING(_) => "STRING",
            JsonToken::NUMBER(_) => "NUMBER",
            JsonToken::BOOLEAN(_) => "BOOLEAN",
            JsonToken::NULL => "NULL",
            JsonToken::BEGIN_ARRAY => "BEGIN_ARRAY",
            JsonToken::END_ARRAY => "END_ARRAY",
            JsonToken::BEGIN_OBJECT => "BEGIN_OBJECT",
            JsonToken::END_OBJECT => "END_OBJECT",
            JsonToken::END_DOCUMENT => "END_DOCUMENT",
        };
        JsonParseException::new(
            "java.lang.IllegalStateException",
            format!(
                "Expected {expected} but was {kind}{}",
                self.location_string()
            ),
        )
    }
}
// port: AbstractCommandLineRunner#parseJsonFilesFromInputStream / Gson#fromJson
pub fn parse_json_files(text: &str) -> Result<Vec<Option<JsonFileSpec>>, JsonError> {
    use crate::abstract_command_line_runner::RunnerException;
    fn cli(error: JsonParseException, line: u32) -> RunnerException {
        error.into_runner().at_cli(
            "AbstractCommandLineRunner",
            "parseJsonFilesFromInputStream",
            line,
        )
    }
    fn gson(error: JsonParseException, adapter: bool) -> RunnerException {
        let class = error.class.clone();
        let message = String::from_utf16_lossy(&error.message);
        let error = error.into_runner();
        if matches!(
            class.as_str(),
            "java.lang.IllegalStateException"
                | "java.io.EOFException"
                | "com.google.gson.stream.MalformedJsonException"
        ) {
            let cause = error
                .at("com.google.gson.Gson.fromJson(Gson.java:1058)")
                .at_cli(
                    "AbstractCommandLineRunner",
                    "parseJsonFilesFromInputStream",
                    558,
                );
            let outer = if adapter {
                RunnerException::java_exception("com.google.gson.JsonSyntaxException", format!("{class}: {message}"),
                    vec!["com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:270)".into(),
                        "com.google.gson.Gson.fromJson(Gson.java:1058)".into()])
            } else {
                RunnerException::java_exception(
                    "com.google.gson.JsonSyntaxException",
                    format!("{class}: {message}"),
                    vec!["com.google.gson.Gson.fromJson(Gson.java:1073)".into()],
                )
            };
            outer
                .at_cli(
                    "AbstractCommandLineRunner",
                    "parseJsonFilesFromInputStream",
                    558,
                )
                .caused_by(cause)
        } else {
            error
                .at("com.google.gson.Gson.fromJson(Gson.java:1058)")
                .at_cli(
                    "AbstractCommandLineRunner",
                    "parseJsonFilesFromInputStream",
                    558,
                )
        }
    }
    let mut reader = JsonReader::new(text);
    let result = (|| -> Result<Vec<Option<JsonFileSpec>>, RunnerException> {
        reader.begin_array().map_err(|e| cli(e, 556))?;
        let mut files = Vec::new();
        while reader.has_next().map_err(|e| cli(e, 557))? {
            // Gson.fromJson temporarily enables lenient reading for each object only.
            reader.lenient = true;
            if matches!(reader.peek().map_err(|e| gson(e, false))?, JsonToken::NULL) {
                reader.next_string().map_err(|e| gson(e, false))?;
                reader.lenient = false;
                files.push(None);
                continue;
            }
            reader.begin_object().map_err(|e| gson(e.at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:259)"), true))?;
            let mut file = JsonFileSpec::default();
            while reader.has_next().map_err(|e| gson(e.at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:260)"), false))? {
                let name = reader.next_name().map_err(|e| gson(e.at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:261)"), false))?.to_string_lossy();
                let target = match name.as_str() {
                    "src" => Some(&mut file.src),
                    "path" => Some(&mut file.path),
                    "sourceMap" | "source_map" => Some(&mut file.source_map),
                    "webpackId" | "webpack_id" => Some(&mut file.webpack_id),
                    _ => None,
                };
                if let Some(target) = target {
                    *target = reader.next_string().map_err(|e| gson(e
                        .at_frame("com.google.gson.internal.bind.TypeAdapters$15.read(TypeAdapters.java:394)")
                        .at_frame("com.google.gson.internal.bind.TypeAdapters$15.read(TypeAdapters.java:382)")
                        .at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$1.read(ReflectiveTypeAdapterFactory.java:161)")
                        .at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:266)"), false))?;
                } else {
                    reader.skip_value().map_err(|e| gson(e.at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:264)"), false))?;
                }
            }
            reader.end_object().map_err(|e| gson(e.at_frame("com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:268)"), false))?;
            reader.lenient = false;
            files.push(Some(file));
        }
        reader.end_array().map_err(|e| cli(e, 560))?;
        Ok(files)
    })();
    result.map_err(|e| {
        let class = match &e.1 {
            crate::abstract_command_line_runner::RunnerExceptionKind::JavaException {
                class,
                ..
            } => class.clone(),
            _ => unreachable!(),
        };
        JsonError(e.0.clone(), class, Box::new(e))
    })
}
