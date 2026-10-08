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
 * (C) Copyright Taligent, Inc. 1996, 1997 - All Rights Reserved
 * (C) Copyright IBM Corp. 1996 - 1998 - All Rights Reserved
 *
 *   The original version of this source code and documentation is copyrighted
 * and owned by Taligent, Inc., a wholly-owned subsidiary of IBM. These
 * materials are provided under terms of a License Agreement between Taligent
 * and Sun. This technology is protected by multiple US and International
 * patents. This notice and attribution to Taligent may not be removed.
 *   Taligent is a registered trademark of Taligent, Inc.
 *
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Integer.java,
//   java.base/java/text/MessageFormat.java.

//! JDK 21 MessageFormat's String-argument path. Diagnostic patterns in the pinned
//! Java src tree have no number, date, time, or choice format elements.
use crate::js_string::JsString;
#[derive(Debug)]
pub struct MessageFormat {
    pattern: Vec<u16>,
    offsets: Vec<usize>,
    argument_numbers: Vec<usize>,
    types: Vec<String>,
}
impl MessageFormat {
    // port: MessageFormat#MessageFormat(String)
    pub fn new(pattern: &str) -> Self {
        let mut result = Self {
            pattern: Vec::new(),
            offsets: Vec::new(),
            argument_numbers: Vec::new(),
            types: Vec::new(),
        };
        result.apply_pattern(pattern);
        result
    }
    // port: MessageFormat#applyPattern
    pub fn apply_pattern(&mut self, pattern: &str) {
        let mut segments: [Vec<u16>; 4] = std::array::from_fn(|_| Vec::new());
        let mut part = 0;
        let mut in_quote = false;
        let mut brace_stack = 0;
        self.offsets.clear();
        self.argument_numbers.clear();
        self.types.clear();
        let chars: Vec<u16> = pattern.encode_utf16().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if part == 0 {
                if ch == 39 {
                    if i + 1 < chars.len() && chars[i + 1] == 39 {
                        segments[part].push(ch);
                        i += 1;
                    } else {
                        in_quote = !in_quote;
                    }
                } else if ch == 123 && !in_quote {
                    part = 1;
                } else {
                    segments[part].push(ch);
                }
            } else if in_quote {
                segments[part].push(ch);
                if ch == 39 {
                    in_quote = false;
                }
            } else {
                match ch {
                    44 if part < 3 => part += 1,
                    123 => {
                        brace_stack += 1;
                        segments[part].push(ch);
                    }
                    125 if brace_stack == 0 => {
                        part = 0;
                        self.make_format(&segments);
                        for s in &mut segments[1..] {
                            s.clear();
                        }
                    }
                    125 => {
                        brace_stack -= 1;
                        segments[part].push(ch);
                    }
                    32 if part == 2 && segments[2].is_empty() => {}
                    39 => {
                        in_quote = true;
                        segments[part].push(ch);
                    }
                    _ => segments[part].push(ch),
                }
            }
            i += 1;
        }
        if brace_stack == 0 && part != 0 {
            panic!("Unmatched braces in the pattern.");
        }
        self.pattern = std::mem::take(&mut segments[0]);
    }
    // port: MessageFormat#makeFormat
    fn make_format(&mut self, segments: &[Vec<u16>; 4]) {
        // Pattern segments come from &str and ASCII delimiters cannot split surrogate pairs.
        let index = String::from_utf16(&segments[1]).unwrap();
        let number: i32 = parse_argument_number(&segments[1])
            .unwrap_or_else(|_| panic!("can't parse argument number: {index}"));
        if number < 0 {
            panic!("negative argument number: {number}");
        }
        if number >= 10000 {
            panic!("{number} exceeds the ArgumentIndex implementation limit");
        }
        // Pattern segments come from &str and ASCII delimiters cannot split surrogate pairs.
        let type_ = String::from_utf16(&segments[2]).unwrap();
        let normalized = type_.trim_matches(|c: char| c as u32 <= 32).to_lowercase();
        match normalized.as_str() {
            "" | "number" | "date" | "time" | "choice" => {}
            _ => panic!("unknown format type: {type_}"),
        }
        self.offsets.push(segments[0].len());
        self.argument_numbers.push(number as usize);
        self.types.push(normalized);
    }
    // port: MessageFormat#format(String,Object...)
    pub fn format(pattern: &str, arguments: &[&str]) -> String {
        let args: Vec<_> = arguments.iter().map(|s| JsString::from(*s)).collect();
        // Both the pattern and arguments are Rust &str, so the result has no lone surrogates.
        String::from_utf16(Self::format_js_strings(pattern, &args).as_units()).unwrap()
    }
    // port: MessageFormat#format(String,Object...)
    pub fn format_js_strings(pattern: &str, arguments: &[JsString]) -> JsString {
        Self::new(pattern).subformat(arguments)
    }
    // port: MessageFormat#subformat
    fn subformat(&self, arguments: &[JsString]) -> JsString {
        let mut result = Vec::new();
        let mut last_offset = 0;
        for (i, &offset) in self.offsets.iter().enumerate() {
            result.extend_from_slice(&self.pattern[last_offset..offset]);
            last_offset = offset;
            let number = self.argument_numbers[i];
            if number >= arguments.len() {
                result.extend(format!("{{{number}}}").encode_utf16());
                continue;
            }
            match self.types[i].as_str() {
                "number" | "choice" => panic!("Cannot format given Object as a Number"),
                "date" | "time" => panic!("Cannot format given Object as a Date"),
                _ => result.extend_from_slice(arguments[number].as_units()),
            }
        }
        result.extend_from_slice(&self.pattern[last_offset..]);
        JsString::from_units(result)
    }
}

// port: Integer#parseInt(String,int) (the decimal path used by MessageFormat)
fn parse_argument_number(units: &[u16]) -> Result<i32, ()> {
    let Some(&first) = units.first() else {
        return Err(());
    };
    let negative = first == 45;
    let start = if first == 43 || negative { 1 } else { 0 };
    if start == units.len() {
        return Err(());
    }
    let limit = if negative {
        2147483648i64
    } else {
        2147483647i64
    };
    let mut result = 0i64;
    for &unit in &units[start..] {
        let digit = super::digit_decimal(unit);
        if digit < 0 {
            return Err(());
        }
        result = result * 10 + digit as i64;
        if result > limit {
            return Err(());
        }
    }
    Ok(if negative {
        (-result) as i32
    } else {
        result as i32
    })
}
