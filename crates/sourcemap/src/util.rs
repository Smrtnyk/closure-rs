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
//   src/com/google/debugging/sourcemap/Util.java.

use closure_rhino::js_string::JsString;
use std::fmt::{self, Write};

pub struct Util;
impl Util {
    const HEX_CHARS: &'static [u8; 16] = b"0123456789abcdef";

    // Preserve the Java statement order.
    #[allow(clippy::if_same_then_else)]
    // port: Util#escapeString
    pub fn escape_string(s: impl Into<JsString>) -> JsString {
        let s = s.into();
        let quote = '"';
        let doublequote_escape = "\\\"";
        let singlequote_escape = "'";
        let backslash_escape = "\\\\";
        let mut sb = String::with_capacity(s.length() + 2);
        sb.push(quote);
        let mut region = UnescapedRegion::new();
        let length = s.length();
        for i in 0..length {
            let c = s.char_at(i);
            match c {
                c if c == b'\n' as u16 => region.append_for_escaped_char(&s, &mut sb, "\\n"),
                c if c == b'\r' as u16 => region.append_for_escaped_char(&s, &mut sb, "\\r"),
                c if c == b'\t' as u16 => region.append_for_escaped_char(&s, &mut sb, "\\t"),
                c if c == b'\\' as u16 => {
                    region.append_for_escaped_char(&s, &mut sb, backslash_escape)
                }
                c if c == b'"' as u16 => {
                    region.append_for_escaped_char(&s, &mut sb, doublequote_escape)
                }
                c if c == b'\'' as u16 => {
                    region.append_for_escaped_char(&s, &mut sb, singlequote_escape)
                }
                c if c == b'>' as u16 => {
                    if i >= 2
                        && ((s.char_at(i - 1) == b'-' as u16 && s.char_at(i - 2) == b'-' as u16)
                            || (s.char_at(i - 1) == b']' as u16 && s.char_at(i - 2) == b']' as u16))
                    {
                        region.append_for_escaped_char(&s, &mut sb, "\\u003e");
                    } else {
                        region.increment_for_normal_char();
                    }
                }
                c if c == b'<' as u16 => {
                    const END_SCRIPT: &str = "/script";
                    const START_COMMENT: &str = "!--";
                    if crate::java_string::region_matches(
                        &s,
                        true,
                        (i + 1) as i32,
                        &END_SCRIPT.into(),
                        0,
                        END_SCRIPT.len() as i32,
                    ) {
                        region.append_for_escaped_char(&s, &mut sb, "\\u003c");
                    } else if crate::java_string::region_matches(
                        &s,
                        false,
                        (i + 1) as i32,
                        &START_COMMENT.into(),
                        0,
                        START_COMMENT.len() as i32,
                    ) {
                        region.append_for_escaped_char(&s, &mut sb, "\\u003c");
                    } else {
                        region.increment_for_normal_char();
                    }
                }
                _ => {
                    if c > 0x1f && c <= 0x7f {
                        region.increment_for_normal_char();
                    } else {
                        region.append_unescaped(&s, &mut sb);
                        region.increment_for_escaped_char();
                        Self::append_hex_java_script_representation(&mut sb, c);
                    }
                }
            }
        }
        region.append_unescaped(&s, &mut sb);
        sb.push(quote);
        sb.into()
    }

    // port: Util#appendHexJavaScriptRepresentation(StringBuilder,char)
    pub fn append_hex_java_script_representation(sb: &mut String, c: u16) {
        if let Err(ex) = Self::append_hex_java_script_representation_code_point(c as i32, sb) {
            // Rust String, like StringBuilder, cannot throw an append error.
            panic!("java.lang.RuntimeException: {ex}");
        }
    }

    // port: Util#appendHexJavaScriptRepresentation(int,Appendable)
    fn append_hex_java_script_representation_code_point(
        code_point: i32,
        out: &mut dyn Write,
    ) -> fmt::Result {
        if (0x10000..=0x10ffff).contains(&code_point) {
            let surrogates = [
                (((code_point - 0x10000) >> 10) + 0xd800) as u16,
                (((code_point - 0x10000) & 0x3ff) + 0xdc00) as u16,
            ];
            Self::append_hex_java_script_representation_code_point(surrogates[0] as i32, out)?;
            Self::append_hex_java_script_representation_code_point(surrogates[1] as i32, out)?;
            return Ok(());
        }
        out.write_str("\\u")?;
        out.write_char(Self::HEX_CHARS[((code_point as u32 >> 12) & 0xf) as usize] as char)?;
        out.write_char(Self::HEX_CHARS[((code_point as u32 >> 8) & 0xf) as usize] as char)?;
        out.write_char(Self::HEX_CHARS[((code_point as u32 >> 4) & 0xf) as usize] as char)?;
        out.write_char(Self::HEX_CHARS[(code_point & 0xf) as usize] as char)
    }

    // port: Util#Util
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}

#[derive(Default)]
struct UnescapedRegion {
    unescaped_region_start: usize,
    unescaped_region_end: usize,
}
impl UnescapedRegion {
    // port: Util.UnescapedRegion#UnescapedRegion
    fn new() -> Self {
        Self {
            unescaped_region_start: 0,
            unescaped_region_end: 0,
        }
    }

    // port: Util.UnescapedRegion#appendUnescaped
    fn append_unescaped(&mut self, s: &JsString, sb: &mut String) {
        if self.unescaped_region_start != self.unescaped_region_end {
            sb.push_str(
                &s.substring(self.unescaped_region_start, self.unescaped_region_end)
                    .to_string_lossy(),
            );
        }
        self.unescaped_region_start = self.unescaped_region_end;
    }
    // port: Util.UnescapedRegion#incrementForNormalChar
    fn increment_for_normal_char(&mut self) {
        self.unescaped_region_end += 1;
    }
    // port: Util.UnescapedRegion#incrementForEscapedChar
    fn increment_for_escaped_char(&mut self) {
        if self.unescaped_region_start != self.unescaped_region_end {
            panic!("java.lang.IllegalStateException");
        }
        self.unescaped_region_start += 1;
        self.unescaped_region_end += 1;
    }
    // port: Util.UnescapedRegion#appendForEscapedChar
    fn append_for_escaped_char(
        &mut self,
        s: &JsString,
        sb: &mut String,
        escaped: impl Into<JsString>,
    ) {
        self.append_unescaped(s, sb);
        self.increment_for_escaped_char();
        sb.push_str(&escaped.into().to_string_lossy());
    }
}
