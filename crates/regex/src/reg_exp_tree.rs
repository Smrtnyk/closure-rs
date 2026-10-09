/*
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/regex/RegExpTree.java.

//! Port of `com.google.javascript.jscomp.regex.RegExpTree`.
//!
//! An AST for JavaScript regular expressions. The Java abstract class and its nested subclasses
//! become the enum [`RegExpTree`] with one variant per concrete subclass; each variant wraps a
//! struct of the same name whose `impl` holds that subclass's methods.

use std::fmt;
use std::sync::LazyLock;

use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::java_lang;
use closure_rhino::js_string::JsString;

use crate::case_canonicalize;
use crate::char_ranges::{self, CharRanges};

/// The Java exceptions `parseRegExp` lets escape and that its callers (`CheckRegExp`,
/// `ReportUntranspilableFeatures`) catch as `IllegalArgumentException | IndexOutOfBoundsException`
/// and report with `getMessage()`. Each variant is the exact Java runtime class; the payload is the
/// message (UTF-16, since messages embed parts of the pattern).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegExpException {
    IllegalArgumentException(Option<JsString>),
    IndexOutOfBoundsException(Option<JsString>),
    StringIndexOutOfBoundsException(JsString),
}

impl RegExpException {
    /// `Throwable.getMessage()`.
    // port: Throwable#getMessage
    pub fn get_message(&self) -> Option<&JsString> {
        match self {
            RegExpException::IllegalArgumentException(m)
            | RegExpException::IndexOutOfBoundsException(m) => m.as_ref(),
            RegExpException::StringIndexOutOfBoundsException(m) => Some(m),
        }
    }

    /// `getClass().getSimpleName()`.
    // port: Class#getSimpleName
    pub fn class_name(&self) -> &'static str {
        match self {
            RegExpException::IllegalArgumentException(_) => "IllegalArgumentException",
            RegExpException::IndexOutOfBoundsException(_) => "IndexOutOfBoundsException",
            RegExpException::StringIndexOutOfBoundsException(_) => {
                "StringIndexOutOfBoundsException"
            }
        }
    }
}

/// `new IllegalArgumentException(prefix + rest)`.
fn illegal_argument(prefix: &str, rest: &JsString) -> RegExpException {
    RegExpException::IllegalArgumentException(Some(JsString::from(prefix).concat(rest)))
}

const fn ch(b: u8) -> u16 {
    b as u16
}

/// Maps a code unit to a byte for matching against ASCII literals: non-ASCII code units map to
/// 0x80, which equals no ASCII literal, so they reach the `default` arm exactly as in Java.
fn ascii(c: u16) -> u8 {
    if c < 0x80 { c as u8 } else { 0x80 }
}

/// Appends a Java string literal (ASCII) to a UTF-16 buffer, like `StringBuilder.append(String)`.
// port: StringBuilder#append(String)
fn append_str(sb: &mut Vec<u16>, s: &str) {
    sb.extend(s.encode_utf16());
}

/// `StringBuilder.append(int)`.
// port: StringBuilder#append(int)
fn append_int(sb: &mut Vec<u16>, i: i32) {
    append_str(sb, &i.to_string());
}

/// A regular expression tree node.
// port: RegExpTree#equals (each variant compares as its Java class does; different classes never
// compare equal, like `instanceof` in the Java equals methods)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegExpTree {
    Empty(Empty),
    Anchor(Anchor),
    WordBoundary(WordBoundary),
    BackReference(BackReference),
    NamedBackReference(NamedBackReference),
    Text(Text),
    Repetition(Repetition),
    Alternation(Alternation),
    LookaheadAssertion(LookaheadAssertion),
    LookbehindAssertion(LookbehindAssertion),
    CapturingGroup(CapturingGroup),
    NamedCaptureGroup(NamedCaptureGroup),
    UnicodePropertyEscape(UnicodePropertyEscape),
    Charset(Charset),
    Concatenation(Concatenation),
}

impl RegExpTree {
    /// Returns a simpler regular expression that is semantically the same assuming the given
    /// flags.
    ///
    /// `flags`: Regular expression flags, e.g. `"igm"`.
    // port: RegExpTree#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        match self {
            RegExpTree::Empty(t) => t.simplify(flags),
            RegExpTree::Anchor(t) => t.simplify(flags),
            RegExpTree::WordBoundary(t) => t.simplify(flags),
            RegExpTree::BackReference(t) => t.simplify(flags),
            RegExpTree::NamedBackReference(t) => t.simplify(flags),
            RegExpTree::Text(t) => t.simplify(flags),
            RegExpTree::Repetition(t) => t.simplify(flags),
            RegExpTree::Alternation(t) => t.simplify(flags),
            RegExpTree::LookaheadAssertion(t) => t.simplify(flags),
            RegExpTree::LookbehindAssertion(t) => t.simplify(flags),
            RegExpTree::CapturingGroup(t) => t.simplify(flags),
            RegExpTree::NamedCaptureGroup(t) => t.simplify(flags),
            RegExpTree::UnicodePropertyEscape(t) => t.simplify(flags),
            RegExpTree::Charset(t) => t.simplify(flags),
            RegExpTree::Concatenation(t) => t.simplify(flags),
        }
    }

    /// True if the presence or absence of an `"i"` flag would change the meaning of this regular
    /// expression.
    // port: RegExpTree#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        match self {
            RegExpTree::Empty(_)
            | RegExpTree::Anchor(_)
            | RegExpTree::WordBoundary(_)
            | RegExpTree::BackReference(_)
            | RegExpTree::NamedBackReference(_)
            | RegExpTree::UnicodePropertyEscape(_) => RegExpTreeAtom::is_case_sensitive(),
            RegExpTree::Text(t) => t.is_case_sensitive(),
            RegExpTree::Repetition(t) => t.is_case_sensitive(),
            RegExpTree::Alternation(t) => t.is_case_sensitive(),
            RegExpTree::LookaheadAssertion(t) => t.is_case_sensitive(),
            RegExpTree::LookbehindAssertion(t) => t.is_case_sensitive(),
            RegExpTree::CapturingGroup(t) => t.is_case_sensitive(),
            RegExpTree::NamedCaptureGroup(t) => t.is_case_sensitive(),
            RegExpTree::Charset(t) => t.is_case_sensitive(),
            RegExpTree::Concatenation(t) => t.is_case_sensitive(),
        }
    }

    /// True if the regular expression contains an anchor : `^` or `$`.
    // port: RegExpTree#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        match self {
            RegExpTree::Empty(_)
            | RegExpTree::WordBoundary(_)
            | RegExpTree::BackReference(_)
            | RegExpTree::NamedBackReference(_)
            | RegExpTree::Text(_)
            | RegExpTree::UnicodePropertyEscape(_)
            | RegExpTree::Charset(_) => RegExpTreeAtom::contains_anchor(),
            RegExpTree::Anchor(t) => t.contains_anchor(),
            RegExpTree::Repetition(t) => t.contains_anchor(),
            RegExpTree::Alternation(t) => t.contains_anchor(),
            RegExpTree::LookaheadAssertion(t) => t.contains_anchor(),
            RegExpTree::LookbehindAssertion(t) => t.contains_anchor(),
            RegExpTree::CapturingGroup(t) => t.contains_anchor(),
            RegExpTree::NamedCaptureGroup(t) => t.contains_anchor(),
            RegExpTree::Concatenation(t) => t.contains_anchor(),
        }
    }

    /// True if the regular expression contains capturing groups.
    // port: RegExpTree#hasCapturingGroup
    pub fn has_capturing_group(&self) -> bool {
        self.num_capturing_groups() != 0
    }

    /// The number of capturing groups.
    // port: RegExpTree#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        match self {
            RegExpTree::Empty(_)
            | RegExpTree::Anchor(_)
            | RegExpTree::WordBoundary(_)
            | RegExpTree::BackReference(_)
            | RegExpTree::NamedBackReference(_)
            | RegExpTree::Text(_)
            | RegExpTree::UnicodePropertyEscape(_)
            | RegExpTree::Charset(_) => RegExpTreeAtom::num_capturing_groups(),
            RegExpTree::Repetition(t) => t.num_capturing_groups(),
            RegExpTree::Alternation(t) => t.num_capturing_groups(),
            RegExpTree::LookaheadAssertion(t) => t.num_capturing_groups(),
            RegExpTree::LookbehindAssertion(t) => t.num_capturing_groups(),
            RegExpTree::CapturingGroup(t) => t.num_capturing_groups(),
            RegExpTree::NamedCaptureGroup(t) => t.num_capturing_groups(),
            RegExpTree::Concatenation(t) => t.num_capturing_groups(),
        }
    }

    /// The children of this node.
    // port: RegExpTree#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        match self {
            RegExpTree::Empty(_)
            | RegExpTree::Anchor(_)
            | RegExpTree::WordBoundary(_)
            | RegExpTree::BackReference(_)
            | RegExpTree::NamedBackReference(_)
            | RegExpTree::Text(_)
            | RegExpTree::UnicodePropertyEscape(_)
            | RegExpTree::Charset(_) => RegExpTreeAtom::children(),
            RegExpTree::Repetition(t) => t.children(),
            RegExpTree::Alternation(t) => t.children(),
            RegExpTree::LookaheadAssertion(t) => t.children(),
            RegExpTree::LookbehindAssertion(t) => t.children(),
            RegExpTree::CapturingGroup(t) => t.children(),
            RegExpTree::NamedCaptureGroup(t) => t.children(),
            RegExpTree::Concatenation(t) => t.children(),
        }
    }

    /// Appends this regular expression source to the given buffer.
    // port: RegExpTree#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        match self {
            RegExpTree::Empty(t) => t.append_source_code(sb),
            RegExpTree::Anchor(t) => t.append_source_code(sb),
            RegExpTree::WordBoundary(t) => t.append_source_code(sb),
            RegExpTree::BackReference(t) => t.append_source_code(sb),
            RegExpTree::NamedBackReference(t) => t.append_source_code(sb),
            RegExpTree::Text(t) => t.append_source_code(sb),
            RegExpTree::Repetition(t) => t.append_source_code(sb),
            RegExpTree::Alternation(t) => t.append_source_code(sb),
            RegExpTree::LookaheadAssertion(t) => t.append_source_code(sb),
            RegExpTree::LookbehindAssertion(t) => t.append_source_code(sb),
            RegExpTree::CapturingGroup(t) => t.append_source_code(sb),
            RegExpTree::NamedCaptureGroup(t) => t.append_source_code(sb),
            RegExpTree::UnicodePropertyEscape(t) => t.append_source_code(sb),
            RegExpTree::Charset(t) => t.append_source_code(sb),
            RegExpTree::Concatenation(t) => t.append_source_code(sb),
        }
    }

    // port: RegExpTree#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        match self {
            RegExpTree::Empty(t) => t.append_debug_info(sb),
            RegExpTree::Anchor(t) => t.append_debug_info(sb),
            RegExpTree::WordBoundary(t) => t.append_debug_info(sb),
            RegExpTree::BackReference(t) => t.append_debug_info(sb),
            RegExpTree::NamedBackReference(t) => t.append_debug_info(sb),
            RegExpTree::Text(t) => t.append_debug_info(sb),
            RegExpTree::Repetition(t) => t.append_debug_info(sb),
            RegExpTree::Alternation(t) => t.append_debug_info(sb),
            RegExpTree::LookaheadAssertion(t) => t.append_debug_info(sb),
            RegExpTree::LookbehindAssertion(t) => t.append_debug_info(sb),
            RegExpTree::CapturingGroup(t) => t.append_debug_info(sb),
            RegExpTree::NamedCaptureGroup(t) => t.append_debug_info(sb),
            RegExpTree::UnicodePropertyEscape(t) => t.append_debug_info(sb),
            RegExpTree::Charset(t) => t.append_debug_info(sb),
            RegExpTree::Concatenation(t) => t.append_debug_info(sb),
        }
    }

    /// Java's `toString()`: the regular expression literal, e.g. `/foo/`. A `JsString` because
    /// the source may carry lone surrogates.
    // port: RegExpTree#toString
    pub fn to_js_string(&self) -> JsString {
        let mut sb: Vec<u16> = Vec::new();
        sb.push(ch(b'/'));
        self.append_source_code(&mut sb);
        // Don't emit a regular expression that looks like a line comment start.
        if sb.len() == 1 {
            append_str(&mut sb, "(?:)");
        }
        sb.push(ch(b'/'));
        JsString::from_units(sb)
    }

    // port: RegExpTree#hashCode
    pub fn hash_code(&self) -> i32 {
        match self {
            RegExpTree::Empty(t) => t.hash_code(),
            RegExpTree::Anchor(t) => t.hash_code(),
            RegExpTree::WordBoundary(t) => t.hash_code(),
            RegExpTree::BackReference(t) => t.hash_code(),
            RegExpTree::NamedBackReference(t) => t.hash_code(),
            RegExpTree::Text(t) => t.hash_code(),
            RegExpTree::Repetition(t) => t.hash_code(),
            RegExpTree::Alternation(t) => t.hash_code(),
            RegExpTree::LookaheadAssertion(t) => t.hash_code(),
            RegExpTree::LookbehindAssertion(t) => t.hash_code(),
            RegExpTree::CapturingGroup(t) => t.hash_code(),
            RegExpTree::NamedCaptureGroup(t) => t.hash_code(),
            RegExpTree::UnicodePropertyEscape(t) => t.hash_code(),
            RegExpTree::Charset(t) => t.hash_code(),
            RegExpTree::Concatenation(t) => t.hash_code(),
        }
    }

    /// Parses a regular expression to an AST.
    ///
    /// `pattern`: The `foo` From `/foo/i`. `flags`: The `i` From `/foo/i`.
    // port: RegExpTree#parseRegExp
    pub fn parse_reg_exp(
        pattern: &JsString,
        flags: &JsString,
    ) -> Result<RegExpTree, RegExpException> {
        Parser {
            pattern,
            flags,
            pos: 0,
            num_capturing_groups: 0,
            capturing_group_names: IndexSet::<_>::default(),
            limit: pattern.length(),
            look_for_named_capture_backreferences: false,
        }
        .parse_top_level()
    }

    /// True if, but not necessarily always when the, given regular expression must match the
    /// whole input or none of it.
    // port: RegExpTree#matchesWholeInput
    pub fn matches_whole_input(t: &RegExpTree, flags: &JsString) -> bool {
        if flags.index_of_char(ch(b'm')) >= 0 {
            return false;
        }

        let RegExpTree::Concatenation(c) = t else {
            return false;
        };

        if c.elements.is_empty() {
            return false;
        }
        let first = &c.elements[0];
        let last = c.elements.last().unwrap();
        let (RegExpTree::Anchor(first_anchor), RegExpTree::Anchor(last_anchor)) = (first, last)
        else {
            return false;
        };
        first_anchor.r#type == ch(b'^') && last_anchor.r#type == ch(b'$')
    }
}

// port: RegExpTree#toString
impl fmt::Display for RegExpTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_js_string().to_string_lossy())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParentheticalType {
    CAPTURING,
    NONCAPTURING,
    POSITIVE_LOOKAHEAD,
    NEGATIVE_LOOKAHEAD,
    POSITIVE_LOOKBEHIND,
    NEGATIVE_LOOKBEHIND,
    NAMED_GROUPS,
}

/// A recursive descent parser that closes over pattern and flags above.
struct Parser<'a> {
    pattern: &'a JsString,
    flags: &'a JsString,

    /// The number of characters in pattern consumed.
    pos: usize,

    /// The number of capturing groups seen so far.
    num_capturing_groups: i32,

    /// The names of capturing groups in the regex expression
    capturing_group_names: IndexSet<JsString>,

    /// The length of pattern.
    limit: usize,

    /// Boolean indicating whether we should look for named capture group backreferences
    look_for_named_capture_backreferences: bool,
}

impl Parser<'_> {
    /// `pattern.charAt(i)`, including its `StringIndexOutOfBoundsException`.
    // port: String#charAt
    fn char_at(&self, i: usize) -> Result<u16, RegExpException> {
        if i < self.limit {
            Ok(self.pattern.char_at(i))
        } else {
            Err(RegExpException::StringIndexOutOfBoundsException(
                JsString::from(format!("Index {i} out of bounds for length {}", self.limit)),
            ))
        }
    }

    /// `pattern.substring(begin, end)`, including its `StringIndexOutOfBoundsException`.
    // port: String#substring(int, int)
    fn substring(&self, begin: usize, end: usize) -> Result<JsString, RegExpException> {
        if begin <= end && end <= self.limit {
            Ok(self.pattern.substring(begin, end))
        } else {
            Err(RegExpException::StringIndexOutOfBoundsException(
                JsString::from(format!(
                    "Range [{begin}, {end}) out of bounds for length {}",
                    self.limit
                )),
            ))
        }
    }

    // port: String#substring(int)
    fn substring_from(&self, begin: usize) -> Result<JsString, RegExpException> {
        self.substring(begin, self.limit)
    }

    // port: RegExpTree#parseRegExp.Parser#parseTopLevel
    fn parse_top_level(&mut self) -> Result<RegExpTree, RegExpException> {
        // First assume there are no named capture backreferences, because the spec says they
        // should only be recognized if the pattern contains at least one named capture group.
        self.pos = 0;
        self.num_capturing_groups = 0;
        self.look_for_named_capture_backreferences = false;
        let mut out = self.parse()?;

        // If there were named capture groups, we must parse the pattern string again so we can
        // check for any backreferences to them.
        if !self.capturing_group_names.is_empty() {
            self.pos = 0;
            self.num_capturing_groups = 0;
            self.look_for_named_capture_backreferences = true;
            out = self.parse()?;
        }

        if self.pos < self.limit {
            // Unmatched closed paren maybe.
            return Err(RegExpException::IllegalArgumentException(Some(
                self.substring_from(self.pos)?,
            )));
        }
        Ok(out)
    }

    // port: RegExpTree#parseRegExp.Parser#parse
    fn parse(&mut self) -> Result<RegExpTree, RegExpException> {
        // Collects ["foo", "bar", "baz"] for /foo|bar|baz/.
        let mut alternatives: Option<Vec<RegExpTree>> = None;
        // The last item parsed within an alternation.
        let mut preceder: Option<RegExpTree> = None;

        'top_loop: while self.pos < self.limit {
            let c = self.char_at(self.pos)?;
            let mut atom: RegExpTree;
            match ascii(c) {
                b'[' => atom = self.parse_charset()?,
                b'(' => atom = self.parse_parenthetical()?,
                b')' => {
                    break 'top_loop;
                }
                b'\\' => atom = self.parse_escape()?,
                b'^' | b'$' => {
                    atom = RegExpTree::Anchor(Anchor::new(c));
                    self.pos += 1;
                }
                b'.' => {
                    // We represent . as a character set to make it easy to simplify things like
                    // /.|[\r\n]/.
                    atom = RegExpTree::Charset(DOT_CHARSET.clone());
                    self.pos += 1;
                }
                b'|' => {
                    // An alternative may be empty as in /foo||bar/.
                    // The '|' is consumed below.
                    atom = empty_instance();
                }
                _ => {
                    // Find a run of concatenated characters to avoid building a tree node per
                    // literal character.
                    let start = self.pos;
                    let mut end = self.pos + 1;
                    'chars_loop: while end < self.limit {
                        match ascii(self.char_at(end)?) {
                            b'[' | b'(' | b')' | b'\\' | b'^' | b'$' | b'|' | b'.' | b'*'
                            | b'+' | b'?' | b'{' => break 'chars_loop,
                            _ => {
                                // Repetition binds more tightly than concatenation.
                                // Only consume up to "foo" in /foob*/ so that the suffix operator
                                // parser below has the right precedence.
                                if end + 1 >= self.limit
                                    || !is_repetition_start(self.char_at(end + 1)?)
                                {
                                    end += 1;
                                } else {
                                    break 'chars_loop;
                                }
                            }
                        }
                    }
                    atom = RegExpTree::Text(Text::new(self.substring(start, end)?));
                    self.pos = end;
                }
            }
            if self.pos < self.limit && is_repetition_start(self.char_at(self.pos)?) {
                atom = self.parse_repetition(atom)?;
            }
            preceder = Some(match preceder.take() {
                None => atom,
                Some(p) => RegExpTree::Concatenation(Concatenation::new2(p, atom)),
            });
            // If this is an alternative in a alternation, then add it to the list of complete
            // alternatives, and reset the parser state for the next alternative.
            if self.pos < self.limit && self.char_at(self.pos)? == ch(b'|') {
                alternatives
                    .get_or_insert_with(Vec::new)
                    .push(preceder.take().unwrap());
                self.pos += 1;
            }
        }
        // An alternative may have no parsed content blank as in /foo|/.
        let preceder = preceder.unwrap_or_else(empty_instance);
        if let Some(mut alternatives) = alternatives {
            alternatives.push(preceder);
            Ok(RegExpTree::Alternation(Alternation::new(alternatives)))
        } else {
            Ok(preceder)
        }
    }

    /// Handles capturing groups `(...)`, non-capturing groups `(?:...)`, and lookahead
    /// assertions `(?=...)`.
    // port: RegExpTree#parseRegExp.Parser#parseParenthetical
    fn parse_parenthetical(&mut self) -> Result<RegExpTree, RegExpException> {
        check_state!(self.char_at(self.pos)? == ch(b'('));
        let start = self.pos;
        self.pos += 1;
        let r#type: ParentheticalType;
        let mut capture_name: Option<JsString> = None;
        if self.pos < self.limit && self.char_at(self.pos)? == ch(b'?') {
            if self.pos + 1 < self.limit {
                let c = self.char_at(self.pos + 1)?;
                match ascii(c) {
                    b':' => {
                        // (?:...) Non-capturing groups.
                        self.pos += 2;
                        r#type = ParentheticalType::NONCAPTURING;
                    }
                    // (?=...) and (?!...) Lookahead Assertions
                    b'=' => {
                        self.pos += 2;
                        r#type = ParentheticalType::POSITIVE_LOOKAHEAD;
                    }
                    b'!' => {
                        self.pos += 2;
                        r#type = ParentheticalType::NEGATIVE_LOOKAHEAD;
                    }
                    // (?<=...) and (?<!...) Lookbehind Assertions, (?<name>) named groups
                    b'<' => {
                        if self.pos + 2 < self.limit && self.char_at(self.pos + 2)? == ch(b'=') {
                            self.pos += 3;
                            r#type = ParentheticalType::POSITIVE_LOOKBEHIND;
                        } else if self.pos + 2 < self.limit
                            && self.char_at(self.pos + 2)? == ch(b'!')
                        {
                            self.pos += 3;
                            r#type = ParentheticalType::NEGATIVE_LOOKBEHIND;
                        } else {
                            self.pos += 2;
                            let name = self.scan_named_group_name()?;
                            self.capturing_group_names.insert(name.clone());
                            capture_name = Some(name);
                            r#type = ParentheticalType::NAMED_GROUPS;
                        }
                    }
                    _ => {
                        return Err(illegal_argument(
                            "Malformed parenthetical: ",
                            &self.substring_from(start)?,
                        ));
                    }
                }
            } else {
                return Err(illegal_argument(
                    "Malformed parenthetical: ",
                    &self.substring_from(start)?,
                ));
            }
        } else {
            r#type = ParentheticalType::CAPTURING;
        }
        let body = self.parse()?;
        if self.pos < self.limit && self.char_at(self.pos)? == ch(b')') {
            self.pos += 1;
        } else {
            return Err(illegal_argument(
                "Unclosed parenthetical group: ",
                &self.substring_from(start)?,
            ));
        }
        match r#type {
            ParentheticalType::CAPTURING => {
                self.num_capturing_groups += 1;
                Ok(RegExpTree::CapturingGroup(CapturingGroup::new(body)))
            }
            ParentheticalType::NONCAPTURING => Ok(body),
            ParentheticalType::POSITIVE_LOOKAHEAD => Ok(RegExpTree::LookaheadAssertion(
                LookaheadAssertion::new(body, true),
            )),
            ParentheticalType::NEGATIVE_LOOKAHEAD => Ok(RegExpTree::LookaheadAssertion(
                LookaheadAssertion::new(body, false),
            )),
            ParentheticalType::POSITIVE_LOOKBEHIND => Ok(RegExpTree::LookbehindAssertion(
                LookbehindAssertion::new(body, true),
            )),
            ParentheticalType::NEGATIVE_LOOKBEHIND => Ok(RegExpTree::LookbehindAssertion(
                LookbehindAssertion::new(body, false),
            )),
            ParentheticalType::NAMED_GROUPS => {
                if let Some(capture_name) = capture_name {
                    self.num_capturing_groups += 1;
                    Ok(RegExpTree::NamedCaptureGroup(NamedCaptureGroup::new(
                        body,
                        capture_name,
                    )))
                } else {
                    Err(illegal_argument(
                        "Malformed named capture group: ",
                        &self.substring_from(start)?,
                    ))
                }
            }
        }
    }

    /// Helper that scans the pattern for a named group name. Assumes that `pos` points to the
    /// character after '<'
    ///
    /// Returns the group name
    // port: RegExpTree#parseRegExp.Parser#scanNamedGroupName
    fn scan_named_group_name(&mut self) -> Result<JsString, RegExpException> {
        let start = self.pos;
        let mut end = self.pos;
        if !is_identifier_start(self.char_at(start)?) {
            return Err(illegal_argument(
                "Invalid capture group name: <",
                &self.substring_from(start)?,
            ));
        }
        end += 1;
        while end < self.limit {
            if self.char_at(end)? == ch(b'>') {
                self.pos = end + 1;
                return self.substring(start, end);
            } else if is_identifier_part(self.char_at(end)?) {
                end += 1;
            } else {
                return Err(illegal_argument(
                    "Invalid capture group name: <",
                    &self.substring_from(start)?,
                ));
            }
        }
        Err(illegal_argument(
            "Malformed named capture group: <",
            &self.substring_from(start)?,
        ))
    }

    /// Parses a square bracketed character set. Standalone character groups `/\d/` are handled by
    /// `parseEscape`.
    // port: RegExpTree#parseRegExp.Parser#parseCharset
    fn parse_charset(&mut self) -> Result<RegExpTree, RegExpException> {
        check_state!(self.char_at(self.pos)? == ch(b'['));
        self.pos += 1;

        let is_case_insensitive = self.flags.index_of_char(ch(b'i')) >= 0;
        let inverse = self.pos < self.limit && self.char_at(self.pos)? == ch(b'^');
        if inverse {
            self.pos += 1;
        }
        let mut ranges = char_ranges::EMPTY.clone();
        let mut ie_explicits = char_ranges::EMPTY.clone();
        while self.pos < self.limit && self.char_at(self.pos)? != ch(b']') {
            let mut c = self.char_at(self.pos)?;
            let start: i32;
            if c == ch(b'\\') {
                self.pos += 1;
                let possible_group_name = self.char_at(self.pos)?;
                let group = named_char_groups_get(possible_group_name);
                if let Some(group) = group {
                    self.pos += 1;
                    ranges = ranges.union(group);
                    continue;
                }
                start = self.parse_escape_char()?;
            } else {
                start = c as i32;
                self.pos += 1;
            }
            let mut end = start;
            if self.pos + 1 < self.limit
                && self.char_at(self.pos)? == ch(b'-')
                && self.char_at(self.pos + 1)? != ch(b']')
            {
                self.pos += 1;
                c = self.char_at(self.pos)?;
                if c == ch(b'\\') {
                    self.pos += 1;
                    end = self.parse_escape_char()?;
                } else {
                    end = c as i32;
                    self.pos += 1;
                }
            }
            let range = CharRanges::inclusive(start, end).map_err(|e| {
                RegExpException::IndexOutOfBoundsException(e.message.map(JsString::from))
            })?;
            ranges = ranges.union(&range);
            if IE_SPEC_ERRORS.contains(start) && IE_SPEC_ERRORS.contains(end) {
                ie_explicits = ie_explicits.union(&range.intersection(&IE_SPEC_ERRORS));
            }
            if is_case_insensitive {
                // If the flags contain the 'i' flag, then it is not correct to say that [^a-z]
                // contains the letter 'A', or that [a-z] does not contain the letter 'A'.
                // We expand out letter groups here so that parse returns something that is valid
                // independent of flags.
                // Calls to simplify(flags) may later reintroduce flag assumptions.
                // but without this step, later steps might conflate
                //     /[a-z]/i
                // and
                //     /[^\0-`{-￿]/i
                // which matches nothing because the information about whether the ^ is present
                // has been lost during optimizations and charset unionizing as in /[...]|[^...]/.
                ranges = case_canonicalize::expand_to_all_matched(&ranges);
            }
        }
        self.pos += 1; // Consume ']'

        if inverse {
            ranges = char_ranges::ALL_CODE_UNITS.difference(&ranges);
        }

        Ok(RegExpTree::Charset(Charset::new(ranges, ie_explicits)))
    }

    /// Parses an escape to a code point. Some of the characters parsed here have special meanings
    /// in various contexts, so contexts must filter those instead. E.g. '\b' means a different
    /// thing inside a charset than without.
    // port: RegExpTree#parseRegExp.Parser#parseEscapeChar
    fn parse_escape_char(&mut self) -> Result<i32, RegExpException> {
        let mut c = self.char_at(self.pos)?;
        self.pos += 1;
        Ok(match ascii(c) {
            b'b' => 0x08,
            b'f' => 0x0c,
            b'n' => 0x0a,
            b'r' => 0x0d,
            b't' => 0x09,
            b'u' => {
                if self.flags.index_of_char(ch(b'u')) >= 0
                    && self.pos < self.limit
                    && self.char_at(self.pos)? == ch(b'{')
                {
                    self.parse_braced_unicode_escape()?
                } else {
                    self.parse_hex(4)?
                }
            }
            b'v' => 0x0b,
            b'x' => self.parse_hex(2)?,
            _ => {
                if ch(b'0') <= c && c <= ch(b'7') {
                    let mut code_unit: u16 = c - ch(b'0');
                    // Allow octal literals in the range \0-\377.
                    // \41 might be a group, but \041 is not a group.
                    // We read, but do not emit octal literals since they are deprecated in ES5.
                    let oct_limit = self.limit.min(
                        self.pos
                            + (if c <= ch(b'3') { 2 } else { 1 })
                            + (if c == ch(b'0') { 1 } else { 0 }),
                    );
                    while self.pos < oct_limit {
                        c = self.char_at(self.pos)?;
                        if ch(b'0') <= c && c <= ch(b'7') {
                            code_unit = (((code_unit as i32) << 3) + (c - ch(b'0')) as i32) as u16;
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    code_unit as i32
                } else {
                    c as i32
                }
            }
        })
    }

    /// Parses an escape that appears outside a charset.
    // port: RegExpTree#parseRegExp.Parser#parseEscape
    fn parse_escape(&mut self) -> Result<RegExpTree, RegExpException> {
        check_state!(self.char_at(self.pos)? == ch(b'\\'));
        let start = self.pos;
        self.pos += 1;
        let mut c = self.char_at(self.pos)?;
        if c == ch(b'b') || c == ch(b'B') {
            self.pos += 1;
            Ok(RegExpTree::WordBoundary(WordBoundary::new(c)))
        } else if (c == ch(b'p') || c == ch(b'P')) && self.flags.index_of_char(ch(b'u')) >= 0 {
            // handle ES2018 unicode property tests, e.g.
            // /\p{ASCII_Hex_Digit=true}/ only hex digits
            // /\P{Script=Greek}/  no greek letters
            let negated = c == ch(b'P');
            self.pos += 1;
            if self.pos < self.limit && self.char_at(self.pos)? == ch(b'{') {
                let mut lhs: Vec<u16> = Vec::new();
                loop {
                    self.pos += 1;
                    if self.pos >= self.limit {
                        break;
                    }
                    c = self.char_at(self.pos)?;
                    if !is_property_char(c) {
                        break;
                    }
                    lhs.push(c);
                }
                if self.pos < self.limit && c == ch(b'}') {
                    // Case of shorthand like /\p{ASCII_Hex_Digit}/u
                    self.pos += 1;
                    Ok(RegExpTree::UnicodePropertyEscape(
                        UnicodePropertyEscape::new(None, JsString::from_units(lhs), negated)?,
                    ))
                } else if self.pos < self.limit && c == ch(b'=') {
                    // Case of having '=' like /\p{Script=Greek}/u
                    let mut rhs: Vec<u16> = Vec::new();
                    loop {
                        self.pos += 1;
                        if self.pos >= self.limit {
                            break;
                        }
                        c = self.char_at(self.pos)?;
                        if !is_property_char(c) {
                            break;
                        }
                        rhs.push(c);
                    }
                    if self.pos < self.limit && c == ch(b'}') {
                        self.pos += 1;
                        Ok(RegExpTree::UnicodePropertyEscape(
                            UnicodePropertyEscape::new(
                                Some(JsString::from_units(lhs)),
                                JsString::from_units(rhs),
                                negated,
                            )?,
                        ))
                    } else {
                        Err(illegal_argument(
                            "Malformed Unicode Property Escape: expected '}' after ",
                            &self.substring(start, self.pos)?,
                        ))
                    }
                } else {
                    Err(illegal_argument(
                        "Malformed Unicode Property Escape: expected '=' or '}' after ",
                        &self.substring(start, self.pos)?,
                    ))
                }
            } else {
                Err(illegal_argument(
                    "Malformed Unicode Property Escape: expected '{' after ",
                    &self.substring(start, self.pos)?,
                ))
            }
        } else if ch(b'1') <= c && c <= ch(b'9') {
            self.pos += 1;
            let mut possible_group_index = (c - ch(b'0')) as i32;
            if self.num_capturing_groups >= possible_group_index {
                if self.pos < self.limit {
                    let next = self.char_at(self.pos)?;
                    if ch(b'0') <= next && next <= ch(b'9') {
                        let two_digit_group_index =
                            possible_group_index * 10 + (next - ch(b'0')) as i32;
                        if self.num_capturing_groups >= two_digit_group_index {
                            self.pos += 1;
                            possible_group_index = two_digit_group_index;
                        }
                    }
                }
                Ok(RegExpTree::BackReference(BackReference::new(
                    possible_group_index,
                )?))
            } else {
                // \1 - \7 are octal escapes if there is no such group.
                // \8 and \9 are the literal characters '8' and '9' if there is no such group.
                Ok(RegExpTree::Text(Text::new(JsString::from_units(vec![
                    if possible_group_index <= 7 {
                        possible_group_index as u16
                    } else {
                        c
                    },
                ]))))
            }
        } else if self.look_for_named_capture_backreferences
            && c == ch(b'k')
            && self.pos + 1 < self.limit
            && self.char_at(self.pos + 1)? == ch(b'<')
            // According to the spec
            // https://github.com/tc39/proposal-regexp-named-groups#backwards-compatibility-of-new-syntax
            // we want to treat \k as a normal string if there are no named capturing groups
            // present
            && !self.capturing_group_names.is_empty()
        {
            self.pos += 2;
            let potential_name = self.scan_named_group_name()?;
            if !self.capturing_group_names.contains(&potential_name) {
                return Err(illegal_argument(
                    "Invalid named capture referenced: ",
                    &self.substring_from(start)?,
                ));
            }
            Ok(RegExpTree::NamedBackReference(NamedBackReference::new(
                potential_name,
            )))
        } else {
            let char_group = named_char_groups_get(c);
            if let Some(char_group) = char_group {
                // Handle \d, etc.
                self.pos += 1;
                return Ok(RegExpTree::Charset(Charset::new(
                    char_group.clone(),
                    char_ranges::EMPTY.clone(),
                )));
            }
            let code_point = self.parse_escape_char()?;
            Ok(RegExpTree::Text(Text::new(JsString::from_units(to_chars(
                code_point,
            )?))))
        }
    }

    /// Parses n hex digits to a code-unit.
    // port: RegExpTree#parseRegExp.Parser#parseHex
    fn parse_hex(&mut self, n: usize) -> Result<i32, RegExpException> {
        if self.pos + n > self.limit {
            return Err(illegal_argument(
                "Abbreviated hex escape ",
                &self.substring_from(self.pos)?,
            ));
        }
        if n > 7 {
            // We need to guard the MSB to prevent overflow.
            return Err(illegal_argument(
                "Cannot parse hexadecimal encoding wider than 28 bits: ",
                &self.substring(self.pos, self.pos + n)?,
            ));
        }

        let mut result: i32 = 0;
        let mut n = n as i32;
        loop {
            n -= 1;
            if n < 0 {
                break;
            }
            let c = self.char_at(self.pos)?;
            let digit: i32;
            if ch(b'0') <= c && c <= ch(b'9') {
                digit = (c - ch(b'0')) as i32;
            } else if ch(b'a') <= c && c <= ch(b'f') {
                digit = c as i32 + (10 - b'a' as i32);
            } else if ch(b'A') <= c && c <= ch(b'F') {
                digit = c as i32 + (10 - b'A' as i32);
            } else {
                return Err(RegExpException::IllegalArgumentException(Some(
                    self.substring_from(self.pos)?,
                )));
            }
            self.pos += 1;
            result = (result << 4) | digit;
        }
        Ok(result)
    }

    // port: RegExpTree#parseRegExp.Parser#parseBracedUnicodeEscape
    fn parse_braced_unicode_escape(&mut self) -> Result<i32, RegExpException> {
        let open_brace = self.pos;
        let c = self.char_at(self.pos)?;
        self.pos += 1;
        check_state!(c == ch(b'{'));

        let mut close_brace = self.pos;
        while close_brace < self.limit && self.char_at(close_brace)? != ch(b'}') {
            close_brace += 1;
        }
        if close_brace == self.limit {
            return Err(illegal_argument(
                "Malformed unicode escape: expected '}' after ",
                &self.substring_from(open_brace)?,
            ));
        } else if close_brace == self.pos {
            return Err(RegExpException::IllegalArgumentException(Some(
                JsString::from("Empty unicode escape"),
            )));
        }

        let result = self.parse_hex(close_brace - self.pos)?;
        if result > 0x10FFFF {
            return Err(illegal_argument(
                "Unicode must be at most 0x10FFFF: ",
                &self.substring(open_brace + 1, self.pos)?,
            ));
        }
        self.pos += 1; // Consume the close brace.
        Ok(result)
    }

    /// Parse a repetition. `x?` is treated as a repetition -- an optional production can be
    /// matched 0 or 1 time.
    // port: RegExpTree#parseRegExp.Parser#parseRepetition
    fn parse_repetition(&mut self, body: RegExpTree) -> Result<RegExpTree, RegExpException> {
        if self.pos == self.limit {
            return Ok(body);
        }
        let min: i32;
        let max: i32;
        match ascii(self.char_at(self.pos)?) {
            b'+' => {
                self.pos += 1;
                min = 1;
                max = i32::MAX;
            }
            b'*' => {
                self.pos += 1;
                min = 0;
                max = i32::MAX;
            }
            b'?' => {
                self.pos += 1;
                min = 0;
                max = 1;
            }
            b'{' => {
                self.pos += 1;
                let start = self.pos;
                let end = index_of_char_from(self.pattern, ch(b'}'), start);
                if end < 0 {
                    self.pos = start - 1;
                    return Ok(body);
                }
                let end = end as usize;
                let counts = self.substring(start, end)?;
                self.pos = end + 1;
                let comma = counts.index_of_char(ch(b','));
                let parsed = (|| -> Result<(i32, i32), java_lang::NumberFormatException> {
                    let min = java_lang::parse_int(
                        if comma >= 0 {
                            &counts.as_units()[..comma as usize]
                        } else {
                            counts.as_units()
                        },
                        10,
                    )?;
                    let max = if comma >= 0 {
                        if comma as usize + 1 != counts.length() {
                            java_lang::parse_int(&counts.as_units()[comma as usize + 1..], 10)?
                        } else {
                            i32::MAX
                        }
                    } else {
                        min
                    };
                    Ok((min, max))
                })();
                match parsed {
                    Ok((parsed_min, parsed_max)) => {
                        min = parsed_min;
                        max = parsed_max;
                    }
                    Err(_) => {
                        min = -1;
                        max = -1;
                    }
                }
                if min < 0 || min > max {
                    // Treat the open curly bracket literally.
                    self.pos = start - 1;
                    return Ok(body);
                }
            }
            _ => {
                return Ok(body);
            }
        }
        let mut greedy = true;
        if self.pos < self.limit && self.char_at(self.pos)? == ch(b'?') {
            greedy = false;
            self.pos += 1;
        }
        Ok(RegExpTree::Repetition(Repetition::new(
            body, min, max, greedy,
        )))
    }
}

// port: RegExpTree#parseRegExp.Parser#isRepetitionStart
fn is_repetition_start(c: u16) -> bool {
    matches!(ascii(c), b'?' | b'*' | b'+' | b'{')
}

/// The character class of `\p{...}` names and values: `_`, `a-z`, `A-Z`, `0-9`.
fn is_property_char(c: u16) -> bool {
    c == ch(b'_')
        || (ch(b'a') <= c && c <= ch(b'z'))
        || (ch(b'A') <= c && c <= ch(b'Z'))
        || (ch(b'0') <= c && c <= ch(b'9'))
}

/// `String.indexOf(int ch, int fromIndex)` for a BMP code unit.
// port: String#indexOf(int, int)
fn index_of_char_from(s: &JsString, c: u16, from: usize) -> i32 {
    let units = s.as_units();
    let mut i = from;
    while i < units.len() {
        if units[i] == c {
            return i as i32;
        }
        i += 1;
    }
    -1
}

/// `Character.toChars(int)`.
// port: Character#toChars(int)
fn to_chars(code_point: i32) -> Result<Vec<u16>, RegExpException> {
    if (0..0x10000).contains(&code_point) {
        Ok(vec![code_point as u16])
    } else if (0x10000..=0x10FFFF).contains(&code_point) {
        let offset = code_point - 0x10000;
        Ok(vec![
            (0xD800 + (offset >> 10)) as u16,
            (0xDC00 + (offset & 0x3FF)) as u16,
        ])
    } else {
        Err(RegExpException::IllegalArgumentException(Some(
            JsString::from(format!("Not a valid Unicode code point: 0x{code_point:X}")),
        )))
    }
}

/// `ch`: the character. Returns true if the character is a valid Javascript identifier start
/// character.
// port: RegExpTree#isIdentifierStart
fn is_identifier_start(c: u16) -> bool {
    // TODO(yitingwang) This and the one in Scanner.java should share the same implementation
    // Most code is written in pure ASCII create a fast path here.
    if c <= 127 {
        return (c >= ch(b'A') && c <= ch(b'Z'))
            || (c >= ch(b'a') && c <= ch(b'z'))
            || (c == ch(b'_') || c == ch(b'$'));
    }

    java_lang::is_letter(c)
}

/// `ch`: the character. Returns true if the character is allowed in Javascript identifiers.
// port: RegExpTree#isIdentifierPart
fn is_identifier_part(c: u16) -> bool {
    // TODO(yitingwang) This and the one in Scanner.java should share the same implementation
    // Most code is written in pure ASCII create a fast path here.
    if c <= 127 {
        return (c >= ch(b'A') && c <= ch(b'Z'))
            || (c >= ch(b'a') && c <= ch(b'z'))
            || (c >= ch(b'0') && c <= ch(b'9'))
            || (c == ch(b'_') || c == ch(b'$')); // _ or $
    }
    // TODO: identifier part character classes
    // CombiningMark
    //   Non-Spacing mark (Mn)
    //   Combining spacing mark(Mc)
    // Connector punctuation (Pc)
    // Zero Width Non-Joiner
    // Zero Width Joiner
    is_identifier_start(c) || java_lang::is_digit(c)
}

/// `ImmutableList.hashCode()` / `List.hashCode()`.
// port: List#hashCode
fn list_hash_code(list: &[RegExpTree]) -> i32 {
    let mut hash_code: i32 = 1;
    for e in list {
        hash_code = hash_code.wrapping_mul(31).wrapping_add(e.hash_code());
    }
    hash_code
}

/// Represents a node that never has children such as an anchor or charset.
pub struct RegExpTreeAtom;

impl RegExpTreeAtom {
    // port: RegExpTree.RegExpTreeAtom#isCaseSensitive
    fn is_case_sensitive() -> bool {
        false
    }

    // port: RegExpTree.RegExpTreeAtom#containsAnchor
    fn contains_anchor() -> bool {
        false
    }

    // port: RegExpTree.RegExpTreeAtom#numCapturingGroups
    fn num_capturing_groups() -> i32 {
        0
    }

    // port: RegExpTree.RegExpTreeAtom#children
    fn children<'a>() -> Vec<&'a RegExpTree> {
        Vec::new()
    }
}

/// Represents an empty portion of a RegExp such as the middle of "||"
// port: RegExpTree.Empty#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Empty;

/// `Empty.INSTANCE`
// port: RegExpTree.Empty#INSTANCE
fn empty_instance() -> RegExpTree {
    RegExpTree::Empty(Empty)
}

impl Empty {
    // port: RegExpTree.Empty#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::Empty(self.clone())
    }

    // port: RegExpTree.Empty#appendSourceCode
    pub fn append_source_code(&self, _sb: &mut Vec<u16>) {
        // No output
    }

    // port: RegExpTree.Empty#appendDebugInfo
    pub fn append_debug_info(&self, _sb: &mut Vec<u16>) {
        // No output
    }

    // port: RegExpTree.Empty#hashCode
    pub fn hash_code(&self) -> i32 {
        0x7ee06141
    }
}

/// Represents an anchor, namely ^ or $.
// port: RegExpTree.Anchor#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub r#type: u16,
}

impl Anchor {
    // port: RegExpTree.Anchor#Anchor
    fn new(r#type: u16) -> Anchor {
        Anchor { r#type }
    }

    // port: RegExpTree.Anchor#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::Anchor(self.clone())
    }

    // port: RegExpTree.Anchor#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        true
    }

    // port: RegExpTree.Anchor#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        sb.push(self.r#type);
    }

    // port: RegExpTree.Anchor#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        sb.push(self.r#type);
    }

    // port: RegExpTree.Anchor#hashCode
    pub fn hash_code(&self) -> i32 {
        self.r#type as i32 ^ 0xe85317ffu32 as i32
    }
}

/// Represents \b or \B
// port: RegExpTree.WordBoundary#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordBoundary {
    pub r#type: u16,
}

impl WordBoundary {
    // port: RegExpTree.WordBoundary#WordBoundary
    fn new(r#type: u16) -> WordBoundary {
        WordBoundary { r#type }
    }

    // port: RegExpTree.WordBoundary#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::WordBoundary(self.clone())
    }

    // port: RegExpTree.WordBoundary#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        sb.push(ch(b'\\'));
        sb.push(self.r#type);
    }

    // port: RegExpTree.WordBoundary#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        sb.push(self.r#type);
    }

    // port: RegExpTree.WordBoundary#hashCode
    pub fn hash_code(&self) -> i32 {
        0x5673aa29 ^ self.r#type as i32
    }
}

/// Represents a reference to a previous group such as \1 or \2
// port: RegExpTree.BackReference#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackReference {
    pub group_index: i32,
}

impl BackReference {
    // port: RegExpTree.BackReference#BackReference
    fn new(group_index: i32) -> Result<BackReference, RegExpException> {
        // checkArgument(groupIndex >= 0 && groupIndex <= 99)
        if !(group_index >= 0 && group_index <= 99) {
            return Err(RegExpException::IllegalArgumentException(None));
        }
        Ok(BackReference { group_index })
    }

    // port: RegExpTree.BackReference#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::BackReference(self.clone())
    }

    // port: RegExpTree.BackReference#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        sb.push(ch(b'\\'));
        append_int(sb, self.group_index);
    }

    // port: RegExpTree.BackReference#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_int(sb, self.group_index);
    }

    // port: RegExpTree.BackReference#hashCode
    pub fn hash_code(&self) -> i32 {
        0xff072663u32 as i32 ^ self.group_index
    }
}

/// Represents a reference to a previous named group
// port: RegExpTree.NamedBackReference#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedBackReference {
    pub group_name: JsString,
}

impl NamedBackReference {
    // port: RegExpTree.NamedBackReference#NamedBackReference
    fn new(group_name: JsString) -> NamedBackReference {
        NamedBackReference { group_name }
    }

    // port: RegExpTree.NamedBackReference#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::NamedBackReference(self.clone())
    }

    // port: RegExpTree.NamedBackReference#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        append_str(sb, "\\k<");
        sb.extend_from_slice(self.group_name.as_units());
        sb.push(ch(b'>'));
    }

    // port: RegExpTree.NamedBackReference#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        sb.extend_from_slice(self.group_name.as_units());
    }

    // port: RegExpTree.NamedBackReference#hashCode
    pub fn hash_code(&self) -> i32 {
        self.group_name.hash_code()
    }
}

/// Represents a run of non-special characters such as "foobar"
// port: RegExpTree.Text#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub text: JsString,
}

impl Text {
    // port: RegExpTree.Text#Text
    fn new(text: JsString) -> Text {
        Text { text }
    }

    /// `ch`: The code-unit to escape. `next`: The next code-unit or -1 if indeterminable.
    // port: RegExpTree.Text#escapeRegularCharOnto
    fn escape_regular_char_onto(c: u16, next: i32, sb: &mut Vec<u16>) {
        match ascii(c) {
            b'$' | b'^' | b'*' | b'(' | b')' | b'+' | b'[' | b'|' | b'.' | b'/' | b'?' => {
                sb.push(ch(b'\\'));
                sb.push(c);
            }
            b'{' => {
                // If possibly part of a repetition, then escape.
                // Concatenation is handled by the digitsMightBleed check.
                if (b'0' as i32) <= next && next <= (b'9' as i32) {
                    sb.push(ch(b'\\'));
                }
                sb.push(c);
            }
            _ => escape_char_onto(c, sb),
        }
    }

    // port: RegExpTree.Text#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let n = self.text.length();
        if n == 0 {
            return empty_instance();
        }
        if flags.index_of_char(ch(b'i')) >= 0 {
            let canonicalized = case_canonicalize::case_canonicalize(self.text.as_units());
            if self.text.as_units() != canonicalized.as_slice() {
                return RegExpTree::Text(Text::new(JsString::from_units(canonicalized)));
            }
        }
        RegExpTree::Text(self.clone())
    }

    // port: RegExpTree.Text#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        for i in 0..self.text.length() {
            if case_canonicalize::CASE_SENSITIVE.contains(self.text.char_at(i) as i32) {
                return true;
            }
        }
        false
    }

    // port: RegExpTree.Text#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        let n = self.text.length();
        for i in 0..n {
            Self::escape_regular_char_onto(
                self.text.char_at(i),
                if i + 1 < n {
                    self.text.char_at(i + 1) as i32
                } else {
                    -1
                },
                sb,
            );
        }
    }

    // port: RegExpTree.Text#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        sb.push(ch(b'`'));
        sb.extend_from_slice(self.text.as_units());
        sb.push(ch(b'`'));
    }

    // port: RegExpTree.Text#hashCode
    pub fn hash_code(&self) -> i32 {
        self.text.hash_code() ^ 0x617e310
    }
}

/// Represents a repeating item such as ...+, ...*, or ...{0,1}
// port: RegExpTree.Repetition#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repetition {
    pub body: Box<RegExpTree>,
    pub min: i32,
    pub max: i32,
    pub greedy: bool,
}

impl Repetition {
    // port: RegExpTree.Repetition#Repetition
    fn new(body: RegExpTree, min: i32, max: i32, greedy: bool) -> Repetition {
        Repetition {
            body: Box::new(body),
            min,
            max,
            greedy,
        }
    }

    // port: RegExpTree.Repetition#simplify
    #[allow(clippy::collapsible_if)] // Java nests the two ifs.
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let mut body = self.body.simplify(flags);
        if self.max == 0 && !body.has_capturing_group() {
            return empty_instance();
        }
        if matches!(body, RegExpTree::Empty(_)) || *NEVER_MATCHES == body {
            return body;
        }
        let mut min = self.min;
        let mut max = self.max;
        if let RegExpTree::Repetition(rbody) = &body {
            if rbody.greedy == self.greedy {
                let lmin = (min as i64) * (rbody.min as i64);
                let lmax = (max as i64) * (rbody.max as i64);
                if lmin < i32::MAX as i64 {
                    let inner = (*rbody.body).clone();
                    body = inner;
                    min = lmin as i32;
                    max = if lmax >= i32::MAX as i64 {
                        i32::MAX
                    } else {
                        lmax as i32
                    };
                }
            }
        }
        if min == 1 && max == 1 {
            return body;
        }
        let greedy = self.greedy || min == max;
        if body == *self.body && min == self.min && max == self.max && greedy == self.greedy {
            RegExpTree::Repetition(self.clone())
        } else {
            Repetition::new(body, min, max, greedy).simplify(flags)
        }
    }

    // port: RegExpTree.Repetition#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        self.body.is_case_sensitive()
    }

    // port: RegExpTree.Repetition#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        self.body.contains_anchor()
    }

    // port: RegExpTree.Repetition#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        self.body.num_capturing_groups()
    }

    // port: RegExpTree.Repetition#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        vec![&*self.body]
    }

    // port: RegExpTree.Repetition#appendBodySourceCode
    fn append_body_source_code(&self, sb: &mut Vec<u16>) {
        if matches!(
            &*self.body,
            RegExpTree::Alternation(_) | RegExpTree::Concatenation(_) | RegExpTree::Repetition(_)
        ) || matches!(&*self.body, RegExpTree::Text(text) if text.text.length() > 1)
        {
            append_str(sb, "(?:");
            self.body.append_source_code(sb);
            sb.push(ch(b')'));
        } else {
            self.body.append_source_code(sb);
        }
    }

    // port: RegExpTree.Repetition#suffixLen
    fn suffix_len(min: i32, max: i32) -> i32 {
        // This mirrors the branches that renders a suffix in appendSourceCode below.
        if max == i32::MAX {
            return match min {
                0 | 1 => 1,                             // * or +
                _ => 3 + Self::num_decimal_digits(min), // {3,}
            };
        }
        if min == 0 && max == 1 {
            return 1; // ?
        }
        if min == max {
            if min == 1 {
                return 0; // No suffix needed for {1}.
            }
            return 2 + Self::num_decimal_digits(min); // {4}
        }
        3 + Self::num_decimal_digits(min) + Self::num_decimal_digits(max) // {2,7}
    }

    // port: RegExpTree.Repetition#numDecimalDigits
    fn num_decimal_digits(n: i32) -> i32 {
        if n < 0 {
            // Negative values should not be passed in.
            panic!("java.lang.AssertionError");
            // If changing this code to support negative values, Integer.MIN_VALUE is a
            // corner-case..
        }
        let mut n = n;
        let mut n_digits = 1;
        while n >= 10 {
            n_digits += 1;
            n /= 10;
        }
        n_digits
    }

    // port: RegExpTree.Repetition#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        let body_start = sb.len();
        self.append_body_source_code(sb);
        let body_end = sb.len();
        let body_len = (body_end - body_start) as i32;
        let mut min = self.min;
        let mut max = self.max;
        if (min >= 2 && max == i32::MAX) || max.wrapping_sub(min) <= 1 {
            let mut expanded =
                // If min == max then we want to try expanding to the limit and attach the empty
                // suffix, which is equivalent to min = max = 1, i.e. /a/ vs /a{1}/.
                if min == max
                    // Give aa+ preference over aaa*.
                    || max == i32::MAX
                {
                    min.wrapping_sub(1)
                } else {
                    min
                };
            let expanded_min = min.wrapping_sub(expanded);
            let expanded_max = if max == i32::MAX {
                max
            } else {
                max.wrapping_sub(expanded)
            };
            let suffix_len = Self::suffix_len(min, max);
            let expanded_suffix_len = Self::suffix_len(expanded_min, expanded_max);
            if body_len
                .wrapping_mul(expanded)
                .wrapping_add(expanded_suffix_len)
                < suffix_len
                && !self.body.has_capturing_group()
            {
                // a{2} -> aa
                // a{2,} -> aa+
                // a{2,3} -> aaa?
                loop {
                    expanded -= 1;
                    if expanded < 0 {
                        break;
                    }
                    sb.extend_from_within(body_start..body_end);
                }
                min = expanded_min;
                max = expanded_max;
            }
        }

        if max == i32::MAX {
            match min {
                0 => sb.push(ch(b'*')),
                1 => sb.push(ch(b'+')),
                _ => {
                    sb.push(ch(b'{'));
                    append_int(sb, min);
                    append_str(sb, ",}");
                }
            }
        } else if min == 0 && max == 1 {
            sb.push(ch(b'?'));
        } else if min == max {
            if min != 1 {
                sb.push(ch(b'{'));
                append_int(sb, min);
                sb.push(ch(b'}'));
            }
        } else {
            sb.push(ch(b'{'));
            append_int(sb, min);
            sb.push(ch(b','));
            append_int(sb, max);
            sb.push(ch(b'}'));
        }
        if !self.greedy {
            sb.push(ch(b'?'));
        }
    }

    // port: RegExpTree.Repetition#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_str(sb, " min=");
        append_int(sb, self.min);
        append_str(sb, ", max=");
        append_int(sb, self.max);
        if !self.greedy {
            append_str(sb, "  not_greedy");
        }
    }

    // port: RegExpTree.Repetition#hashCode
    pub fn hash_code(&self) -> i32 {
        self.min.wrapping_add(
            31i32.wrapping_mul(
                self.max.wrapping_add(
                    31i32.wrapping_mul(
                        (if self.greedy { 1i32 } else { 0i32 })
                            .wrapping_add(31i32.wrapping_mul(self.body.hash_code())),
                    ),
                ),
            ),
        )
    }
}

/// Represents the possibilities ["foo", "bar" ] for a RegExp /foo|bar/
// port: RegExpTree.Alternation#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alternation {
    pub alternatives: Vec<RegExpTree>,
}

impl Alternation {
    // port: RegExpTree.Alternation#Alternation
    fn new(alternatives: Vec<RegExpTree>) -> Alternation {
        Alternation { alternatives }
    }

    // port: RegExpTree.Alternation#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let mut alternatives: Vec<RegExpTree> = Vec::new();
        for alternative in &self.alternatives {
            let alternative = alternative.simplify(flags);
            if let RegExpTree::Alternation(alternation) = alternative {
                alternatives.extend(alternation.alternatives);
            } else {
                alternatives.push(alternative);
            }
        }
        // Remove duplicates
        let mut last: Option<RegExpTree> = None;
        let mut it = 0;
        while it < alternatives.len() {
            let alternative = &alternatives[it];
            if *alternative == *NEVER_MATCHES {
                it += 1;
                continue;
            }
            if last.as_ref() == Some(alternative) && !alternative.has_capturing_group() {
                alternatives.remove(it);
            } else {
                last = Some(alternative.clone());
                it += 1;
            }
        }
        // Collapse character alternatives into character sets.
        let mut i = 0;
        let mut n = alternatives.len();
        while i < n {
            let alternative = &alternatives[i];
            if matches!(alternative, RegExpTree::Text(text) if text.text.length() == 1)
                || matches!(alternative, RegExpTree::Charset(_))
            {
                let mut end = i;
                let mut n_charsets = 0;
                while end < n {
                    let follower = &alternatives[end];
                    if matches!(follower, RegExpTree::Charset(_)) {
                        n_charsets += 1;
                    } else if !matches!(follower, RegExpTree::Text(text) if text.text.length() == 1)
                    {
                        break;
                    }
                    end += 1;
                }
                if end - i >= 3 || (n_charsets != 0 && end - i >= 2) {
                    let mut members = vec![0i32; end - i - n_charsets];
                    let mut member_idx = 0;
                    let mut chars = char_ranges::EMPTY.clone();
                    let mut ie_explicits = char_ranges::EMPTY.clone();
                    let char_alternatives = &alternatives[i..end];
                    for char_alternative in char_alternatives {
                        if let RegExpTree::Text(text) = char_alternative {
                            let c = text.text.char_at(0);
                            members[member_idx] = c as i32;
                            member_idx += 1;
                            if IE_SPEC_ERRORS.contains(c as i32) {
                                ie_explicits = ie_explicits
                                    .union(&CharRanges::inclusive(c as i32, c as i32).unwrap());
                            }
                        } else if let RegExpTree::Charset(cs) = char_alternative {
                            chars = chars.union(&cs.ranges);
                            ie_explicits = ie_explicits.union(&cs.ie_explicits);
                        }
                    }
                    chars = chars.union(&CharRanges::with_members(&members));
                    let replacement = Charset::new(chars, ie_explicits).simplify(flags);
                    alternatives.splice(i..end, [replacement]);
                    n = alternatives.len();
                }
            }
            i += 1;
        }
        match alternatives.len() {
            0 => {
                return empty_instance();
            }
            1 => {
                return alternatives.swap_remove(0);
            }
            2 => {
                if matches!(alternatives[1], RegExpTree::Empty(_)) {
                    // (?:a|) -> a?
                    return RegExpTree::Repetition(Repetition::new(
                        alternatives.swap_remove(0),
                        0,
                        1,
                        true,
                    ));
                } else if matches!(alternatives[0], RegExpTree::Empty(_)) {
                    return RegExpTree::Repetition(Repetition::new(
                        alternatives.swap_remove(1),
                        0,
                        1,
                        false,
                    ));
                }
            }
            _ => {}
        }
        // TODO: maybe pull out common prefix or suffix
        if alternatives == self.alternatives {
            RegExpTree::Alternation(self.clone())
        } else {
            RegExpTree::Alternation(Alternation::new(alternatives))
        }
    }

    // port: RegExpTree.Alternation#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        for alternative in &self.alternatives {
            if alternative.is_case_sensitive() {
                return true;
            }
        }
        false
    }

    // port: RegExpTree.Alternation#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        for alternative in &self.alternatives {
            if alternative.contains_anchor() {
                return true;
            }
        }
        false
    }

    // port: RegExpTree.Alternation#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        let mut n = 0;
        for alternative in &self.alternatives {
            n += alternative.num_capturing_groups();
        }
        n
    }

    // port: RegExpTree.Alternation#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        self.alternatives.iter().collect()
    }

    // port: RegExpTree.Alternation#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        for i in 0..self.alternatives.len() {
            if i != 0 {
                sb.push(ch(b'|'));
            }
            self.alternatives[i].append_source_code(sb);
        }
    }

    // port: RegExpTree.Alternation#appendDebugInfo
    pub fn append_debug_info(&self, _sb: &mut Vec<u16>) {
        // Nothing besides children.
    }

    // port: RegExpTree.Alternation#hashCode
    pub fn hash_code(&self) -> i32 {
        0x51b57cd1 ^ list_hash_code(&self.alternatives)
    }
}

/// `NEVER_MATCHES`
static NEVER_MATCHES: LazyLock<RegExpTree> = LazyLock::new(|| {
    RegExpTree::LookaheadAssertion(LookaheadAssertion::new(empty_instance(), false))
});

/// Represents a lookahead assertion such as (?=...) or (?!...)
// port: RegExpTree.LookaheadAssertion#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookaheadAssertion {
    pub body: Box<RegExpTree>,
    pub positive: bool,
}

impl LookaheadAssertion {
    // port: RegExpTree.LookaheadAssertion#LookaheadAssertion
    fn new(body: RegExpTree, positive: bool) -> LookaheadAssertion {
        LookaheadAssertion {
            body: Box::new(body),
            positive,
        }
    }

    // port: RegExpTree.LookaheadAssertion#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let simple_body = self.body.simplify(flags);
        if matches!(simple_body, RegExpTree::Empty(_)) && self.positive {
            // Always true
            return simple_body;
        }
        RegExpTree::LookaheadAssertion(LookaheadAssertion::new(simple_body, self.positive))
    }

    // port: RegExpTree.LookaheadAssertion#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        self.body.is_case_sensitive()
    }

    // port: RegExpTree.LookaheadAssertion#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        self.body.contains_anchor()
    }

    // port: RegExpTree.LookaheadAssertion#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        self.body.num_capturing_groups()
    }

    // port: RegExpTree.LookaheadAssertion#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        vec![&*self.body]
    }

    // port: RegExpTree.LookaheadAssertion#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        append_str(sb, if self.positive { "(?=" } else { "(?!" });
        self.body.append_source_code(sb);
        sb.push(ch(b')'));
    }

    // port: RegExpTree.LookaheadAssertion#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_str(
            sb,
            if self.positive {
                "positive"
            } else {
                "negative"
            },
        );
    }

    // port: RegExpTree.LookaheadAssertion#hashCode
    pub fn hash_code(&self) -> i32 {
        0x723aba9 ^ self.body.hash_code()
    }
}

/// Represents a lookbehind assertion such as `(?<=...)` or `(?<!...)`
// port: RegExpTree.LookbehindAssertion#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookbehindAssertion {
    pub body: Box<RegExpTree>,
    pub positive: bool,
}

impl LookbehindAssertion {
    // port: RegExpTree.LookbehindAssertion#LookbehindAssertion
    fn new(body: RegExpTree, positive: bool) -> LookbehindAssertion {
        LookbehindAssertion {
            body: Box::new(body),
            positive,
        }
    }

    // port: RegExpTree.LookbehindAssertion#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let simple_body = self.body.simplify(flags);
        if matches!(simple_body, RegExpTree::Empty(_)) && self.positive {
            // Always true
            return simple_body;
        }
        RegExpTree::LookbehindAssertion(LookbehindAssertion::new(simple_body, self.positive))
    }

    // port: RegExpTree.LookbehindAssertion#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        self.body.is_case_sensitive()
    }

    // port: RegExpTree.LookbehindAssertion#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        self.body.contains_anchor()
    }

    // port: RegExpTree.LookbehindAssertion#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        self.body.num_capturing_groups()
    }

    // port: RegExpTree.LookbehindAssertion#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        vec![&*self.body]
    }

    // port: RegExpTree.LookbehindAssertion#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        append_str(sb, if self.positive { "(?<=" } else { "(?<!" });
        self.body.append_source_code(sb);
        sb.push(ch(b')'));
    }

    // port: RegExpTree.LookbehindAssertion#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_str(
            sb,
            if self.positive {
                "positive"
            } else {
                "negative"
            },
        );
    }

    // port: RegExpTree.LookbehindAssertion#hashCode
    pub fn hash_code(&self) -> i32 {
        0x723aba9 ^ self.body.hash_code()
    }
}

/// Represents a capturing group such as (asdf)
// port: RegExpTree.CapturingGroup#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturingGroup {
    pub body: Box<RegExpTree>,
}

impl CapturingGroup {
    // port: RegExpTree.CapturingGroup#CapturingGroup
    fn new(body: RegExpTree) -> CapturingGroup {
        CapturingGroup {
            body: Box::new(body),
        }
    }

    // port: RegExpTree.CapturingGroup#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        RegExpTree::CapturingGroup(CapturingGroup::new(self.body.simplify(flags)))
    }

    // port: RegExpTree.CapturingGroup#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        self.body.is_case_sensitive()
    }

    // port: RegExpTree.CapturingGroup#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        self.body.contains_anchor()
    }

    // port: RegExpTree.CapturingGroup#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        1 + self.body.num_capturing_groups()
    }

    // port: RegExpTree.CapturingGroup#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        vec![&*self.body]
    }

    // port: RegExpTree.CapturingGroup#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        sb.push(ch(b'('));
        self.body.append_source_code(sb);
        sb.push(ch(b')'));
    }

    // port: RegExpTree.CapturingGroup#appendDebugInfo
    pub fn append_debug_info(&self, _sb: &mut Vec<u16>) {
        // Nothing besides children.
    }

    // port: RegExpTree.CapturingGroup#hashCode
    pub fn hash_code(&self) -> i32 {
        0x55781738 ^ self.body.hash_code()
    }
}

/// Represents a named capture group
// port: RegExpTree.NamedCaptureGroup#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedCaptureGroup {
    pub body: Box<RegExpTree>,
    pub name: JsString,
}

impl NamedCaptureGroup {
    // port: RegExpTree.NamedCaptureGroup#NamedCaptureGroup
    fn new(body: RegExpTree, name: JsString) -> NamedCaptureGroup {
        NamedCaptureGroup {
            body: Box::new(body),
            name,
        }
    }

    // port: RegExpTree.NamedCaptureGroup#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        RegExpTree::NamedCaptureGroup(NamedCaptureGroup::new(
            self.body.simplify(flags),
            self.name.clone(),
        ))
    }

    // port: RegExpTree.NamedCaptureGroup#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        self.body.is_case_sensitive()
    }

    // port: RegExpTree.NamedCaptureGroup#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        self.body.contains_anchor()
    }

    // port: RegExpTree.NamedCaptureGroup#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        1 + self.body.num_capturing_groups()
    }

    // port: RegExpTree.NamedCaptureGroup#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        vec![&*self.body]
    }

    // port: RegExpTree.NamedCaptureGroup#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        append_str(sb, "(?<");
        sb.extend_from_slice(self.name.as_units());
        sb.push(ch(b'>'));
        self.body.append_source_code(sb);
        sb.push(ch(b')'));
    }

    // port: RegExpTree.NamedCaptureGroup#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_str(sb, " name=");
        sb.extend_from_slice(self.name.as_units());
    }

    // port: RegExpTree.NamedCaptureGroup#hashCode
    pub fn hash_code(&self) -> i32 {
        self.name.hash_code() ^ self.body.hash_code()
    }
}

/// Represents a Unicode Property Escape such as in /\p{Script=Greek}/u
// port: RegExpTree.UnicodePropertyEscape#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnicodePropertyEscape {
    property_name: Option<JsString>,
    property_value: JsString,
    negated: bool,
}

impl UnicodePropertyEscape {
    // port: RegExpTree.UnicodePropertyEscape#UnicodePropertyEscape
    fn new(
        property_name: Option<JsString>,
        property_value: JsString,
        negated: bool,
    ) -> Result<UnicodePropertyEscape, RegExpException> {
        // checkState(propertyValue != null): the Rust type cannot be null.
        let name_ok = property_name.is_none() || !property_name.as_ref().unwrap().is_empty();
        if !name_ok {
            return Err(RegExpException::IllegalArgumentException(Some(
                JsString::from(
                    "if '=' is present in a unicode property escape, the name cannot be empty",
                ),
            )));
        }
        if property_value.is_empty() {
            return Err(RegExpException::IllegalArgumentException(Some(
                JsString::from("unicode property escape value cannot be empty"),
            )));
        }
        Ok(UnicodePropertyEscape {
            property_name,
            property_value,
            negated,
        })
    }

    // port: RegExpTree.UnicodePropertyEscape#simplify
    pub fn simplify(&self, _flags: &JsString) -> RegExpTree {
        RegExpTree::UnicodePropertyEscape(self.clone())
    }

    // port: RegExpTree.UnicodePropertyEscape#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        append_str(sb, if self.negated { "\\P{" } else { "\\p{" });
        if let Some(property_name) = &self.property_name {
            sb.extend_from_slice(property_name.as_units());
            sb.push(ch(b'='));
        }
        sb.extend_from_slice(self.property_value.as_units());
        append_str(sb, "}");
    }

    // port: RegExpTree.UnicodePropertyEscape#appendDebugInfo
    pub fn append_debug_info(&self, _sb: &mut Vec<u16>) {
        // Nothing besides properties and possible negation.
    }

    // port: RegExpTree.UnicodePropertyEscape#hashCode
    pub fn hash_code(&self) -> i32 {
        // Objects.hash(negated, propertyName, propertyValue)
        let mut result: i32 = 1;
        result = result
            .wrapping_mul(31)
            .wrapping_add(if self.negated { 1231 } else { 1237 });
        result = result
            .wrapping_mul(31)
            .wrapping_add(self.property_name.as_ref().map_or(0, JsString::hash_code));
        result
            .wrapping_mul(31)
            .wrapping_add(self.property_value.hash_code())
    }
}

static DIGITS: LazyLock<CharRanges> =
    LazyLock::new(|| CharRanges::inclusive(b'0' as i32, b'9' as i32).unwrap());

static UCASE_LETTERS: LazyLock<CharRanges> =
    LazyLock::new(|| CharRanges::inclusive(b'A' as i32, b'Z' as i32).unwrap());

static LCASE_LETTERS: LazyLock<CharRanges> =
    LazyLock::new(|| CharRanges::inclusive(b'a' as i32, b'z' as i32).unwrap());

static LETTERS: LazyLock<CharRanges> = LazyLock::new(|| UCASE_LETTERS.union(&LCASE_LETTERS));

static WORD_CHARS: LazyLock<CharRanges> = LazyLock::new(|| {
    DIGITS
        .union(&LETTERS)
        .union(&CharRanges::with_members(&[b'_' as i32]))
});

static INVERSE_WORD_CHARS: LazyLock<CharRanges> =
    LazyLock::new(|| char_ranges::ALL_CODE_UNITS.difference(&WORD_CHARS));

static SPACE_CHARS: LazyLock<CharRanges> = LazyLock::new(|| {
    CharRanges::with_members(&[
        0x09, // '\t'
        0x0a, // '\n'
        0x0b, // '\u000b'
        0x0c, // '\u000c'
        0x0d, // '\r'
        0x20, // ' '
        0xa0, // ' '
        // Unicode 3.0 Zs
        0x1680, 0x180e, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004, 0x2005, 0x2006, 0x2007, 0x2008,
        0x2009, 0x200a, // Line terminator chars
        0x2028, 0x2029, // Unicode 3.0 Zs
        0x202f, 0x205f, 0x3000,
        // Byte order marker is a space character in ES5 but not ES3.
        0xfeff,
    ])
});

/// IE is broken around \s. IE (6, 7, 8 at least), only recognize these.
static IE_SPACE_CHARS: LazyLock<CharRanges> =
    LazyLock::new(|| CharRanges::with_members(&[0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x20]));

/// IE is broken around \s. IE (6, 7, 8 at least), only recognize these.
static IE_SPEC_ERRORS: LazyLock<CharRanges> =
    LazyLock::new(|| SPACE_CHARS.difference(&IE_SPACE_CHARS));

/// `NAMED_CHAR_GROUPS`, an `ImmutableMap` (iteration in insertion order).
static NAMED_CHAR_GROUPS: LazyLock<Vec<(u16, CharRanges)>> = LazyLock::new(|| {
    vec![
        (ch(b'd'), DIGITS.clone()),
        (ch(b'D'), char_ranges::ALL_CODE_UNITS.difference(&DIGITS)),
        (ch(b's'), SPACE_CHARS.clone()),
        (
            ch(b'S'),
            char_ranges::ALL_CODE_UNITS.difference(&SPACE_CHARS),
        ),
        (ch(b'w'), WORD_CHARS.clone()),
        (ch(b'W'), INVERSE_WORD_CHARS.clone()),
    ]
});

/// `NAMED_CHAR_GROUPS.get(ch)`
// port: ImmutableMap#get
fn named_char_groups_get(c: u16) -> Option<&'static CharRanges> {
    NAMED_CHAR_GROUPS
        .iter()
        .find(|(key, _)| *key == c)
        .map(|(_, group)| group)
}

static DOT_CHARSET: LazyLock<Charset> = LazyLock::new(|| {
    Charset::new(
        char_ranges::ALL_CODE_UNITS
            .difference(&CharRanges::with_members(&[0x0a, 0x0d, 0x2028, 0x2029])),
        char_ranges::EMPTY.clone(),
    )
});

/// Represents a set of possible characters structured as [a-zA-Z] or [^a-zA-Z]
#[derive(Clone, Debug, Eq)]
pub struct Charset {
    pub ranges: CharRanges,

    /// Code units that were mentioned explicitly and that might be matched by a group according
    /// to ECMAScript 5 but would not because of specification violations in IE.
    pub ie_explicits: CharRanges,
}

impl Charset {
    // port: RegExpTree.Charset#Charset
    fn new(ranges: CharRanges, ie_explicits: CharRanges) -> Charset {
        Charset {
            ranges,
            ie_explicits,
        }
    }

    // port: RegExpTree.Charset#complexityWordFolded
    fn complexity_word_folded(ranges: &CharRanges) -> i32 {
        Self::complexity_word_folded_helper(ranges).min(
            1 + Self::complexity_word_folded_helper(
                &char_ranges::ALL_CODE_UNITS.difference(ranges),
            ),
        )
    }

    // port: RegExpTree.Charset#complexityWordFoldedHelper
    fn complexity_word_folded_helper(ranges: &CharRanges) -> i32 {
        let mut complexity = DecomposedCharset::complexity(ranges);
        if ranges.contains_all(&WORD_CHARS) {
            complexity =
                complexity.min(1 + DecomposedCharset::complexity(&ranges.difference(&WORD_CHARS)));
        }
        if ranges.contains_all(&INVERSE_WORD_CHARS) {
            complexity = complexity
                .min(1 + DecomposedCharset::complexity(&ranges.difference(&INVERSE_WORD_CHARS)));
        }
        complexity
    }

    // port: RegExpTree.Charset#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        if self.ranges.is_empty() {
            return NEVER_MATCHES.clone();
        }
        let mut best = self.ranges.clone();
        if flags.index_of_char(ch(b'i')) >= 0 {
            let mut options: IndexSet<CharRanges> = IndexSet::<_>::default();
            options.insert(case_canonicalize::expand_to_all_matched(&self.ranges));
            options.insert(case_canonicalize::reduce_to_minimum(&self.ranges));

            let lcase_letters = self.ranges.intersection(&LCASE_LETTERS);
            let ucase_letters = self.ranges.intersection(&UCASE_LETTERS);

            let lcase_letters_to_upper = lcase_letters.shift(-32);
            let ucase_letters_to_lower = ucase_letters.shift(32);

            options.insert(self.ranges.union(&ucase_letters_to_lower));
            options.insert(self.ranges.union(&lcase_letters_to_upper));
            options.insert(
                self.ranges
                    .union(&lcase_letters_to_upper)
                    .union(&ucase_letters_to_lower),
            );

            options.insert(
                self.ranges
                    .union(&ucase_letters_to_lower)
                    .difference(&ucase_letters),
            );
            options.insert(
                self.ranges
                    .union(&lcase_letters_to_upper)
                    .difference(&lcase_letters),
            );

            let mut best_complexity = Self::complexity_word_folded(&self.ranges);

            for option in &options {
                let complexity = Self::complexity_word_folded(option);
                if complexity < best_complexity {
                    best_complexity = complexity;
                    best = option.clone();
                }
            }
        }

        if best.get_num_ranges() == 1 && best.end(0).wrapping_sub(best.start(0)) == 1 {
            return RegExpTree::Text(Text::new(JsString::from_units(vec![best.start(0) as u16])));
        }

        if best != self.ranges {
            return RegExpTree::Charset(Charset::new(best, self.ie_explicits.clone()));
        }

        RegExpTree::Charset(self.clone())
    }

    // port: RegExpTree.Charset#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        // We could test
        //     !ranges.equals(CaseCanonicalize.expandToAllMatched(ranges))
        // but we get better optimizations by leaving the 'i' flag on in most cases.

        // Check whether skipping all the character groups that are known case-insensitive leaves
        // us with something that matches the above definition.
        let without_named_groups = self.decompose().ranges;
        without_named_groups != case_canonicalize::expand_to_all_matched(&without_named_groups)
    }

    // port: RegExpTree.Charset#decompose(CharRanges, boolean)
    fn decompose_with(&self, ranges: CharRanges, inverted: bool) -> DecomposedCharset {
        let mut ranges = ranges;
        let mut named_groups: Vec<u16> = Vec::new();
        let ranges_inter_ie_explicits = ranges.intersection(&self.ie_explicits);
        loop {
            let mut group_name: u16 = 0;
            let mut simplest: Option<CharRanges> = None;
            let mut min_complexity = DecomposedCharset::complexity(&ranges);
            for (key, group) in NAMED_CHAR_GROUPS.iter() {
                if ranges.contains_all(group) {
                    let without_group = ranges.difference(group).union(&ranges_inter_ie_explicits);
                    let complexity = DecomposedCharset::complexity(&without_group);
                    if complexity < min_complexity {
                        simplest = Some(without_group);
                        group_name = *key;
                        min_complexity = complexity;
                    }
                }
            }
            if let Some(simplest) = simplest {
                named_groups.push(ch(b'\\'));
                named_groups.push(group_name);
                ranges = simplest;
            } else {
                break;
            }
        }
        DecomposedCharset::new(inverted, ranges, named_groups)
    }

    // port: RegExpTree.Charset#decompose()
    pub fn decompose(&self) -> DecomposedCharset {
        let neg_ranges = char_ranges::ALL_CODE_UNITS.difference(&self.ranges);
        if !self.ie_explicits.is_empty() {
            if neg_ranges.intersection(&self.ie_explicits).is_empty() {
                return self.decompose_with(self.ranges.clone(), false);
            } else if self.ranges.intersection(&self.ie_explicits).is_empty() {
                return self.decompose_with(neg_ranges, true);
            }
        }
        let positive = self.decompose_with(self.ranges.clone(), false);
        let negative = self.decompose_with(neg_ranges, true);
        if positive.complexity_total() <= negative.complexity_total() {
            positive
        } else {
            negative
        }
    }

    // port: RegExpTree.Charset#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        if DOT_CHARSET.ranges == self.ranges {
            sb.push(ch(b'.'));
            return;
        }
        self.decompose().append_source_code(sb);
    }

    // port: RegExpTree.Charset#appendDebugInfo
    pub fn append_debug_info(&self, sb: &mut Vec<u16>) {
        append_str(sb, &self.ranges.to_string());
    }

    // port: RegExpTree.Charset#hashCode
    pub fn hash_code(&self) -> i32 {
        self.ranges.hash_code() ^ 0xdede2246u32 as i32
    }
}

// port: RegExpTree.Charset#equals
impl PartialEq for Charset {
    fn eq(&self, other: &Charset) -> bool {
        self.ranges == other.ranges
    }
}

/// Internal representation for [] charsets
#[derive(Clone, Debug)]
pub struct DecomposedCharset {
    pub inverted: bool,
    pub ranges: CharRanges,
    pub named_groups: Vec<u16>,
}

impl DecomposedCharset {
    // port: RegExpTree.DecomposedCharset#DecomposedCharset
    fn new(inverted: bool, ranges: CharRanges, named_groups: Vec<u16>) -> DecomposedCharset {
        DecomposedCharset {
            inverted,
            ranges,
            named_groups,
        }
    }

    // port: RegExpTree.DecomposedCharset#complexity()
    pub fn complexity_total(&self) -> i32 {
        (if self.inverted { 1 } else { 0 })
            + self.named_groups.len() as i32
            + Self::complexity(&self.ranges)
    }

    // port: RegExpTree.DecomposedCharset#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        if self.ranges.is_empty() {
            if !self.inverted && self.named_groups.len() == 2 {
                sb.extend_from_slice(&self.named_groups);
                return;
            } else if self.ranges.is_empty() && self.named_groups.is_empty() {
                append_str(sb, if self.inverted { "[\\S\\s]" } else { "(?!)" });
                return;
            }
        }
        sb.push(ch(b'['));
        if self.inverted {
            sb.push(ch(b'^'));
        }
        sb.extend_from_slice(&self.named_groups);
        let ranges_start_charset = !self.inverted && self.named_groups.is_empty();
        let mut emit_dash_at_end = false;
        let n = self.ranges.get_num_ranges();
        for i in 0..n {
            let start = self.ranges.start(i) as u16;
            let end = self.ranges.end(i).wrapping_sub(1) as u16;
            match end as i32 - start as i32 {
                0 => {
                    if start == ch(b'-') {
                        // Put it at the end where it doesn't need escaping.
                        emit_dash_at_end = true;
                    } else {
                        Self::escape_range_char_onto(
                            start,
                            ranges_start_charset,
                            i == 0,
                            i + 1 == n,
                            sb,
                        );
                    }
                }
                1 => {
                    Self::escape_range_char_onto(start, ranges_start_charset, i == 0, false, sb);
                    Self::escape_range_char_onto(end, ranges_start_charset, false, i + 1 == n, sb);
                }
                _ => {
                    Self::escape_range_char_onto(start, ranges_start_charset, i == 0, false, sb);
                    sb.push(ch(b'-'));
                    Self::escape_range_char_onto(end, ranges_start_charset, false, true, sb);
                }
            }
        }
        if emit_dash_at_end {
            sb.push(ch(b'-'));
        }
        sb.push(ch(b']'));
    }

    // port: RegExpTree.DecomposedCharset#escapeRangeCharOnto
    fn escape_range_char_onto(
        c: u16,
        start_is_flush: bool,
        at_start: bool,
        at_end: bool,
        sb: &mut Vec<u16>,
    ) {
        match ascii(c) {
            0x08 => append_str(sb, "\\b"),
            b'^' => append_str(
                sb,
                if at_start && start_is_flush {
                    "\\^"
                } else {
                    "^"
                },
            ),
            b'-' => append_str(sb, if at_start || at_end { "-" } else { "\\-" }),
            b'\\' | b']' => {
                sb.push(ch(b'\\'));
                sb.push(c);
            }
            _ => escape_char_onto(c, sb),
        }
    }

    // port: RegExpTree.DecomposedCharset#complexity(CharRanges)
    pub fn complexity(ranges: &CharRanges) -> i32 {
        let mut complexity = 0;
        for i in 0..ranges.get_num_ranges() {
            let start = ranges.start(i);
            let end = ranges.end(i).wrapping_sub(1);
            if start < 0x20 || start >= 0x7f {
                complexity += if start >= 0x100 { 6 } else { 4 };
            } else {
                complexity += 1;
            }
            match end.wrapping_sub(start) {
                0 => {
                    continue;
                }
                1 => {}
                _ => complexity += 1,
            }
            if end < 0x20 || end >= 0x7f {
                complexity += if end >= 0x100 { 6 } else { 4 };
            } else {
                complexity += 1;
            }
        }
        complexity
    }

    /// Java's `equals` assigns to `this.inverted` (`this.inverted = that.inverted && ...`) and
    /// returns the assigned value; kept as is.
    // port: RegExpTree.DecomposedCharset#equals
    pub fn equals(&mut self, that: &DecomposedCharset) -> bool {
        self.inverted =
            that.inverted && self.ranges == that.ranges && self.named_groups == that.named_groups;
        self.inverted
    }

    // port: RegExpTree.DecomposedCharset#hashCode
    pub fn hash_code(&self) -> i32 {
        self.ranges.hash_code().wrapping_add(
            31i32.wrapping_mul(
                JsString::from_units(self.named_groups.clone())
                    .hash_code()
                    .wrapping_add(if self.inverted { 1 } else { 0 }),
            ),
        )
    }
}

/// Represents a series of nodes chained one after another such as (?:...)[a-z]*(...)
// port: RegExpTree.Concatenation#equals (derived PartialEq over the same fields)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Concatenation {
    pub elements: Vec<RegExpTree>,
}

impl Concatenation {
    // port: RegExpTree.Concatenation#Concatenation(RegExpTree, RegExpTree)
    fn new2(a: RegExpTree, b: RegExpTree) -> Concatenation {
        Concatenation {
            elements: vec![a, b],
        }
    }

    // port: RegExpTree.Concatenation#Concatenation(List)
    fn new(elements: Vec<RegExpTree>) -> Concatenation {
        Concatenation { elements }
    }

    // port: RegExpTree.Concatenation#simplify
    pub fn simplify(&self, flags: &JsString) -> RegExpTree {
        let mut s = Simplifier {
            simplified: Vec::new(),
            flags,
        };
        for element in &self.elements {
            s.simplify(element.simplify(flags));
        }

        match s.simplified.len() {
            0 => empty_instance(),
            1 => s.simplified.swap_remove(0),
            _ => RegExpTree::Concatenation(Concatenation::new(s.simplified)),
        }
    }

    // port: RegExpTree.Concatenation#isCaseSensitive
    pub fn is_case_sensitive(&self) -> bool {
        for element in &self.elements {
            if element.is_case_sensitive() {
                return true;
            }
        }
        false
    }

    // port: RegExpTree.Concatenation#containsAnchor
    pub fn contains_anchor(&self) -> bool {
        for element in &self.elements {
            if element.contains_anchor() {
                return true;
            }
        }
        false
    }

    // port: RegExpTree.Concatenation#numCapturingGroups
    pub fn num_capturing_groups(&self) -> i32 {
        let mut n = 0;
        for element in &self.elements {
            n += element.num_capturing_groups();
        }
        n
    }

    // port: RegExpTree.Concatenation#children
    pub fn children(&self) -> Vec<&RegExpTree> {
        self.elements.iter().collect()
    }

    // port: RegExpTree.Concatenation#appendSourceCode
    pub fn append_source_code(&self, sb: &mut Vec<u16>) {
        // True if the last content written might consume decimal digits written subsequently.
        let mut digits_might_bleed = false;
        for element in &self.elements {
            let mut parenthesize = false;
            if matches!(
                element,
                RegExpTree::Alternation(_) | RegExpTree::Concatenation(_)
            ) {
                parenthesize = true;
            }
            if parenthesize {
                append_str(sb, "(?:");
                element.append_source_code(sb);
                sb.push(ch(b')'));
            } else {
                let start = sb.len();
                element.append_source_code(sb);
                if digits_might_bleed && sb.len() > start {
                    let first_char = sb[start];
                    if ch(b'0') <= first_char && first_char <= ch(b'9') {
                        // Bleeding happened.
                        // If the last character would be ambiguous with a repetition, escape it.
                        if sb[start - 1] == ch(b'{') {
                            // Concatenation from optimization of /{(?:0,}/ -> /\{0,}/
                            sb.insert(start - 1, ch(b'\\'));
                        } else {
                            // Or parenthesize otherwise.
                            // Concatenation from optimization of /(.)\1(?:0)/ -> /(.)\1(?:0)/.
                            sb.splice(start..start, "(?:".encode_utf16());
                            sb.push(ch(b')'));
                        }
                    }
                }
            }
            digits_might_bleed =
                // \1(?:0) bleeds if there are 10 or more capturing groups preceding.
                matches!(element, RegExpTree::BackReference(back_reference) if back_reference.group_index < 10)
                    // foo{(?:10}) bleeds.
                    || matches!(element, RegExpTree::Text(text) if text.text.ends_with("{"));
        }
    }

    // port: RegExpTree.Concatenation#appendDebugInfo
    pub fn append_debug_info(&self, _sb: &mut Vec<u16>) {
        // Nothing besides children.
    }

    // port: RegExpTree.Concatenation#hashCode
    pub fn hash_code(&self) -> i32 {
        0x20997e3e ^ list_hash_code(&self.elements)
    }
}

/// The local class `Simplifier` inside `Concatenation.simplify`.
struct Simplifier<'f> {
    simplified: Vec<RegExpTree>,
    flags: &'f JsString,
}

impl Simplifier<'_> {
    // port: RegExpTree.Concatenation#simplify.Simplifier#simplify
    fn simplify(&mut self, t: RegExpTree) {
        match t {
            RegExpTree::Concatenation(concatenation) => {
                for child in concatenation.elements {
                    self.simplify(child);
                }
            }
            RegExpTree::Empty(_) => {
                // Do nothing
            }
            t => {
                let last_index = self.simplified.len() as i32 - 1;
                if last_index >= 0 {
                    let pairwise =
                        self.simplify_pairwise(&self.simplified[last_index as usize], &t);
                    if let Some(pairwise) = pairwise {
                        self.simplified[last_index as usize] = pairwise;
                        return;
                    }
                }
                self.simplified.push(t);
            }
        }
    }

    // port: RegExpTree.Concatenation#simplify.Simplifier#simplifyPairwise
    fn simplify_pairwise(&self, before: &RegExpTree, after: &RegExpTree) -> Option<RegExpTree> {
        if let (RegExpTree::Text(before_text), RegExpTree::Text(after_text)) = (before, after) {
            return Some(Text::new(before_text.text.concat(&after_text.text)).simplify(self.flags));
        }
        // Fold adjacent repetitions.
        let mut before_min = 1;
        let mut before_max = 1;
        let mut before_body = before;
        let mut before_greedy = false;
        if let RegExpTree::Repetition(r) = before {
            before_min = r.min;
            before_max = r.max;
            before_body = &r.body;
            before_greedy = r.greedy;
        }
        let mut after_min = 1;
        let mut after_max = 1;
        let mut after_body = after;
        let mut after_greedy = false;
        if let RegExpTree::Repetition(r) = after {
            after_min = r.min;
            after_max = r.max;
            after_body = &r.body;
            after_greedy = r.greedy;
        }
        if before_body == after_body && !before_body.has_capturing_group() {
            let lmin = (before_min as i64) + after_min as i64;
            let lmax = (before_max as i64) + after_max as i64;
            if lmin < i32::MAX as i64 {
                let min = lmin as i32;
                let max = if lmax >= i32::MAX as i64 {
                    i32::MAX
                } else {
                    lmax as i32
                };
                return Some(RegExpTree::Repetition(Repetition::new(
                    before_body.clone(),
                    min,
                    max,
                    before_greedy || after_greedy || min == max,
                )));
            }
        }
        None
    }
}

// port: RegExpTree#escapeCharOnto
fn escape_char_onto(c: u16, sb: &mut Vec<u16>) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    match c {
        0x0000 => append_str(sb, "\\0"),
        0x000c => append_str(sb, "\\f"),
        0x0009 => append_str(sb, "\\t"),
        0x000a => append_str(sb, "\\n"),
        0x000d => append_str(sb, "\\r"),
        0x005c => append_str(sb, "\\\\"),
        _ => {
            if c < 0x20 || c >= 0x7f {
                if c >= 0x100 {
                    append_str(sb, "\\u");
                    sb.push(HEX[((c >> 12) & 0xf) as usize] as u16);
                    sb.push(HEX[((c >> 8) & 0xf) as usize] as u16);
                    sb.push(HEX[((c >> 4) & 0xf) as usize] as u16);
                    sb.push(HEX[(c & 0xf) as usize] as u16);
                } else {
                    append_str(sb, "\\x");
                    sb.push(HEX[((c >> 4) & 0xf) as usize] as u16);
                    sb.push(HEX[(c & 0xf) as usize] as u16);
                }
            } else {
                sb.push(c);
            }
        }
    }
}
