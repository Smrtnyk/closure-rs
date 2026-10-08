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
 * Copyright (c) 2000, 2021, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1996, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2001, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1):
//   java.base/java/io/BufferedReader.java, java.base/java/lang/String.java,
//   java.base/java/nio/charset/CharsetDecoder.java, java.base/sun/nio/cs/StreamDecoder.java,
//   java.base/sun/nio/cs/StreamEncoder.java, java.base/sun/nio/cs/UTF_8.java.

//! JDK 21 UTF-8 replacement encoding/decoding and BufferedReader line boundaries.
use crate::js_string::JsString;
use std::io::{self, Read};

// port: java.lang.String#isNotContinuation / sun.nio.cs.UTF_8.Decoder#isNotContinuation
fn is_not_continuation(b: u8) -> bool {
    b & 0xc0 != 0x80
}
// port: java.lang.String#isMalformed3_2 / sun.nio.cs.UTF_8.Decoder#isMalformed3_2
fn is_malformed3_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xe0 && b2 & 0xe0 == 0x80) || is_not_continuation(b2)
}
// port: java.lang.String#isMalformed3 / sun.nio.cs.UTF_8.Decoder#isMalformed3
fn is_malformed3(b1: u8, b2: u8, b3: u8) -> bool {
    is_malformed3_2(b1, b2) || is_not_continuation(b3)
}
// port: java.lang.String#isMalformed4_2 / sun.nio.cs.UTF_8.Decoder#isMalformed4_2
fn is_malformed4_2(b1: u8, b2: u8) -> bool {
    (b1 == 0xf0 && !(0x90..=0xbf).contains(&b2))
        || (b1 == 0xf4 && b2 & 0xf0 != 0x80)
        || is_not_continuation(b2)
}
// port: java.lang.String#isMalformed4 / sun.nio.cs.UTF_8.Decoder#isMalformed4
fn is_malformed4(b2: u8, b3: u8, b4: u8) -> bool {
    is_not_continuation(b2) || is_not_continuation(b3) || is_not_continuation(b4)
}
// port: java.lang.String#decode2
fn decode2(b1: u8, b2: u8) -> u16 {
    (((b1 & 0x1f) as u16) << 6) | (b2 & 0x3f) as u16
}
// port: java.lang.String#decode3
fn decode3(b1: u8, b2: u8, b3: u8) -> u16 {
    (((b1 & 0x0f) as u16) << 12) | (((b2 & 0x3f) as u16) << 6) | (b3 & 0x3f) as u16
}
// port: java.lang.String#decode4
fn decode4(b1: u8, b2: u8, b3: u8, b4: u8) -> u32 {
    (((b1 & 7) as u32) << 18)
        | (((b2 & 0x3f) as u32) << 12)
        | (((b3 & 0x3f) as u32) << 6)
        | (b4 & 0x3f) as u32
}
// port: java.lang.String#malformed3 / sun.nio.cs.UTF_8.Decoder#malformedN(3)
fn malformed3(src: &[u8], sp: usize) -> usize {
    if is_malformed3_2(src[sp], src[sp + 1]) {
        1
    } else {
        2
    }
}
// port: java.lang.String#malformed4 / sun.nio.cs.UTF_8.Decoder#malformedN(4)
fn malformed4(src: &[u8], sp: usize) -> usize {
    if src[sp] > 0xf4 || is_malformed4_2(src[sp], src[sp + 1]) {
        return 1;
    }
    if is_not_continuation(src[sp + 2]) {
        return 2;
    }
    3
}

// port: java.lang.String#decodeUTF8_UTF16 (doReplace=true)
pub fn decode(bytes: &[u8]) -> JsString {
    let src = bytes;
    let sl = src.len();
    let mut sp = 0;
    let mut dst = Vec::with_capacity(sl);
    while sp < sl {
        let b1 = src[sp];
        sp += 1;
        if b1 < 0x80 {
            dst.push(b1 as u16);
        } else if b1 >> 5 == 6 && b1 & 0x1e != 0 {
            if sp < sl {
                let b2 = src[sp];
                sp += 1;
                if is_not_continuation(b2) {
                    dst.push(0xfffd);
                    sp -= 1;
                } else {
                    dst.push(decode2(b1, b2));
                }
                continue;
            }
            dst.push(0xfffd);
            break;
        } else if b1 >> 4 == 0xe {
            if sp + 1 < sl {
                let b2 = src[sp];
                let b3 = src[sp + 1];
                sp += 2;
                if is_malformed3(b1, b2, b3) {
                    dst.push(0xfffd);
                    sp -= 3;
                    sp += malformed3(src, sp);
                } else {
                    let c = decode3(b1, b2, b3);
                    dst.push(if (0xd800..=0xdfff).contains(&c) {
                        0xfffd
                    } else {
                        c
                    });
                }
                continue;
            }
            if sp < sl && is_malformed3_2(b1, src[sp]) {
                dst.push(0xfffd);
                continue;
            }
            dst.push(0xfffd);
            break;
        } else if b1 >> 3 == 0x1e {
            if sp + 2 < sl {
                let b2 = src[sp];
                let b3 = src[sp + 1];
                let b4 = src[sp + 2];
                sp += 3;
                let uc = decode4(b1, b2, b3, b4);
                if is_malformed4(b2, b3, b4) || !(0x10000..=0x10ffff).contains(&uc) {
                    dst.push(0xfffd);
                    sp -= 4;
                    sp += malformed4(src, sp);
                } else {
                    dst.push(((uc - 0x10000) >> 10) as u16 + 0xd800);
                    dst.push(((uc - 0x10000) & 0x3ff) as u16 + 0xdc00);
                }
                continue;
            }
            if b1 > 0xf4 || sp < sl && is_malformed4_2(b1, src[sp]) {
                dst.push(0xfffd);
                continue;
            }
            sp += 1;
            dst.push(0xfffd);
            if sp < sl && is_not_continuation(src[sp]) {
                continue;
            }
            break;
        } else {
            dst.push(0xfffd);
        }
    }
    JsString::from_units(dst)
}

// port: sun.nio.cs.UTF_8.Encoder#encodeArrayLoopSlow / sun.nio.cs.StreamEncoder#implWrite
pub fn encode(value: &JsString) -> Vec<u8> {
    let sa = value.as_units();
    let mut sp = 0;
    let sl = sa.len();
    let mut da = Vec::new();
    while sp < sl {
        let c = sa[sp];
        if c < 0x80 {
            da.push(c as u8);
        } else if c < 0x800 {
            da.push((0xc0 | c >> 6) as u8);
            da.push((0x80 | c & 0x3f) as u8);
        } else if (0xd800..=0xdfff).contains(&c) {
            if (0xd800..=0xdbff).contains(&c)
                && sp + 1 < sl
                && (0xdc00..=0xdfff).contains(&sa[sp + 1])
            {
                let uc = 0x10000 + (((c as u32) - 0xd800) << 10) + sa[sp + 1] as u32 - 0xdc00;
                da.push((0xf0 | uc >> 18) as u8);
                da.push((0x80 | (uc >> 12) & 0x3f) as u8);
                da.push((0x80 | (uc >> 6) & 0x3f) as u8);
                da.push((0x80 | uc & 0x3f) as u8);
                sp += 1;
            } else {
                da.push(b'?');
            }
        } else {
            da.push((0xe0 | c >> 12) as u8);
            da.push((0x80 | (c >> 6) & 0x3f) as u8);
            da.push((0x80 | c & 0x3f) as u8);
        }
        sp += 1;
    }
    da
}

// port: sun.nio.cs.UTF_8.Decoder#decodeArrayLoop / java.nio.charset.CharsetDecoder#decode
fn decode_array_loop(src: &[u8], end_of_input: bool) -> (Vec<u16>, usize) {
    let mut dst = Vec::new();
    let mut sp = 0;
    while sp < src.len() {
        let b1 = src[sp];
        let remaining = src.len() - sp;
        let (needed, malformed) = if b1 < 0x80 {
            dst.push(b1 as u16);
            sp += 1;
            continue;
        } else if b1 >> 5 == 6 && b1 & 0x1e != 0 {
            (
                2,
                if remaining >= 2 && is_not_continuation(src[sp + 1]) {
                    1
                } else {
                    0
                },
            )
        } else if b1 >> 4 == 0xe {
            (
                3,
                if remaining >= 2 && is_malformed3_2(b1, src[sp + 1]) {
                    1
                } else if remaining >= 3 && is_not_continuation(src[sp + 2]) {
                    2
                } else if remaining >= 3
                    && (0xd800..=0xdfff).contains(&decode3(b1, src[sp + 1], src[sp + 2]))
                {
                    3
                } else {
                    0
                },
            )
        } else if b1 >> 3 == 0x1e {
            (
                4,
                if b1 > 0xf4 || remaining >= 2 && is_malformed4_2(b1, src[sp + 1]) {
                    1
                } else if remaining >= 3 && is_not_continuation(src[sp + 2]) {
                    2
                } else if remaining >= 4 && is_not_continuation(src[sp + 3]) {
                    3
                } else {
                    0
                },
            )
        } else {
            (1, 1)
        };
        if malformed != 0 {
            dst.push(0xfffd);
            sp += malformed;
            continue;
        }
        if remaining < needed {
            if end_of_input {
                dst.push(0xfffd);
                sp = src.len();
            }
            break;
        }
        match needed {
            2 => dst.push(decode2(b1, src[sp + 1])),
            3 => dst.push(decode3(b1, src[sp + 1], src[sp + 2])),
            4 => {
                let uc = decode4(b1, src[sp + 1], src[sp + 2], src[sp + 3]);
                dst.push(((uc - 0x10000) >> 10) as u16 + 0xd800);
                dst.push(((uc - 0x10000) & 0x3ff) as u16 + 0xdc00);
            }
            _ => unreachable!(),
        }
        sp += needed;
    }
    (dst, sp)
}

pub struct BufferedReader<R> {
    stream: R,
    bytes: Vec<u8>,
    chars: Vec<u16>,
    next_char: usize,
    skip_lf: bool,
    eof: bool,
}
impl<R: Read> BufferedReader<R> {
    // port: java.io.BufferedReader#BufferedReader / sun.nio.cs.StreamDecoder#StreamDecoder
    pub fn new(stream: R) -> Self {
        Self {
            stream,
            bytes: Vec::new(),
            chars: Vec::new(),
            next_char: 0,
            skip_lf: false,
            eof: false,
        }
    }
    // port: sun.nio.cs.StreamDecoder#implRead / java.io.BufferedReader#fill
    fn fill(&mut self) -> io::Result<()> {
        self.chars.clear();
        self.next_char = 0;
        loop {
            let (chars, consumed) = decode_array_loop(&self.bytes, self.eof);
            self.bytes.drain(..consumed);
            self.chars = chars;
            if !self.chars.is_empty() || self.eof {
                return Ok(());
            }
            let mut buffer = [0u8; 8192];
            let n = self.stream.read(&mut buffer)?;
            if n == 0 {
                self.eof = true;
            } else {
                self.bytes.extend_from_slice(&buffer[..n]);
            }
        }
    }
    // port: java.io.BufferedReader#implReadLine
    pub fn read_line(&mut self) -> io::Result<Option<JsString>> {
        let mut s = Vec::new();
        let mut omit_lf = self.skip_lf;
        loop {
            if self.next_char >= self.chars.len() {
                self.fill()?;
            }
            if self.next_char >= self.chars.len() {
                return Ok(if s.is_empty() {
                    None
                } else {
                    Some(JsString::from_units(s))
                });
            }
            if omit_lf && self.chars[self.next_char] == 10 {
                self.next_char += 1;
            }
            self.skip_lf = false;
            omit_lf = false;
            let start_char = self.next_char;
            while self.next_char < self.chars.len()
                && !matches!(self.chars[self.next_char], 10 | 13)
            {
                self.next_char += 1;
            }
            s.extend_from_slice(&self.chars[start_char..self.next_char]);
            if self.next_char < self.chars.len() {
                let c = self.chars[self.next_char];
                self.next_char += 1;
                if c == 13 {
                    self.skip_lf = true;
                }
                return Ok(Some(JsString::from_units(s)));
            }
        }
    }
}
