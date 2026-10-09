/*
 * Copyright (c) 1995, 2021, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/util/Properties.java.

//! `java.util.Properties#load(Reader)` (JDK 21), as used by `PropertyResourceBundle`: the logical
//! line reader (comments, blank lines, continuation lines with their leading whitespace dropped),
//! the key/value split and the escape conversion. Values are returned in file order; a later
//! duplicate key replaces the earlier value, as `Hashtable#put` does.

use crate::fast_hash::IndexMap;
use crate::js_string::JsString;

/// `Properties.LineReader` over an in-memory character source.
struct LineReader<'a> {
    in_char_buf: &'a [u16],
    in_off: usize,
    in_limit: i32,
    line_buf: Vec<u16>,
    exhausted: bool,
}

impl<'a> LineReader<'a> {
    fn new(source: &'a [u16]) -> Self {
        Self {
            in_char_buf: source,
            in_off: 0,
            in_limit: 0,
            line_buf: Vec::new(),
            exhausted: false,
        }
    }

    /// `reader.read(charBuf)`: the whole source is one buffer, read once.
    fn fill(&mut self) -> i32 {
        if self.exhausted {
            -1
        } else {
            self.exhausted = true;
            self.in_char_buf.len() as i32
        }
    }

    /// Reads one logical line into `line_buf`; returns its length, or -1 at end of input.
    // port: Properties.LineReader#readLine
    fn read_line(&mut self) -> i32 {
        let mut len: usize = 0;
        let mut off = self.in_off;
        let mut limit = self.in_limit;

        let mut skip_white_space = true;
        let mut appended_line_begin = false;
        let mut preceding_backslash = false;
        self.line_buf.clear();
        let mut c: u16;

        loop {
            if off as i32 >= limit {
                self.in_limit = self.fill();
                limit = self.in_limit;
                if limit <= 0 {
                    if len == 0 {
                        return -1;
                    }
                    return if preceding_backslash {
                        len as i32 - 1
                    } else {
                        len as i32
                    };
                }
                off = 0;
            }

            c = self.in_char_buf[off];
            off += 1;

            if skip_white_space {
                if c == b' ' as u16 || c == b'\t' as u16 || c == 0x0c {
                    continue;
                }
                if !appended_line_begin && (c == b'\r' as u16 || c == b'\n' as u16) {
                    continue;
                }
                skip_white_space = false;
                appended_line_begin = false;
            }
            if len == 0 {
                // Still on a new logical line
                if c == b'#' as u16 || c == b'!' as u16 {
                    // Comment, quickly consume the rest of the line
                    'comment_loop: loop {
                        while (off as i32) < limit {
                            c = self.in_char_buf[off];
                            off += 1;
                            if c <= b'\r' as u16 && (c == b'\r' as u16 || c == b'\n' as u16) {
                                break 'comment_loop;
                            }
                        }
                        if off as i32 == limit {
                            self.in_limit = self.fill();
                            limit = self.in_limit;
                            if limit <= 0 {
                                // EOF
                                return -1;
                            }
                            off = 0;
                        }
                    }
                    skip_white_space = true;
                    continue;
                }
            }

            if c != b'\n' as u16 && c != b'\r' as u16 {
                self.line_buf.push(c);
                len += 1;
                // flip the preceding backslash flag
                preceding_backslash = if c == b'\\' as u16 {
                    !preceding_backslash
                } else {
                    false
                };
            } else {
                // reached EOL
                if len == 0 {
                    skip_white_space = true;
                    continue;
                }
                if off as i32 >= limit {
                    self.in_limit = self.fill();
                    limit = self.in_limit;
                    off = 0;
                    if limit <= 0 {
                        // EOF
                        return if preceding_backslash {
                            len as i32 - 1
                        } else {
                            len as i32
                        };
                    }
                }
                if preceding_backslash {
                    // backslash at EOL is not part of the line
                    len -= 1;
                    self.line_buf.truncate(len);
                    // skip leading whitespace characters in the following line
                    skip_white_space = true;
                    appended_line_begin = true;
                    preceding_backslash = false;
                    // take care not to include any subsequent \n
                    if c == b'\r' as u16 && self.in_char_buf[off] == b'\n' as u16 {
                        off += 1;
                    }
                } else {
                    self.in_off = off;
                    return len as i32;
                }
            }
        }
    }
}

/// Loads a properties file (`Properties#load0`).
// port: Properties#load0
pub fn load(source: &str) -> IndexMap<String, String> {
    load_js_strings(&source.into())
        .into_iter()
        .map(|(k, v)| {
            // The UTF-8 overload is for scalar configuration resources; source values use load_js_strings.
            (
                String::from_utf16(k.as_units()).unwrap(),
                String::from_utf16(v.as_units()).unwrap(),
            )
        })
        .collect()
}
// port: Properties#load0
pub fn load_js_strings(source: &JsString) -> IndexMap<JsString, JsString> {
    let units = source.as_units();
    let mut lr = LineReader::new(units);
    let mut result = IndexMap::<_, _>::default();
    let mut limit: i32;
    let mut key_len: usize;
    let mut value_start: usize;
    let mut has_sep: bool;
    let mut preceding_backslash: bool;

    loop {
        limit = lr.read_line();
        if limit < 0 {
            break;
        }
        let limit = limit as usize;
        key_len = 0;
        value_start = limit;
        has_sep = false;

        preceding_backslash = false;
        while key_len < limit {
            let c = lr.line_buf[key_len];
            //need check if escaped.
            if (c == b'=' as u16 || c == b':' as u16) && !preceding_backslash {
                value_start = key_len + 1;
                has_sep = true;
                break;
            } else if (c == b' ' as u16 || c == b'\t' as u16 || c == 0x0c) && !preceding_backslash {
                value_start = key_len + 1;
                break;
            }
            if c == b'\\' as u16 {
                preceding_backslash = !preceding_backslash;
            } else {
                preceding_backslash = false;
            }
            key_len += 1;
        }
        while value_start < limit {
            let c = lr.line_buf[value_start];
            if c != b' ' as u16 && c != b'\t' as u16 && c != 0x0c {
                if !has_sep && (c == b'=' as u16 || c == b':' as u16) {
                    has_sep = true;
                } else {
                    break;
                }
            }
            value_start += 1;
        }
        let key = load_convert(&lr.line_buf[0..key_len]);
        let value = load_convert(&lr.line_buf[value_start..limit]);
        result.insert(key, value);
    }
    result
}

// port: Properties#loadConvert
fn load_convert(input: &[u16]) -> JsString {
    let mut out: Vec<u16> = Vec::with_capacity(input.len());
    let end = input.len();
    let mut off = 0;
    while off < end {
        let mut a_char = input[off];
        off += 1;
        if a_char == b'\\' as u16 {
            // No need to bounds check since LineReader::readLine excludes
            // unescaped \s at the end of the line
            a_char = input[off];
            off += 1;
            if a_char == b'u' as u16 {
                // Read the xxxx
                if off + 4 > end {
                    panic!("Malformed \\uxxxx encoding.");
                }
                let mut value: u32 = 0;
                for _ in 0..4 {
                    a_char = input[off];
                    off += 1;
                    value = match a_char {
                        0x30..=0x39 => (value << 4) + a_char as u32 - 0x30,
                        0x61..=0x66 => (value << 4) + 10 + a_char as u32 - 0x61,
                        0x41..=0x46 => (value << 4) + 10 + a_char as u32 - 0x41,
                        _ => panic!("Malformed \\uxxxx encoding."),
                    };
                }
                out.push(value as u16);
            } else {
                if a_char == b't' as u16 {
                    a_char = b'\t' as u16;
                } else if a_char == b'r' as u16 {
                    a_char = b'\r' as u16;
                } else if a_char == b'n' as u16 {
                    a_char = b'\n' as u16;
                } else if a_char == b'f' as u16 {
                    a_char = 0x0c;
                }
                out.push(a_char);
            }
        } else {
            out.push(a_char);
        }
    }
    JsString::from_units(out)
}

#[cfg(test)]
mod tests {
    use super::load;

    #[test]
    fn continuation_lines_drop_leading_whitespace() {
        let p = load("# c\n! c\nk =\\\n    a,\\\n    b, \\\n  c\nx:y\n z w\n");
        assert_eq!(p.get("k").map(String::as_str), Some("a,b, c"));
        assert_eq!(p.get("x").map(String::as_str), Some("y"));
        assert_eq!(p.get("z").map(String::as_str), Some("w"));
        assert_eq!(p.len(), 3);
    }
}
