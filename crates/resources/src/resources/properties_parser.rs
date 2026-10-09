/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/resources/PropertiesParser.java.

//! Port of `com.google.javascript.jscomp.resources.PropertiesParser`.
//!
//! Java indexes strings by UTF-16 code unit. Every character this parser looks for or trims
//! (`#`, `!`, `:`, `=`, ' ', `\\`, chars <= ' ') is ASCII, so byte indices into the UTF-8 `&str`
//! split the text at exactly the same characters.

use indexmap::IndexMap;

use crate::guava::ImmutableMapBuilder;

/// Parses a Java properties file in a a way that can be transpiled into JS.
///
/// The format is probably not fully parsed by this code, but is suitable for simple use-cases
/// inside Closure.
pub struct PropertiesParser;

impl PropertiesParser {
    // port: PropertiesParser#parse
    pub fn parse(source: &str) -> IndexMap<String, String> {
        let mut builder = ImmutableMapBuilder::new();

        let lines = split_lines(source);
        let mut i = 0;
        while i < lines.len() {
            'body: {
                let mut line: &str = lines[i];
                if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                    break 'body; // skip if empty or starts with # or !
                }

                let delimeter_index = Self::find_delimiter(line);
                if delimeter_index == -1 {
                    break 'body;
                }
                let delimeter_index = delimeter_index as usize;

                let mut data = String::new();
                // Remove whitespace on both sides of key.
                let key = java_trim(&line[..delimeter_index]).to_string();
                // Remove whitespace only on left side of data value. Trailing white space is data.
                line = Self::trim_left(&line[delimeter_index + 1..]);
                loop {
                    if line.ends_with('\\') {
                        data.push_str(&line[..line.len() - 1]);
                        if i + 1 == lines.len() {
                            break;
                        }
                        i += 1;
                        line = Self::trim_left(lines[i]);
                    } else {
                        data.push_str(line);
                        break;
                    }
                }

                builder.put(key, data);
            }
            i += 1;
        }

        builder.build_or_throw()
    }

    // port: PropertiesParser#trimLeft
    fn trim_left(str: &str) -> &str {
        for (i, c) in str.char_indices() {
            if c != ' ' {
                return &str[i..];
            }
        }
        str
    }

    // port: PropertiesParser#findDelimiter
    fn find_delimiter(line: &str) -> isize {
        for (i, c) in line.char_indices() {
            match c {
                ':' | '=' => {
                    return i as isize;
                }
                _ => {}
            }
        }

        // If no : or =, delimiter is first whitespace.
        match line.find(' ') {
            Some(i) => i as isize,
            None => -1,
        }
    }
}

/// `source.split("\r?\n")`: Java's `String#split(regex)` with limit 0, so trailing empty strings
/// are removed, and an input without any match is returned whole (`""` gives `[""]`).
// port: String#split(String) with the regex "\r?\n"
fn split_lines(source: &str) -> Vec<&str> {
    if !source.contains('\n') {
        return vec![source];
    }
    let mut list: Vec<&str> = Vec::new();
    let mut index = 0;
    while let Some(offset) = source[index..].find('\n') {
        let newline = index + offset;
        // The match is "\r\n" when a '\r' directly precedes the '\n'.
        let end = if newline > index && source.as_bytes()[newline - 1] == b'\r' {
            newline - 1
        } else {
            newline
        };
        list.push(&source[index..end]);
        index = newline + 1;
    }
    list.push(&source[index..]);
    while list.last().is_some_and(|s| s.is_empty()) {
        list.pop();
    }
    list
}

/// Java's `String#trim()`: strips leading and trailing chars <= ' '.
// port: String#trim
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}
