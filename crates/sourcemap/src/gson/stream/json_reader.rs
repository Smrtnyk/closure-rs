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
// Ported from Gson 2.9.1 (https://github.com/google/gson):
//   com/google/gson/internal/bind/TypeAdapters.java, com/google/gson/stream/JsonReader.java.

use super::json_scope::*;
use crate::gson::{
    JsonArray, JsonElement, JsonObject, JsonParseException, JsonPrimitive,
    lazily_parsed_number::LazilyParsedNumber,
};
use closure_rhino::js_string::JsString;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonToken {
    BEGIN_ARRAY,
    END_ARRAY,
    BEGIN_OBJECT,
    END_OBJECT,
    NAME,
    STRING,
    NUMBER,
    BOOLEAN,
    NULL,
    END_DOCUMENT,
}
pub struct JsonReader {
    // Java Reader is a StringReader on this package's only input path.
    input: JsString,
    input_position: usize,
    pub lenient: bool,
    buffer: [u16; Self::BUFFER_SIZE],
    pos: usize,
    limit: usize,
    line_number: i32,
    line_start: i32,
    peeked: i32,
    peeked_long: i64,
    peeked_number_length: usize,
    peeked_string: Option<JsString>,
    stack: Vec<i32>,
    stack_size: usize,
    path_names: Vec<Option<JsString>>,
    path_indices: Vec<i32>,
}
impl JsonReader {
    const MIN_INCOMPLETE_INTEGER: i64 = i64::MIN / 10;
    const PEEKED_NONE: i32 = 0;
    const BUFFER_SIZE: usize = 1024;
    const PEEKED_BEGIN_OBJECT: i32 = 1;
    const PEEKED_END_OBJECT: i32 = 2;
    const PEEKED_BEGIN_ARRAY: i32 = 3;
    const PEEKED_END_ARRAY: i32 = 4;
    const PEEKED_TRUE: i32 = 5;
    const PEEKED_FALSE: i32 = 6;
    const PEEKED_NULL: i32 = 7;
    const PEEKED_SINGLE_QUOTED: i32 = 8;
    const PEEKED_DOUBLE_QUOTED: i32 = 9;
    const PEEKED_UNQUOTED: i32 = 10;
    const PEEKED_BUFFERED: i32 = 11;
    const PEEKED_SINGLE_QUOTED_NAME: i32 = 12;
    const PEEKED_DOUBLE_QUOTED_NAME: i32 = 13;
    const PEEKED_UNQUOTED_NAME: i32 = 14;
    const PEEKED_LONG: i32 = 15;
    const PEEKED_NUMBER: i32 = 16;
    const PEEKED_EOF: i32 = 17;
    const NUMBER_CHAR_NONE: i32 = 0;
    const NUMBER_CHAR_SIGN: i32 = 1;
    const NUMBER_CHAR_DIGIT: i32 = 2;
    const NUMBER_CHAR_DECIMAL: i32 = 3;
    const NUMBER_CHAR_FRACTION_DIGIT: i32 = 4;
    const NUMBER_CHAR_EXP_E: i32 = 5;
    const NUMBER_CHAR_EXP_SIGN: i32 = 6;
    const NUMBER_CHAR_EXP_DIGIT: i32 = 7;
    // port: com.google.gson.stream.JsonReader#JsonReader
    pub fn new(input: impl Into<JsString>) -> Self {
        let mut stack = vec![0; 32];
        stack[0] = EMPTY_DOCUMENT;
        Self {
            input: input.into(),
            input_position: 0,
            lenient: false,
            buffer: [0; Self::BUFFER_SIZE],
            pos: 0,
            limit: 0,
            line_number: 0,
            line_start: 0,
            peeked: Self::PEEKED_NONE,
            peeked_long: 0,
            peeked_number_length: 0,
            peeked_string: None,
            stack,
            stack_size: 1,
            path_names: vec![None; 32],
            path_indices: vec![0; 32],
        }
    }
    // port: com.google.gson.stream.JsonReader#setLenient
    pub fn set_lenient(&mut self, lenient: bool) {
        self.lenient = lenient;
    }
    // port: com.google.gson.stream.JsonReader#isLenient
    pub fn is_lenient(&self) -> bool {
        self.lenient
    }
    // port: com.google.gson.stream.JsonReader#beginArray
    pub fn begin_array(&mut self) -> Result<(), JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_BEGIN_ARRAY {
            self.push(EMPTY_ARRAY);
            self.path_indices[self.stack_size - 1] = 0;
            self.peeked = Self::PEEKED_NONE;
            Ok(())
        } else {
            let token = self.peek()?;
            Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected BEGIN_ARRAY but was {token:?}"))
                    .concat(&self.location_string()),
            ))
        }
    }
    // port: com.google.gson.stream.JsonReader#endArray
    pub fn end_array(&mut self) -> Result<(), JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_END_ARRAY {
            self.stack_size -= 1;
            self.path_indices[self.stack_size - 1] += 1;
            self.peeked = Self::PEEKED_NONE;
            Ok(())
        } else {
            let token = self.peek()?;
            Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected END_ARRAY but was {token:?}"))
                    .concat(&self.location_string()),
            ))
        }
    }
    // port: com.google.gson.stream.JsonReader#beginObject
    pub fn begin_object(&mut self) -> Result<(), JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_BEGIN_OBJECT {
            self.push(EMPTY_OBJECT);
            self.peeked = Self::PEEKED_NONE;
            Ok(())
        } else {
            let token = self.peek()?;
            Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected BEGIN_OBJECT but was {token:?}"))
                    .concat(&self.location_string()),
            ))
        }
    }
    // port: com.google.gson.stream.JsonReader#endObject
    pub fn end_object(&mut self) -> Result<(), JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_END_OBJECT {
            self.stack_size -= 1;
            self.path_names[self.stack_size] = None;
            self.path_indices[self.stack_size - 1] += 1;
            self.peeked = Self::PEEKED_NONE;
            Ok(())
        } else {
            let token = self.peek()?;
            Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected END_OBJECT but was {token:?}"))
                    .concat(&self.location_string()),
            ))
        }
    }
    // port: com.google.gson.stream.JsonReader#hasNext
    pub fn has_next(&mut self) -> Result<bool, JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        Ok(p != Self::PEEKED_END_OBJECT && p != Self::PEEKED_END_ARRAY)
    }
    // port: com.google.gson.stream.JsonReader#peek
    pub fn peek(&mut self) -> Result<JsonToken, JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        match p {
            Self::PEEKED_BEGIN_OBJECT => Ok(JsonToken::BEGIN_OBJECT),
            Self::PEEKED_END_OBJECT => Ok(JsonToken::END_OBJECT),
            Self::PEEKED_BEGIN_ARRAY => Ok(JsonToken::BEGIN_ARRAY),
            Self::PEEKED_END_ARRAY => Ok(JsonToken::END_ARRAY),
            Self::PEEKED_SINGLE_QUOTED_NAME
            | Self::PEEKED_DOUBLE_QUOTED_NAME
            | Self::PEEKED_UNQUOTED_NAME => Ok(JsonToken::NAME),
            Self::PEEKED_TRUE | Self::PEEKED_FALSE => Ok(JsonToken::BOOLEAN),
            Self::PEEKED_NULL => Ok(JsonToken::NULL),
            Self::PEEKED_SINGLE_QUOTED
            | Self::PEEKED_DOUBLE_QUOTED
            | Self::PEEKED_UNQUOTED
            | Self::PEEKED_BUFFERED => Ok(JsonToken::STRING),
            Self::PEEKED_LONG | Self::PEEKED_NUMBER => Ok(JsonToken::NUMBER),
            Self::PEEKED_EOF => Ok(JsonToken::END_DOCUMENT),
            _ => panic!("java.lang.AssertionError"),
        }
    }
    // port: com.google.gson.stream.JsonReader#doPeek
    fn do_peek(&mut self) -> Result<i32, JsonParseException> {
        let peek_stack = self.stack[self.stack_size - 1];
        if peek_stack == EMPTY_ARRAY {
            self.stack[self.stack_size - 1] = NONEMPTY_ARRAY;
        } else if peek_stack == NONEMPTY_ARRAY {
            let c = self.next_non_whitespace(true)?;
            match c {
                c if c == b']' as i32 => {
                    self.peeked = Self::PEEKED_END_ARRAY;
                    return Ok(self.peeked);
                }
                c if c == b';' as i32 => {
                    self.check_lenient()?;
                }
                c if c == b',' as i32 => {}
                _ => return Err(self.syntax_error("Unterminated array")),
            }
        } else if peek_stack == EMPTY_OBJECT || peek_stack == NONEMPTY_OBJECT {
            self.stack[self.stack_size - 1] = DANGLING_NAME;
            if peek_stack == NONEMPTY_OBJECT {
                let c = self.next_non_whitespace(true)?;
                match c {
                    c if c == b'}' as i32 => {
                        self.peeked = Self::PEEKED_END_OBJECT;
                        return Ok(self.peeked);
                    }
                    c if c == b';' as i32 => {
                        self.check_lenient()?;
                    }
                    c if c == b',' as i32 => {}
                    _ => return Err(self.syntax_error("Unterminated object")),
                }
            }
            let c = self.next_non_whitespace(true)?;
            match c {
                c if c == b'"' as i32 => {
                    self.peeked = Self::PEEKED_DOUBLE_QUOTED_NAME;
                    return Ok(self.peeked);
                }
                c if c == b'\'' as i32 => {
                    self.check_lenient()?;
                    self.peeked = Self::PEEKED_SINGLE_QUOTED_NAME;
                    return Ok(self.peeked);
                }
                c if c == b'}' as i32 => {
                    if peek_stack != NONEMPTY_OBJECT {
                        self.peeked = Self::PEEKED_END_OBJECT;
                        return Ok(self.peeked);
                    }
                    return Err(self.syntax_error("Expected name"));
                }
                _ => {
                    self.check_lenient()?;
                    self.pos -= 1;
                    if self.is_literal(c as u16)? {
                        self.peeked = Self::PEEKED_UNQUOTED_NAME;
                        return Ok(self.peeked);
                    }
                    return Err(self.syntax_error("Expected name"));
                }
            }
        } else if peek_stack == DANGLING_NAME {
            self.stack[self.stack_size - 1] = NONEMPTY_OBJECT;
            let c = self.next_non_whitespace(true)?;
            match c {
                c if c == b':' as i32 => {}
                c if c == b'=' as i32 => {
                    self.check_lenient()?;
                    if (self.pos < self.limit || self.fill_buffer(1))
                        && self.buffer[self.pos] == b'>' as u16
                    {
                        self.pos += 1;
                    }
                }
                _ => return Err(self.syntax_error("Expected ':'")),
            }
        } else if peek_stack == EMPTY_DOCUMENT {
            if self.lenient {
                self.consume_non_execute_prefix()?;
            }
            self.stack[self.stack_size - 1] = NONEMPTY_DOCUMENT;
        } else if peek_stack == NONEMPTY_DOCUMENT {
            let c = self.next_non_whitespace(false)?;
            if c == -1 {
                self.peeked = Self::PEEKED_EOF;
                return Ok(self.peeked);
            }
            self.check_lenient()?;
            self.pos -= 1;
        } else if peek_stack == CLOSED {
            panic!("java.lang.IllegalStateException: JsonReader is closed");
        }
        let c = self.next_non_whitespace(true)?;
        match c {
            c if c == b']' as i32 && peek_stack == EMPTY_ARRAY => {
                self.peeked = Self::PEEKED_END_ARRAY;
                return Ok(self.peeked);
            }
            c if c == b']' as i32 || c == b';' as i32 || c == b',' as i32 => {
                if peek_stack == EMPTY_ARRAY || peek_stack == NONEMPTY_ARRAY {
                    self.check_lenient()?;
                    self.pos -= 1;
                    self.peeked = Self::PEEKED_NULL;
                    return Ok(self.peeked);
                }
                return Err(self.syntax_error("Unexpected value"));
            }
            c if c == b'\'' as i32 => {
                self.check_lenient()?;
                self.peeked = Self::PEEKED_SINGLE_QUOTED;
                return Ok(self.peeked);
            }
            c if c == b'"' as i32 => {
                self.peeked = Self::PEEKED_DOUBLE_QUOTED;
                return Ok(self.peeked);
            }
            c if c == b'[' as i32 => {
                self.peeked = Self::PEEKED_BEGIN_ARRAY;
                return Ok(self.peeked);
            }
            c if c == b'{' as i32 => {
                self.peeked = Self::PEEKED_BEGIN_OBJECT;
                return Ok(self.peeked);
            }
            _ => self.pos -= 1,
        }
        let mut result = self.peek_keyword()?;
        if result != Self::PEEKED_NONE {
            return Ok(result);
        }
        result = self.peek_number()?;
        if result != Self::PEEKED_NONE {
            return Ok(result);
        }
        if !self.is_literal(self.buffer[self.pos])? {
            return Err(self.syntax_error("Expected value"));
        }
        self.check_lenient()?;
        self.peeked = Self::PEEKED_UNQUOTED;
        Ok(self.peeked)
    }
    // port: com.google.gson.stream.JsonReader#peekKeyword
    fn peek_keyword(&mut self) -> Result<i32, JsonParseException> {
        let c = self.buffer[self.pos];
        let keyword: &[u8];
        let keyword_upper: &[u8];
        let peeking;
        if c == b't' as u16 || c == b'T' as u16 {
            keyword = b"true";
            keyword_upper = b"TRUE";
            peeking = Self::PEEKED_TRUE;
        } else if c == b'f' as u16 || c == b'F' as u16 {
            keyword = b"false";
            keyword_upper = b"FALSE";
            peeking = Self::PEEKED_FALSE;
        } else if c == b'n' as u16 || c == b'N' as u16 {
            keyword = b"null";
            keyword_upper = b"NULL";
            peeking = Self::PEEKED_NULL;
        } else {
            return Ok(Self::PEEKED_NONE);
        }
        let length = keyword.len();
        for i in 1..length {
            if self.pos + i >= self.limit && !self.fill_buffer(i + 1) {
                return Ok(Self::PEEKED_NONE);
            }
            let c = self.buffer[self.pos + i];
            if c != keyword[i] as u16 && c != keyword_upper[i] as u16 {
                return Ok(Self::PEEKED_NONE);
            }
        }
        if (self.pos + length < self.limit || self.fill_buffer(length + 1))
            && self.is_literal(self.buffer[self.pos + length])?
        {
            return Ok(Self::PEEKED_NONE);
        }
        self.pos += length;
        self.peeked = peeking;
        Ok(self.peeked)
    }
    // port: com.google.gson.stream.JsonReader#peekNumber
    fn peek_number(&mut self) -> Result<i32, JsonParseException> {
        let mut p = self.pos;
        let mut l = self.limit;
        let mut value = 0i64;
        let mut negative = false;
        let mut fits_in_long = true;
        let mut last = Self::NUMBER_CHAR_NONE;
        let mut i = 0;
        loop {
            if p + i == l {
                if i == self.buffer.len() {
                    return Ok(Self::PEEKED_NONE);
                }
                if !self.fill_buffer(i + 1) {
                    break;
                }
                p = self.pos;
                l = self.limit;
            }
            let c = self.buffer[p + i];
            match c {
                c if c == b'-' as u16 => {
                    if last == Self::NUMBER_CHAR_NONE {
                        negative = true;
                        last = Self::NUMBER_CHAR_SIGN;
                    } else if last == Self::NUMBER_CHAR_EXP_E {
                        last = Self::NUMBER_CHAR_EXP_SIGN;
                    } else {
                        return Ok(Self::PEEKED_NONE);
                    }
                }
                c if c == b'+' as u16 => {
                    if last == Self::NUMBER_CHAR_EXP_E {
                        last = Self::NUMBER_CHAR_EXP_SIGN;
                    } else {
                        return Ok(Self::PEEKED_NONE);
                    }
                }
                c if c == b'e' as u16 || c == b'E' as u16 => {
                    if last == Self::NUMBER_CHAR_DIGIT || last == Self::NUMBER_CHAR_FRACTION_DIGIT {
                        last = Self::NUMBER_CHAR_EXP_E;
                    } else {
                        return Ok(Self::PEEKED_NONE);
                    }
                }
                c if c == b'.' as u16 => {
                    if last == Self::NUMBER_CHAR_DIGIT {
                        last = Self::NUMBER_CHAR_DECIMAL;
                    } else {
                        return Ok(Self::PEEKED_NONE);
                    }
                }
                _ => {
                    if c < b'0' as u16 || c > b'9' as u16 {
                        if !self.is_literal(c)? {
                            break;
                        }
                        return Ok(Self::PEEKED_NONE);
                    }
                    if last == Self::NUMBER_CHAR_SIGN || last == Self::NUMBER_CHAR_NONE {
                        value = -((c - b'0' as u16) as i64);
                        last = Self::NUMBER_CHAR_DIGIT;
                    } else if last == Self::NUMBER_CHAR_DIGIT {
                        if value == 0 {
                            return Ok(Self::PEEKED_NONE);
                        }
                        let new_value = value
                            .wrapping_mul(10)
                            .wrapping_sub((c - b'0' as u16) as i64);
                        fits_in_long &= value > Self::MIN_INCOMPLETE_INTEGER
                            || (value == Self::MIN_INCOMPLETE_INTEGER && new_value < value);
                        value = new_value;
                    } else if last == Self::NUMBER_CHAR_DECIMAL {
                        last = Self::NUMBER_CHAR_FRACTION_DIGIT;
                    } else if last == Self::NUMBER_CHAR_EXP_E || last == Self::NUMBER_CHAR_EXP_SIGN
                    {
                        last = Self::NUMBER_CHAR_EXP_DIGIT;
                    }
                }
            }
            i += 1;
        }
        if last == Self::NUMBER_CHAR_DIGIT
            && fits_in_long
            && (value != i64::MIN || negative)
            && (value != 0 || !negative)
        {
            self.peeked_long = if negative { value } else { -value };
            self.pos += i;
            self.peeked = Self::PEEKED_LONG;
            return Ok(self.peeked);
        } else if last == Self::NUMBER_CHAR_DIGIT
            || last == Self::NUMBER_CHAR_FRACTION_DIGIT
            || last == Self::NUMBER_CHAR_EXP_DIGIT
        {
            self.peeked_number_length = i;
            self.peeked = Self::PEEKED_NUMBER;
            return Ok(self.peeked);
        }
        Ok(Self::PEEKED_NONE)
    }
    // port: com.google.gson.stream.JsonReader#isLiteral
    fn is_literal(&self, c: u16) -> Result<bool, JsonParseException> {
        match c {
            c if c == b'/' as u16
                || c == b'\\' as u16
                || c == b';' as u16
                || c == b'#' as u16
                || c == b'=' as u16 =>
            {
                self.check_lenient()?;
                Ok(false)
            }
            c if c == b'{' as u16
                || c == b'}' as u16
                || c == b'[' as u16
                || c == b']' as u16
                || c == b':' as u16
                || c == b',' as u16
                || c == b' ' as u16
                || c == b'\t' as u16
                || c == b'\x0c' as u16
                || c == b'\r' as u16
                || c == b'\n' as u16 =>
            {
                Ok(false)
            }
            _ => Ok(true),
        }
    }
    // port: com.google.gson.stream.JsonReader#nextName
    pub fn next_name(&mut self) -> Result<JsString, JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        let result;
        if p == Self::PEEKED_UNQUOTED_NAME {
            result = self.next_unquoted_value()?;
        } else if p == Self::PEEKED_SINGLE_QUOTED_NAME {
            result = self.next_quoted_value(b'\'' as u16)?;
        } else if p == Self::PEEKED_DOUBLE_QUOTED_NAME {
            result = self.next_quoted_value(b'"' as u16)?;
        } else {
            let token = self.peek()?;
            return Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected a name but was {token:?}"))
                    .concat(&self.location_string()),
            ));
        }
        self.peeked = Self::PEEKED_NONE;
        self.path_names[self.stack_size - 1] = Some(result.clone());
        Ok(result)
    }
    // port: com.google.gson.stream.JsonReader#nextString
    pub fn next_string(&mut self) -> Result<JsString, JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        let result;
        if p == Self::PEEKED_UNQUOTED {
            result = self.next_unquoted_value()?;
        } else if p == Self::PEEKED_SINGLE_QUOTED {
            result = self.next_quoted_value(b'\'' as u16)?;
        } else if p == Self::PEEKED_DOUBLE_QUOTED {
            result = self.next_quoted_value(b'"' as u16)?;
        } else if p == Self::PEEKED_BUFFERED {
            result = self.peeked_string.take().unwrap();
        } else if p == Self::PEEKED_LONG {
            result = self.peeked_long.to_string().into();
        } else if p == Self::PEEKED_NUMBER {
            result = JsString::from_units(
                self.buffer[self.pos..self.pos + self.peeked_number_length].to_vec(),
            );
            self.pos += self.peeked_number_length;
        } else {
            let token = self.peek()?;
            return Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                JsString::from(format!("Expected a string but was {token:?}"))
                    .concat(&self.location_string()),
            ));
        }
        self.peeked = Self::PEEKED_NONE;
        self.path_indices[self.stack_size - 1] += 1;
        Ok(result)
    }
    // port: com.google.gson.stream.JsonReader#nextBoolean
    pub fn next_boolean(&mut self) -> Result<bool, JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_TRUE {
            self.peeked = Self::PEEKED_NONE;
            self.path_indices[self.stack_size - 1] += 1;
            return Ok(true);
        } else if p == Self::PEEKED_FALSE {
            self.peeked = Self::PEEKED_NONE;
            self.path_indices[self.stack_size - 1] += 1;
            return Ok(false);
        }
        let token = self.peek()?;
        Err(JsonParseException::new(
            "java.lang.IllegalStateException",
            JsString::from(format!("Expected a boolean but was {token:?}"))
                .concat(&self.location_string()),
        ))
    }
    // port: com.google.gson.stream.JsonReader#nextNull
    pub fn next_null(&mut self) -> Result<(), JsonParseException> {
        let mut p = self.peeked;
        if p == Self::PEEKED_NONE {
            p = self.do_peek()?;
        }
        if p == Self::PEEKED_NULL {
            self.peeked = Self::PEEKED_NONE;
            self.path_indices[self.stack_size - 1] += 1;
            return Ok(());
        }
        let token = self.peek()?;
        Err(JsonParseException::new(
            "java.lang.IllegalStateException",
            JsString::from(format!("Expected null but was {token:?}"))
                .concat(&self.location_string()),
        ))
    }
    // port: com.google.gson.stream.JsonReader#nextQuotedValue
    fn next_quoted_value(&mut self, quote: u16) -> Result<JsString, JsonParseException> {
        let mut builder: Option<Vec<u16>> = None;
        loop {
            let mut p = self.pos;
            let mut l = self.limit;
            let mut start = p;
            while p < l {
                let c = self.buffer[p];
                p += 1;
                if c == quote {
                    self.pos = p;
                    let len = p - start - 1;
                    if builder.is_none() {
                        return Ok(JsString::from_units(
                            self.buffer[start..start + len].to_vec(),
                        ));
                    }
                    builder
                        .as_mut()
                        .unwrap()
                        .extend_from_slice(&self.buffer[start..start + len]);
                    return Ok(JsString::from_units(builder.unwrap()));
                } else if c == b'\\' as u16 {
                    self.pos = p;
                    let len = p - start - 1;
                    if builder.is_none() {
                        let estimated_length = (len + 1) * 2;
                        builder = Some(Vec::with_capacity(estimated_length.max(16)));
                    }
                    builder
                        .as_mut()
                        .unwrap()
                        .extend_from_slice(&self.buffer[start..start + len]);
                    builder
                        .as_mut()
                        .unwrap()
                        .push(self.read_escape_character()?);
                    p = self.pos;
                    l = self.limit;
                    start = p;
                } else if c == b'\n' as u16 {
                    self.line_number += 1;
                    self.line_start = p as i32;
                }
            }
            if builder.is_none() {
                let estimated_length = (p - start) * 2;
                builder = Some(Vec::with_capacity(estimated_length.max(16)));
            }
            builder
                .as_mut()
                .unwrap()
                .extend_from_slice(&self.buffer[start..p]);
            self.pos = p;
            if !self.fill_buffer(1) {
                return Err(self.syntax_error("Unterminated string"));
            }
        }
    }
    // port: com.google.gson.stream.JsonReader#nextUnquotedValue
    fn next_unquoted_value(&mut self) -> Result<JsString, JsonParseException> {
        let mut builder: Option<Vec<u16>> = None;
        let mut i = 0;
        'find: loop {
            while self.pos + i < self.limit {
                let c = self.buffer[self.pos + i];
                match c {
                    c if c == b'/' as u16
                        || c == b'\\' as u16
                        || c == b';' as u16
                        || c == b'#' as u16
                        || c == b'=' as u16 =>
                    {
                        self.check_lenient()?;
                        break 'find;
                    }
                    c if c == b'{' as u16
                        || c == b'}' as u16
                        || c == b'[' as u16
                        || c == b']' as u16
                        || c == b':' as u16
                        || c == b',' as u16
                        || c == b' ' as u16
                        || c == b'\t' as u16
                        || c == b'\x0c' as u16
                        || c == b'\r' as u16
                        || c == b'\n' as u16 =>
                    {
                        break 'find;
                    }
                    _ => i += 1,
                }
            }
            if i < self.buffer.len() {
                if !self.fill_buffer(i + 1) {
                    break;
                }
            } else {
                if builder.is_none() {
                    builder = Some(Vec::with_capacity(i.max(16)));
                }
                builder
                    .as_mut()
                    .unwrap()
                    .extend_from_slice(&self.buffer[self.pos..self.pos + i]);
                self.pos += i;
                i = 0;
                if !self.fill_buffer(1) {
                    break;
                }
            }
        }
        let result = if let Some(mut builder) = builder {
            builder.extend_from_slice(&self.buffer[self.pos..self.pos + i]);
            JsString::from_units(builder)
        } else {
            JsString::from_units(self.buffer[self.pos..self.pos + i].to_vec())
        };
        self.pos += i;
        Ok(result)
    }
    // port: com.google.gson.stream.JsonReader#push
    fn push(&mut self, new_top: i32) {
        if self.stack_size == self.stack.len() {
            let new_length = self.stack_size * 2;
            self.stack.resize(new_length, 0);
            self.path_indices.resize(new_length, 0);
            self.path_names.resize(new_length, None);
        }
        self.stack[self.stack_size] = new_top;
        self.stack_size += 1;
    }
    // port: com.google.gson.stream.JsonReader#fillBuffer
    fn fill_buffer(&mut self, mut minimum: usize) -> bool {
        self.line_start -= self.pos as i32;
        if self.limit != self.pos {
            self.limit -= self.pos;
            self.buffer.copy_within(self.pos..self.pos + self.limit, 0);
        } else {
            self.limit = 0;
        }
        self.pos = 0;
        while self.input_position < self.input.length() {
            let total =
                (self.buffer.len() - self.limit).min(self.input.length() - self.input_position);
            self.buffer[self.limit..self.limit + total].copy_from_slice(
                &self.input.as_units()[self.input_position..self.input_position + total],
            );
            self.input_position += total;
            self.limit += total;
            if self.line_number == 0
                && self.line_start == 0
                && self.limit > 0
                && self.buffer[0] == 0xfeff
            {
                self.pos += 1;
                self.line_start += 1;
                minimum += 1;
            }
            if self.limit >= minimum {
                return true;
            }
        }
        false
    }
    // port: com.google.gson.stream.JsonReader#nextNonWhitespace
    fn next_non_whitespace(&mut self, throw_on_eof: bool) -> Result<i32, JsonParseException> {
        let mut p = self.pos;
        let mut l = self.limit;
        loop {
            if p == l {
                self.pos = p;
                if !self.fill_buffer(1) {
                    break;
                }
                p = self.pos;
                l = self.limit;
            }
            let c = self.buffer[p];
            p += 1;
            if c == b'\n' as u16 {
                self.line_number += 1;
                self.line_start = p as i32;
                continue;
            } else if c == b' ' as u16 || c == b'\r' as u16 || c == b'\t' as u16 {
                continue;
            }
            if c == b'/' as u16 {
                self.pos = p;
                if p == l {
                    self.pos -= 1;
                    let chars_loaded = self.fill_buffer(2);
                    self.pos += 1;
                    if !chars_loaded {
                        return Ok(c as i32);
                    }
                }
                self.check_lenient()?;
                let peek = self.buffer[self.pos];
                match peek {
                    c if c == b'*' as u16 => {
                        self.pos += 1;
                        if !self.skip_to(&"*/".into()) {
                            return Err(self.syntax_error("Unterminated comment"));
                        }
                        p = self.pos + 2;
                        l = self.limit;
                        continue;
                    }
                    c if c == b'/' as u16 => {
                        self.pos += 1;
                        self.skip_to_end_of_line();
                        p = self.pos;
                        l = self.limit;
                        continue;
                    }
                    _ => return Ok(c as i32),
                }
            } else if c == b'#' as u16 {
                self.pos = p;
                self.check_lenient()?;
                self.skip_to_end_of_line();
                p = self.pos;
                l = self.limit;
            } else {
                self.pos = p;
                return Ok(c as i32);
            }
        }
        if throw_on_eof {
            return Err(JsonParseException::new(
                "java.io.EOFException",
                JsString::from("End of input").concat(&self.location_string()),
            ));
        }
        Ok(-1)
    }
    // port: com.google.gson.stream.JsonReader#checkLenient
    fn check_lenient(&self) -> Result<(), JsonParseException> {
        if !self.lenient {
            return Err(
                self.syntax_error("Use JsonReader.setLenient(true) to accept malformed JSON")
            );
        }
        Ok(())
    }
    // port: com.google.gson.stream.JsonReader#skipToEndOfLine
    fn skip_to_end_of_line(&mut self) {
        while self.pos < self.limit || self.fill_buffer(1) {
            let c = self.buffer[self.pos];
            self.pos += 1;
            if c == b'\n' as u16 {
                self.line_number += 1;
                self.line_start = self.pos as i32;
                break;
            } else if c == b'\r' as u16 {
                break;
            }
        }
    }
    // port: com.google.gson.stream.JsonReader#skipTo
    fn skip_to(&mut self, to_find: &JsString) -> bool {
        let length = to_find.length();
        'outer: while self.pos + length <= self.limit || self.fill_buffer(length) {
            if self.buffer[self.pos] == b'\n' as u16 {
                self.line_number += 1;
                self.line_start = self.pos as i32 + 1;
                self.pos += 1;
                continue;
            }
            for c in 0..length {
                if self.buffer[self.pos + c] != to_find.char_at(c) {
                    self.pos += 1;
                    continue 'outer;
                }
            }
            return true;
        }
        false
    }
    // port: com.google.gson.stream.JsonReader#locationString
    pub fn location_string(&self) -> JsString {
        let line = self.line_number + 1;
        let column = self.pos as i32 - self.line_start + 1;
        JsString::from(format!(" at line {line} column {column} path ")).concat(&self.get_path())
    }
    // port: com.google.gson.stream.JsonReader#getPath(boolean)
    fn get_path_with_previous(&self, use_previous_path: bool) -> JsString {
        let mut result = vec![b'$' as u16];
        for i in 0..self.stack_size {
            match self.stack[i] {
                EMPTY_ARRAY | NONEMPTY_ARRAY => {
                    let mut path_index = self.path_indices[i];
                    if use_previous_path && path_index > 0 && i == self.stack_size - 1 {
                        path_index -= 1;
                    }
                    result.push(b'[' as u16);
                    result.extend_from_slice(JsString::from(path_index.to_string()).as_units());
                    result.push(b']' as u16);
                }
                EMPTY_OBJECT | DANGLING_NAME | NONEMPTY_OBJECT => {
                    result.push(b'.' as u16);
                    if let Some(name) = &self.path_names[i] {
                        result.extend_from_slice(name.as_units());
                    }
                }
                NONEMPTY_DOCUMENT | EMPTY_DOCUMENT | CLOSED => {}
                _ => {}
            }
        }
        JsString::from_units(result)
    }
    // port: com.google.gson.stream.JsonReader#getPath()
    pub fn get_path(&self) -> JsString {
        self.get_path_with_previous(false)
    }
    // port: com.google.gson.stream.JsonReader#getPreviousPath
    pub fn get_previous_path(&self) -> JsString {
        self.get_path_with_previous(true)
    }
    // port: com.google.gson.stream.JsonReader#readEscapeCharacter
    fn read_escape_character(&mut self) -> Result<u16, JsonParseException> {
        if self.pos == self.limit && !self.fill_buffer(1) {
            return Err(self.syntax_error("Unterminated escape sequence"));
        }
        let escaped = self.buffer[self.pos];
        self.pos += 1;
        match escaped {
            c if c == b'u' as u16 => {
                if self.pos + 4 > self.limit && !self.fill_buffer(4) {
                    return Err(self.syntax_error("Unterminated escape sequence"));
                }
                let mut result = 0u16;
                for i in self.pos..self.pos + 4 {
                    let c = self.buffer[i];
                    result <<= 4;
                    if c >= b'0' as u16 && c <= b'9' as u16 {
                        result += c - b'0' as u16;
                    } else if c >= b'a' as u16 && c <= b'f' as u16 {
                        result += c - b'a' as u16 + 10;
                    } else if c >= b'A' as u16 && c <= b'F' as u16 {
                        result += c - b'A' as u16 + 10;
                    } else {
                        crate::JavaException::throw(
                            "java.lang.NumberFormatException: \\u",
                            &JsString::from_units(self.buffer[self.pos..self.pos + 4].to_vec()),
                            "",
                        );
                    }
                }
                self.pos += 4;
                Ok(result)
            }
            c if c == b't' as u16 => Ok(b'\t' as u16),
            c if c == b'b' as u16 => Ok(b'\x08' as u16),
            c if c == b'n' as u16 => Ok(b'\n' as u16),
            c if c == b'r' as u16 => Ok(b'\r' as u16),
            c if c == b'f' as u16 => Ok(b'\x0c' as u16),
            c if c == b'\n' as u16 => {
                self.line_number += 1;
                self.line_start = self.pos as i32;
                Ok(escaped)
            }
            c if c == b'\'' as u16 || c == b'"' as u16 || c == b'\\' as u16 || c == b'/' as u16 => {
                Ok(escaped)
            }
            _ => Err(self.syntax_error("Invalid escape sequence")),
        }
    }
    // port: com.google.gson.stream.JsonReader#syntaxError
    fn syntax_error(&self, message: impl Into<JsString>) -> JsonParseException {
        JsonParseException::new(
            "com.google.gson.stream.MalformedJsonException",
            message.into().concat(&self.location_string()),
        )
    }
    // port: com.google.gson.stream.JsonReader#consumeNonExecutePrefix
    fn consume_non_execute_prefix(&mut self) -> Result<(), JsonParseException> {
        self.next_non_whitespace(true)?;
        self.pos -= 1;
        let length = 5;
        if self.pos + length > self.limit && !self.fill_buffer(length) {
            return Ok(());
        }
        let p = self.pos;
        if self.buffer[p] != b')' as u16
            || self.buffer[p + 1] != b']' as u16
            || self.buffer[p + 2] != b'}' as u16
            || self.buffer[p + 3] != b'\'' as u16
            || self.buffer[p + 4] != b'\n' as u16
        {
            return Ok(());
        }
        self.pos += length;
        Ok(())
    }
    // Rust entry point for the Gson adapter.
    pub fn read_element(&mut self) -> Result<JsonElement, JsonParseException> {
        JsonElementTypeAdapter::read(self)
    }
}
impl std::fmt::Display for JsonReader {
    // port: com.google.gson.stream.JsonReader#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsonReader{}", self.location_string())
    }
}
struct JsonElementTypeAdapter;
impl JsonElementTypeAdapter {
    // port: com.google.gson.internal.bind.TypeAdapters.JSON_ELEMENT#tryBeginNesting
    fn try_begin_nesting(
        reader: &mut JsonReader,
        token: JsonToken,
    ) -> Result<Option<JsonElement>, JsonParseException> {
        match token {
            JsonToken::BEGIN_ARRAY => {
                reader.begin_array()?;
                Ok(Some(JsonElement::Array(JsonArray::default())))
            }
            JsonToken::BEGIN_OBJECT => {
                reader.begin_object()?;
                Ok(Some(JsonElement::Object(JsonObject::default())))
            }
            _ => Ok(None),
        }
    }
    // port: com.google.gson.internal.bind.TypeAdapters.JSON_ELEMENT#readTerminal
    fn read_terminal(
        reader: &mut JsonReader,
        token: JsonToken,
    ) -> Result<JsonElement, JsonParseException> {
        match token {
            JsonToken::STRING => Ok(JsonElement::Primitive(JsonPrimitive::new_string(
                reader.next_string()?,
            ))),
            JsonToken::NUMBER => {
                let number = reader.next_string()?;
                Ok(JsonElement::Primitive(JsonPrimitive::new_number(
                    LazilyParsedNumber::new(number),
                )))
            }
            JsonToken::BOOLEAN => Ok(JsonElement::Primitive(JsonPrimitive::new_boolean(
                reader.next_boolean()?,
            ))),
            JsonToken::NULL => {
                reader.next_null()?;
                Ok(JsonElement::Null)
            }
            _ => Err(JsonParseException::new(
                "java.lang.IllegalStateException",
                format!("Unexpected token: {token:?}"),
            )),
        }
    }
    // port: com.google.gson.internal.bind.TypeAdapters.JSON_ELEMENT#read
    fn read(reader: &mut JsonReader) -> Result<JsonElement, JsonParseException> {
        let mut token = reader.peek()?;
        let current = Self::try_begin_nesting(reader, token)?;
        if current.is_none() {
            return Self::read_terminal(reader, token);
        }
        let mut current = current.unwrap();
        // Java attaches each mutable child immediately. Rust holds the child outside
        // its owned parent until closing it, preserving the same read/insert order.
        let mut stack: Vec<(JsonElement, Option<JsString>)> = Vec::new();
        loop {
            while reader.has_next()? {
                let mut name = None;
                if matches!(current, JsonElement::Object(_)) {
                    name = Some(reader.next_name()?);
                }
                token = reader.peek()?;
                let value = Self::try_begin_nesting(reader, token)?;
                let is_nesting = value.is_some();
                let value = if is_nesting {
                    value.unwrap()
                } else {
                    Self::read_terminal(reader, token)?
                };
                if is_nesting {
                    stack.push((current, name));
                    current = value;
                } else {
                    match &mut current {
                        JsonElement::Array(array) => array.add(value),
                        JsonElement::Object(object) => object.add(name.unwrap(), value),
                        _ => unreachable!(),
                    }
                }
            }
            if matches!(current, JsonElement::Array(_)) {
                reader.end_array()?;
            } else {
                reader.end_object()?;
            }
            if stack.is_empty() {
                return Ok(current);
            }
            let (mut parent, name) = stack.pop().unwrap();
            match &mut parent {
                JsonElement::Array(array) => array.add(current),
                JsonElement::Object(object) => object.add(name.unwrap(), current),
                _ => unreachable!(),
            }
            current = parent;
        }
    }
}
