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
//   src/com/google/javascript/jscomp/parsing/parser/Scanner.java.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use closure_rhino::java_lang::{digit, parse_int};

use super::{
    JsString,
    identifier_token::IdentifierToken,
    identifiers::Identifiers,
    keywords::Keywords,
    line_number_scanner::LineNumberScanner,
    literal_token::LiteralToken,
    source_file::SourceFile,
    string_literal_token::StringLiteralToken,
    template_literal_token::{ErrorLevel, TemplateLiteralToken},
    token::Token,
    token_type::TokenType,
    trees::comment,
    util::{Reporter, SourcePosition, SourceRange, format_message},
};

pub trait CommentRecorder {
    // port: Scanner.CommentRecorder#recordComment
    fn record_comment(&mut self, type_: comment::Type, range: SourceRange, value: JsString);
}

// Scans javascript source code into tokens. All entrypoints assume the caller is not expecting a
// regular expression literal except for nextRegularExpressionLiteralToken.
//
// <p>7 Lexical Conventions
pub struct Scanner {
    error_reporter: Reporter,
    source: Arc<SourceFile>,
    line_number_scanner: LineNumberScanner,
    contents_length: i32,
    current_tokens: Vec<Token>,
    index: i32,
    comment_recorder: Rc<RefCell<dyn CommentRecorder>>,
    type_parameter_level: i32,
}

impl Scanner {
    // port: Scanner#<init>
    pub fn new(
        error_reporter: Reporter,
        comment_recorder: Rc<RefCell<dyn CommentRecorder>>,
        file: Arc<SourceFile>,
        offset: i32,
    ) -> Self {
        Self {
            error_reporter,
            comment_recorder,
            line_number_scanner: LineNumberScanner::new(file.clone()),
            // To help reason about the expected JVM performance unwrap "file" values.
            // The scanner is key to the parsing speed.
            contents_length: file.contents.length() as i32,
            source: file,
            current_tokens: Vec::new(),
            index: offset,
            type_parameter_level: 0,
        }
    }
    // port: Scanner#getFile
    pub fn get_file(&self) -> Arc<SourceFile> {
        self.source.clone()
    }
    // port: Scanner#getOffset
    pub fn get_offset(&mut self) -> i32 {
        if self.current_tokens.is_empty() {
            self.index
        } else {
            self.peek_token().location.start.offset
        }
    }
    // port: Scanner#setPosition
    pub fn set_position(&mut self, position: SourcePosition) {
        self.line_number_scanner.rewind_to(&position);
        self.current_tokens.clear();
        self.index = position.offset;
    }
    // port: Scanner#getPosition
    pub fn get_position(&mut self) -> SourcePosition {
        if self.current_tokens.is_empty() {
            self.get_position_at(self.index)
        } else {
            self.peek_token().location.start
        }
    }
    // port: Scanner#getPosition
    fn get_position_at(&mut self, offset: i32) -> SourcePosition {
        self.line_number_scanner.get_source_position(offset)
    }
    // port: Scanner#getTokenRange
    fn get_token_range(&mut self, start_offset: i32) -> SourceRange {
        self.line_number_scanner
            .get_source_range(start_offset, self.index)
    }
    /// Prefer this to {@link #getTokenRange(int)} when the token might span multiple lines.
    // port: Scanner#getTokenRange
    fn get_token_range_at(&mut self, position: SourcePosition) -> SourceRange {
        self.line_number_scanner.rewind_to(&position);
        self.line_number_scanner
            .get_source_range(position.offset, self.index)
    }
    // port: Scanner#nextToken
    pub fn next_token(&mut self) -> Token {
        self.peek_token();
        self.current_tokens.remove(0)
    }
    // port: Scanner#clearTokenLookahead
    fn clear_token_lookahead(&mut self) {
        if !self.current_tokens.is_empty() {
            let position = self.peek_token().location.start;
            self.set_position(position);
        }
    }
    // port: Scanner#nextRegularExpressionLiteralToken
    pub fn next_regular_expression_literal_token(&mut self) -> Token {
        self.clear_token_lookahead();
        let begin_token = self.index;
        // leading '/'
        self.next_char();
        // body
        if !self.skip_regular_expression_body() {
            return LiteralToken::new(
                TokenType::REGULAR_EXPRESSION,
                self.get_token_string(begin_token),
                self.get_token_range(begin_token),
            );
        }
        // separating '/'
        if self.peek_char() != u16::from(b'/') {
            self.report_error("Expected '/' in regular expression literal", &[]);
            return LiteralToken::new(
                TokenType::REGULAR_EXPRESSION,
                self.get_token_string(begin_token),
                self.get_token_range(begin_token),
            );
        }
        self.next_char();
        // flags
        while Identifiers::is_identifier_part(self.peek_char()) {
            self.next_char();
        }
        LiteralToken::new(
            TokenType::REGULAR_EXPRESSION,
            self.get_token_string(begin_token),
            self.get_token_range(begin_token),
        )
    }
    // port: Scanner#nextTemplateLiteralToken
    pub fn next_template_literal_token(&mut self) -> Token {
        let token = self.next_token();
        if self.is_at_end() || token.type_ != TokenType::CLOSE_CURLY {
            let position = self.get_position_at(self.index);
            self.report_error_at(
                position,
                "Expected '}' after expression in template literal",
                &[],
            );
        }
        self.next_template_literal_token_shared(
            TokenType::TEMPLATE_TAIL,
            TokenType::TEMPLATE_MIDDLE,
        )
    }
    // port: Scanner#skipRegularExpressionBody
    fn skip_regular_expression_body(&mut self) -> bool {
        if !Self::is_regular_expression_first_char(self.peek_char()) {
            self.report_error("Expected regular expression first char", &[]);
            return false;
        }
        if !self.skip_regular_expression_char() {
            return false;
        }
        while !self.is_at_end() && Self::is_regular_expression_char(self.peek_char()) {
            if !self.skip_regular_expression_char() {
                return false;
            }
        }
        true
    }
    // port: Scanner#skipRegularExpressionChar
    fn skip_regular_expression_char(&mut self) -> bool {
        match self.peek_char() {
            ch if ch == u16::from(b'\\') => self.skip_regular_expression_backslash_sequence(),
            ch if ch == u16::from(b'[') => self.skip_regular_expression_class(),
            _ => {
                self.next_char();
                true
            }
        }
    }
    // port: Scanner#skipRegularExpressionBackslashSequence
    fn skip_regular_expression_backslash_sequence(&mut self) -> bool {
        // TODO(tbreisacher): Warn if this is an unnecessary escape, like we do for string literals.
        self.next_char();
        if Self::is_line_terminator(self.peek_char()) {
            self.report_error("New line not allowed in regular expression literal", &[]);
            return false;
        }
        self.next_char();
        true
    }
    // port: Scanner#skipRegularExpressionClass
    fn skip_regular_expression_class(&mut self) -> bool {
        self.next_char();
        while !self.is_at_end() && self.peek_regular_expression_class_char() {
            if !self.skip_regular_expression_class_char() {
                return false;
            }
        }
        if self.peek_char() != u16::from(b']') {
            self.report_error("']' expected", &[]);
            return false;
        }
        self.next_char();
        true
    }
    // port: Scanner#peekRegularExpressionClassChar
    fn peek_regular_expression_class_char(&self) -> bool {
        self.peek_char() != u16::from(b']') && !Self::is_line_terminator(self.peek_char())
    }
    // port: Scanner#skipRegularExpressionClassChar
    fn skip_regular_expression_class_char(&mut self) -> bool {
        if self.peek(u16::from(b'\\')) {
            return self.skip_regular_expression_backslash_sequence();
        }
        self.next_char();
        true
    }
    // port: Scanner#isRegularExpressionFirstChar
    fn is_regular_expression_first_char(ch: u16) -> bool {
        Self::is_regular_expression_char(ch) && ch != u16::from(b'*')
    }
    // port: Scanner#isRegularExpressionChar
    fn is_regular_expression_char(ch: u16) -> bool {
        match ch {
            ch if ch == u16::from(b'/') => false,
            ch if ch == u16::from(b'\\') || ch == u16::from(b'[') => true,
            _ => !Self::is_line_terminator(ch),
        }
    }
    // port: Scanner#peekToken
    pub fn peek_token(&mut self) -> Token {
        self.peek_token_at(0)
    }
    // port: Scanner#peekToken
    pub fn peek_token_at(&mut self, index: usize) -> Token {
        while self.current_tokens.len() <= index {
            let token = self.scan_token();
            self.current_tokens.push(token);
        }
        self.current_tokens[index].clone()
    }
    // port: Scanner#isAtEnd
    fn is_at_end(&self) -> bool {
        !self.is_valid_index(self.index)
    }
    // port: Scanner#isValidIndex
    fn is_valid_index(&self, index: i32) -> bool {
        index >= 0 && index < self.contents_length
    }
    // 7.2 White Space
    // Returns true if the whitespace that was skipped included any line terminators.
    // port: Scanner#skipWhitespace
    fn skip_whitespace(&mut self) -> bool {
        let mut found_line_terminator = false;
        while !self.is_at_end() && self.peek_whitespace() {
            if Self::is_line_terminator(self.next_char()) {
                found_line_terminator = true;
            }
        }
        found_line_terminator
    }
    // port: Scanner#peekWhitespace
    fn peek_whitespace(&self) -> bool {
        Self::is_whitespace(self.peek_char())
    }
    // port: Scanner#isWhitespace
    fn is_whitespace(ch: u16) -> bool {
        matches!(
            ch,
            ch if ch == u16::from(b'\t') // Tab
                || ch == u16::from(b'\x0b') // Vertical Tab
                || ch == u16::from(b'\x0c') // Form Feed
                || ch == u16::from(b' ') // Space
                || ch == '\u{00a0}' as u16 // No-break space
                || ch == '\u{feff}' as u16 // Byte Order Mark
                || ch == u16::from(b'\n') // Line Feed
                || ch == u16::from(b'\r') // Carriage Return
                || ch == '\u{2028}' as u16 // Line Separator
                || ch == '\u{2029}' as u16 // Paragraph Separator
                || ch == '\u{3000}' as u16 /* Ideographic Space */
                         // TODO: there are other Unicode Category 'Zs' chars that should go here.
        )
    }
    // 7.3 Line Terminators
    // port: Scanner#isLineTerminator
    fn is_line_terminator(ch: u16) -> bool {
        matches!(
            ch,
            ch if ch == u16::from(b'\n') // Line Feed
                || ch == u16::from(b'\r') // Carriage Return
                || ch == '\u{2028}' as u16 // Line Separator
                || ch == '\u{2029}' as u16 // Paragraph Separator
        )
    }
    // Allow line separator and paragraph separator in string literals.
    // https://github.com/tc39/proposal-json-superset
    // port: Scanner#isStringLineTerminator
    fn is_string_line_terminator(ch: u16) -> bool {
        match ch {
            ch if ch == '\u{2028}' as u16 // Line Separator
                || ch == '\u{2029}' as u16 =>
            {
                false
            } // Paragraph Separator
            _ => Self::is_line_terminator(ch),
        }
    }
    // 7.4 Comments
    // port: Scanner#skipComments
    fn skip_comments(&mut self) {
        while self.skip_comment() {}
    }
    // port: Scanner#skipComment
    fn skip_comment(&mut self) -> bool {
        let is_start_of_line = self.skip_whitespace();
        if !self.is_at_end() {
            match self.peek_char_at(0) {
                ch if ch == u16::from(b'/') => match self.peek_char_at(1) {
                    ch if ch == u16::from(b'/') => {
                        self.skip_single_line_comment();
                        return true;
                    }
                    ch if ch == u16::from(b'*') => {
                        self.skip_multi_line_comment();
                        return true;
                    }
                    _ => {} // fall out
                },
                // Check if this is the start of an HTML comment ("<!--").
                // http://www.w3.org/TR/REC-html40/interact/scripts.html#h-18.3.2
                ch if (ch == u16::from(b'<'))
                    && self.peek_char_at(1) == u16::from(b'!')
                    && self.peek_char_at(2) == u16::from(b'-')
                    && self.peek_char_at(3) == u16::from(b'-') =>
                {
                    self.report_html_comment_warning();
                    self.skip_single_line_comment();
                    return true;
                }
                // Check if this is the start of an HTML comment ("-->").
                // Note that the spec does not require us to check for this case,
                // but there is some legacy code that depends on this behavior.
                ch if (ch == u16::from(b'-'))
                    && is_start_of_line
                    && self.peek_char_at(1) == u16::from(b'-')
                    && self.peek_char_at(2) == u16::from(b'>') =>
                {
                    self.report_html_comment_warning();
                    self.skip_single_line_comment();
                    return true;
                }
                ch if (ch == u16::from(b'#'))
                    && self.index == 0
                    && self.peek_char_at(1) == u16::from(b'!') =>
                {
                    self.skip_single_line_comment_type(comment::Type::SHEBANG);
                    return true;
                }
                _ => {}
            }
        }
        false
    }
    // port: Scanner#reportHtmlCommentWarning
    fn report_html_comment_warning(&mut self) {
        self.report_warning("In some cases, '<!--' and '-->' are treated as a '//' for legacy reasons. Removing this from your code is safe for all browsers currently in use.", &[]);
    }
    // port: Scanner#skipSingleLineComment
    fn skip_single_line_comment(&mut self) {
        self.skip_single_line_comment_type(comment::Type::LINE);
    }
    // port: Scanner#skipSingleLineComment
    fn skip_single_line_comment_type(&mut self, type_: comment::Type) {
        let start_offset = self.index;
        while !self.is_at_end() && !Self::is_line_terminator(self.peek_char()) {
            self.next_char();
        }
        let range = self
            .line_number_scanner
            .get_source_range(start_offset, self.index);
        let value = self
            .source
            .contents
            .substring(start_offset as usize, self.index as usize);
        self.record_comment(type_, range, value);
    }
    // port: Scanner#recordComment
    fn record_comment(&mut self, type_: comment::Type, range: SourceRange, value: JsString) {
        self.comment_recorder
            .borrow_mut()
            .record_comment(type_, range, value);
    }
    // port: Scanner#skipMultiLineComment
    fn skip_multi_line_comment(&mut self) {
        let start_offset = self.index;
        self.next_char(); // '/'
        self.next_char(); // '*'
        while !self.is_at_end()
            && (self.peek_char() != u16::from(b'*') || self.peek_char_at(1) != u16::from(b'/'))
        {
            self.next_char();
        }
        if !self.is_at_end() {
            self.next_char();
            self.next_char();
            let mut type_ = comment::Type::BLOCK;
            if self.index - start_offset > 4 {
                if self.source.contents.char_at((start_offset + 2) as usize) == u16::from(b'*') {
                    type_ = comment::Type::JSDOC;
                } else if self.source.contents.char_at((start_offset + 2) as usize)
                    == u16::from(b'!')
                {
                    type_ = comment::Type::IMPORTANT;
                }
            }
            let range = self
                .line_number_scanner
                .get_source_range(start_offset, self.index);
            let value = self
                .source
                .contents
                .substring(start_offset as usize, self.index as usize);
            self.record_comment(type_, range, value);
        } else {
            self.report_error("unterminated comment", &[]);
        }
    }

    // port: Scanner#scanToken
    fn scan_token(&mut self) -> Token {
        self.skip_comments();
        let begin_token = self.index;
        if self.is_at_end() {
            return self.create_token(TokenType::END_OF_FILE, begin_token);
        }
        let ch = self.next_char();
        match ch {
            ch if ch == u16::from(b'{') => self.create_token(TokenType::OPEN_CURLY, begin_token),
            ch if ch == u16::from(b'}') => self.create_token(TokenType::CLOSE_CURLY, begin_token),
            ch if ch == u16::from(b'(') => self.create_token(TokenType::OPEN_PAREN, begin_token),
            ch if ch == u16::from(b')') => self.create_token(TokenType::CLOSE_PAREN, begin_token),
            ch if ch == u16::from(b'[') => self.create_token(TokenType::OPEN_SQUARE, begin_token),
            ch if ch == u16::from(b']') => self.create_token(TokenType::CLOSE_SQUARE, begin_token),
            ch if ch == u16::from(b'.') => {
                if Self::is_decimal_digit(self.peek_char()) {
                    return self.scan_number_post_period(begin_token);
                }

                // Harmony spread operator
                if self.peek(u16::from(b'.')) && self.peek_char_at(1) == u16::from(b'.') {
                    self.next_char();
                    self.next_char();
                    return self.create_token(TokenType::ELLIPSIS, begin_token);
                }

                self.create_token(TokenType::PERIOD, begin_token)
            }
            ch if ch == u16::from(b';') => self.create_token(TokenType::SEMI_COLON, begin_token),
            ch if ch == u16::from(b',') => self.create_token(TokenType::COMMA, begin_token),
            ch if ch == u16::from(b'~') => self.create_token(TokenType::TILDE, begin_token),
            ch if ch == u16::from(b'?') => {
                if self.peek(u16::from(b'?')) {
                    // see ??
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::QUESTION_QUESTION_EQUAL, begin_token);
                    }
                    return self.create_token(TokenType::QUESTION_QUESTION, begin_token);
                }
                if self.peek(u16::from(b'.')) {
                    // see ?.
                    if !Self::is_decimal_digit(self.peek_char_at(1)) {
                        self.next_char();
                        // a?.1:2 should be a ? 0.1 : 2 not a ?. 1 : 2 (syntax error)
                        return self.create_token(TokenType::QUESTION_DOT, begin_token);
                    }
                }
                self.create_token(TokenType::QUESTION, begin_token)
            }
            ch if ch == u16::from(b':') => self.create_token(TokenType::COLON, begin_token),
            ch if ch == u16::from(b'<') => match self.peek_char() {
                ch if ch == u16::from(b'<') => {
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::LEFT_SHIFT_EQUAL, begin_token);
                    }
                    self.create_token(TokenType::LEFT_SHIFT, begin_token)
                }
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    self.create_token(TokenType::LESS_EQUAL, begin_token)
                }
                _ => self.create_token(TokenType::OPEN_ANGLE, begin_token),
            },
            ch if ch == u16::from(b'>') => {
                if self.type_parameter_level > 0 {
                    return self.create_token(TokenType::CLOSE_ANGLE, begin_token);
                }
                match self.peek_char() {
                    ch if ch == u16::from(b'>') => {
                        self.next_char();
                        match self.peek_char() {
                            ch if ch == u16::from(b'=') => {
                                self.next_char();
                                self.create_token(TokenType::RIGHT_SHIFT_EQUAL, begin_token)
                            }
                            ch if ch == u16::from(b'>') => {
                                self.next_char();
                                if self.peek(u16::from(b'=')) {
                                    self.next_char();
                                    return self.create_token(
                                        TokenType::UNSIGNED_RIGHT_SHIFT_EQUAL,
                                        begin_token,
                                    );
                                }
                                self.create_token(TokenType::UNSIGNED_RIGHT_SHIFT, begin_token)
                            }
                            _ => self.create_token(TokenType::RIGHT_SHIFT, begin_token),
                        }
                    }
                    ch if ch == u16::from(b'=') => {
                        self.next_char();
                        self.create_token(TokenType::GREATER_EQUAL, begin_token)
                    }
                    _ => self.create_token(TokenType::CLOSE_ANGLE, begin_token),
                }
            }
            ch if ch == u16::from(b'=') => match self.peek_char() {
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::EQUAL_EQUAL_EQUAL, begin_token);
                    }
                    self.create_token(TokenType::EQUAL_EQUAL, begin_token)
                }
                ch if ch == u16::from(b'>') => {
                    self.next_char();
                    self.create_token(TokenType::ARROW, begin_token)
                }
                _ => self.create_token(TokenType::EQUAL, begin_token),
            },
            ch if ch == u16::from(b'!') => {
                if self.peek(u16::from(b'=')) {
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::NOT_EQUAL_EQUAL, begin_token);
                    }
                    return self.create_token(TokenType::NOT_EQUAL, begin_token);
                }
                self.create_token(TokenType::BANG, begin_token)
            }
            ch if ch == u16::from(b'*') => {
                if self.peek(u16::from(b'=')) {
                    self.next_char();
                    return self.create_token(TokenType::STAR_EQUAL, begin_token);
                } else if self.peek(u16::from(b'*')) {
                    self.next_char();
                    // '**' seen so far
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::STAR_STAR_EQUAL, begin_token);
                    } else {
                        return self.create_token(TokenType::STAR_STAR, begin_token);
                    }
                }
                self.create_token(TokenType::STAR, begin_token)
            }
            ch if ch == u16::from(b'%') => {
                if self.peek(u16::from(b'=')) {
                    self.next_char();
                    return self.create_token(TokenType::PERCENT_EQUAL, begin_token);
                }
                self.create_token(TokenType::PERCENT, begin_token)
            }
            ch if ch == u16::from(b'^') => {
                if self.peek(u16::from(b'=')) {
                    self.next_char();
                    return self.create_token(TokenType::CARET_EQUAL, begin_token);
                }
                self.create_token(TokenType::CARET, begin_token)
            }
            ch if ch == u16::from(b'/') => {
                if self.peek(u16::from(b'=')) {
                    self.next_char();
                    return self.create_token(TokenType::SLASH_EQUAL, begin_token);
                }
                self.create_token(TokenType::SLASH, begin_token)
            }
            ch if ch == u16::from(b'+') => match self.peek_char() {
                ch if ch == u16::from(b'+') => {
                    self.next_char();
                    self.create_token(TokenType::PLUS_PLUS, begin_token)
                }
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    self.create_token(TokenType::PLUS_EQUAL, begin_token)
                }
                _ => self.create_token(TokenType::PLUS, begin_token),
            },
            ch if ch == u16::from(b'-') => match self.peek_char() {
                ch if ch == u16::from(b'-') => {
                    self.next_char();
                    self.create_token(TokenType::MINUS_MINUS, begin_token)
                }
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    self.create_token(TokenType::MINUS_EQUAL, begin_token)
                }
                _ => self.create_token(TokenType::MINUS, begin_token),
            },
            ch if ch == u16::from(b'&') => match self.peek_char() {
                ch if ch == u16::from(b'&') => {
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::AND_EQUAL, begin_token);
                    }
                    self.create_token(TokenType::AND, begin_token)
                }
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    self.create_token(TokenType::AMPERSAND_EQUAL, begin_token)
                }
                _ => self.create_token(TokenType::AMPERSAND, begin_token),
            },
            ch if ch == u16::from(b'|') => match self.peek_char() {
                ch if ch == u16::from(b'|') => {
                    self.next_char();
                    if self.peek(u16::from(b'=')) {
                        self.next_char();
                        return self.create_token(TokenType::OR_EQUAL, begin_token);
                    }
                    self.create_token(TokenType::OR, begin_token)
                }
                ch if ch == u16::from(b'=') => {
                    self.next_char();
                    self.create_token(TokenType::BAR_EQUAL, begin_token)
                }
                _ => self.create_token(TokenType::BAR, begin_token),
            },
            ch if ch == u16::from(b'#') => {
                // Shebang is not actually ever parsed here (when used correctly, it's handled above in the
                // skipComments() call) so its token is an error.
                if self.peek(u16::from(b'!')) {
                    let position = self.get_position_at(self.index);
                    self.report_error_at(
                        position,
                        "Shebang comment must be at the start of the file",
                        &[],
                    );
                    return self.create_token(TokenType::ERROR, begin_token);
                }
                // Handle private identifiers.
                self.scan_identifier_or_keyword(begin_token, ch)
            }
            // TODO: add NumberToken
            // TODO: character following NumericLiteral must not be an IdentifierStart or DecimalDigit
            ch if ch == u16::from(b'0') => self.scan_post_zero(begin_token),
            ch if (u16::from(b'1')..=u16::from(b'9')).contains(&ch) => {
                self.scan_post_digit(begin_token)
            }
            ch if ch == u16::from(b'"') || ch == u16::from(b'\'') => {
                self.scan_string_literal(begin_token, ch)
            }
            ch if ch == u16::from(b'`') => self.scan_template_literal(begin_token),
            _ => self.scan_identifier_or_keyword(begin_token, ch),
        }
    }
    // port: Scanner#scanNumberPostPeriod
    fn scan_number_post_period(&mut self, begin_token: i32) -> Token {
        self.skip_decimal_digits();
        self.scan_exponent_of_numeric_literal(begin_token)
    }
    // port: Scanner#scanPostDigit
    fn scan_post_digit(&mut self, begin_token: i32) -> Token {
        self.skip_decimal_digits();
        if self.peek(u16::from(b'n')) {
            self.next_char();
            return LiteralToken::new(
                TokenType::BIGINT,
                self.get_token_string(begin_token),
                self.get_token_range(begin_token),
            );
        }
        self.scan_fractional_numeric_literal(begin_token)
    }
    // port: Scanner#scanPostZero
    fn scan_post_zero(&mut self, begin_token: i32) -> Token {
        match self.peek_char() {
            ch if ch == u16::from(b'b') || ch == u16::from(b'B') => {
                // binary
                self.next_char();
                if !Self::is_binary_digit(self.peek_char()) {
                    self.report_error(
                        "Binary Integer Literal must contain at least one digit",
                        &[],
                    );
                }
                self.skip_binary_digits();
                let is_big_int = self.peek(u16::from(b'n'));
                if is_big_int {
                    self.next_char();
                }
                LiteralToken::new(
                    if is_big_int {
                        TokenType::BIGINT
                    } else {
                        TokenType::NUMBER
                    },
                    self.get_token_string(begin_token),
                    self.get_token_range(begin_token),
                )
            }
            ch if ch == u16::from(b'o') || ch == u16::from(b'O') => {
                // octal
                self.next_char();
                if !Self::is_octal_digit(self.peek_char()) {
                    self.report_error("Octal Integer Literal must contain at least one digit", &[]);
                }
                self.skip_octal_digits();
                if self.peek(u16::from(b'8')) || self.peek(u16::from(b'9')) {
                    self.report_error("Invalid octal digit in octal literal.", &[]);
                }
                let is_big_int = self.peek(u16::from(b'n'));
                if is_big_int {
                    self.next_char();
                }
                LiteralToken::new(
                    if is_big_int {
                        TokenType::BIGINT
                    } else {
                        TokenType::NUMBER
                    },
                    self.get_token_string(begin_token),
                    self.get_token_range(begin_token),
                )
            }
            ch if ch == u16::from(b'x') || ch == u16::from(b'X') => {
                self.next_char();
                if !self.peek_hex_digit() {
                    self.report_error("Hex Integer Literal must contain at least one digit", &[]);
                }
                self.skip_hex_digits();
                let is_big_int = self.peek(u16::from(b'n'));
                if is_big_int {
                    self.next_char();
                }
                LiteralToken::new(
                    if is_big_int {
                        TokenType::BIGINT
                    } else {
                        TokenType::NUMBER
                    },
                    self.get_token_string(begin_token),
                    self.get_token_range(begin_token),
                )
            }
            ch if ch == u16::from(b'e') || ch == u16::from(b'E') => {
                self.scan_exponent_of_numeric_literal(begin_token)
            }
            ch if ch == u16::from(b'.') => self.scan_fractional_numeric_literal(begin_token),
            ch if (u16::from(b'0')..=u16::from(b'9')).contains(&ch) => {
                self.skip_decimal_digits();
                if self.peek(u16::from(b'.')) {
                    self.next_char();
                    self.skip_decimal_digits_leading(/* hasLeadingDigit= */ false);
                }
                if self.peek(u16::from(b'n')) {
                    self.report_error("SyntaxError: nonzero BigInt can't have leading zero", &[]);
                }
                LiteralToken::new(
                    TokenType::NUMBER,
                    self.get_token_string(begin_token),
                    self.get_token_range(begin_token),
                )
            }
            ch if ch == u16::from(b'n') => {
                self.next_char();
                LiteralToken::new(
                    TokenType::BIGINT,
                    self.get_token_string(begin_token),
                    self.get_token_range(begin_token),
                )
            }
            _ => LiteralToken::new(
                TokenType::NUMBER,
                self.get_token_string(begin_token),
                self.get_token_range(begin_token),
            ),
        }
    }
    // port: Scanner#createToken
    fn create_token(&mut self, type_: TokenType, begin_token: i32) -> Token {
        Token::new(type_, self.get_token_range(begin_token))
    }
    // port: Scanner#scanIdentifierOrKeyword
    fn scan_identifier_or_keyword(&mut self, begin_token: i32, mut ch: u16) -> Token {
        // NOTE: This code previously used a StringBuilder to collect the characters of the identifier
        // or keyword. Recording the staring position and using contents.substring() below instead was
        // found to eliminate 1.84% of all JVM "frequently collected garbage" in the compilation of a
        // large project.
        let value_start_index = self.index - 1;
        let mut contains_unicode_escape = ch == u16::from(b'\\');
        let mut braced_unicode_escape = false;
        let is_private_identifier = ch == u16::from(b'#');
        let mut unicode_escape_len = if contains_unicode_escape { 1 } else { 0 };
        ch = self.peek_char();
        while Identifiers::is_identifier_part(ch)
            || ch == u16::from(b'\\')
            || (ch == u16::from(b'{') && unicode_escape_len == 2)
            || (ch == u16::from(b'}') && braced_unicode_escape)
        {
            if ch == u16::from(b'\\') {
                contains_unicode_escape = true;
            }
            // Update length of current Unicode escape.
            if ch == u16::from(b'\\') || unicode_escape_len > 0 {
                unicode_escape_len += 1;
            }
            // Enter Unicode point escape.
            if ch == u16::from(b'{') {
                braced_unicode_escape = true;
            }
            // Exit Unicode escape
            if ch == u16::from(b'}') || (unicode_escape_len >= 6 && !braced_unicode_escape) {
                braced_unicode_escape = false;
                unicode_escape_len = 0;
            }
            // Add character to token
            self.next_char();
            ch = self.peek_char();
        }
        let mut value = self
            .source
            .contents
            .substring(value_start_index as usize, self.index as usize);
        if is_private_identifier && value == "#" {
            let position = self.get_position_at(begin_token);
            self.report_error_at(position, "Invalid usage of #", &[]);
            return self.create_token(TokenType::ERROR, begin_token);
        }
        // Process unicode escapes.
        if contains_unicode_escape {
            match Self::process_unicode_escapes(value) {
                Some(processed) => value = processed,
                None => {
                    let position = self.get_position_at(self.index);
                    self.report_error_at(position, "Invalid escape sequence", &[]);
                    return self.create_token(TokenType::ERROR, begin_token);
                }
            }
        }
        // Check to make sure the first character (or the unicode escape at the
        // beginning of the identifier) is a valid identifier start character.
        let mut start = value.char_at(0);
        if is_private_identifier {
            // Skip the leading # for name validation.
            start = value.char_at(1);
        }
        if !Identifiers::is_identifier_start(start) {
            let position = self.get_position_at(begin_token);
            self.report_error_at(
                position,
                "Character '%c' (U+%04X) is not a valid identifier start char",
                &[
                    JsString::from_units(vec![start]),
                    JsString::from(start.to_string()),
                ],
            );
            return self.create_token(TokenType::ERROR, begin_token);
        }
        let k = Keywords::get_by_name(&value);
        // Keywords containing unicode escape sequences are treated as IdentifierTokens rather than
        // keyword Tokens (ECMA-262 11.6.2).
        if let Some(k) = k.filter(|_| !contains_unicode_escape) {
            return Token::new(k.type_(), self.get_token_range(begin_token));
        }
        IdentifierToken::new(self.get_token_range(begin_token), value)
    }
    // Converts unicode escapes in the given string to the equivalent unicode character. If there are
    // no escapes, returns the input unchanged. If there is an invalid escape sequence, returns null.
    // port: Scanner#processUnicodeEscapes
    fn process_unicode_escapes(value: JsString) -> Option<JsString> {
        let mut value = value.as_units().to_vec();
        while let Some(escape_start) = value.iter().position(|&ch| ch == u16::from(b'\\')) {
            if *value.get(escape_start + 1)? != u16::from(b'u') {
                return None;
            }
            let hex_digits;
            let mut escape_end;
            if *value.get(escape_start + 2)? != u16::from(b'{') {
                // Simple escape with exactly four hex digits: \\uXXXX
                escape_end = escape_start + 6;
                // TODO(b/155480859): Don't trust String#substring to throw on out of bounds. J2CL
                // implements it incorrectly.
                if escape_end > value.len() {
                    return None;
                }
                hex_digits = &value[escape_start + 2..escape_end];
            } else {
                // Escape with braces can have any number of hex digits: \\u{XXXXXXX}
                escape_end = escape_start + 3;
                while Self::is_hex_digit(*value.get(escape_end)?) {
                    escape_end += 1;
                }
                if *value.get(escape_end)? != u16::from(b'}') {
                    return None;
                }
                hex_digits = &value[escape_start + 3..escape_end];
                escape_end += 1;
            }
            // Identifiers are carried around the compiler as sequences of char (UTF-16 code
            // units), so a code point that does not fit in a single char cannot be represented
            // here. The value used to be cast straight to char, which silently kept only the low
            // 16 bits: a braced escape for U+10041 was accepted as the identifier "A", and even
            // code points above the U+10FFFF maximum (e.g. U+110041) slipped through the same
            // way. Reject anything outside the BMP rather than aliasing it to a different character.
            // TODO(mattloring): Allow code points >= 0xFFFF (greater than the size of a char).
            let code_point = parse_int(hex_digits, 0x10).ok()?;
            if code_point > 0xffff {
                return None;
            }
            let ch = code_point as u16;
            if !Identifiers::is_identifier_part(ch) {
                return None;
            }
            let mut new_value = value[..escape_start].to_vec();
            new_value.push(ch);
            new_value.extend_from_slice(&value[escape_end..]);
            value = new_value;
        }
        Some(JsString::from_units(value))
    }
    // port: Scanner#scanStringLiteral
    fn scan_string_literal(&mut self, begin_index: i32, terminator: u16) -> Token {
        // String literals might span multiple lines.
        let starting_position = self.get_position_at(begin_index);
        let mut has_unescaped_unicode_line_or_paragraph_separator = false;
        while self.peek_string_literal_char(terminator) {
            let c = self.peek_char();
            has_unescaped_unicode_line_or_paragraph_separator =
                has_unescaped_unicode_line_or_paragraph_separator
                    || c == '\u{2028}' as u16
                    || c == '\u{2029}' as u16;
            if !self.skip_string_literal_char() {
                return StringLiteralToken::new(
                    self.get_token_string(begin_index),
                    self.get_token_range_at(starting_position),
                    has_unescaped_unicode_line_or_paragraph_separator,
                );
            }
        }
        if self.peek_char() != terminator {
            self.report_error_at(
                starting_position.clone(),
                "Unterminated string literal",
                &[],
            );
        } else {
            self.next_char();
        }
        StringLiteralToken::new(
            self.get_token_string(begin_index),
            self.get_token_range_at(starting_position),
            has_unescaped_unicode_line_or_paragraph_separator,
        )
    }
    // port: Scanner#scanTemplateLiteral
    fn scan_template_literal(&mut self, begin_index: i32) -> Token {
        if self.is_at_end() {
            let position = self.get_position_at(begin_index);
            self.report_error_at(position, "Unterminated template literal", &[]);
        }
        self.next_template_literal_token_shared(
            TokenType::NO_SUBSTITUTION_TEMPLATE,
            TokenType::TEMPLATE_HEAD,
        )
    }
    // port: Scanner#nextTemplateLiteralTokenShared
    fn next_template_literal_token_shared(
        &mut self,
        end_type: TokenType,
        middle_type: TokenType,
    ) -> Token {
        let begin_index = self.index;
        // Save the starting position to use with the multi-line safe version of getTokenRange().
        let starting_position = self.get_position_at(begin_index);
        let skip_template_characters_result = self.skip_template_characters();
        if self.is_at_end() {
            self.report_error_at(
                starting_position.clone(),
                "Unterminated template literal",
                &[],
            );
        }
        let value = self.get_token_string(begin_index);
        let type_ = match self.peek_char() {
            ch if ch == u16::from(b'`') => {
                self.next_char();
                end_type
            }
            ch if ch == u16::from(b'$') => {
                self.next_char(); // $
                self.next_char(); // {
                middle_type
            }
            _ => end_type, // Should have reported error already
        };
        TemplateLiteralToken::new(
            type_,
            value,
            skip_template_characters_result.get_error_message(),
            skip_template_characters_result.get_error_level(),
            Some(skip_template_characters_result.get_position()),
            self.get_token_range_at(starting_position),
        )
    }
    // port: Scanner#getTokenString
    fn get_token_string(&self, begin_index: i32) -> JsString {
        self.source
            .contents
            .substring(begin_index as usize, self.index as usize)
    }
    // port: Scanner#peekStringLiteralChar
    fn peek_string_literal_char(&self, terminator: u16) -> bool {
        !self.is_at_end()
            && self.peek_char() != terminator
            && !Self::is_string_line_terminator(self.peek_char())
    }
    // port: Scanner#skipStringLiteralChar
    fn skip_string_literal_char(&mut self) -> bool {
        if self.peek(u16::from(b'\\')) {
            return self.skip_string_literal_escape_sequence();
        }
        self.next_char();
        true
    }
    // port: Scanner#skipTemplateCharacters
    fn skip_template_characters(&mut self) -> SkipTemplateCharactersResult {
        let mut result = self.create_skip_template_characters_result(None, None);
        while !self.is_at_end() {
            match self.peek_char() {
                ch if ch == u16::from(b'`') => return result,
                ch if ch == u16::from(b'\\') => {
                    // There might be multiple errors. Take the first one but continue scanning
                    let new_error = self.skip_template_literal_escape_sequence();
                    if let Some(new_error) = new_error.filter(|_| !result.has_error()) {
                        result = new_error;
                    }
                }
                ch if ch == u16::from(b'$') => {
                    if self.peek_char_at(1) == u16::from(b'{') {
                        return result;
                    }
                    // Fall through.
                    self.next_char();
                }
                _ => {
                    self.next_char();
                }
            }
        }
        result
    }
    // for "skipHexDigit() && skipHexDigit()"
    // port: Scanner#skipTemplateLiteralEscapeSequence
    fn skip_template_literal_escape_sequence(&mut self) -> Option<SkipTemplateCharactersResult> {
        self.next_char();
        if self.is_at_end() {
            self.report_error("Unterminated template literal escape sequence", &[]);
            return None;
        }
        if Self::is_line_terminator(self.peek_char()) {
            self.skip_line_terminator();
            return None;
        }
        let next = self.next_char();
        match next {
            ch if ch == u16::from(b'0') => {
                if Self::is_decimal_digit(self.peek_char()) {
                    return Some(self.create_skip_template_characters_result(
                        Some(JsString::from("Invalid escape sequence")),
                        Some(ErrorLevel::ERROR),
                    ));
                }
                None
            }
            ch if (u16::from(b'1')..=u16::from(b'9')).contains(&ch) => {
                Some(self.create_skip_template_characters_result(
                    Some(JsString::from("Invalid escape sequence")),
                    Some(ErrorLevel::ERROR),
                ))
            }
            ch if ch == u16::from(b'x') => {
                let double_hex_digit = self.skip_hex_digit() && self.skip_hex_digit();
                if !double_hex_digit {
                    return Some(self.create_skip_template_characters_result(
                        Some(JsString::from("Hex digit expected")),
                        Some(ErrorLevel::ERROR),
                    ));
                }
                None
            }
            ch if ch == u16::from(b'u') => {
                if self.peek(u16::from(b'{')) {
                    self.next_char();
                    if self.peek(u16::from(b'}')) {
                        return Some(self.create_skip_template_characters_result(
                            Some(JsString::from("Empty unicode escape")),
                            Some(ErrorLevel::ERROR),
                        ));
                    }
                    let mut all_hex_digits = true;
                    while !self.peek(u16::from(b'}')) && all_hex_digits {
                        all_hex_digits = all_hex_digits && self.skip_hex_digit();
                    }
                    if !all_hex_digits {
                        return Some(self.create_skip_template_characters_result(
                            Some(JsString::from("Hex digit expected")),
                            Some(ErrorLevel::ERROR),
                        ));
                    }
                    self.next_char();
                    None
                } else {
                    let quad_hex_digit = self.skip_hex_digit()
                        && self.skip_hex_digit()
                        && self.skip_hex_digit()
                        && self.skip_hex_digit();
                    if !quad_hex_digit {
                        return Some(self.create_skip_template_characters_result(
                            Some(JsString::from("Hex digit expected")),
                            Some(ErrorLevel::ERROR),
                        ));
                    }
                    None
                }
                // https://tc39.es/ecma262/#prod-TemplateEscapeSequence
            }
            // special meaning in template literal
            ch if ch == u16::from(b'\\')
                || ch == u16::from(b'b')
                || ch == u16::from(b'f')
                || ch == u16::from(b'n')
                || ch == u16::from(b'r')
                || ch == u16::from(b't')
                || ch == u16::from(b'v')
                || ch == u16::from(b'$')
                || ch == u16::from(b'`') =>
            {
                None
            }
            ch if ch == u16::from(b'\'') => {
                // special the error message for a single quote
                let message = format_message(
                    &JsString::from("Unnecessary escape: \"\\%s\" is equivalent to just \"%s\""),
                    &[
                        JsString::from_units(vec![next]),
                        JsString::from_units(vec![next]),
                    ],
                );
                Some(self.create_skip_template_characters_result(
                    Some(message),
                    Some(ErrorLevel::WARNING),
                ))
            }
            _ => {
                let message = format_message(
                    &JsString::from("Unnecessary escape: '\\%s' is equivalent to just '%s'"),
                    &[
                        JsString::from_units(vec![next]),
                        JsString::from_units(vec![next]),
                    ],
                );
                Some(self.create_skip_template_characters_result(
                    Some(message),
                    Some(ErrorLevel::WARNING),
                ))
            }
        }
    }
    // for "skipHexDigit() && skipHexDigit()"
    // port: Scanner#skipStringLiteralEscapeSequence
    fn skip_string_literal_escape_sequence(&mut self) -> bool {
        self.next_char();
        if self.is_at_end() {
            self.report_error("Unterminated string literal escape sequence", &[]);
            return false;
        }
        if Self::is_string_line_terminator(self.peek_char()) {
            self.skip_line_terminator();
            return true;
        }
        let next = self.next_char();
        match next {
            ch if ch == u16::from(b'\'')
                || ch == u16::from(b'"')
                || ch == u16::from(b'`')
                || ch == u16::from(b'\\')
                || ch == u16::from(b'b')
                || ch == u16::from(b'f')
                || ch == u16::from(b'n')
                || ch == u16::from(b'r')
                || ch == u16::from(b't')
                || ch == u16::from(b'v')
                || ch == u16::from(b'0') =>
            {
                return true;
            }
            ch if ch == u16::from(b'x') => {
                let double_hex_digit = self.skip_hex_digit() && self.skip_hex_digit();
                if !double_hex_digit {
                    self.report_error("Hex digit expected", &[]);
                }
                return double_hex_digit;
            }
            ch if ch == u16::from(b'u') => {
                if self.peek(u16::from(b'{')) {
                    self.next_char();
                    if self.peek(u16::from(b'}')) {
                        self.report_error("Empty unicode escape", &[]);
                        return false;
                    }
                    let mut all_hex_digits = true;
                    while !self.peek(u16::from(b'}')) && all_hex_digits {
                        all_hex_digits = all_hex_digits && self.skip_hex_digit();
                    }
                    if !all_hex_digits {
                        self.report_error("Hex digit expected", &[]);
                    }
                    self.next_char();
                    return all_hex_digits;
                } else {
                    let quad_hex_digit = self.skip_hex_digit()
                        && self.skip_hex_digit()
                        && self.skip_hex_digit()
                        && self.skip_hex_digit();
                    if !quad_hex_digit {
                        self.report_error("Hex digit expected", &[]);
                    }
                    return quad_hex_digit;
                }
            }
            _ => {}
        }
        if next == u16::from(b'/') {
            // Don't warn for '\/' (for now) since it's common in "<\/script>"
        } else {
            self.report_warning(
                "Unnecessary escape: '\\%s' is equivalent to just '%s'",
                &[
                    JsString::from_units(vec![next]),
                    JsString::from_units(vec![next]),
                ],
            );
        }
        true
    }
    // port: Scanner#skipHexDigit
    fn skip_hex_digit(&mut self) -> bool {
        if !self.peek_hex_digit() {
            return false;
        }
        self.next_char();
        true
    }
    // port: Scanner#skipLineTerminator
    fn skip_line_terminator(&mut self) {
        let first = self.next_char();
        if first == u16::from(b'\r') && self.peek(u16::from(b'\n')) {
            self.next_char();
        }
    }
    // port: Scanner#scanFractionalNumericLiteral
    fn scan_fractional_numeric_literal(&mut self, begin_token: i32) -> Token {
        if self.peek(u16::from(b'.')) {
            self.next_char();
            self.skip_decimal_digits_leading(/* hasLeadingDigit= */ false);
        }
        self.scan_exponent_of_numeric_literal(begin_token)
    }
    // port: Scanner#scanExponentOfNumericLiteral
    fn scan_exponent_of_numeric_literal(&mut self, begin_token: i32) -> Token {
        match self.peek_char() {
            ch if ch == u16::from(b'e') || ch == u16::from(b'E') => {
                self.next_char();
                match self.peek_char() {
                    ch if ch == u16::from(b'+') || ch == u16::from(b'-') => {
                        self.next_char();
                    }
                    _ => {} // fall out
                }
                if !Self::is_decimal_digit(self.peek_char()) {
                    self.report_error("Exponent part must contain at least one digit", &[]);
                }
                self.skip_decimal_digits();
            }
            _ => {}
        }
        LiteralToken::new(
            TokenType::NUMBER,
            self.get_token_string(begin_token),
            self.get_token_range(begin_token),
        )
    }
    // port: Scanner#skipDecimalDigits
    fn skip_decimal_digits(&mut self) {
        self.skip_decimal_digits_leading(/* hasLeadingDigit= */ true);
    }
    // port: Scanner#skipDecimalDigits
    fn skip_decimal_digits_leading(&mut self, has_leading_digit: bool) {
        let mut has_digit = has_leading_digit;
        let mut ch = self.peek_char();
        while Self::is_decimal_digit(ch) || ch == u16::from(b'_') {
            self.next_char();
            if ch == u16::from(b'_') {
                if has_digit && Self::is_decimal_digit(self.peek_char()) {
                    self.next_char();
                } else {
                    self.report_error("Trailing numeric separator", &[]);
                }
            } else {
                has_digit = true;
            }
            ch = self.peek_char();
        }
    }
    // port: Scanner#isDecimalDigit
    fn is_decimal_digit(ch: u16) -> bool {
        (u16::from(b'0')..=u16::from(b'9')).contains(&ch)
    }
    // port: Scanner#peekHexDigit
    fn peek_hex_digit(&self) -> bool {
        Self::is_hex_digit(self.peek_char())
    }
    // port: Scanner#isHexDigit
    fn is_hex_digit(ch: u16) -> bool {
        digit(ch, 0x10) >= 0
    }
    // port: Scanner#skipHexDigits
    fn skip_hex_digits(&mut self) {
        let mut ch = self.peek_char();
        while Self::is_hex_digit(ch) || ch == u16::from(b'_') {
            self.next_char();
            if ch == u16::from(b'_') {
                if self.peek_hex_digit() {
                    self.next_char();
                } else {
                    self.report_error("Trailing numeric separator", &[]);
                }
            }
            ch = self.peek_char();
        }
    }
    // port: Scanner#skipOctalDigits
    fn skip_octal_digits(&mut self) {
        let mut ch = self.peek_char();
        while Self::is_octal_digit(ch) || ch == u16::from(b'_') {
            self.next_char();
            if ch == u16::from(b'_') {
                if Self::is_octal_digit(self.peek_char()) {
                    self.next_char();
                } else {
                    self.report_error("Trailing numeric separator", &[]);
                }
            }
            ch = self.peek_char();
        }
    }
    // port: Scanner#isOctalDigit
    fn is_octal_digit(ch: u16) -> bool {
        Self::value_of_octal_digit(ch) >= 0
    }
    // port: Scanner#valueOfOctalDigit
    fn value_of_octal_digit(ch: u16) -> i32 {
        match ch {
            ch if (u16::from(b'0')..=u16::from(b'7')).contains(&ch) => {
                i32::from(ch) - i32::from(b'0')
            }
            _ => -1,
        }
    }
    // port: Scanner#skipBinaryDigits
    fn skip_binary_digits(&mut self) {
        let mut ch = self.peek_char();
        while Self::is_binary_digit(ch) || ch == u16::from(b'_') {
            self.next_char();
            if ch == u16::from(b'_') {
                if Self::is_binary_digit(self.peek_char()) {
                    self.next_char();
                } else {
                    self.report_error("Trailing numeric separator", &[]);
                }
            }
            ch = self.peek_char();
        }
    }
    // port: Scanner#isBinaryDigit
    fn is_binary_digit(ch: u16) -> bool {
        Self::value_of_binary_digit(ch) >= 0
    }
    // port: Scanner#valueOfBinaryDigit
    fn value_of_binary_digit(ch: u16) -> i32 {
        match ch {
            ch if ch == u16::from(b'0') => 0,
            ch if ch == u16::from(b'1') => 1,
            _ => -1,
        }
    }
    // port: Scanner#nextChar
    fn next_char(&mut self) -> u16 {
        if self.is_at_end() {
            return 0;
        }
        let ch = self.source.contents.char_at(self.index as usize);
        self.index += 1;
        ch
    }
    // port: Scanner#peek
    fn peek(&self, ch: u16) -> bool {
        self.peek_char() == ch
    }
    // port: Scanner#peekChar
    fn peek_char(&self) -> u16 {
        self.peek_char_at(0)
    }
    // port: Scanner#peekChar
    fn peek_char_at(&self, offset: i32) -> u16 {
        if !self.is_valid_index(self.index + offset) {
            0
        } else {
            self.source.contents.char_at((self.index + offset) as usize)
        }
    }
    // port: Scanner#reportError
    fn report_error(&mut self, format: &str, arguments: &[JsString]) {
        let position = self.get_position();
        self.report_error_at(position, format, arguments);
    }
    // port: Scanner#reportError
    fn report_error_at(&self, position: SourcePosition, format: &str, arguments: &[JsString]) {
        self.error_reporter
            .report_error(position, format, arguments);
    }
    // port: Scanner#reportWarning
    fn report_warning(&mut self, format: &str, arguments: &[JsString]) {
        let position = self.get_position();
        self.error_reporter
            .report_warning(position, format, arguments);
    }
    // port: Scanner#incTypeParameterLevel
    pub fn inc_type_parameter_level(&mut self) {
        self.type_parameter_level = self.type_parameter_level.wrapping_add(1);
    }
    // port: Scanner#decTypeParameterLevel
    pub fn dec_type_parameter_level(&mut self) {
        self.type_parameter_level = self.type_parameter_level.wrapping_sub(1);
    }
    // port: Scanner#createSkipTemplateCharactersResult
    fn create_skip_template_characters_result(
        &mut self,
        message: Option<JsString>,
        error_level: Option<ErrorLevel>,
    ) -> SkipTemplateCharactersResult {
        SkipTemplateCharactersResult::new(message, error_level, self.get_position())
    }
}

struct SkipTemplateCharactersResult {
    error_message: Option<JsString>,
    position: SourcePosition,
    error_level: Option<ErrorLevel>,
}
impl SkipTemplateCharactersResult {
    // port: Scanner.SkipTemplateCharactersResult#<init>
    fn new(
        message: Option<JsString>,
        error_level: Option<ErrorLevel>,
        position: SourcePosition,
    ) -> Self {
        Self {
            error_message: message,
            error_level,
            position,
        }
    }
    // port: Scanner.SkipTemplateCharactersResult#getErrorMessage
    fn get_error_message(&self) -> Option<JsString> {
        self.error_message.clone()
    }
    // port: Scanner.SkipTemplateCharactersResult#getErrorLevel
    fn get_error_level(&self) -> Option<ErrorLevel> {
        self.error_level
    }
    // port: Scanner.SkipTemplateCharactersResult#getPosition
    fn get_position(&self) -> SourcePosition {
        self.position.clone()
    }
    // port: Scanner.SkipTemplateCharactersResult#hasError
    fn has_error(&self) -> bool {
        self.error_message.is_some()
    }
}
