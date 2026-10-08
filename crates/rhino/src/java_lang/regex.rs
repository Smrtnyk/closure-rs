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
 * Copyright (c) 2011, 2023, Oracle and/or its affiliates. All rights reserved.
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
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Character.java,
//   java.base/java/lang/String.java, java.base/java/util/regex/CharPredicates.java,
//   java.base/java/util/regex/Matcher.java, java.base/java/util/regex/Pattern.java,
//   java.base/java/util/regex/PatternSyntaxException.java.

//! JDK 21 Pattern/Matcher subset used by jscomp.deps. No flags are enabled.
use crate::js_string::JsString;
use indexmap::IndexMap;
use std::sync::Arc;
#[path = "regex_categories.rs"]
mod categories;
#[derive(Clone, Debug)]
enum Predicate {
    Literal(u32),
    Range(u32, u32),
    Space,
    Word,
    Digit,
    Category(u8),
    Dot,
    Union(Vec<Predicate>, bool),
}
impl Predicate {
    // port: Pattern#newCharProperty, Pattern#single, Pattern#union
    fn is_bmp(&self) -> bool {
        match self {
            Self::Literal(c) => *c < 0x10000 && !(0xd800..=0xdfff).contains(c),
            Self::Range(a, b) => *b < 0x10000 && (*b < 0xd800 || *a > 0xdfff),
            Self::Space | Self::Word | Self::Digit => true,
            Self::Union(items, false) => items.iter().all(Self::is_bmp),
            _ => false,
        }
    }
    // port: Pattern.CharPredicate#is
    fn is(&self, c: u32) -> bool {
        match self {
            Self::Literal(v) => c == *v,
            Self::Range(a, b) => *a <= c && c <= *b,
            Self::Space => matches!(c, 0x20 | 0x09 | 0x0a | 0x0b | 0x0c | 0x0d),
            Self::Word => is_word(c),
            Self::Digit => (48..=57).contains(&c),
            Self::Category(mask) => category(c) & mask != 0,
            Self::Dot => !matches!(c, 0x0a | 0x0d | 0x85 | 0x2028 | 0x2029),
            Self::Union(items, negate) => items.iter().any(|p| p.is(c)) != *negate,
        }
    }
}
// port: Character#getType
fn category(c: u32) -> u8 {
    let i = categories::RANGES.partition_point(|r| r.1 < c);
    if i < categories::RANGES.len() && categories::RANGES[i].0 <= c {
        categories::RANGES[i].2
    } else {
        0
    }
}
// port: CharPredicates#ASCII_WORD
fn is_word(c: u32) -> bool {
    matches!(c, 48..=57|65..=90|97..=122|95)
}
#[derive(Clone, Debug)]
enum Expr {
    Char(Predicate),
    Begin,
    Dollar,
    Bound,
    Sequence(Vec<Expr>),
    Branch(Vec<Expr>),
    Group(usize, Box<Expr>),
    Repeat(Box<Expr>, u8, u8, bool),
}
impl Expr {
    // port: Pattern#newCharProperty, Pattern#compile (hasSupplementary)
    fn has_supplementary(&self) -> bool {
        match self {
            Self::Char(p) => !p.is_bmp(),
            Self::Sequence(v) | Self::Branch(v) => v.iter().any(Self::has_supplementary),
            Self::Group(_, v) | Self::Repeat(v, _, _, _) => v.has_supplementary(),
            _ => false,
        }
    }
    // port: Pattern#compile (Begin roots bypass Start)
    fn anchored(&self) -> bool {
        match self {
            Self::Begin => true,
            Self::Sequence(v) => v.first().is_some_and(Self::anchored),
            Self::Group(_, v) => v.anchored(),
            _ => false,
        }
    }
    // port: Pattern.Node#study
    fn min_length(&self) -> usize {
        match self {
            Self::Char(_) => 1,
            Self::Sequence(v) => v.iter().map(Self::min_length).sum(),
            Self::Branch(v) => v.iter().map(Self::min_length).min().unwrap_or(0),
            Self::Group(_, v) => v.min_length(),
            Self::Repeat(v, min, _, _) => v.min_length() * *min as usize,
            _ => 0,
        }
    }
}
/// Port of `java.util.regex.PatternSyntaxException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternSyntaxException {
    desc: String,
    pattern: String,
    index: i32,
}
impl PatternSyntaxException {
    // port: PatternSyntaxException#getDescription
    pub fn get_description(&self) -> &str {
        &self.desc
    }
    // port: PatternSyntaxException#getPattern
    pub fn get_pattern(&self) -> &str {
        &self.pattern
    }
    // port: PatternSyntaxException#getIndex
    pub fn get_index(&self) -> i32 {
        self.index
    }
}
impl std::fmt::Display for PatternSyntaxException {
    // port: PatternSyntaxException#getMessage
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.desc)?;
        if self.index >= 0 {
            write!(f, " near index {}", self.index)?;
        }
        write!(f, "\n{}", self.pattern)?;
        if self.index >= 0 && !self.pattern.is_empty() {
            write!(f, "\n{}^", " ".repeat(self.index as usize))?;
        }
        Ok(())
    }
}
struct Parser {
    chars: Vec<char>,
    cursor: usize,
    groups: usize,
    named_groups: IndexMap<String, usize>,
}
impl Parser {
    // port: Pattern#expr
    fn expr(&mut self) -> Result<Expr, PatternSyntaxException> {
        let mut branches = vec![self.sequence()?];
        while self.peek() == Some('|') {
            self.cursor += 1;
            branches.push(self.sequence()?);
        }
        Ok(if branches.len() == 1 {
            branches.pop().unwrap()
        } else {
            Expr::Branch(branches)
        })
    }
    // port: Pattern#peek
    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }
    // port: Pattern#read
    fn read(&mut self) -> Result<char, PatternSyntaxException> {
        let Some(c) = self.peek() else {
            return Err(self.error("Incomplete Java regex"));
        };
        self.cursor += 1;
        Ok(c)
    }
    // port: Pattern#error
    fn error(&self, desc: &str) -> PatternSyntaxException {
        PatternSyntaxException {
            desc: desc.to_string(),
            pattern: self.chars.iter().collect(),
            index: self.cursor as i32 - 1,
        }
    }
    // port: Pattern#sequence
    fn sequence(&mut self) -> Result<Expr, PatternSyntaxException> {
        let mut seq = vec![];
        while !matches!(self.peek(), None | Some('|' | ')')) {
            let atom = match self.read()? {
                '(' => self.group0()?,
                '[' => Expr::Char(self.clazz()?),
                '\\' => {
                    if self.peek() == Some('b') {
                        self.cursor += 1;
                        Expr::Bound
                    } else {
                        Expr::Char(self.escape()?)
                    }
                }
                '.' => Expr::Char(Predicate::Dot),
                '^' => Expr::Begin,
                '$' => Expr::Dollar,
                c @ ('*' | '+' | '?' | '{') => {
                    return Err(self.error(&format!("Dangling meta character '{c}'")));
                }
                c => Expr::Char(Predicate::Literal(c as u32)),
            };
            seq.push(self.closure(atom)?);
        }
        Ok(Expr::Sequence(seq))
    }
    // port: Pattern#group0
    fn group0(&mut self) -> Result<Expr, PatternSyntaxException> {
        let mut name = None;
        let capture = if self.peek() == Some('?') {
            self.cursor += 1;
            match self.read()? {
                ':' => false,
                '<' => {
                    let mut n = String::new();
                    while self.peek() != Some('>') {
                        n.push(self.read()?);
                    }
                    self.cursor += 1;
                    assert!(
                        n.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                            && n.bytes().all(|c| c.is_ascii_alphanumeric()),
                        "Invalid Java named group"
                    );
                    name = Some(n);
                    true
                }
                c => panic!("Unsupported Java regex group syntax: (?{c}"),
            }
        } else {
            true
        };
        let group = if capture {
            self.groups += 1;
            self.groups
        } else {
            0
        };
        if let Some(name) = name {
            assert!(
                self.named_groups.insert(name, group).is_none(),
                "Duplicate named group"
            );
        }
        let expr = self.expr()?;
        if self.peek() != Some(')') {
            return Err(self.error("Unclosed group"));
        }
        self.cursor += 1;
        Ok(if capture {
            Expr::Group(group, Box::new(expr))
        } else {
            expr
        })
    }
    // port: Pattern#closure
    fn closure(&mut self, expr: Expr) -> Result<Expr, PatternSyntaxException> {
        let (min, max) = match self.peek() {
            Some('*') => (0, 255),
            Some('+') => (1, 255),
            Some('?') => (0, 1),
            _ => return Ok(expr),
        };
        self.cursor += 1;
        let lazy = self.peek() == Some('?');
        if lazy {
            self.cursor += 1;
        }
        assert!(
            !matches!(self.peek(), Some('+' | '*' | '?' | '{')),
            "Unsupported Java regex quantifier"
        );
        Ok(Expr::Repeat(Box::new(expr), min, max, lazy))
    }
    // port: Pattern#escape
    fn escape(&mut self) -> Result<Predicate, PatternSyntaxException> {
        Ok(match self.read()? {
            's' => Predicate::Space,
            // CharPredicates.ASCII_SPACE().negate(): a plain (non-BMP) CharPredicate
            'S' => Predicate::Union(vec![Predicate::Space], true),
            'w' => Predicate::Word,
            'd' => Predicate::Digit,
            'n' => Predicate::Literal(10),
            'r' => Predicate::Literal(13),
            't' => Predicate::Literal(9),
            'f' => Predicate::Literal(12),
            'p' => {
                assert_eq!(self.read()?, '{');
                let mut name = String::new();
                while self.peek() != Some('}') {
                    name.push(self.read()?);
                }
                self.cursor += 1;
                Predicate::Category(match name.as_str() {
                    "L" => 1,
                    "Nl" => 2,
                    "Nd" => 4,
                    _ => panic!("Unsupported Java regex category: {name}"),
                })
            }
            'x' => {
                let a = self.read()?.to_digit(16).expect("Invalid hex escape");
                let b = self.read()?.to_digit(16).expect("Invalid hex escape");
                Predicate::Literal(a * 16 + b)
            }
            'u' => {
                let mut n = 0;
                for _ in 0..4 {
                    n = n * 16 + self.read()?.to_digit(16).expect("Invalid Unicode escape");
                }
                Predicate::Literal(n)
            }
            c if c.is_ascii_alphabetic() || c.is_ascii_digit() => {
                panic!("Unsupported Java regex escape: \\{c}")
            }
            c => Predicate::Literal(c as u32),
        })
    }
    // port: Pattern#clazz
    fn clazz(&mut self) -> Result<Predicate, PatternSyntaxException> {
        let negate = self.peek() == Some('^');
        if negate {
            self.cursor += 1;
        }
        let mut items = vec![];
        while self.peek() != Some(']') || items.is_empty() {
            if self.peek().is_none() {
                return Err(self.error("Unclosed character class"));
            }
            let a = self.class_atom()?;
            if self.peek() == Some('-') && self.chars.get(self.cursor + 1) != Some(&']') {
                self.cursor += 1;
                let b = self.class_atom()?;
                if let (Predicate::Literal(a), Predicate::Literal(b)) = (a, b) {
                    assert!(a <= b, "Invalid regex range");
                    items.push(Predicate::Range(a, b));
                } else {
                    panic!("Unsupported Java regex range");
                }
            } else {
                items.push(a);
            }
        }
        self.cursor += 1;
        Ok(Predicate::Union(items, negate))
    }
    // port: Pattern#range
    fn class_atom(&mut self) -> Result<Predicate, PatternSyntaxException> {
        Ok(match self.read()? {
            '\\' => self.escape()?,
            '[' | '&' => panic!("Unsupported nested/intersecting Java regex class"),
            c => Predicate::Literal(c as u32),
        })
    }
}
#[derive(Debug)]
enum Node {
    Accept,
    Char(Predicate, usize),
    Begin(usize),
    Dollar(usize),
    Bound(usize),
    Split(usize, usize),
    GroupHead(usize, usize),
    GroupTail(usize, usize),
    Loop {
        body: usize,
        next: usize,
        lazy: bool,
        local: usize,
    },
}
#[derive(Debug)]
struct Compiled {
    nodes: Vec<Node>,
    root: usize,
    groups: usize,
    named_groups: IndexMap<String, usize>,
    locals: usize,
    min_length: usize,
    anchored: bool,
    has_supplementary: bool,
}
impl Compiled {
    // port: Pattern#compile
    fn add(&mut self, node: Node) -> usize {
        let i = self.nodes.len();
        self.nodes.push(node);
        i
    }
    // port: Pattern#sequence (link nodes to their continuation)
    fn compile(&mut self, expr: Expr, next: usize) -> usize {
        match expr {
            Expr::Char(p) => self.add(Node::Char(p, next)),
            Expr::Begin => self.add(Node::Begin(next)),
            Expr::Dollar => self.add(Node::Dollar(next)),
            Expr::Bound => self.add(Node::Bound(next)),
            Expr::Sequence(v) => v
                .into_iter()
                .rev()
                .fold(next, |next, e| self.compile(e, next)),
            Expr::Branch(v) => {
                let mut iter = v.into_iter().rev();
                let mut first = self.compile(iter.next().unwrap(), next);
                for expr in iter {
                    let branch = self.compile(expr, next);
                    first = self.add(Node::Split(branch, first));
                }
                first
            }
            Expr::Group(group, expr) => {
                let tail = self.add(Node::GroupTail(group, next));
                let body = self.compile(*expr, tail);
                self.add(Node::GroupHead(group, body))
            }
            Expr::Repeat(expr, min, max, lazy) => {
                if max == 1 {
                    let body = self.compile(*expr, next);
                    return if lazy {
                        self.add(Node::Split(next, body))
                    } else {
                        self.add(Node::Split(body, next))
                    };
                }
                let local = self.locals;
                self.locals += 1;
                let root = self.add(Node::Accept);
                let body = self.compile(*expr, root);
                self.nodes[root] = Node::Loop {
                    body,
                    next,
                    lazy,
                    local,
                };
                if min == 1 { body } else { root }
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Pattern {
    compiled: Arc<Compiled>,
}
impl Pattern {
    // port: Pattern#compile(String)
    pub fn compile(regex: &str) -> Self {
        Self::try_compile(regex).unwrap_or_else(|e| panic!("{e}"))
    }
    /// `Pattern#compile(String)` with its `PatternSyntaxException` as a `Result` (callers that
    /// catch the exception). Syntax this subset does not support still panics.
    // port: Pattern#compile(String)
    pub fn try_compile(regex: &str) -> Result<Self, PatternSyntaxException> {
        let mut parser = Parser {
            chars: regex.chars().collect(),
            cursor: 0,
            groups: 0,
            named_groups: IndexMap::new(),
        };
        let expr = parser.expr()?;
        if parser.cursor != parser.chars.len() {
            parser.cursor += 1;
            return Err(parser.error("Unmatched closing ')'"));
        }
        let mut compiled = Compiled {
            nodes: vec![Node::Accept],
            root: 0,
            groups: parser.groups,
            named_groups: parser.named_groups,
            locals: 0,
            min_length: expr.min_length(),
            anchored: expr.anchored(),
            has_supplementary: expr.has_supplementary(),
        };
        compiled.root = compiled.compile(expr, 0);
        Ok(Self {
            compiled: Arc::new(compiled),
        })
    }
    // port: Pattern#matcher
    pub fn matcher(&self, input: impl Into<JsString>) -> Matcher {
        Matcher::new(self.compiled.clone(), input.into())
    }
}
#[derive(Clone)]
struct State {
    node: usize,
    pos: usize,
    groups: Vec<Option<usize>>,
    locals: Vec<Option<usize>>,
}
pub struct Matcher {
    compiled: Arc<Compiled>,
    input: JsString,
    from: usize,
    to: usize,
    first: Option<usize>,
    last: usize,
    groups: Vec<Option<usize>>,
    hit_end: bool,
}
impl Matcher {
    // port: Matcher#Matcher
    fn new(compiled: Arc<Compiled>, input: JsString) -> Self {
        let to = input.length();
        let groups = vec![None; (compiled.groups + 1) * 2];
        Self {
            compiled,
            input,
            from: 0,
            to,
            first: None,
            last: 0,
            groups,
            hit_end: false,
        }
    }
    // port: Matcher#reset(CharSequence)
    pub fn reset(&mut self, input: impl Into<JsString>) -> &mut Self {
        self.input = input.into();
        self.from = 0;
        self.to = self.input.length();
        self.first = None;
        self.last = 0;
        self.groups.fill(None);
        self.hit_end = false;
        self
    }
    // port: Matcher#region
    pub fn region(&mut self, start: usize, end: usize) -> &mut Self {
        assert!(
            start <= end && end <= self.input.length(),
            "Invalid matcher region"
        );
        self.first = None;
        self.last = 0;
        self.groups.fill(None);
        self.from = start;
        self.to = end;
        self
    }
    // port: Matcher#regionEnd
    pub fn region_end(&self) -> usize {
        self.to
    }
    // port: Matcher#matches
    pub fn matches(&mut self) -> bool {
        self.match_at(self.from, true)
    }
    // port: Matcher#lookingAt
    pub fn looking_at(&mut self) -> bool {
        self.match_at(self.from, false)
    }
    // port: Matcher#find
    pub fn find(&mut self) -> bool {
        let mut start = self.last;
        if self.first == Some(start) {
            start += 1;
        }
        start = start.max(self.from);
        if start > self.to {
            self.groups.fill(None);
            return false;
        }
        self.hit_end = false;
        if self.compiled.anchored {
            let ok = self.run(start, false);
            if !ok {
                self.first = None;
                self.groups.fill(None);
            }
            return ok;
        }
        while start <= self.to.saturating_sub(self.compiled.min_length)
            && self.compiled.min_length <= self.to
        {
            if self.run(start, false) {
                return true;
            }
            if start == self.to.saturating_sub(self.compiled.min_length) {
                break;
            }
            // port: Pattern.StartS#match
            let c = self.input.char_at(start);
            start += 1;
            if self.compiled.has_supplementary
                && (0xd800..=0xdbff).contains(&c)
                && start < self.input.length()
                && (0xdc00..=0xdfff).contains(&self.input.char_at(start))
            {
                start += 1;
            }
        }
        self.first = None;
        self.groups.fill(None);
        self.hit_end = true;
        false
    }
    // port: Matcher#match
    fn match_at(&mut self, start: usize, end_anchor: bool) -> bool {
        self.hit_end = false;
        self.groups.fill(None);
        let result = self.run(start, end_anchor);
        if !result {
            self.first = None;
        }
        result
    }
    // port: Pattern.Node#match, Pattern.Branch#match, Pattern.Loop#match
    fn run(&mut self, start: usize, end_anchor: bool) -> bool {
        let mut stack = vec![State {
            node: self.compiled.root,
            pos: start,
            groups: vec![None; (self.compiled.groups + 1) * 2],
            locals: vec![None; self.compiled.locals],
        }];
        while let Some(mut state) = stack.pop() {
            loop {
                match &self.compiled.nodes[state.node] {
                    Node::Accept => {
                        if end_anchor && state.pos != self.to {
                            break;
                        }
                        state.groups[0] = Some(start);
                        state.groups[1] = Some(state.pos);
                        self.groups = state.groups;
                        self.first = Some(start);
                        self.last = state.pos;
                        return true;
                    }
                    Node::Char(p, next) => {
                        if state.pos >= self.to {
                            self.hit_end = true;
                            break;
                        }
                        let cp = if p.is_bmp() {
                            self.input.char_at(state.pos) as u32
                        } else {
                            self.input.code_point_at(state.pos)
                        };
                        let width = if cp > 0xffff { 2 } else { 1 };
                        if state.pos + width > self.to {
                            self.hit_end = true;
                            break;
                        }
                        if !p.is(cp) {
                            break;
                        }
                        state.pos += width;
                        state.node = *next;
                    }
                    Node::Begin(next) => {
                        if state.pos != self.from {
                            break;
                        }
                        state.node = *next;
                    }
                    Node::Dollar(next) => {
                        let rest = &self.input.as_units()[state.pos..self.to];
                        if !rest.is_empty()
                            && rest != [13, 10]
                            && !(rest.len() == 1
                                && matches!(rest[0], 10 | 13 | 0x85 | 0x2028 | 0x2029)
                                && !(rest[0] == 10
                                    && state.pos > 0
                                    && self.input.char_at(state.pos - 1) == 13))
                        {
                            break;
                        }
                        self.hit_end = true;
                        state.node = *next;
                    }
                    Node::Bound(next) => {
                        let left = state.pos > self.from && self.bound_word_before(state.pos);
                        let right = state.pos < self.to && self.bound_word_at(state.pos);
                        if state.pos >= self.to {
                            self.hit_end = true;
                        }
                        if left == right {
                            break;
                        }
                        state.node = *next;
                    }
                    Node::Split(first, second) => {
                        let mut other = state.clone();
                        other.node = *second;
                        stack.push(other);
                        state.node = *first;
                    }
                    Node::GroupHead(group, next) => {
                        state.groups[group * 2] = Some(state.pos);
                        state.node = *next;
                    }
                    Node::GroupTail(group, next) => {
                        state.groups[group * 2 + 1] = Some(state.pos);
                        state.node = *next;
                    }
                    Node::Loop {
                        body,
                        next,
                        lazy,
                        local,
                    } => {
                        if state.locals[*local] == Some(state.pos) {
                            state.node = *next;
                            continue;
                        }
                        state.locals[*local] = Some(state.pos);
                        let mut other = state.clone();
                        other.node = if *lazy { *body } else { *next };
                        stack.push(other);
                        state.node = if *lazy { *next } else { *body };
                    }
                }
            }
        }
        false
    }
    // port: Pattern.Bound#check
    fn bound_word_before(&self, pos: usize) -> bool {
        let mut i = pos - 1;
        if (0xdc00..=0xdfff).contains(&self.input.char_at(i))
            && i > self.from
            && (0xd800..=0xdbff).contains(&self.input.char_at(i - 1))
        {
            i -= 1;
        }
        self.bound_word_at(i)
    }
    // port: Pattern.Bound#isWord
    fn bound_word_at(&self, i: usize) -> bool {
        let c = self.input.code_point_at(i);
        is_word(c) || category(c) & 8 != 0 && self.has_base_character(i)
    }
    // port: Pattern#hasBaseCharacter
    fn has_base_character(&self, i: usize) -> bool {
        for x in (self.from..=i).rev() {
            let c = self.input.code_point_at(x);
            let t = category(c);
            if t & 5 != 0 {
                return true;
            }
            if t & 8 != 0 {
                continue;
            }
            return false;
        }
        false
    }
    // port: Matcher#group(int)
    pub fn group(&self, group: usize) -> Option<String> {
        self.group_units(group).map(|s| s.to_string_lossy())
    }
    // port: Matcher#group(int) (UTF-16-preserving form)
    pub fn group_units(&self, group: usize) -> Option<JsString> {
        assert!(self.first.is_some(), "No match found");
        assert!(group <= self.compiled.groups, "No group {group}");
        match (self.groups[group * 2], self.groups[group * 2 + 1]) {
            (Some(a), Some(b)) => Some(self.input.substring(a, b)),
            _ => None,
        }
    }
    // port: Matcher#group(String)
    pub fn group_named(&self, name: &str) -> Option<String> {
        self.group(
            *self
                .compiled
                .named_groups
                .get(name)
                .expect("No group with that name"),
        )
    }
    // port: Matcher#group(String) (UTF-16-preserving form)
    pub fn group_named_units(&self, name: &str) -> Option<JsString> {
        self.group_units(
            *self
                .compiled
                .named_groups
                .get(name)
                .expect("No group with that name"),
        )
    }
    // port: Matcher#start
    pub fn start(&self) -> usize {
        self.first.expect("No match available")
    }
    // port: Matcher#end
    pub fn end(&self) -> usize {
        assert!(self.first.is_some(), "No match available");
        self.last
    }
    // port: Matcher#hitEnd
    pub fn hit_end(&self) -> bool {
        self.hit_end
    }
    // port: Matcher#groupCount
    pub fn group_count(&self) -> usize {
        self.compiled.groups
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // port: MatcherTest#depsPatternDifferential (JDK 21 generated fixtures)
    #[test]
    fn deps_pattern_differential() {
        let mut patterns = vec![];
        let mut inputs = vec![];
        let mut case_count = 0;
        for line in include_str!("testdata/regex.tsv").lines() {
            let fields: Vec<_> = line.split('\t').collect();
            match fields[0] {
                "p" => patterns.push(Pattern::compile(&decode(fields[2]).to_string_lossy())),
                "i" => inputs.push(decode(fields[2])),
                "m" => {
                    case_count += 1;
                    let p: usize = fields[1].parse().unwrap();
                    let i: usize = fields[2].parse().unwrap();
                    let mode: u8 = fields[3].parse().unwrap();
                    let mut m = patterns[p].matcher(inputs[i].clone());
                    let mut output = String::new();
                    let mut ok = match mode {
                        0 => m.matches(),
                        1 => m.looking_at(),
                        _ => m.find(),
                    };
                    while ok {
                        output.push_str(&format!("{},{}", m.start(), m.end()));
                        for g in 0..=m.group_count() {
                            output.push(',');
                            output.push_str(
                                &m.group_units(g)
                                    .map(|s| encode(&s))
                                    .unwrap_or_else(|| "-".into()),
                            );
                        }
                        output.push(';');
                        if mode != 2 {
                            break;
                        }
                        ok = m.find();
                    }
                    assert_eq!(output, fields[5], "pattern {p} input {i} mode {mode}");
                    assert_eq!(
                        m.hit_end().to_string(),
                        fields[4],
                        "hitEnd pattern {p} input {i} mode {mode}"
                    );
                }
                _ => panic!("Invalid fixture"),
            }
        }
        assert_eq!(patterns.len(), 16);
        assert_eq!(inputs.len(), 252);
        assert_eq!(case_count, 3096);
    }
    // port: String#charAt (fixture encoding)
    fn decode(s: &str) -> JsString {
        JsString::from_units(
            (0..s.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
                .collect::<Vec<_>>(),
        )
    }
    // port: String#charAt (fixture encoding)
    fn encode(s: &JsString) -> String {
        s.as_units().iter().map(|c| format!("{c:04x}")).collect()
    }
}
