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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/debugging/sourcemap/Util.java.

use closure_rhino::js_string::JsString;
use std::fmt::{self, Write};

pub struct Util;
impl Util {
    const HEX_CHARS: &'static [u8; 16] = b"0123456789abcdef";

    // port: Util#escapeString
    pub fn escape_string(s: impl Into<JsString>) -> JsString {
        let s = s.into();
        let mut sb = String::with_capacity(s.length() + 2);
        Self::write_escaped_string(&mut sb, &s).expect("writing to a String");
        sb.into()
    }

    /// Rust-only (performance divergence): escape UTF-16 code units straight to the destination.
    /// The result is ASCII. Bounded writes avoid building a second copy of embedded source files.
    pub(crate) fn write_escaped_string(out: &mut dyn Write, s: &JsString) -> fmt::Result {
        let units = s.as_units();
        let mut buf = [0u8; 16384];
        buf[0] = b'"';
        let mut len = 1;
        let mut i = 0;
        let n = units.len();
        while i < n {
            // Fast-path runs of plain printable ASCII that never need escaping.
            let avail = (buf.len() - 7).saturating_sub(len);
            if avail == 0 {
                out.write_str(std::str::from_utf8(&buf[..len]).unwrap())?;
                len = 0;
                continue;
            }
            let c = units[i];
            if (0x20..=0x7f).contains(&c) && !matches!(c, 0x22 | 0x3c | 0x3e | 0x5c) {
                let run_limit = (i + avail).min(n);
                let mut j = i;
                while j < run_limit {
                    let u = units[j];
                    if !(0x20..=0x7f).contains(&u) || matches!(u, 0x22 | 0x3c | 0x3e | 0x5c) {
                        break;
                    }
                    buf[len] = u as u8;
                    len += 1;
                    j += 1;
                }
                i = j;
                continue;
            }
            let hex;
            let escape: &[u8] = match c {
                0x0a => br"\n",
                0x0d => br"\r",
                0x09 => br"\t",
                0x5c => br"\\",
                0x22 => b"\\\"",
                0x3e if i >= 2
                    && units[i - 1] == units[i - 2]
                    && matches!(units[i - 1], 0x2d | 0x5d) =>
                {
                    br"\u003e"
                }
                0x3c if Self::starts_html_sequence(s, i + 1) => br"\u003c",
                0x20..=0x7f => {
                    buf[len] = c as u8;
                    len += 1;
                    i += 1;
                    continue;
                }
                _ => {
                    hex = Self::hex_escape(c);
                    &hex
                }
            };
            buf[len..len + escape.len()].copy_from_slice(escape);
            len += escape.len();
            i += 1;
        }
        buf[len] = b'"';
        out.write_str(std::str::from_utf8(&buf[..=len]).unwrap())
    }

    fn starts_html_sequence(s: &JsString, start: usize) -> bool {
        let tail = &s.as_units()[start..];
        if tail.starts_with(&[0x21, 0x2d, 0x2d]) {
            return true;
        }
        // Java's case folding for "/script" also accepts non-ASCII letters.
        static END_SCRIPT: std::sync::LazyLock<JsString> =
            std::sync::LazyLock::new(|| "/script".into());
        tail.first() == Some(&0x2f)
            && crate::java_string::region_matches(s, true, start as i32, &END_SCRIPT, 0, 7)
    }

    fn hex_escape(c: u16) -> [u8; 6] {
        [
            b'\\',
            b'u',
            Self::HEX_CHARS[(c >> 12) as usize],
            Self::HEX_CHARS[((c >> 8) & 15) as usize],
            Self::HEX_CHARS[((c >> 4) & 15) as usize],
            Self::HEX_CHARS[(c & 15) as usize],
        ]
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
        out.write_str(std::str::from_utf8(&Self::hex_escape(code_point as u16)).unwrap())
    }

    // port: Util#Util
    #[allow(dead_code)]
    fn new() -> Self {
        Self
    }
}
