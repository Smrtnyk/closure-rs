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
 * Copyright (c) 2002, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1994, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1994, 2021, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 1994, 2022, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Boolean.java,
//   java.base/java/lang/Character.java, java.base/java/lang/Long.java,
//   java.base/java/lang/String.java, java.base/java/lang/Throwable.java.

use closure_rhino::js_string::JsString;

// port: java.lang.String#startsWith(String,int)
pub(crate) fn starts_with(s: &JsString, prefix: &JsString, toffset: i32) -> bool {
    let ta = s.as_units();
    let mut to = toffset;
    let pa = prefix.as_units();
    let mut po = 0;
    let mut pc = prefix.length();
    if toffset < 0 || toffset as i64 > s.length() as i64 - pc as i64 {
        return false;
    }
    while pc > 0 {
        pc -= 1;
        let c1 = ta[to as usize];
        to += 1;
        let c2 = pa[po];
        po += 1;
        if c1 != c2 {
            return false;
        }
    }
    true
}

// port: java.lang.Character#toUpperCase(char)
pub(crate) fn to_upper_case(ch: u16) -> u16 {
    CASE_MAP
        .binary_search_by_key(&ch, |entry| entry.0)
        .map_or(ch, |i| CASE_MAP[i].1)
}

// port: java.lang.Character#toLowerCase(char)
pub(crate) fn to_lower_case(ch: u16) -> u16 {
    CASE_MAP
        .binary_search_by_key(&ch, |entry| entry.0)
        .map_or(ch, |i| CASE_MAP[i].2)
}

// port: java.lang.String#regionMatches(boolean,int,String,int,int)
pub(crate) fn region_matches(
    s: &JsString,
    ignore_case: bool,
    toffset: i32,
    other: &JsString,
    ooffset: i32,
    len: i32,
) -> bool {
    if ooffset < 0
        || toffset < 0
        || toffset as i64 > s.length() as i64 - len as i64
        || ooffset as i64 > other.length() as i64 - len as i64
    {
        return false;
    }
    let mut to = toffset as usize;
    let mut po = ooffset as usize;
    let mut len = len;
    while len > 0 {
        len -= 1;
        let c1 = s.char_at(to);
        to += 1;
        let c2 = other.char_at(po);
        po += 1;
        if c1 == c2 {
            continue;
        }
        if ignore_case {
            let u1 = to_upper_case(c1);
            let u2 = to_upper_case(c2);
            if u1 == u2 {
                continue;
            }
            if to_lower_case(u1) == to_lower_case(u2) {
                continue;
            }
        }
        return false;
    }
    true
}

include!("java_character_case.rs");

// port: java.lang.Long#parseLong(String)
pub(crate) fn parse_long(s: &JsString) -> Result<i64, ()> {
    parse_long_with_radix(s, 10)
}

// port: java.lang.Long#parseLong(String,int)
fn parse_long_with_radix(s: &JsString, radix: i32) -> Result<i64, ()> {
    if radix < 2 {
        return Err(());
    }
    if radix > 36 {
        return Err(());
    }
    let mut negative = false;
    let mut i = 0;
    let len = s.length();
    let mut limit = -i64::MAX;
    if len > 0 {
        let first_char = s.char_at(0);
        if first_char < b'0' as u16 {
            if first_char == b'-' as u16 {
                negative = true;
                limit = i64::MIN;
            } else if first_char != b'+' as u16 {
                return Err(());
            }
            if len == 1 {
                return Err(());
            }
            i += 1;
        }
        let multmin = limit / radix as i64;
        let mut result = 0i64;
        while i < len {
            let digit = closure_rhino::java_lang::digit(s.char_at(i), radix);
            i += 1;
            if digit < 0 || result < multmin {
                return Err(());
            }
            result *= radix as i64;
            if result < limit + digit as i64 {
                return Err(());
            }
            result -= digit as i64;
        }
        Ok(if negative { result } else { -result })
    } else {
        Err(())
    }
}

// port: java.lang.Boolean#parseBoolean
pub(crate) fn parse_boolean(s: &JsString) -> bool {
    // Rust references enforce the Java s != null guard.
    s.length() == 4 && region_matches(s, true, 0, &"true".into(), 0, 4)
}

// port: java.lang.Throwable#toString
pub(crate) fn throwable_to_string(
    class_name: impl Into<JsString>,
    message: Option<&JsString>,
) -> JsString {
    let s = class_name.into();
    if let Some(message) = message {
        s.concat(&": ".into()).concat(message)
    } else {
        s
    }
}
