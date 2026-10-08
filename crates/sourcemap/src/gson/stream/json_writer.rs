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
//   com/google/gson/internal/bind/TypeAdapters.java, com/google/gson/stream/JsonWriter.java.

use super::json_scope::*;
use crate::gson::{JsonElement, lazily_parsed_number::LazilyParsedNumber};
use closure_rhino::js_string::JsString;
use std::sync::LazyLock;

pub struct JsonWriter {
    out: Vec<u16>,
    stack: Vec<i32>,
    stack_size: usize,
    indent: Option<JsString>,
    separator: JsString,
    lenient: bool,
    html_safe: bool,
    deferred_name: Option<JsString>,
    serialize_nulls: bool,
}
static REPLACEMENT_CHARS: LazyLock<[Option<JsString>; 128]> =
    LazyLock::new(|| JsonWriter::init_replacement_chars().0);
static HTML_SAFE_REPLACEMENT_CHARS: LazyLock<[Option<JsString>; 128]> =
    LazyLock::new(|| JsonWriter::init_replacement_chars().1);
impl JsonWriter {
    // The only Number implementation in this subset is Gson's trusted LazilyParsedNumber.
    const VALID_JSON_NUMBER_PATTERN: &'static str =
        "-?(?:0|[1-9][0-9]*)(?:\\.[0-9]+)?(?:[eE][+-]?[0-9]+)?";
    // port: com.google.gson.stream.JsonWriter#<clinit>
    fn init_replacement_chars() -> ([Option<JsString>; 128], [Option<JsString>; 128]) {
        let mut replacement_chars: [Option<JsString>; 128] = std::array::from_fn(|_| None);
        for (i, slot) in replacement_chars.iter_mut().enumerate().take(0x20) {
            *slot = Some(format!("\\u{i:04x}").into());
        }
        replacement_chars[b'"' as usize] = Some("\\\"".into());
        replacement_chars[b'\\' as usize] = Some("\\\\".into());
        replacement_chars[b'\t' as usize] = Some("\\t".into());
        replacement_chars[b'\x08' as usize] = Some("\\b".into());
        replacement_chars[b'\n' as usize] = Some("\\n".into());
        replacement_chars[b'\r' as usize] = Some("\\r".into());
        replacement_chars[b'\x0c' as usize] = Some("\\f".into());
        let mut html_safe_replacement_chars = replacement_chars.clone();
        html_safe_replacement_chars[b'<' as usize] = Some("\\u003c".into());
        html_safe_replacement_chars[b'>' as usize] = Some("\\u003e".into());
        html_safe_replacement_chars[b'&' as usize] = Some("\\u0026".into());
        html_safe_replacement_chars[b'=' as usize] = Some("\\u003d".into());
        html_safe_replacement_chars[b'\'' as usize] = Some("\\u0027".into());
        (replacement_chars, html_safe_replacement_chars)
    }
    // port: com.google.gson.stream.JsonWriter#JsonWriter
    pub fn new(out: Vec<u16>) -> Self {
        let mut stack = vec![0; 32];
        stack[0] = EMPTY_DOCUMENT;
        Self {
            out,
            stack,
            stack_size: 1,
            indent: None,
            separator: ":".into(),
            lenient: false,
            html_safe: false,
            deferred_name: None,
            serialize_nulls: true,
        }
    }
    // port: com.google.gson.stream.JsonWriter#setIndent
    pub(crate) fn set_indent(&mut self, indent: impl Into<JsString>) {
        let indent = indent.into();
        if indent.is_empty() {
            self.indent = None;
            self.separator = ":".into();
        } else {
            self.indent = Some(indent);
            self.separator = ": ".into();
        }
    }

    // port: com.google.gson.stream.JsonWriter#setLenient
    pub fn set_lenient(&mut self, lenient: bool) {
        self.lenient = lenient;
    }
    // port: com.google.gson.stream.JsonWriter#isLenient
    pub fn is_lenient(&self) -> bool {
        self.lenient
    }
    // port: com.google.gson.stream.JsonWriter#setHtmlSafe
    pub fn set_html_safe(&mut self, html_safe: bool) {
        self.html_safe = html_safe;
    }
    // port: com.google.gson.stream.JsonWriter#isHtmlSafe
    pub fn is_html_safe(&self) -> bool {
        self.html_safe
    }
    // port: com.google.gson.stream.JsonWriter#setSerializeNulls
    pub fn set_serialize_nulls(&mut self, serialize_nulls: bool) {
        self.serialize_nulls = serialize_nulls;
    }
    // port: com.google.gson.stream.JsonWriter#getSerializeNulls
    pub fn get_serialize_nulls(&self) -> bool {
        self.serialize_nulls
    }
    // port: com.google.gson.stream.JsonWriter#beginArray
    pub fn begin_array(&mut self) -> &mut Self {
        self.write_deferred_name();
        self.open(EMPTY_ARRAY, b'[' as u16)
    }
    // port: com.google.gson.stream.JsonWriter#endArray
    pub fn end_array(&mut self) -> &mut Self {
        self.close_container(EMPTY_ARRAY, NONEMPTY_ARRAY, b']' as u16)
    }
    // port: com.google.gson.stream.JsonWriter#beginObject
    pub fn begin_object(&mut self) -> &mut Self {
        self.write_deferred_name();
        self.open(EMPTY_OBJECT, b'{' as u16)
    }
    // port: com.google.gson.stream.JsonWriter#endObject
    pub fn end_object(&mut self) -> &mut Self {
        self.close_container(EMPTY_OBJECT, NONEMPTY_OBJECT, b'}' as u16)
    }
    // port: com.google.gson.stream.JsonWriter#open
    fn open(&mut self, empty: i32, open_bracket: u16) -> &mut Self {
        self.before_value();
        self.push(empty);
        self.out.push(open_bracket);
        self
    }
    // port: com.google.gson.stream.JsonWriter#close(int,int,char)
    fn close_container(&mut self, empty: i32, nonempty: i32, close_bracket: u16) -> &mut Self {
        let context = self.peek();
        if context != nonempty && context != empty {
            panic!("java.lang.IllegalStateException: Nesting problem.");
        }
        if let Some(name) = &self.deferred_name {
            crate::JavaException::throw(
                "java.lang.IllegalStateException: Dangling name: ",
                name,
                "",
            );
        }
        self.stack_size -= 1;
        if context == nonempty {
            self.newline();
        }
        self.out.push(close_bracket);
        self
    }
    // port: com.google.gson.stream.JsonWriter#push
    fn push(&mut self, new_top: i32) {
        if self.stack_size == self.stack.len() {
            self.stack.resize(self.stack_size * 2, 0);
        }
        self.stack[self.stack_size] = new_top;
        self.stack_size += 1;
    }
    // port: com.google.gson.stream.JsonWriter#peek
    fn peek(&self) -> i32 {
        if self.stack_size == 0 {
            panic!("java.lang.IllegalStateException: JsonWriter is closed.");
        }
        self.stack[self.stack_size - 1]
    }
    // port: com.google.gson.stream.JsonWriter#replaceTop
    fn replace_top(&mut self, top_of_stack: i32) {
        self.stack[self.stack_size - 1] = top_of_stack;
    }
    // port: com.google.gson.stream.JsonWriter#name
    pub fn name(&mut self, name: impl Into<JsString>) -> &mut Self {
        if self.deferred_name.is_some() {
            panic!("java.lang.IllegalStateException");
        }
        if self.stack_size == 0 {
            panic!("java.lang.IllegalStateException: JsonWriter is closed.");
        }
        self.deferred_name = Some(name.into());
        self
    }
    // port: com.google.gson.stream.JsonWriter#writeDeferredName
    fn write_deferred_name(&mut self) {
        if self.deferred_name.is_some() {
            self.before_name();
            self.string(&self.deferred_name.as_ref().unwrap().clone());
            self.deferred_name = None;
        }
    }
    // port: com.google.gson.stream.JsonWriter#value(String)
    pub fn value(&mut self, value: Option<JsString>) -> &mut Self {
        if value.is_none() {
            return self.null_value();
        }
        self.write_deferred_name();
        self.before_value();
        self.string(&value.unwrap());
        self
    }
    // port: com.google.gson.stream.JsonWriter#nullValue
    pub fn null_value(&mut self) -> &mut Self {
        if self.deferred_name.is_some() {
            if self.serialize_nulls {
                self.write_deferred_name();
            } else {
                self.deferred_name = None;
                return self;
            }
        }
        self.before_value();
        self.out
            .extend_from_slice(JsString::from("null").as_units());
        self
    }
    // port: com.google.gson.stream.JsonWriter#value(boolean)
    pub fn value_boolean(&mut self, value: bool) -> &mut Self {
        self.write_deferred_name();
        self.before_value();
        self.out
            .extend_from_slice(JsString::from(if value { "true" } else { "false" }).as_units());
        self
    }
    // port: com.google.gson.stream.JsonWriter#jsonValue
    pub fn json_value(&mut self, value: Option<&JsString>) -> &mut Self {
        let Some(value) = value else {
            return self.null_value();
        };
        self.write_deferred_name();
        self.before_value();
        self.out.extend_from_slice(value.as_units());
        self
    }
    // port: com.google.gson.stream.JsonWriter#close()
    pub fn close(&mut self) -> std::io::Result<()> {
        let size = self.stack_size;
        if size > 1 || (size == 1 && self.stack[size - 1] != NONEMPTY_DOCUMENT) {
            return Err(std::io::Error::other("Incomplete document"));
        }
        self.stack_size = 0;
        Ok(())
    }
    // port: com.google.gson.stream.JsonWriter#value(long)
    pub fn value_long(&mut self, value: i64) -> &mut Self {
        self.write_deferred_name();
        self.before_value();
        self.out
            .extend_from_slice(JsString::from(value.to_string()).as_units());
        self
    }
    // port: com.google.gson.stream.JsonWriter#isTrustedNumberType
    fn is_trusted_number_type(_value: &LazilyParsedNumber) -> bool {
        true // LazilyParsedNumber is one of the Java method's explicitly trusted types.
    }
    // port: com.google.gson.stream.JsonWriter#value(Number)
    pub fn value_number(&mut self, value: Option<&LazilyParsedNumber>) -> &mut Self {
        if value.is_none() {
            return self.null_value();
        }
        let value = value.unwrap();
        self.write_deferred_name();
        let string = &value.value;
        if string == "-Infinity" || string == "Infinity" || string == "NaN" {
            if !self.lenient {
                crate::JavaException::throw(
                    "java.lang.IllegalArgumentException: Numeric values must be finite, but was ",
                    string,
                    "",
                );
            }
        } else if !Self::is_trusted_number_type(value) {
            // The Java && VALID_JSON_NUMBER_PATTERN.matcher(string).matches()
            // condition short-circuits for every Number type in this subset.
            let _ = Self::VALID_JSON_NUMBER_PATTERN;
            unreachable!("all supported Number implementations are trusted");
        }
        self.before_value();
        self.out.extend_from_slice(string.as_units());
        self
    }
    // port: com.google.gson.stream.JsonWriter#string
    fn string(&mut self, value: &JsString) {
        let replacements = if self.html_safe {
            &*HTML_SAFE_REPLACEMENT_CHARS
        } else {
            &*REPLACEMENT_CHARS
        };
        self.out.push(b'"' as u16);
        let mut last = 0;
        let length = value.length();
        for i in 0..length {
            let c = value.char_at(i);
            let replacement;
            if c < 128 {
                replacement = replacements[c as usize].clone();
                if replacement.is_none() {
                    continue;
                }
            } else if c == 0x2028 {
                replacement = Some("\\u2028".into());
            } else if c == 0x2029 {
                replacement = Some("\\u2029".into());
            } else {
                continue;
            }
            if last < i {
                self.out.extend_from_slice(&value.as_units()[last..i]);
            }
            self.out
                .extend_from_slice(replacement.as_ref().unwrap().as_units());
            last = i + 1;
        }
        if last < length {
            self.out.extend_from_slice(&value.as_units()[last..length]);
        }
        self.out.push(b'"' as u16);
    }
    // port: com.google.gson.stream.JsonWriter#newline
    fn newline(&mut self) {
        if self.indent.is_none() {
            return;
        }
        self.out.push(b'\n' as u16);
        for _i in 1..self.stack_size {
            self.out
                .extend_from_slice(self.indent.as_ref().unwrap().as_units());
        }
    }
    // port: com.google.gson.stream.JsonWriter#beforeName
    fn before_name(&mut self) {
        let context = self.peek();
        if context == NONEMPTY_OBJECT {
            self.out.push(b',' as u16);
        } else if context != EMPTY_OBJECT {
            panic!("java.lang.IllegalStateException: Nesting problem.");
        }
        self.newline();
        self.replace_top(DANGLING_NAME);
    }
    // port: com.google.gson.stream.JsonWriter#beforeValue
    fn before_value(&mut self) {
        match self.peek() {
            NONEMPTY_DOCUMENT => {
                if !self.lenient {
                    panic!(
                        "java.lang.IllegalStateException: JSON must have only one top-level value."
                    );
                }
                self.replace_top(NONEMPTY_DOCUMENT);
            }
            EMPTY_DOCUMENT => self.replace_top(NONEMPTY_DOCUMENT),
            EMPTY_ARRAY => {
                self.replace_top(NONEMPTY_ARRAY);
                self.newline();
            }
            NONEMPTY_ARRAY => {
                self.out.push(b',' as u16);
                self.newline();
            }
            DANGLING_NAME => {
                self.out.extend_from_slice(self.separator.as_units());
                self.replace_top(NONEMPTY_OBJECT);
            }
            _ => panic!("java.lang.IllegalStateException: Nesting problem."),
        }
    }
    // Owned lossless output boundary for Rust callers.
    pub fn into_string(self) -> JsString {
        JsString::from_units(self.out)
    }
    // Rust adapter entry point; the Java body belongs to TypeAdapters.JSON_ELEMENT#write.
    pub fn write(&mut self, value: &JsonElement) {
        JsonElementTypeAdapter::write(self, value);
    }
}
struct JsonElementTypeAdapter;
impl JsonElementTypeAdapter {
    // port: com.google.gson.internal.bind.TypeAdapters.JSON_ELEMENT#write
    fn write(out: &mut JsonWriter, value: &JsonElement) {
        if value.is_json_null() {
            out.null_value();
        } else if value.is_json_primitive() {
            let primitive = value.get_as_json_primitive();
            if primitive.is_number() {
                out.value_number(Some(&primitive.get_as_number()));
            } else if primitive.is_boolean() {
                out.value_boolean(primitive.get_as_boolean());
            } else {
                out.value(Some(primitive.get_as_string()));
            }
        } else if let JsonElement::Array(array) = value {
            out.begin_array();
            for element in &array.elements {
                Self::write(out, element);
            }
            out.end_array();
        } else if let JsonElement::Object(object) = value {
            out.begin_object();
            for (key, element) in object.entry_set() {
                out.name(key);
                Self::write(out, element);
            }
            out.end_object();
        }
    }
}
