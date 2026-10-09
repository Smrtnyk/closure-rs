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
/*
 * Copyright (c) 1999, 2021, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2008, 2023, Oracle and/or its affiliates. All rights reserved.
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
 * Copyright (c) 2008, 2009, Oracle and/or its affiliates. All rights reserved.
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
//   java.base/java/util/regex/Matcher.java, java.base/java/util/regex/Pattern.java,
//   java.base/java/util/regex/PatternSyntaxException.java, java.base/sun/nio/fs/Globs.java,
//   java.base/sun/nio/fs/UnixFileSystem.java.

//! The Unix glob grammar and regular-expression subset emitted by the JDK.
use closure_rhino::fast_hash::IndexSet;

#[derive(Clone, Debug)]
enum Token {
    Literal(char),
    Any,
    Star(bool),
    Class(CharPredicate),
    Group(Vec<Vec<Token>>),
}
#[derive(Clone, Debug)]
enum CharPredicate {
    Range(char, char),
    Union(Vec<Self>),
    And(Box<Self>, Box<Self>),
    Negate(Box<Self>),
}
impl CharPredicate {
    // port: Pattern.CharPredicate#is
    fn is(&self, c: char) -> bool {
        match self {
            Self::Range(start, end) => *start <= c && c <= *end,
            Self::Union(items) => items.iter().any(|p| p.is(c)),
            Self::And(left, right) => left.is(c) && right.is(c),
            Self::Negate(inner) => !inner.is(c),
        }
    }
    // port: Pattern.CharPredicate#union
    fn union(self, right: Self) -> Self {
        Self::Union(vec![self, right])
    }
}
#[derive(Clone, Debug)]
pub struct Glob {
    tokens: Vec<Token>,
}
#[derive(Clone, Debug)]
pub struct PatternSyntaxException {
    pub description: String,
    pub pattern: String,
    pub index: usize,
}
impl std::fmt::Display for PatternSyntaxException {
    // port: PatternSyntaxException#getMessage
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} near index {}\n{}",
            self.description, self.index, self.pattern
        )?;
        if self.index < self.pattern.encode_utf16().count() {
            write!(f, "\n{}^", " ".repeat(self.index))?;
        }
        Ok(())
    }
}
impl std::error::Error for PatternSyntaxException {}
impl Glob {
    // port: UnixFileSystem#getPathMatcher
    pub fn new(pattern: &str) -> Result<Self, PatternSyntaxException> {
        let regex = to_unix_regex_pattern(pattern)?;
        let mut parser = PatternParser {
            chars: regex.chars().collect(),
            pattern: regex,
            cursor: 1,
        };
        let tokens = parser.sequence(false)?;
        Ok(Self { tokens })
    }
    // port: Matcher#matches (the regular-expression subset emitted by Globs)
    pub fn matches(&self, path: &str) -> bool {
        let chars: Vec<char> = path.chars().collect();
        advance(&self.tokens, &chars, IndexSet::<_>::from_iter([0])).contains(&chars.len())
    }
}
// port: Globs#toUnixRegexPattern / Globs#toRegexPattern
fn to_unix_regex_pattern(pattern: &str) -> Result<String, PatternSyntaxException> {
    let chars: Vec<u16> = pattern.encode_utf16().collect();
    let error = |description: &str, index| PatternSyntaxException {
        description: description.into(),
        pattern: pattern.into(),
        index,
    };
    let meta = |c| ".^$+{[]|()".encode_utf16().any(|m| m == c);
    let mut regex = vec![b'^' as u16];
    let mut in_group = false;
    let mut i = 0;
    while i < chars.len() {
        let mut c = chars[i];
        i += 1;
        match c {
            92 => {
                if i == chars.len() {
                    return Err(error("No character to escape", i - 1));
                }
                let next = chars[i];
                i += 1;
                if "\\*?[{".encode_utf16().any(|m| m == next) || meta(next) {
                    regex.push(92);
                }
                regex.push(next);
            }
            47 => regex.push(c),
            91 => {
                regex.extend("[[^/]&&[".encode_utf16());
                if chars.get(i) == Some(&94) {
                    regex.extend("\\^".encode_utf16());
                    i += 1;
                } else {
                    if chars.get(i) == Some(&33) {
                        regex.push(94);
                        i += 1;
                    }
                    if chars.get(i) == Some(&45) {
                        regex.push(45);
                        i += 1;
                    }
                }
                let mut has_range_start = false;
                let mut last = 0;
                while i < chars.len() {
                    c = chars[i];
                    i += 1;
                    if c == 93 {
                        break;
                    }
                    if c == 47 {
                        return Err(error("Explicit 'name separator' in class", i - 1));
                    }
                    if c == 92 || c == 91 || c == 38 && chars.get(i) == Some(&38) {
                        regex.push(92);
                    }
                    regex.push(c);
                    if c == 45 {
                        if !has_range_start {
                            return Err(error("Invalid range", i - 1));
                        }
                        c = chars.get(i).copied().unwrap_or(0);
                        i += 1;
                        if c == 0 || c == 93 {
                            break;
                        }
                        if c < last {
                            return Err(error("Invalid range", i - 3));
                        }
                        regex.push(c);
                        has_range_start = false;
                    } else {
                        has_range_start = true;
                        last = c;
                    }
                }
                if c != 93 {
                    return Err(error("Missing ']", i - 1));
                }
                regex.extend("]]".encode_utf16());
            }
            123 => {
                if in_group {
                    return Err(error("Cannot nest groups", i - 1));
                }
                regex.extend("(?:(?:".encode_utf16());
                in_group = true;
            }
            125 => {
                if in_group {
                    regex.extend("))".encode_utf16());
                    in_group = false;
                } else {
                    regex.push(c);
                }
            }
            44 => {
                if in_group {
                    regex.extend(")|(?:".encode_utf16());
                } else {
                    regex.push(c);
                }
            }
            42 => {
                if chars.get(i) == Some(&42) {
                    regex.extend(".*".encode_utf16());
                    i += 1;
                } else {
                    regex.extend("[^/]*".encode_utf16());
                }
            }
            63 => regex.extend("[^/]".encode_utf16()),
            _ => {
                if meta(c) {
                    regex.push(92);
                }
                regex.push(c);
            }
        }
    }
    if in_group {
        return Err(error("Missing '}", i - 1));
    }
    regex.push(b'$' as u16);
    Ok(String::from_utf16(&regex).expect("a glob supplied through argv has valid UTF-16"))
}
struct PatternParser {
    chars: Vec<char>,
    pattern: String,
    cursor: usize,
}
impl PatternParser {
    // port: Pattern#peek
    fn peek(&self) -> char {
        self.chars.get(self.cursor).copied().unwrap_or('\0')
    }
    // port: Pattern#next
    fn next(&mut self) -> char {
        self.cursor += 1;
        self.peek()
    }
    // port: Pattern#error
    fn error(&self, description: &str) -> PatternSyntaxException {
        PatternSyntaxException {
            description: description.into(),
            pattern: self.pattern.clone(),
            index: self.cursor.saturating_sub(1),
        }
    }
    // port: Pattern#sequence (the regular-expression subset emitted by Globs)
    fn sequence(&mut self, in_group: bool) -> Result<Vec<Token>, PatternSyntaxException> {
        let mut tokens = Vec::new();
        while self.cursor < self.chars.len() {
            let c = self.peek();
            if in_group && matches!(c, ')' | '|') || c == '$' && self.cursor + 1 == self.chars.len()
            {
                break;
            }
            let token = match c {
                '\\' => {
                    self.next();
                    let c = self.peek();
                    self.next();
                    Token::Literal(c)
                }
                '.' => {
                    self.cursor += 2;
                    Token::Star(true)
                }
                '[' => {
                    let predicate = self.clazz(true)?;
                    if self.peek() == '*' {
                        self.next();
                        Token::Star(false)
                    } else {
                        Token::Class(predicate)
                    }
                }
                '(' => {
                    self.cursor += 3;
                    let mut groups = vec![self.sequence(true)?];
                    while self.peek() == '|' {
                        self.next();
                        groups.push(self.sequence(true)?);
                    }
                    self.next();
                    Token::Group(groups)
                }
                _ => {
                    self.next();
                    Token::Literal(c)
                }
            };
            // The JDK emits [^/] for '?' and [^/]* for '*'.
            if matches!(&token, Token::Class(CharPredicate::Negate(inner)) if matches!(inner.as_ref(), CharPredicate::Range('/', '/')))
            {
                tokens.push(Token::Any);
            } else {
                tokens.push(token);
            }
        }
        Ok(tokens)
    }
    // port: Pattern#clazz (the regular-expression subset emitted by Globs)
    fn clazz(&mut self, consume: bool) -> Result<CharPredicate, PatternSyntaxException> {
        let mut prev: Option<CharPredicate> = None;
        let mut curr: Option<CharPredicate> = None;
        let mut ch = self.next();
        let mut is_neg = false;
        if ch == '^' && self.chars[self.cursor - 1] == '[' {
            ch = self.next();
            is_neg = true;
        }
        loop {
            match ch {
                '[' => {
                    let value = self.clazz(true)?;
                    prev = Some(match prev {
                        None => value,
                        Some(p) => p.union(value),
                    });
                    ch = self.peek();
                    continue;
                }
                '&' => {
                    ch = self.next();
                    if ch == '&' {
                        ch = self.next();
                        let mut right: Option<CharPredicate> = None;
                        while ch != ']' && ch != '&' {
                            let value = if ch == '[' {
                                self.clazz(true)?
                            } else {
                                self.cursor -= 1;
                                self.clazz(false)?
                            };
                            right = Some(match right {
                                None => value,
                                Some(p) => p.union(value),
                            });
                            ch = self.peek();
                        }
                        if let Some(right) = right {
                            curr = Some(right);
                        }
                        prev = Some(match prev {
                            None => curr.take().ok_or_else(|| self.error("Bad class syntax"))?,
                            Some(p) => CharPredicate::And(
                                Box::new(p),
                                Box::new(
                                    curr.take()
                                        .ok_or_else(|| self.error("Bad intersection syntax"))?,
                                ),
                            ),
                        });
                    } else {
                        self.cursor -= 1;
                    }
                    if ch == '&' || self.peek() != '&' {
                        ch = self.peek();
                        continue;
                    }
                }
                '\0' if self.cursor >= self.chars.len() => {
                    return Err(self.error("Unclosed character class"));
                }
                ']' if prev.is_some() => {
                    if consume {
                        self.next();
                    }
                    let value = prev.unwrap();
                    return Ok(if is_neg {
                        CharPredicate::Negate(Box::new(value))
                    } else {
                        value
                    });
                }
                _ => {}
            }
            let value = self.range()?;
            curr = Some(value.clone());
            prev = Some(match prev {
                None => value,
                Some(p) => p.union(value),
            });
            ch = self.peek();
        }
    }
    // port: Pattern#range (the regular-expression subset emitted by Globs)
    fn range(&mut self) -> Result<CharPredicate, PatternSyntaxException> {
        let mut ch = self.peek();
        if ch == '\\' {
            ch = self.next();
        }
        self.next();
        if self.peek() == '-' && !matches!(self.chars.get(self.cursor + 1), Some('[' | ']')) {
            self.next();
            let mut end = self.peek();
            if end == '\\' {
                end = self.next();
            }
            self.next();
            if end < ch {
                return Err(self.error("Illegal character range"));
            }
            return Ok(CharPredicate::Range(ch, end));
        }
        Ok(CharPredicate::Range(ch, ch))
    }
}
// port: Matcher#matches (the regular-expression subset emitted by Globs)
fn advance(tokens: &[Token], chars: &[char], mut positions: IndexSet<usize>) -> IndexSet<usize> {
    for token in tokens {
        let mut next = IndexSet::<_>::default();
        for pos in positions {
            match token {
                Token::Literal(c) => {
                    if chars.get(pos) == Some(c) {
                        next.insert(pos + 1);
                    }
                }
                Token::Any => {
                    if chars.get(pos).is_some_and(|c| *c != '/') {
                        next.insert(pos + 1);
                    }
                }
                Token::Class(predicate) => {
                    if chars.get(pos).is_some_and(|c| predicate.is(*c)) {
                        next.insert(pos + 1);
                    }
                }
                Token::Star(double) => {
                    next.insert(pos);
                    let mut end = pos;
                    while let Some(c) = chars.get(end) {
                        if if *double {
                            matches!(*c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
                        } else {
                            *c == '/'
                        } {
                            break;
                        }
                        end += 1;
                        next.insert(end);
                    }
                }
                Token::Group(groups) => {
                    for group in groups {
                        next.extend(advance(group, chars, IndexSet::<_>::from_iter([pos])));
                    }
                }
            }
        }
        positions = next;
    }
    positions
}
