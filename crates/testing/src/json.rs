/*
 * Copyright 2026 The closure-rs Authors.
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

//! A strict RFC 8259 JSON reader and writer whose strings are WTF-16.
//!
//! FORMAT.md "Strings": record strings are UTF-16 strings; a lone surrogate is written as a
//! `\uXXXX` escape, which UTF-8 cannot hold. `serde_json` rejects lone surrogates, so the corpus is
//! read with this reader, which decodes every JSON string into a [`JsString`] (UTF-16 code
//! units) and keeps lone surrogates. Object keys are field names and are kept as `String`; a key
//! holding a lone surrogate is an error. Object key order is preserved ([`IndexMap`]); equality of
//! objects is map equality (key order ignored), as JSON object semantics and Gson's
//! `JsonObject.equals` define it.

use closure_rhino::fx_hash::IndexMap;
use std::fmt;

/// A JS (UTF-16) string: a sequence of UTF-16 code units, lone surrogates allowed (WTF-16).
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JsString(pub Vec<u16>);

impl JsString {
    /// The UTF-16 encoding of a Rust string.
    pub fn from_rust_str(s: &str) -> JsString {
        JsString(s.encode_utf16().collect())
    }

    pub fn as_units(&self) -> &[u16] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The string as UTF-8, or `None` when it holds a lone surrogate.
    pub fn to_string_strict(&self) -> Option<String> {
        String::from_utf16(&self.0).ok()
    }

    /// The string as UTF-8 with each lone surrogate replaced by U+FFFD.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf16_lossy(&self.0)
    }

    /// True when the string holds at least one unpaired surrogate code unit.
    pub fn has_lone_surrogate(&self) -> bool {
        char::decode_utf16(self.0.iter().copied()).any(|r| r.is_err())
    }

    /// True when the string equals the given Rust string.
    pub fn eq_str(&self, s: &str) -> bool {
        self.0.iter().copied().eq(s.encode_utf16())
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        write_js_string(&mut out, &self.0);
        f.write_str(&out)
    }
}

impl From<&str> for JsString {
    fn from(s: &str) -> JsString {
        JsString::from_rust_str(s)
    }
}

/// A JSON number, kept as its literal text so nothing is lost (`long`/`double` values in the
/// corpus are tagged strings, but plain JSON numbers are kept exact too).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JsonNumber(pub String);

impl JsonNumber {
    pub fn from_i64(v: i64) -> JsonNumber {
        JsonNumber(v.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the literal has no fraction and no exponent.
    pub fn is_integral_literal(&self) -> bool {
        !self.0.contains(['.', 'e', 'E'])
    }

    pub fn as_i64(&self) -> Option<i64> {
        if self.is_integral_literal() {
            self.0.parse().ok()
        } else {
            None
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        self.as_i64().and_then(|v| i32::try_from(v).ok())
    }

    pub fn as_f64(&self) -> Option<f64> {
        self.0.parse().ok()
    }
}

/// A parsed JSON value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(JsString),
    Array(Vec<JsonValue>),
    Object(IndexMap<String, JsonValue>),
}

impl JsonValue {
    pub fn str(s: &str) -> JsonValue {
        JsonValue::String(JsString::from_rust_str(s))
    }

    pub fn int(v: i64) -> JsonValue {
        JsonValue::Number(JsonNumber::from_i64(v))
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }

    pub fn as_object(&self) -> Option<&IndexMap<String, JsonValue>> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_js_string(&self) -> Option<&JsString> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<&JsonNumber> {
        match self {
            JsonValue::Number(n) => Some(n),
            _ => None,
        }
    }

    /// Member `key` of an object value.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object().and_then(|o| o.get(key))
    }

    /// The JSON type name, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            JsonValue::Null => "null",
            JsonValue::Bool(_) => "bool",
            JsonValue::Number(_) => "number",
            JsonValue::String(_) => "string",
            JsonValue::Array(_) => "array",
            JsonValue::Object(_) => "object",
        }
    }

    /// Compact JSON text (see [`write_json`]).
    pub fn to_json_string(&self) -> String {
        let mut out = String::new();
        write_json(&mut out, self);
        out
    }

    /// Value equality as Gson's `JsonElement.equals` defines it for parsed JSON (used by DSL.md
    /// case selection). Gson 2.9.1 (the version in the reference jar) parses every number as a
    /// `LazilyParsedNumber`, which `JsonPrimitive#isIntegral` does not count as integral, so two
    /// numbers compare as doubles (`Double.parseDouble`); strings by code units; arrays in order;
    /// objects as maps.
    pub fn gson_equals(&self, other: &JsonValue) -> bool {
        match (self, other) {
            (JsonValue::Number(a), JsonValue::Number(b)) => match (a.as_f64(), b.as_f64()) {
                (Some(x), Some(y)) => x == y,
                _ => a.0 == b.0,
            },
            (JsonValue::Array(a), JsonValue::Array(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.gson_equals(y))
            }
            (JsonValue::Object(a), JsonValue::Object(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.get(k).is_some_and(|w| v.gson_equals(w)))
            }
            _ => self == other,
        }
    }
}

/// A JSON syntax error at a byte offset of the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonSyntaxError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for JsonSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON syntax error at byte {}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for JsonSyntaxError {}

/// Parses one JSON text (surrounding whitespace allowed, nothing else).
pub fn parse_json(text: &str) -> Result<JsonValue, JsonSyntaxError> {
    let mut p = Parser {
        src: text,
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    let v = p.parse_value(0)?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(p.err("trailing characters after the JSON value"));
    }
    Ok(v)
}

/// Nesting limit (the deepest corpus value is far below it).
const MAX_DEPTH: usize = 512;

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn err(&self, message: &str) -> JsonSyntaxError {
        JsonSyntaxError {
            offset: self.pos,
            message: message.to_string(),
        }
    }

    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect_literal(&mut self, lit: &str, v: JsonValue) -> Result<JsonValue, JsonSyntaxError> {
        if self.bytes[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonValue, JsonSyntaxError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        match self.bytes.get(self.pos) {
            None => Err(self.err("unexpected end of input")),
            Some(b'n') => self.expect_literal("null", JsonValue::Null),
            Some(b't') => self.expect_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.expect_literal("false", JsonValue::Bool(false)),
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.bytes.get(self.pos) == Some(&b']') {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                loop {
                    self.skip_ws();
                    items.push(self.parse_value(depth + 1)?);
                    self.skip_ws();
                    match self.bytes.get(self.pos) {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(JsonValue::Array(items));
                        }
                        _ => return Err(self.err("expected ',' or ']' in array")),
                    }
                }
            }
            Some(b'{') => {
                self.pos += 1;
                let mut members = IndexMap::<_, _>::default();
                self.skip_ws();
                if self.bytes.get(self.pos) == Some(&b'}') {
                    self.pos += 1;
                    return Ok(JsonValue::Object(members));
                }
                loop {
                    self.skip_ws();
                    if self.bytes.get(self.pos) != Some(&b'"') {
                        return Err(self.err("expected a string key in object"));
                    }
                    let key_start = self.pos;
                    let key = self.parse_string()?;
                    let key = match key.to_string_strict() {
                        Some(k) => k,
                        None => {
                            self.pos = key_start;
                            return Err(self.err("object key holds a lone surrogate"));
                        }
                    };
                    self.skip_ws();
                    if self.bytes.get(self.pos) != Some(&b':') {
                        return Err(self.err("expected ':' after object key"));
                    }
                    self.pos += 1;
                    self.skip_ws();
                    let v = self.parse_value(depth + 1)?;
                    if members.contains_key(&key) {
                        self.pos = key_start;
                        return Err(self.err(&format!("duplicate object key {key:?}")));
                    }
                    members.insert(key, v);
                    self.skip_ws();
                    match self.bytes.get(self.pos) {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(JsonValue::Object(members));
                        }
                        _ => return Err(self.err("expected ',' or '}' in object")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonSyntaxError> {
        let start = self.pos;
        let digits = |p: &mut Parser<'_>| {
            let s = p.pos;
            while matches!(p.bytes.get(p.pos), Some(b'0'..=b'9')) {
                p.pos += 1;
            }
            p.pos - s
        };
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        match self.bytes.get(self.pos) {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                digits(self);
            }
            _ => return Err(self.err("invalid number")),
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            if digits(self) == 0 {
                return Err(self.err("invalid number fraction"));
            }
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if digits(self) == 0 {
                return Err(self.err("invalid number exponent"));
            }
        }
        Ok(JsonValue::Number(JsonNumber(
            self.src[start..self.pos].to_string(),
        )))
    }

    fn hex4(&mut self) -> Result<u16, JsonSyntaxError> {
        let h = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| self.err("truncated \\u escape"))?;
        let s = std::str::from_utf8(h).map_err(|_| self.err("invalid \\u escape"))?;
        if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(self.err("invalid \\u escape"));
        }
        let v = u16::from_str_radix(s, 16).map_err(|_| self.err("invalid \\u escape"))?;
        self.pos += 4;
        Ok(v)
    }

    fn parse_string(&mut self) -> Result<JsString, JsonSyntaxError> {
        debug_assert_eq!(self.bytes[self.pos], b'"');
        self.pos += 1;
        let mut units: Vec<u16> = Vec::new();
        loop {
            // Copy the run of plain characters in one step.
            let run_start = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            if self.pos > run_start {
                let run = &self.src[run_start..self.pos];
                if run.is_ascii() {
                    units.extend(run.bytes().map(u16::from));
                } else {
                    units.extend(run.encode_utf16());
                }
            }
            match self.bytes.get(self.pos) {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(JsString(units));
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let c = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| self.err("unterminated escape"))?;
                    self.pos += 1;
                    match c {
                        b'"' => units.push(u16::from(b'"')),
                        b'\\' => units.push(u16::from(b'\\')),
                        b'/' => units.push(u16::from(b'/')),
                        b'b' => units.push(0x08),
                        b'f' => units.push(0x0c),
                        b'n' => units.push(u16::from(b'\n')),
                        b'r' => units.push(u16::from(b'\r')),
                        b't' => units.push(u16::from(b'\t')),
                        b'u' => {
                            let u = self.hex4()?;
                            units.push(u);
                        }
                        _ => {
                            self.pos -= 1;
                            return Err(self.err("invalid escape"));
                        }
                    }
                }
                Some(_) => return Err(self.err("control character in string")),
            }
        }
    }
}

/// Appends the compact JSON text of `v`: no whitespace, members in stored order, strings as
/// [`write_js_string`] writes them, numbers as their literal text.
pub fn write_json(out: &mut String, v: &JsonValue) {
    match v {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(n) => out.push_str(&n.0),
        JsonValue::String(s) => write_js_string(out, &s.0),
        JsonValue::Array(items) => {
            out.push('[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(out, x);
            }
            out.push(']');
        }
        JsonValue::Object(members) => {
            out.push('{');
            for (i, (k, x)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_js_string(out, &JsString::from_rust_str(k).0);
                out.push(':');
                write_json(out, x);
            }
            out.push('}');
        }
    }
}

/// Appends a quoted JSON string for UTF-16 `units`: `"` and `\` escaped, control characters as
/// `\b \f \n \r \t` or `\u00xx`, paired surrogates as their UTF-8 character, and each lone
/// surrogate as a lowercase `\uxxxx` escape (FORMAT.md "Strings").
pub fn write_js_string(out: &mut String, units: &[u16]) {
    out.push('"');
    for r in char::decode_utf16(units.iter().copied()) {
        match r {
            Ok('"') => out.push_str("\\\""),
            Ok('\\') => out.push_str("\\\\"),
            Ok('\n') => out.push_str("\\n"),
            Ok('\r') => out.push_str("\\r"),
            Ok('\t') => out.push_str("\\t"),
            Ok('\u{8}') => out.push_str("\\b"),
            Ok('\u{c}') => out.push_str("\\f"),
            Ok(c) if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            Ok(c) => out.push(c),
            Err(e) => out.push_str(&format!("\\u{:04x}", e.unpaired_surrogate())),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_and_containers() {
        let v = parse_json(r#" {"a": [1, -2.5e3, true, false, null, "x"], "b": {}} "#).unwrap();
        let o = v.as_object().unwrap();
        assert_eq!(o.keys().collect::<Vec<_>>(), vec!["a", "b"]);
        let a = o["a"].as_array().unwrap();
        assert_eq!(a[0].as_number().unwrap().as_i64(), Some(1));
        assert_eq!(a[1].as_number().unwrap().as_str(), "-2.5e3");
        assert_eq!(a[1].as_number().unwrap().as_f64(), Some(-2500.0));
        assert_eq!(a[2], JsonValue::Bool(true));
        assert_eq!(a[3], JsonValue::Bool(false));
        assert!(a[4].is_null());
        assert_eq!(a[5], JsonValue::str("x"));
        assert_eq!(o["b"], JsonValue::Object(IndexMap::<_, _>::default()));
    }

    #[test]
    fn decodes_every_escape() {
        let v = parse_json(r#""\"\\\/\b\f\n\r\t\u0041\u00e9""#).unwrap();
        let s = v.as_js_string().unwrap();
        assert_eq!(
            s.as_units(),
            &[0x22, 0x5c, 0x2f, 0x08, 0x0c, 0x0a, 0x0d, 0x09, 0x41, 0xe9]
        );
    }

    #[test]
    fn keeps_lone_and_paired_surrogates() {
        // A lone high surrogate, a lone low surrogate, an escaped pair, and a literal pair.
        let text = "\"a\\ud800b\\udc00c\\ud83d\\ude00\u{1F600}\"";
        let v = parse_json(text).unwrap();
        let s = v.as_js_string().unwrap();
        assert_eq!(
            s.as_units(),
            &[
                0x61, 0xd800, 0x62, 0xdc00, 0x63, 0xd83d, 0xde00, 0xd83d, 0xde00
            ]
        );
        assert!(s.has_lone_surrogate());
        assert_eq!(s.to_string_strict(), None);
        // The writer re-emits lone surrogates as lowercase escapes and pairs as UTF-8.
        assert_eq!(
            v.to_json_string(),
            "\"a\\ud800b\\udc00c\u{1F600}\u{1F600}\""
        );
        let again = parse_json(&v.to_json_string()).unwrap();
        assert_eq!(again, v);
    }

    #[test]
    fn writer_escapes_control_characters() {
        let v = JsonValue::String(JsString(vec![0x01, 0x1f, 0x22, 0x5c, 0x0a, 0x7f]));
        assert_eq!(v.to_json_string(), "\"\\u0001\\u001f\\\"\\\\\\n\u{7f}\"");
        assert_eq!(parse_json(&v.to_json_string()).unwrap(), v);
    }

    #[test]
    fn round_trips_through_text() {
        let text = r#"{"k":[1,2.0,{"x":null,"y":"\u00e9\ud800"}],"z":-0}"#;
        let v = parse_json(text).unwrap();
        let w = parse_json(&v.to_json_string()).unwrap();
        assert_eq!(v, w);
        assert_eq!(
            v.to_json_string(),
            "{\"k\":[1,2.0,{\"x\":null,\"y\":\"\u{e9}\\ud800\"}],\"z\":-0}"
        );
    }

    #[test]
    fn rejects_malformed_input() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "01",
            "1.",
            "1e",
            "\"abc",
            "\"\\x\"",
            "\"\\u12\"",
            "tru",
            "{\"a\":1,\"a\":2}",
            "{\"\\ud800\":1}",
            "\"a\u{1}b\"",
            "1 2",
            "{a:1}",
        ] {
            assert!(parse_json(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn object_equality_ignores_key_order() {
        let a = parse_json(r#"{"a":1,"b":2}"#).unwrap();
        let b = parse_json(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn gson_number_equality() {
        let one = parse_json("1").unwrap();
        let one_f = parse_json("1.0").unwrap();
        assert_ne!(one, one_f);
        assert!(one.gson_equals(&one_f));
        assert!(!one.gson_equals(&parse_json("2").unwrap()));
        assert!(
            parse_json(r#"{"a":[1]}"#)
                .unwrap()
                .gson_equals(&parse_json(r#"{"a":[1.0]}"#).unwrap())
        );
        // Both round to the same double, so Gson's LazilyParsedNumber equality holds.
        assert!(
            parse_json("9007199254740993")
                .unwrap()
                .gson_equals(&parse_json("9007199254740992").unwrap())
        );
        assert!(!one.gson_equals(&JsonValue::str("1")));
    }
}
