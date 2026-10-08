/*
 * Copyright (c) 2000, 2025, Oracle and/or its affiliates. All rights reserved.
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
/*
 * Copyright (c) 2000, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/String.java,
//   java.base/java/net/URI.java, java.base/java/net/URISyntaxException.java.

//! Observable JDK 21 java.net.URI(String) parsing, equality, and hashCode.
use crate::{
    java_lang::{JavaHashCode, is_space_separator},
    js_string::JsString,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
};
#[derive(Clone, Debug)]
pub struct URI {
    string: String,
    scheme: Option<String>,
    fragment: Option<String>,
    scheme_specific_part: Option<String>,
    path: Option<String>,
    query: Option<String>,
    authority: Option<String>,
    user_info: Option<String>,
    host: Option<String>,
    port: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct URISyntaxException {
    pub input: String,
    pub reason: String,
    pub index: Option<usize>,
}
impl fmt::Display for URISyntaxException {
    // port: URISyntaxException#getMessage
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.reason)?;
        if let Some(i) = self.index {
            write!(f, " at index {i}")?;
        }
        write!(f, ": {}", self.input)
    }
}
impl std::error::Error for URISyntaxException {}
impl URI {
    // port: URI#URI(String)
    pub fn new(input: &str) -> Result<Self, URISyntaxException> {
        let uri = Self {
            string: input.into(),
            scheme: None,
            fragment: None,
            scheme_specific_part: None,
            path: None,
            query: None,
            authority: None,
            user_info: None,
            host: None,
            port: -1,
        };
        Parser {
            input: JsString::from(input),
            uri,
            ipv6byte_count: 0,
        }
        .parse()
    }
    // port: URI#getScheme
    pub fn get_scheme(&self) -> Option<&str> {
        self.scheme.as_deref()
    }
    // port: URI#isOpaque
    pub fn is_opaque(&self) -> bool {
        self.path.is_none()
    }
    // port: URI#isAbsolute
    pub fn is_absolute(&self) -> bool {
        self.scheme.is_some()
    }
    // port: URI#getRawPath
    pub fn get_raw_path(&self) -> Option<&str> {
        self.path.as_deref()
    }
    // port: URI#getPath
    pub fn get_path(&self) -> Option<String> {
        self.path.as_deref().map(decode)
    }
    // port: URI#hashCode
    pub fn hash_code(&self) -> i32 {
        let mut h = hash_ignoring_case(0, self.scheme.as_deref());
        h = hash(h, self.fragment.as_deref());
        if self.is_opaque() {
            h = hash(h, self.scheme_specific_part.as_deref());
        } else {
            h = hash(h, self.path.as_deref());
            h = hash(h, self.query.as_deref());
            if self.host.is_some() {
                h = hash(h, self.user_info.as_deref());
                h = hash_ignoring_case(h, self.host.as_deref());
                h = h.wrapping_add(1949i32.wrapping_mul(self.port));
            } else {
                h = hash(h, self.authority.as_deref());
            }
        }
        h
    }
}
impl fmt::Display for URI {
    // port: URI#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.string)
    }
}
impl PartialEq for URI {
    // port: URI#equals
    fn eq(&self, that: &Self) -> bool {
        if self.is_opaque() != that.is_opaque()
            || !equal_ignoring_case(self.scheme.as_deref(), that.scheme.as_deref())
            || !equal(self.fragment.as_deref(), that.fragment.as_deref())
        {
            return false;
        }
        if self.is_opaque() {
            return equal(
                self.scheme_specific_part.as_deref(),
                that.scheme_specific_part.as_deref(),
            );
        }
        if !equal(self.path.as_deref(), that.path.as_deref())
            || !equal(self.query.as_deref(), that.query.as_deref())
        {
            return false;
        }
        if self.authority == that.authority {
            return true;
        }
        if self.host.is_some() {
            equal(self.user_info.as_deref(), that.user_info.as_deref())
                && equal_ignoring_case(self.host.as_deref(), that.host.as_deref())
                && self.port == that.port
        } else if self.authority.is_some() {
            equal(self.authority.as_deref(), that.authority.as_deref())
        } else {
            that.authority.is_none()
        }
    }
}
impl Eq for URI {}
impl Hash for URI {
    // port: URI#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code());
    }
}
// port: URI#decode(char,char)
fn decode_hex_pair(c1: u16, c2: u16) -> u8 {
    ((decode_hex(c1) & 0xf) << 4 | (decode_hex(c2) & 0xf)) as u8
}
// port: URI#decode(char)
fn decode_hex(c: u16) -> u16 {
    if (u16::from(b'0')..=u16::from(b'9')).contains(&c) {
        return c - u16::from(b'0');
    }
    if (u16::from(b'a')..=u16::from(b'f')).contains(&c) {
        return c - u16::from(b'a') + 10;
    }
    if (u16::from(b'A')..=u16::from(b'F')).contains(&c) {
        return c - u16::from(b'A') + 10;
    }
    unreachable!("assert false");
}
// port: URI#decode(String)
fn decode(s: &str) -> String {
    decode_ignoring(s, true)
}
// port: URI#decode(String,boolean)
fn decode_ignoring(s: &str, ignore_percent_in_square_brackets: bool) -> String {
    let n = s.len();
    if n == 0 {
        return s.into();
    }
    if !s.contains('%') {
        return s.into();
    }
    let s: Vec<u16> = s.encode_utf16().collect();
    let n = s.len();
    let mut sb: Vec<u16> = Vec::with_capacity(n);
    let mut bb: Vec<u8> = Vec::with_capacity(n);
    // This is not horribly efficient, but it will do for now
    let mut c = s[0];
    let mut between_brackets = false;
    let mut i = 0;
    while i < n {
        debug_assert!(c == s[i]); // Loop invariant
        if c == u16::from(b'[') {
            between_brackets = true;
        } else if between_brackets && c == u16::from(b']') {
            between_brackets = false;
        }
        if c != u16::from(b'%') || (between_brackets && ignore_percent_in_square_brackets) {
            sb.push(c);
            i += 1;
            if i >= n {
                break;
            }
            c = s[i];
            continue;
        }
        bb.clear();
        loop {
            debug_assert!(n - i >= 2);
            bb.push(decode_hex_pair(s[i + 1], s[i + 2]));
            i += 3;
            if i >= n {
                break;
            }
            c = s[i];
            if c != u16::from(b'%') {
                break;
            }
        }
        // CharsetDecoder UTF_8 with CodingErrorAction.REPLACE
        sb.extend(String::from_utf8_lossy(&bb).encode_utf16());
    }
    String::from_utf16_lossy(&sb)
}
// port: URI#equal, URI#percentNormalizedComparison
fn equal(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => percent_normalized(a) == percent_normalized(b),
        _ => false,
    }
}
// port: URI#normalizedHash (percent normalization also used by equality)
fn percent_normalized(s: &str) -> Vec<u16> {
    let mut v: Vec<_> = s.encode_utf16().collect();
    let mut i = 0;
    while i < v.len() {
        if v[i] == b'%' as u16 {
            v[i + 1] = to_upper(v[i + 1]);
            v[i + 2] = to_upper(v[i + 2]);
            i += 2;
        }
        i += 1;
    }
    v
}
// port: URI#toLower
fn to_lower(c: u16) -> u16 {
    if (65..=90).contains(&c) { c + 32 } else { c }
}
// port: URI#toUpper
fn to_upper(c: u16) -> u16 {
    if (97..=122).contains(&c) { c - 32 } else { c }
}
// port: URI#equalIgnoringCase
fn equal_ignoring_case(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a
            .encode_utf16()
            .map(to_lower)
            .eq(b.encode_utf16().map(to_lower)),
        _ => false,
    }
}
// port: URI#hash
fn hash(h: i32, s: Option<&str>) -> i32 {
    let Some(s) = s else {
        return h;
    };
    let value = if s.contains('%') {
        percent_normalized(s)
            .into_iter()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
    } else {
        s.hash_code()
    };
    h.wrapping_mul(127).wrapping_add(value)
}
// port: URI#hashIgnoringCase
fn hash_ignoring_case(h: i32, s: Option<&str>) -> i32 {
    s.map_or(h, |s| {
        s.encode_utf16().fold(h, |h, c| {
            h.wrapping_mul(31).wrapping_add(to_lower(c) as i32)
        })
    })
}
// port: URI#character masks
const L_DIGIT: u64 = 0x3FF000000000000;
const H_DIGIT: u64 = 0;
const L_UPALPHA: u64 = 0;
const H_UPALPHA: u64 = 0x7FFFFFE;
const L_LOWALPHA: u64 = 0;
const H_LOWALPHA: u64 = 0x7FFFFFE00000000;
const L_ALPHA: u64 = L_LOWALPHA | L_UPALPHA;
const H_ALPHA: u64 = H_LOWALPHA | H_UPALPHA;
const L_ALPHANUM: u64 = L_DIGIT | L_ALPHA;
const H_ALPHANUM: u64 = H_DIGIT | H_ALPHA;
const L_HEX: u64 = L_DIGIT;
const H_HEX: u64 = 0x7E0000007E;
const L_MARK: u64 = 0x678200000000;
const H_MARK: u64 = 0x4000000080000000;
const L_UNRESERVED: u64 = L_ALPHANUM | L_MARK;
const H_UNRESERVED: u64 = H_ALPHANUM | H_MARK;
const L_RESERVED: u64 = 0xAC00985000000000;
const H_RESERVED: u64 = 0x28000001;
const L_ESCAPED: u64 = 1;
const H_ESCAPED: u64 = 0;
const L_URIC: u64 = L_RESERVED | L_UNRESERVED | L_ESCAPED;
const H_URIC: u64 = H_RESERVED | H_UNRESERVED | H_ESCAPED;
const L_PCHAR: u64 = L_UNRESERVED | L_ESCAPED | 0x2400185000000000;
const H_PCHAR: u64 = H_UNRESERVED | H_ESCAPED | 0x1;
const L_PATH: u64 = L_PCHAR | 0x800800000000000;
const H_PATH: u64 = H_PCHAR;
const L_DASH: u64 = 0x200000000000;
const H_DASH: u64 = 0x0;
const L_DOT: u64 = 0x400000000000;
const H_DOT: u64 = 0x0;
const L_USERINFO: u64 = L_UNRESERVED | L_ESCAPED | 0x2C00185000000000;
const H_USERINFO: u64 = H_UNRESERVED | H_ESCAPED;
const L_REG_NAME: u64 = L_UNRESERVED | L_ESCAPED | 0x2C00185000000000;
const H_REG_NAME: u64 = H_UNRESERVED | H_ESCAPED | 0x1;
const L_SERVER: u64 = L_USERINFO | L_ALPHANUM | L_DASH | 0x400400000000000;
const H_SERVER: u64 = H_USERINFO | H_ALPHANUM | H_DASH | 0x28000001;
const L_SERVER_PERCENT: u64 = L_SERVER | 0x2000000000;
const H_SERVER_PERCENT: u64 = H_SERVER;
const L_SCHEME: u64 = L_ALPHA | L_DIGIT | 0x680000000000;
const H_SCHEME: u64 = H_ALPHA | H_DIGIT;
const L_SCOPE_ID: u64 = L_ALPHANUM | 0x400000000000;
const H_SCOPE_ID: u64 = H_ALPHANUM | 0x80000000;
// port: URI#match
fn mask_match(c: u16, low: u64, high: u64) -> bool {
    if c == 0 {
        return false;
    }
    if c < 64 {
        1u64 << c & low != 0
    } else if c < 128 {
        1u64 << (c - 64) & high != 0
    } else {
        false
    }
}
struct Parser {
    input: JsString,
    uri: URI,
    ipv6byte_count: usize,
}
impl Parser {
    // port: URI.Parser#fail
    fn fail(&self, reason: &str, index: Option<usize>) -> URISyntaxException {
        URISyntaxException {
            input: self.uri.string.clone(),
            reason: reason.into(),
            index,
        }
    }
    // port: URI.Parser#failExpecting
    fn fail_expecting(&self, expected: &str, index: Option<usize>) -> URISyntaxException {
        self.fail(&format!("Expected {expected}"), index)
    }
    // port: URI.Parser#at(int,int,char)
    fn at(&self, start: usize, end: usize, c: u16) -> bool {
        start < end && self.input.char_at(start) == c
    }
    // port: URI.Parser#at(int,int,String)
    fn at_string(&self, start: usize, end: usize, s: &str) -> bool {
        let units: Vec<_> = s.encode_utf16().collect();
        start + units.len() <= end && self.input.as_units()[start..start + units.len()] == units
    }
    // port: String#substring (URI component extraction)
    fn substring(&self, start: usize, end: usize) -> String {
        self.input.substring(start, end).to_string_lossy()
    }
    // port: URI.Parser#scan(int,int,String,String)
    fn scan_delimiters(&self, start: usize, end: usize, err: &str, stop: &str) -> Option<usize> {
        let mut p = start;
        while p < end {
            let c = self.input.char_at(p);
            if err.encode_utf16().any(|e| e == c) {
                return None;
            }
            if stop.encode_utf16().any(|s| s == c) {
                break;
            }
            p += 1;
        }
        Some(p)
    }
    // port: URI.Parser#scan(int,int,char)
    fn scan_char(&self, start: usize, end: usize, c: u16) -> usize {
        if self.at(start, end, c) {
            start + 1
        } else {
            start
        }
    }
    // port: URI.Parser#scanEscape
    fn scan_escape(&self, start: usize, n: usize, first: u16) -> Result<usize, URISyntaxException> {
        if first == b'%' as u16 {
            if start + 3 <= n
                && mask_match(self.input.char_at(start + 1), L_HEX, H_HEX)
                && mask_match(self.input.char_at(start + 2), L_HEX, H_HEX)
            {
                return Ok(start + 3);
            }
            return Err(self.fail("Malformed escape pair", Some(start)));
        }
        if first > 128
            && !is_space_separator(first as i32)
            && !matches!(first,0x2028|0x2029|0..=0x1f|0x7f..=0x9f)
        {
            return Ok(start + 1);
        }
        Ok(start)
    }
    // port: URI.Parser#scan(int,int,long,long)
    fn scan(
        &self,
        start: usize,
        n: usize,
        low: u64,
        high: u64,
    ) -> Result<usize, URISyntaxException> {
        let mut p = start;
        while p < n {
            let c = self.input.char_at(p);
            if mask_match(c, low, high) {
                p += 1;
                continue;
            }
            if low & L_ESCAPED != 0 {
                let q = self.scan_escape(p, n, c)?;
                if q > p {
                    p = q;
                    continue;
                }
            }
            break;
        }
        Ok(p)
    }
    // port: URI.Parser#checkChars
    fn check_chars(
        &self,
        start: usize,
        end: usize,
        low: u64,
        high: u64,
        what: &str,
    ) -> Result<(), URISyntaxException> {
        let p = self.scan(start, end, low, high)?;
        if p < end {
            return Err(self.fail(&format!("Illegal character in {what}"), Some(p)));
        }
        Ok(())
    }
    // port: URI.Parser#checkChar
    fn check_char(
        &self,
        p: usize,
        low: u64,
        high: u64,
        what: &str,
    ) -> Result<(), URISyntaxException> {
        self.check_chars(p, p + 1, low, high, what)
    }
    // port: URI.Parser#parse
    fn parse(mut self) -> Result<URI, URISyntaxException> {
        let n = self.input.length();
        let scanned = self.scan_delimiters(0, n, "/?#", ":");
        let mut p;
        if let Some(q) = scanned
            && self.at(q, n, b':' as u16)
        {
            if q == 0 {
                return Err(self.fail_expecting("scheme name", Some(0)));
            }
            self.check_char(0, L_ALPHA, H_ALPHA, "scheme name")?;
            self.check_chars(1, q, L_SCHEME, H_SCHEME, "scheme name")?;
            self.uri.scheme = Some(self.substring(0, q));
            p = q + 1;
            if self.at(p, n, b'/' as u16) {
                p = self.parse_hierarchical(p, n)?;
            } else {
                let q = self.scan_delimiters(p, n, "", "#").unwrap();
                if q <= p {
                    return Err(self.fail_expecting("scheme-specific part", Some(p)));
                }
                self.check_chars(p, q, L_URIC, H_URIC, "opaque part")?;
                self.uri.scheme_specific_part = Some(self.substring(p, q));
                p = q;
            }
        } else {
            p = self.parse_hierarchical(0, n)?;
        }
        if self.at(p, n, b'#' as u16) {
            self.check_chars(p + 1, n, L_URIC, H_URIC, "fragment")?;
            self.uri.fragment = Some(self.substring(p + 1, n));
            p = n;
        }
        if p < n {
            return Err(self.fail("end of URI", Some(p)));
        }
        Ok(self.uri)
    }
    // port: URI.Parser#parseHierarchical
    fn parse_hierarchical(&mut self, start: usize, n: usize) -> Result<usize, URISyntaxException> {
        let mut p = start;
        if self.at(p, n, b'/' as u16) && self.at(p + 1, n, b'/' as u16) {
            p += 2;
            let q = self.scan_delimiters(p, n, "", "/?#").unwrap();
            if q > p {
                p = self.parse_authority(p, q)?;
            } else if q == n {
                return Err(self.fail_expecting("authority", Some(p)));
            }
        }
        let q = self.scan_delimiters(p, n, "", "?#").unwrap();
        self.check_chars(p, q, L_PATH, H_PATH, "path")?;
        self.uri.path = Some(self.substring(p, q));
        p = q;
        if self.at(p, n, b'?' as u16) {
            p += 1;
            let q = self.scan_delimiters(p, n, "", "#").unwrap();
            self.check_chars(p, q, L_URIC, H_URIC, "query")?;
            self.uri.query = Some(self.substring(p, q));
            p = q;
        }
        Ok(p)
    }
    // port: URI.Parser#parseAuthority
    fn parse_authority(&mut self, start: usize, n: usize) -> Result<usize, URISyntaxException> {
        let mut q = start;
        let mut ex = None;
        let has_bracket = self.scan_delimiters(start, n, "", "]").unwrap() > start;
        let server_chars = self.scan(
            start,
            n,
            if has_bracket {
                L_SERVER_PERCENT
            } else {
                L_SERVER
            },
            if has_bracket {
                H_SERVER_PERCENT
            } else {
                H_SERVER
            },
        )? == n;
        let qreg = self.scan(start, n, L_REG_NAME, H_REG_NAME)?;
        let reg_chars = qreg == n;
        if reg_chars && !server_chars {
            self.uri.authority = Some(self.substring(start, n));
            return Ok(n);
        }
        let skip_parse_exception = reg_chars;
        if server_chars {
            match self.parse_server(start, n, skip_parse_exception) {
                Ok(parsed) => {
                    q = parsed;
                    if q < n {
                        if skip_parse_exception {
                            self.uri.user_info = None;
                            self.uri.host = None;
                            self.uri.port = -1;
                            q = start;
                        } else {
                            return Err(self.fail_expecting("end of authority", Some(q)));
                        }
                    } else {
                        self.uri.authority = Some(self.substring(start, n));
                    }
                }
                Err(e) => {
                    self.uri.user_info = None;
                    self.uri.host = None;
                    self.uri.port = -1;
                    ex = Some(e);
                    q = start;
                }
            }
        }
        if q < n {
            if reg_chars {
                self.uri.authority = Some(self.substring(start, n));
            } else if let Some(e) = ex {
                return Err(e);
            } else {
                return Err(self.fail(
                    "Illegal character in authority",
                    Some(if server_chars { q } else { qreg }),
                ));
            }
        }
        Ok(n)
    }
    // port: URI.Parser#parseServer
    fn parse_server(
        &mut self,
        start: usize,
        n: usize,
        skip: bool,
    ) -> Result<usize, URISyntaxException> {
        let mut p = start;
        if let Some(q) = self.scan_delimiters(p, n, "/?#", "@")
            && self.at(q, n, b'@' as u16)
        {
            self.check_chars(p, q, L_USERINFO, H_USERINFO, "user info")?;
            self.uri.user_info = Some(self.substring(p, q));
            p = q + 1;
        }
        if self.at(p, n, b'[' as u16) {
            p += 1;
            let q = self.scan_delimiters(p, n, "/?#", "]");
            if let Some(q) = q
                && q > p
                && self.at(q, n, b']' as u16)
            {
                let r = self.scan_delimiters(p, q, "", "%").unwrap();
                if r > p {
                    self.parse_ipv6_reference(p, r)?;
                    if r + 1 == q {
                        return Err(self.fail("scope id expected", None));
                    }
                    self.check_chars(r + 1, q, L_SCOPE_ID, H_SCOPE_ID, "scope id")?;
                } else {
                    self.parse_ipv6_reference(p, q)?;
                }
                self.uri.host = Some(self.substring(p - 1, q + 1));
                p = q + 1;
            } else {
                return Err(self.fail_expecting("closing bracket for IPv6 address", q));
            }
        } else {
            p = if let Some(q) = self.parse_ipv4_address(p, n) {
                q
            } else {
                self.parse_hostname(p, n, skip)?
            };
        }
        if self.at(p, n, b':' as u16) {
            p += 1;
            let q = self.scan_delimiters(p, n, "", "/").unwrap();
            if q > p {
                self.check_chars(p, q, L_DIGIT, H_DIGIT, "port number")?;
                self.uri.port = self
                    .substring(p, q)
                    .parse::<i32>()
                    .map_err(|_| self.fail("Malformed port number", Some(p)))?;
                p = q;
            }
        } else if p < n && skip {
            return Ok(p);
        }
        if p < n {
            return Err(self.fail_expecting("port number", Some(p)));
        }
        Ok(p)
    }
    // port: URI.Parser#scanByte
    fn scan_byte(&self, start: usize, n: usize) -> Result<usize, URISyntaxException> {
        let q = self.scan(start, n, L_DIGIT, H_DIGIT)?;
        if q <= start {
            return Ok(q);
        }
        let mut i = start;
        while self.at(i, q, b'0' as u16) {
            i += 1;
        }
        let significant = q - i;
        if significant < 3 {
            return Ok(q);
        }
        if significant > 3 {
            return Ok(start);
        }
        if self.substring(i, q).parse::<u32>().unwrap() > 255 {
            return Ok(start);
        }
        Ok(q)
    }
    // port: URI.Parser#scanIPv4Address
    fn scan_ipv4_address(
        &self,
        start: usize,
        n: usize,
        strict: bool,
    ) -> Result<Option<usize>, URISyntaxException> {
        let m = self.scan(start, n, L_DIGIT | L_DOT, H_DIGIT | H_DOT)?;
        if m <= start || strict && m != n {
            return Ok(None);
        }
        let mut p = start;
        let mut q = p;
        for i in 0..4 {
            q = self.scan_byte(p, m)?;
            if q <= p {
                break;
            }
            p = q;
            if i == 3 {
                if q == m {
                    return Ok(Some(q));
                }
                break;
            }
            q = self.scan_char(p, m, b'.' as u16);
            if q <= p {
                break;
            }
            p = q;
        }
        if strict {
            return Err(self.fail("Malformed IPv4 address", Some(q)));
        }
        Ok(None)
    }
    // port: URI.Parser#takeIPv4Address
    fn take_ipv4_address(
        &self,
        start: usize,
        n: usize,
        expected: &str,
    ) -> Result<usize, URISyntaxException> {
        self.scan_ipv4_address(start, n, true)?
            .filter(|p| *p > start)
            .ok_or_else(|| self.fail_expecting(expected, Some(start)))
    }
    // port: URI.Parser#parseIPv4Address
    fn parse_ipv4_address(&mut self, start: usize, n: usize) -> Option<usize> {
        let p = self.scan_ipv4_address(start, n, false).ok()??;
        if p > start && p < n && !self.at(p, n, b':' as u16) {
            return None;
        }
        if p > start {
            self.uri.host = Some(self.substring(start, p));
        }
        Some(p)
    }
    // port: URI.Parser#parseHostname
    fn parse_hostname(
        &mut self,
        start: usize,
        n: usize,
        skip: bool,
    ) -> Result<usize, URISyntaxException> {
        let mut p = start;
        let mut l = None;
        loop {
            let q = self.scan(p, n, L_ALPHANUM, H_ALPHANUM)?;
            if q <= p {
                break;
            }
            l = Some(p);
            p = q;
            let q = self.scan(p, n, L_ALPHANUM | L_DASH, H_ALPHANUM | H_DASH)?;
            if q > p {
                if self.input.char_at(q - 1) == b'-' as u16 {
                    return Err(self.fail("Illegal character in hostname", Some(q - 1)));
                }
                p = q;
            }
            let q = self.scan_char(p, n, b'.' as u16);
            if q <= p {
                break;
            }
            p = q;
            if p >= n {
                break;
            }
        }
        if p < n && !self.at(p, n, b':' as u16) {
            if skip {
                return Ok(p);
            }
            return Err(self.fail("Illegal character in hostname", Some(p)));
        }
        let Some(l) = l else {
            return Err(self.fail_expecting("hostname", Some(start)));
        };
        if l > start && !mask_match(self.input.char_at(l), L_ALPHA, H_ALPHA) {
            return Err(self.fail("Illegal character in hostname", Some(l)));
        }
        self.uri.host = Some(self.substring(start, p));
        Ok(p)
    }
    // port: URI.Parser#parseIPv6Reference
    fn parse_ipv6_reference(
        &mut self,
        start: usize,
        n: usize,
    ) -> Result<usize, URISyntaxException> {
        let mut p = start;
        let mut compressed_zeros = false;
        if let Some(q) = self.scan_hex_seq(p, n)?
            && q > p
        {
            p = q;
            if self.at_string(p, n, "::") {
                compressed_zeros = true;
                p = self.scan_hex_post(p + 2, n)?;
            } else if self.at(p, n, b':' as u16) {
                p = self.take_ipv4_address(p + 1, n, "IPv4 address")?;
                self.ipv6byte_count += 4;
            }
        } else if self.at_string(p, n, "::") {
            compressed_zeros = true;
            p = self.scan_hex_post(p + 2, n)?;
        }
        if p < n {
            return Err(self.fail("Malformed IPv6 address", Some(start)));
        }
        if self.ipv6byte_count > 16 {
            return Err(self.fail("IPv6 address too long", Some(start)));
        }
        if !compressed_zeros && self.ipv6byte_count < 16 {
            return Err(self.fail("IPv6 address too short", Some(start)));
        }
        if compressed_zeros && self.ipv6byte_count == 16 {
            return Err(self.fail("Malformed IPv6 address", Some(start)));
        }
        Ok(p)
    }
    // port: URI.Parser#scanHexPost
    fn scan_hex_post(&mut self, start: usize, n: usize) -> Result<usize, URISyntaxException> {
        let mut p = start;
        if p == n {
            return Ok(p);
        }
        if let Some(q) = self.scan_hex_seq(p, n)?
            && q > p
        {
            p = q;
            if self.at(p, n, b':' as u16) {
                p = self.take_ipv4_address(p + 1, n, "hex digits or IPv4 address")?;
                self.ipv6byte_count += 4;
            }
        } else {
            p = self.take_ipv4_address(p, n, "hex digits or IPv4 address")?;
            self.ipv6byte_count += 4;
        }
        Ok(p)
    }
    // port: URI.Parser#scanHexSeq
    fn scan_hex_seq(
        &mut self,
        start: usize,
        n: usize,
    ) -> Result<Option<usize>, URISyntaxException> {
        let mut p = start;
        let q = self.scan(p, n, L_HEX, H_HEX)?;
        if q <= p || self.at(q, n, b'.' as u16) {
            return Ok(None);
        }
        if q > p + 4 {
            return Err(self.fail("IPv6 hexadecimal digit sequence too long", Some(p)));
        }
        self.ipv6byte_count += 2;
        p = q;
        while p < n {
            if !self.at(p, n, b':' as u16) || self.at(p + 1, n, b':' as u16) {
                break;
            }
            p += 1;
            let q = self.scan(p, n, L_HEX, H_HEX)?;
            if q <= p {
                return Err(self.fail_expecting("digits for an IPv6 address", Some(p)));
            }
            if self.at(q, n, b'.' as u16) {
                p -= 1;
                break;
            }
            if q > p + 4 {
                return Err(self.fail("IPv6 hexadecimal digit sequence too long", Some(p)));
            }
            self.ipv6byte_count += 2;
            p = q;
        }
        Ok(Some(p))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // port: URITest#depsURIDifferential (JDK 21 generated fixtures)
    #[test]
    fn deps_uri_differential() {
        let mut cases = vec![];
        let mut parsed = vec![];
        for line in include_str!("testdata/uri.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            match f[0] {
                "i" => {
                    let s = decode(f[2]);
                    parsed.push(URI::new(&s));
                    cases.push(s);
                }
                "u" => {
                    let i: usize = f[1].parse().unwrap();
                    let uri = parsed[i].as_ref().unwrap();
                    assert_eq!(uri.to_string(), cases[i]);
                    assert_eq!(
                        uri.hash_code(),
                        f[2].parse::<i32>().unwrap(),
                        "{}",
                        cases[i]
                    );
                    assert_eq!(uri.is_opaque().to_string(), f[3]);
                }
                "e" => {
                    let i: usize = f[1].parse().unwrap();
                    assert_eq!(parsed[i].as_ref().unwrap_err().to_string(), decode(f[2]));
                }
                "q" => {
                    let i: usize = f[1].parse().unwrap();
                    let j: usize = f[2].parse().unwrap();
                    assert_eq!(
                        (parsed[i].as_ref().unwrap() == parsed[j].as_ref().unwrap()).to_string(),
                        f[3],
                        "{} vs {}",
                        cases[i],
                        cases[j]
                    );
                }
                _ => panic!("Invalid fixture"),
            }
        }
    }
    // port: String#charAt (fixture encoding)
    fn decode(s: &str) -> String {
        String::from_utf16(
            &(0..s.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }
}
