/*
 * Copyright (c) 1994, 2024, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/String.java.

use crate::js_string::JsString;

/// Java String.trim removes only code units at or below U+0020.
// port: java.lang.String#trim
pub(super) fn trim(value: &JsString) -> JsString {
    let mut start = 0;
    let mut end = value.length();
    while start < end && value.char_at(start) <= u16::from(b' ') {
        start += 1;
    }
    while start < end && value.char_at(end - 1) <= u16::from(b' ') {
        end -= 1;
    }
    value.substring(start, end)
}

/// The String.format substitutions used by this package. Arguments already have their
/// Java textual representation, so substitution never decodes UTF-16.
// port: java.lang.String#format
pub(super) fn format_message(format: &JsString, arguments: &[JsString]) -> JsString {
    let format = format.as_units();
    let mut result = Vec::new();
    let mut i = 0;
    let mut argument = 0;
    while i < format.len() {
        if format[i] == u16::from(b'%') && i + 1 < format.len() {
            match format[i + 1] {
                ch if ch == u16::from(b's') || ch == u16::from(b'd') || ch == u16::from(b'c') => {
                    result.extend_from_slice(arguments[argument].as_units());
                    argument += 1;
                }
                ch if ch == u16::from(b'%') => result.push(u16::from(b'%')),
                ch if ch == u16::from(b'n') => result.push(u16::from(b'\n')),
                ch if ch == u16::from(b'0')
                    && format.get(i + 2..i + 4) == Some(&[u16::from(b'4'), u16::from(b'X')]) =>
                {
                    // %04X receives the ASCII decimal rendering of an integer, never JS text.
                    let value: u32 = arguments[argument].to_string_lossy().parse().unwrap();
                    result.extend(format!("{value:04X}").encode_utf16());
                    argument += 1;
                    i += 2;
                }
                _ => panic!("Unsupported Java format conversion"),
            }
            i += 2;
        } else {
            result.push(format[i]);
            i += 1;
        }
    }
    JsString::from_units(result)
}

// port: java.lang.String#split(String)
pub fn split(value: &str, regex: &str) -> Vec<String> {
    split_units(&JsString::from(value), regex)
        .into_iter()
        .map(|s| s.to_string_lossy())
        .collect()
}

// port: java.lang.String#split(String) (UTF-16-preserving form)
pub fn split_units(value: &JsString, regex: &str) -> Vec<JsString> {
    let chars: Vec<char> = regex.chars().collect();
    let literal = if chars.len() == 1 && !".$|()[{^?*+\\".contains(chars[0]) {
        Some(chars[0])
    } else if chars.len() == 2 && chars[0] == '\\' && !chars[1].is_ascii_alphanumeric() {
        Some(chars[1])
    } else {
        None
    };
    if let Some(literal) = literal.filter(|c| (*c as u32) < 0x10000) {
        let mut parts: Vec<JsString> = value
            .as_units()
            .split(|c| *c == literal as u16)
            .map(|s| JsString::from_units(s.to_vec()))
            .collect();
        if parts.len() == 1 {
            return parts;
        }
        while parts.last().is_some_and(JsString::is_empty) {
            parts.pop();
        }
        return parts;
    }
    let mut matcher = super::regex::Pattern::compile(regex).matcher(value.clone());
    let mut index = 0;
    let mut parts = Vec::new();
    while matcher.find() {
        if index == 0 && matcher.start() == 0 && matcher.end() == 0 {
            continue;
        }
        parts.push(value.substring(index, matcher.start()));
        index = matcher.end();
    }
    if index == 0 {
        return vec![value.clone()];
    }
    parts.push(value.substring_from(index));
    while parts.last().is_some_and(JsString::is_empty) {
        parts.pop();
    }
    parts
}

// port: java.lang.String#encodeUTF8_UTF16 (String#getBytes(UTF_8), replacement enabled)
pub fn get_bytes_utf8(value: &JsString) -> Vec<u8> {
    let mut dst = Vec::new();
    let mut sp = 0;
    let sl = value.length();
    while sp < sl {
        let c = value.char_at(sp);
        sp += 1;
        if c < 0x80 {
            dst.push(c as u8);
        } else if c < 0x800 {
            dst.push((0xc0 | (c >> 6)) as u8);
            dst.push((0x80 | (c & 0x3f)) as u8);
        } else if (0xd800..=0xdfff).contains(&c) {
            if (0xd800..=0xdbff).contains(&c)
                && sp < sl
                && (0xdc00..=0xdfff).contains(&value.char_at(sp))
            {
                let uc =
                    0x10000 + ((c as u32 - 0xd800) << 10) + (value.char_at(sp) as u32 - 0xdc00);
                sp += 1;
                dst.push((0xf0 | (uc >> 18)) as u8);
                dst.push((0x80 | ((uc >> 12) & 0x3f)) as u8);
                dst.push((0x80 | ((uc >> 6) & 0x3f)) as u8);
                dst.push((0x80 | (uc & 0x3f)) as u8);
            } else {
                dst.push(b'?');
            }
        } else {
            dst.push((0xe0 | (c >> 12)) as u8);
            dst.push((0x80 | ((c >> 6) & 0x3f)) as u8);
            dst.push((0x80 | (c & 0x3f)) as u8);
        }
    }
    dst
}

/// Rust-only JDK emulation of `String.toLowerCase(Locale.ROOT)`: the Unicode default case
/// mapping of every code point (SpecialCasing included), with the Final_Sigma condition for
/// U+03A3. Unpaired surrogates are kept, as Java keeps them.
// port: java.lang.String#toLowerCase(Locale)
pub fn to_lower_case_root(value: &JsString) -> JsString {
    map_valid_runs(value, str::to_lowercase)
}

/// Rust-only JDK emulation of `String.toUpperCase(Locale.ROOT)`: the Unicode default case
/// mapping of every code point (SpecialCasing included, e.g. U+00DF to "SS"). Unpaired
/// surrogates are kept, as Java keeps them.
// port: java.lang.String#toUpperCase(Locale)
pub fn to_upper_case_root(value: &JsString) -> JsString {
    map_valid_runs(value, str::to_uppercase)
}

/// Applies `f` to every run of well-formed UTF-16 in `value`, copying unpaired surrogates.
fn map_valid_runs(value: &JsString, f: fn(&str) -> String) -> JsString {
    let mut out: Vec<u16> = Vec::with_capacity(value.length());
    let mut run = String::new();
    for decoded in char::decode_utf16(value.as_units().iter().copied()) {
        match decoded {
            Ok(c) => run.push(c),
            Err(e) => {
                out.extend(f(&run).encode_utf16());
                run.clear();
                out.push(e.unpaired_surrogate());
            }
        }
    }
    out.extend(f(&run).encode_utf16());
    JsString::from_units(out)
}
