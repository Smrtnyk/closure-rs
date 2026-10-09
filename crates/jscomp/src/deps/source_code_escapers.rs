/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/SourceCodeEscapers.java,
//   test/com/google/javascript/jscomp/deps/SourceCodeEscapersTest.java.

use closure_rhino::js_string::JsString;
use std::{fmt, sync::LazyLock};
pub struct SourceCodeEscapers;
const PRINTABLE_ASCII_MIN: u16 = 0x20;
const PRINTABLE_ASCII_MAX: u16 = 0x7e;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
static JAVASCRIPT_ESCAPER: JavaScriptEscaper = JavaScriptEscaper;
// port: SourceCodeEscapers.JavaScriptEscaper#REPLACEMENT_CHARS
static REPLACEMENT_CHARS: LazyLock<[Option<String>; 256]> = LazyLock::new(|| {
    let mut replacements = std::array::from_fn(|_| None);
    for i in (0..PRINTABLE_ASCII_MIN).chain(PRINTABLE_ASCII_MAX + 1..0x100) {
        let mut c = i;
        let mut r = [0u8; 4];
        r[3] = HEX_DIGITS[(c & 0xf) as usize];
        c >>= 4;
        r[2] = HEX_DIGITS[(c & 0xf) as usize];
        r[1] = b'x';
        r[0] = b'\\';
        replacements[i as usize] = Some(String::from_utf8(r.to_vec()).unwrap());
    }
    for (c, r) in [
        (b'\'', "\\x27"),
        (b'"', "\\x22"),
        (b'<', "\\x3c"),
        (b'=', "\\x3d"),
        (b'>', "\\x3e"),
        (b'&', "\\x26"),
        (8, "\\b"),
        (9, "\\t"),
        (10, "\\n"),
        (12, "\\f"),
        (13, "\\r"),
        (b'\\', "\\\\"),
    ] {
        replacements[c as usize] = Some(r.into());
    }
    replacements
});
impl SourceCodeEscapers {
    // port: SourceCodeEscapers#javascriptEscaper
    pub fn javascript_escaper() -> &'static JavaScriptEscaper {
        &JAVASCRIPT_ESCAPER
    }
    // port: SourceCodeEscapers#appendWithJavascriptEscaper
    pub fn append_with_javascript_escaper(c: &JsString, to: &mut dyn fmt::Write) -> fmt::Result {
        JAVASCRIPT_ESCAPER.append_to(c, to)
    }
}
pub struct JavaScriptEscaper;
impl JavaScriptEscaper {
    // port: SourceCodeEscapers.JavaScriptEscaper#appendTo
    pub fn append_to(&self, cs: &JsString, to: &mut dyn fmt::Write) -> fmt::Result {
        let mut last = 0;
        let length = cs.length();
        for i in 0..length {
            let c = cs.char_at(i);
            let replacement = if c < 0x100 {
                let Some(r) = &REPLACEMENT_CHARS[c as usize] else {
                    continue;
                };
                r.clone()
            } else {
                Self::as_unicode_hex_escape(c)
            };
            if last < i {
                to.write_str(&cs.substring(last, i).to_string_lossy())?;
            }
            for r in replacement.chars() {
                to.write_char(r)?;
            }
            last = i + 1;
        }
        if last < length {
            to.write_str(&cs.substring(last, length).to_string_lossy())?;
        }
        Ok(())
    }
    // port: SourceCodeEscapers.JavaScriptEscaper#asUnicodeHexEscape
    fn as_unicode_hex_escape(mut c: u16) -> String {
        let mut r = [0; 6];
        r[0] = b'\\';
        r[1] = b'u';
        r[5] = HEX_DIGITS[(c & 0xf) as usize];
        c >>= 4;
        r[4] = HEX_DIGITS[(c & 0xf) as usize];
        c >>= 4;
        r[3] = HEX_DIGITS[(c & 0xf) as usize];
        c >>= 4;
        r[2] = HEX_DIGITS[(c & 0xf) as usize];
        String::from_utf8(r.to_vec()).unwrap()
    }
    // port: SourceCodeEscapers.JavaScriptEscaper#escape
    pub fn escape(&self, string: impl Into<JsString>) -> String {
        let mut sb = String::new();
        self.append_to(&string.into(), &mut sb).expect(
            "This should never throw - StringBuilder.append doesn't actually throw IOException",
        );
        sb
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // port: SourceCodeEscapersTest#testGoogModuleCode
    #[test]
    fn test_goog_module_code() {
        let input = "goog.module('Foo');\nclass Foo {}";
        let escaped = SourceCodeEscapers::javascript_escaper().escape(input);
        assert_eq!(escaped, "goog.module(\\x27Foo\\x27);\\nclass Foo {}");
    }
    // port: SourceCodeEscapersTest#testExplicitCharacterEscaping
    #[test]
    fn test_explicit_character_escaping() {
        let input = JsString::from_units(vec![
            b'\'' as u16,
            b'"' as u16,
            b'<' as u16,
            b'=' as u16,
            b'>' as u16,
            b'&' as u16,
            8,
            9,
            10,
            12,
            13,
            b'\\' as u16,
        ]);
        let escaped = SourceCodeEscapers::javascript_escaper().escape(input);
        assert_eq!(escaped, "\\x27\\x22\\x3c\\x3d\\x3e\\x26\\b\\t\\n\\f\\r\\\\");
    }
    // port: SourceCodeEscapersTest#lowUnicodeValueEscaping
    #[test]
    fn low_unicode_value_escaping() {
        let input = JsString::from_units(vec![0x00, 0x10, 0x1f, 0x20, 0x7e, 0x7f, 0xc0, 0xff]);
        let escaped = SourceCodeEscapers::javascript_escaper().escape(input);
        assert_eq!(escaped, "\\x00\\x10\\x1f ~\\x7f\\xc0\\xff");
    }
    // port: SourceCodeEscapersTest#highUnicodeValueEscaping
    #[test]
    fn high_unicode_value_escaping() {
        let input = JsString::from_units(vec![0x100, 0xf00, 0xffff]);
        let escaped = SourceCodeEscapers::javascript_escaper().escape(input);
        assert_eq!(escaped, "\\u0100\\u0f00\\uffff");
    }
}
