/*
 * Copyright 2026 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapJsonLexer.java.

use crate::source_map_parse_exception::SourceMapParseException as Error;
use closure_rhino::js_string::JsString;
pub struct SourceMapJsonLexer {
    pub json: JsString,
    pub pos: usize,
    pub length: usize,
}
impl SourceMapJsonLexer {
    // port: SourceMapJsonLexer#SourceMapJsonLexer
    pub fn new(json: impl Into<JsString>) -> Self {
        let json = json.into();
        let length = json.length();
        Self {
            json,
            pos: 0,
            length,
        }
    }
    // port: SourceMapJsonLexer#skipWhitespace
    pub fn skip_whitespace(&mut self) {
        while self.pos < self.length {
            let c = self.json.char_at(self.pos);
            if c == b' ' as u16 || c == b'\t' as u16 || c == b'\n' as u16 || c == b'\r' as u16 {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
    // port: SourceMapJsonLexer#hasNext
    pub fn has_next(&mut self) -> bool {
        self.skip_whitespace();
        if self.pos >= self.length {
            return false;
        }
        let c = self.json.char_at(self.pos);
        c != b']' as u16 && c != b'}' as u16
    }
    // port: SourceMapJsonLexer#beginObject
    pub fn begin_object(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b'{' as u16 {
            self.pos += 1;
        } else {
            return Err(Error::new("Expected '{'"));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#endObject
    pub fn end_object(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b'}' as u16 {
            self.pos += 1;
        } else {
            return Err(Error::new("Expected '}'"));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#beginArray
    pub fn begin_array(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b'[' as u16 {
            self.pos += 1;
        } else {
            return Err(Error::new("Expected '['"));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#endArray
    pub fn end_array(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b']' as u16 {
            self.pos += 1;
        } else {
            return Err(Error::new("Expected ']'"));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#nextName
    pub fn next_name(&mut self) -> Result<JsString, Error> {
        let name = self.next_string()?;
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b':' as u16 {
            self.pos += 1;
        } else {
            return Err(Error::new("Expected ':'"));
        }
        Ok(name)
    }
    // port: SourceMapJsonLexer#nextStringOrNull
    pub fn next_string_or_null(&mut self) -> Result<Option<JsString>, Error> {
        self.skip_whitespace();
        if self.pos < self.length
            && crate::java_string::starts_with(&self.json, &"null".into(), self.pos as i32)
        {
            self.pos += 4;
            return Ok(None);
        }
        Ok(Some(self.next_string()?))
    }
    // Preserve the Java statement order.
    #[allow(clippy::needless_late_init)]
    // port: SourceMapJsonLexer#nextString
    pub fn next_string(&mut self) -> Result<JsString, Error> {
        self.skip_whitespace();
        if self.pos >= self.length || self.json.char_at(self.pos) != b'"' as u16 {
            return Err(Error::new("Expected string"));
        }
        self.pos += 1;
        let start = self.pos;
        let mut has_escape = false;
        while self.pos < self.length {
            let c = self.json.char_at(self.pos);
            if c == b'"' as u16 {
                break;
            }
            if c == b'\\' as u16 {
                has_escape = true;
                self.pos += 1;
            }
            self.pos += 1;
        }
        if self.pos >= self.length {
            return Err(Error::new("Unterminated string"));
        }
        let result;
        if !has_escape {
            result = self.json.substring(start, self.pos);
        } else {
            result = Self::unescape(&self.json, start, self.pos)?;
        }
        self.pos += 1;
        Ok(result)
    }
    // port: SourceMapJsonLexer#nextInt
    pub fn next_int(&mut self) -> Result<i32, Error> {
        self.skip_whitespace();
        let start = self.pos;
        if self.pos < self.length && self.json.char_at(self.pos) == b'-' as u16 {
            self.pos += 1;
        }
        while self.pos < self.length
            && self.json.char_at(self.pos) >= b'0' as u16
            && self.json.char_at(self.pos) <= b'9' as u16
        {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(Error::new("Expected integer"));
        }
        closure_rhino::java_lang::parse_int(&self.json.as_units()[start..self.pos], 10).map_err(
            |_| {
                Error::new(
                    JsString::from("Invalid integer: ")
                        .concat(&self.json.substring(start, self.pos)),
                )
            },
        )
    }
    // port: SourceMapJsonLexer#nextRawValue
    pub fn next_raw_value(&mut self) -> Result<JsString, Error> {
        self.skip_whitespace();
        let start = self.pos;
        self.skip_value()?;
        Ok(self.json.substring(start, self.pos))
    }
    // port: SourceMapJsonLexer#skipValue
    pub fn skip_value(&mut self) -> Result<(), Error> {
        self.skip_whitespace();
        if self.pos >= self.length {
            return Err(Error::new("Unexpected end of input"));
        }
        let c = self.json.char_at(self.pos);
        if c == b'{' as u16 {
            self.pos += 1;
            while self.has_next() {
                self.next_name()?;
                self.skip_value()?;
                self.check_comma()?;
            }
            self.end_object()?;
        } else if c == b'[' as u16 {
            self.pos += 1;
            while self.has_next() {
                self.skip_value()?;
                self.check_comma()?;
            }
            self.end_array()?;
        } else if c == b'"' as u16 {
            self.next_string()?;
        } else if c == b't' as u16
            && crate::java_string::starts_with(&self.json, &"true".into(), self.pos as i32)
        {
            self.pos += 4;
        } else if c == b'f' as u16
            && crate::java_string::starts_with(&self.json, &"false".into(), self.pos as i32)
        {
            self.pos += 5;
        } else if c == b'n' as u16
            && crate::java_string::starts_with(&self.json, &"null".into(), self.pos as i32)
        {
            self.pos += 4;
        } else if c == b'-' as u16 || (c >= b'0' as u16 && c <= b'9' as u16) {
            if c == b'-' as u16 {
                self.pos += 1;
            }
            while self.pos < self.length
                && ((self.json.char_at(self.pos) >= b'0' as u16
                    && self.json.char_at(self.pos) <= b'9' as u16)
                    || self.json.char_at(self.pos) == b'.' as u16
                    || self.json.char_at(self.pos) == b'e' as u16
                    || self.json.char_at(self.pos) == b'E' as u16
                    || self.json.char_at(self.pos) == b'+' as u16
                    || self.json.char_at(self.pos) == b'-' as u16)
            {
                self.pos += 1;
            }
        } else {
            return Err(Error::new(
                JsString::from("Unexpected character: ").concat(&JsString::from_units(vec![c])),
            ));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#checkComma
    pub fn check_comma(&mut self) -> Result<(), Error> {
        let has_comma = self.try_consume_comma();
        if has_comma && !self.has_next() {
            return Err(Error::new("Unexpected trailing comma"));
        }
        if !has_comma && self.has_next() {
            return Err(Error::new("Expected ','"));
        }
        Ok(())
    }
    // port: SourceMapJsonLexer#tryConsumeComma
    pub fn try_consume_comma(&mut self) -> bool {
        self.skip_whitespace();
        if self.pos < self.length && self.json.char_at(self.pos) == b',' as u16 {
            self.pos += 1;
            return true;
        }
        false
    }
    // port: SourceMapJsonLexer#unescape
    fn unescape(s: &JsString, start: usize, end: usize) -> Result<JsString, Error> {
        let mut sb = Vec::with_capacity(end - start);
        let mut i = start;
        while i < end {
            let c = s.char_at(i);
            if c == b'\\' as u16 && i + 1 < end {
                i += 1;
                let next = s.char_at(i);
                match next {
                    c if c == b'"' as u16 => sb.push(b'"' as u16),
                    c if c == b'\\' as u16 => sb.push(b'\\' as u16),
                    c if c == b'/' as u16 => sb.push(b'/' as u16),
                    c if c == b'b' as u16 => sb.push(b'\x08' as u16),
                    c if c == b'f' as u16 => sb.push(b'\x0c' as u16),
                    c if c == b'n' as u16 => sb.push(b'\n' as u16),
                    c if c == b'r' as u16 => sb.push(b'\r' as u16),
                    c if c == b't' as u16 => sb.push(b'\t' as u16),
                    c if c == b'u' as u16 => {
                        if i + 4 >= end {
                            return Err(Error::new("Invalid unicode escape sequence"));
                        }

                        let v =
                            closure_rhino::java_lang::parse_int(&s.as_units()[i + 1..i + 5], 16)
                                .map_err(|_| {
                                    Error::new(
                                        JsString::from("Invalid unicode escape: ")
                                            .concat(&s.substring(i + 1, i + 5)),
                                    )
                                })?;
                        sb.push(v as u16);
                        i += 4;
                    }
                    _ => sb.push(next),
                }
            } else {
                sb.push(c);
            }
            i += 1;
        }
        Ok(JsString::from_units(sb))
    }
}
