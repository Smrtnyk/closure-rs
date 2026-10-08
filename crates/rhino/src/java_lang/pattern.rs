/*
 * Copyright (c) 1999, 2023, Oracle and/or its affiliates. All rights reserved.
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
//   java.base/java/util/regex/Matcher.java, java.base/java/util/regex/Pattern.java.

//! JDK Pattern operations needed by compiler options and coding conventions.
use regex::{Captures, Regex};
use std::fmt;
#[derive(Clone, Debug)]
pub struct Pattern {
    pattern: String,
    flags: i32,
    compiled: Regex,
    complete: Regex,
    final_terminator_group: Option<usize>,
}
impl Pattern {
    // port: Pattern#compile(String)
    pub fn compile(pattern: &str) -> Self {
        Self::compile_with_flags(pattern, 0)
    }
    // port: Pattern#compile(String,int)
    pub fn compile_with_flags(pattern: &str, flags: i32) -> Self {
        // The four pinned Java patterns have flags=0. Preserve the Java source separately.
        crate::check_argument!(flags == 0, "Unsupported Pattern flags: %s", flags);
        let mut translated = String::new();
        let mut escaped = false;
        let mut in_class = false;
        for c in pattern.chars() {
            if escaped {
                translated.push(c);
                escaped = false;
                continue;
            }
            match c {
                '\\' => {
                    translated.push(c);
                    escaped = true;
                }
                '[' => {
                    translated.push(c);
                    in_class = true;
                }
                ']' => {
                    translated.push(c);
                    in_class = false;
                }
                '.' if !in_class => translated.push_str(r"[^\n\r\x{85}\x{2028}\x{2029}]"),
                _ => translated.push(c),
            }
        }
        let final_anchor = translated.ends_with('$') && !translated.ends_with(r"\$");
        if final_anchor {
            translated.pop();
        }
        let complete = Regex::new(&format!(r"\A(?:{translated})\z")).unwrap();
        let compiled = if final_anchor {
            // The suffix models JDK Dollar's final-line-terminator alternative. The matcher
            // excludes that capture from the returned match, since Dollar is zero width.
            Regex::new(&format!(
                r"{translated}(?P<__java_end>\r\n|[\n\r\x{{85}}\x{{2028}}\x{{2029}}])?\z"
            ))
            .unwrap()
        } else {
            Regex::new(&translated).unwrap()
        };
        let final_terminator_group = compiled
            .capture_names()
            .position(|name| name == Some("__java_end"));
        Self {
            pattern: pattern.into(),
            flags,
            compiled,
            complete,
            final_terminator_group,
        }
    }
    // port: Pattern#pattern
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    // port: Pattern#flags
    pub fn flags(&self) -> i32 {
        self.flags
    }
    // port: Pattern#matcher
    pub fn matcher<'p, 'i>(&'p self, input: &'i str) -> Matcher<'p, 'i> {
        Matcher {
            pattern: self,
            input,
            next: Some(0),
            captures: None,
        }
    }
}
impl PartialEq for Pattern {
    // port: Pattern#pattern / Pattern#flags (record comparison)
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.flags == other.flags
    }
}
impl Eq for Pattern {}
impl fmt::Display for Pattern {
    // port: Pattern#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pattern)
    }
}
pub struct Matcher<'p, 'i> {
    pattern: &'p Pattern,
    input: &'i str,
    next: Option<usize>,
    captures: Option<Captures<'i>>,
}
impl<'i> Matcher<'_, 'i> {
    // port: Matcher#matches
    pub fn matches(&mut self) -> bool {
        self.captures = self.pattern.complete.captures(self.input);
        self.captures.is_some()
    }
    // port: Matcher#find
    pub fn find(&mut self) -> bool {
        let Some(start) = self.next else {
            self.captures = None;
            return false;
        };
        self.captures = self.pattern.compiled.captures_at(self.input, start);
        let Some(captures) = &self.captures else {
            self.next = None;
            return false;
        };
        let matched = captures.get(0).unwrap();
        let end = self
            .pattern
            .final_terminator_group
            .and_then(|g| captures.get(g))
            .map_or(matched.end(), |m| m.start());
        self.next = if matched.start() == end {
            self.input[end..].chars().next().map(|c| end + c.len_utf8())
        } else {
            Some(end)
        };
        true
    }
    // port: Matcher#group(int)
    pub fn group(&self, group: usize) -> Option<&'i str> {
        let captures = self.captures.as_ref().expect("No match found");
        let matched = captures.get(group)?;
        if group == 0 {
            let end = self
                .pattern
                .final_terminator_group
                .and_then(|g| captures.get(g))
                .map_or(matched.end(), |m| m.start());
            Some(&self.input[matched.start()..end])
        } else {
            Some(matched.as_str())
        }
    }
    // port: Matcher#replaceAll
    pub fn replace_all(&mut self, replacement: &str) -> String {
        self.replace(replacement, false)
    }
    // port: Matcher#replaceFirst
    pub fn replace_first(&mut self, replacement: &str) -> String {
        self.replace(replacement, true)
    }
    /// The shared body of `Matcher#replaceAll` and `Matcher#replaceFirst` (`appendReplacement`
    /// for every match, or for the first one only, then `appendTail`).
    fn replace(&mut self, replacement: &str, first_only: bool) -> String {
        self.next = Some(0);
        let mut output = String::new();
        let mut end = 0;
        let mut replaced = false;
        while !(first_only && replaced) && self.find() {
            replaced = true;
            let captures = self.captures.as_ref().unwrap();
            let matched = captures.get(0).unwrap();
            output.push_str(&self.input[end..matched.start()]);
            let mut chars = replacement.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    output.push(chars.next().expect("character to be escaped is missing"));
                } else if c == '$' {
                    let c = chars
                        .next()
                        .expect("Illegal group reference: group index is missing");
                    let mut group = c.to_digit(10).expect("Illegal group reference") as usize;
                    while let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
                        let next = group * 10 + d as usize;
                        let count = captures.len()
                            - usize::from(self.pattern.final_terminator_group.is_some());
                        if next >= count {
                            break;
                        }
                        chars.next();
                        group = next;
                    }
                    if let Some(value) = self.group(group) {
                        output.push_str(value);
                    }
                } else {
                    output.push(c);
                }
            }
            end = matched.start() + self.group(0).unwrap().len();
        }
        output.push_str(&self.input[end..]);
        output
    }
}
