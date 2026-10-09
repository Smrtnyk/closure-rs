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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/parsing/parser/TokenType.java.

use super::{JsString, keywords::Keywords};

// 7.5 Tokens
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenType {
    END_OF_FILE,
    ERROR,
    // 7.6 Identifier Names and Identifiers
    IDENTIFIER,
    // 7.6.1.1 keywords
    BREAK,
    CASE,
    CATCH,
    CONTINUE,
    DEBUGGER,
    DEFAULT,
    DELETE,
    DO,
    ELSE,
    FINALLY,
    FOR,
    FUNCTION,
    IF,
    IN,
    INSTANCEOF,
    NEW,
    RETURN,
    SWITCH,
    THIS,
    THROW,
    TRY,
    TYPEOF,
    VAR,
    VOID,
    WHILE,
    WITH,
    // 7.6.1.2 Future reserved words
    CLASS,
    CONST,
    ENUM,
    EXPORT,
    EXTENDS,
    IMPORT,
    SUPER,
    // Future reserved words in strict mode
    IMPLEMENTS,
    INTERFACE,
    LET,
    PACKAGE,
    PRIVATE,
    PROTECTED,
    PUBLIC,
    STATIC,
    YIELD,
    // 7.7 Punctuators
    OPEN_CURLY,
    CLOSE_CURLY,
    OPEN_PAREN,
    CLOSE_PAREN,
    OPEN_SQUARE,
    CLOSE_SQUARE,
    PERIOD,
    SEMI_COLON,
    COMMA,
    OPEN_ANGLE,
    CLOSE_ANGLE,
    LESS_EQUAL,
    GREATER_EQUAL,
    ARROW,
    EQUAL_EQUAL,
    NOT_EQUAL,
    EQUAL_EQUAL_EQUAL,
    NOT_EQUAL_EQUAL,
    PLUS,
    MINUS,
    STAR,
    STAR_STAR,
    PERCENT,
    PLUS_PLUS,
    MINUS_MINUS,
    LEFT_SHIFT,
    RIGHT_SHIFT,
    UNSIGNED_RIGHT_SHIFT,
    AMPERSAND,
    BAR,
    CARET,
    BANG,
    TILDE,
    AND,
    OR,
    QUESTION,
    QUESTION_QUESTION,
    QUESTION_DOT,
    COLON,
    EQUAL,
    PLUS_EQUAL,
    MINUS_EQUAL,
    STAR_EQUAL,
    STAR_STAR_EQUAL,
    PERCENT_EQUAL,
    LEFT_SHIFT_EQUAL,
    RIGHT_SHIFT_EQUAL,
    UNSIGNED_RIGHT_SHIFT_EQUAL,
    AMPERSAND_EQUAL,
    BAR_EQUAL,
    CARET_EQUAL,
    SLASH,
    SLASH_EQUAL,
    // Logical Assignment Punctuators
    AND_EQUAL,
    OR_EQUAL,
    QUESTION_QUESTION_EQUAL,
    // 7.8 Literals
    NULL,
    TRUE,
    FALSE,
    NUMBER,
    STRING,
    BIGINT,
    REGULAR_EXPRESSION,
    // Harmony extensions
    ELLIPSIS,
    // 12.2.9 Template Literals
    // Template literal tokens corresponding to different parts of the literal
    // Eg: `hello` is scanned as a single NO_SUBSTITUTION_TEMPLATE: hello
    // `hello${world}!` is scanned as TEMPLATE_HEAD: hello, TEMPLATE_MIDDLE: world,
    // and TEMPLATE_TAIL: !
    TEMPLATE_HEAD,
    TEMPLATE_MIDDLE,
    TEMPLATE_TAIL,
    NO_SUBSTITUTION_TEMPLATE,
    // TypeScript
    TYPE,
    DECLARE,
    MODULE,
    NAMESPACE,
}
// The enum discriminant identifies the Java singleton; its declared fields live here.
struct TokenTypeData {
    value: Option<JsString>,
}
impl Default for TokenTypeData {
    // port: TokenType#<init>
    fn default() -> Self {
        Self::new(None)
    }
}
impl TokenTypeData {
    // port: TokenType#<init>
    fn new(value: Option<JsString>) -> Self {
        Self { value }
    }
}
impl TokenType {
    pub const ALL: &'static [Self] = &[
        Self::END_OF_FILE,
        Self::ERROR,
        Self::IDENTIFIER,
        Self::BREAK,
        Self::CASE,
        Self::CATCH,
        Self::CONTINUE,
        Self::DEBUGGER,
        Self::DEFAULT,
        Self::DELETE,
        Self::DO,
        Self::ELSE,
        Self::FINALLY,
        Self::FOR,
        Self::FUNCTION,
        Self::IF,
        Self::IN,
        Self::INSTANCEOF,
        Self::NEW,
        Self::RETURN,
        Self::SWITCH,
        Self::THIS,
        Self::THROW,
        Self::TRY,
        Self::TYPEOF,
        Self::VAR,
        Self::VOID,
        Self::WHILE,
        Self::WITH,
        Self::CLASS,
        Self::CONST,
        Self::ENUM,
        Self::EXPORT,
        Self::EXTENDS,
        Self::IMPORT,
        Self::SUPER,
        Self::IMPLEMENTS,
        Self::INTERFACE,
        Self::LET,
        Self::PACKAGE,
        Self::PRIVATE,
        Self::PROTECTED,
        Self::PUBLIC,
        Self::STATIC,
        Self::YIELD,
        Self::OPEN_CURLY,
        Self::CLOSE_CURLY,
        Self::OPEN_PAREN,
        Self::CLOSE_PAREN,
        Self::OPEN_SQUARE,
        Self::CLOSE_SQUARE,
        Self::PERIOD,
        Self::SEMI_COLON,
        Self::COMMA,
        Self::OPEN_ANGLE,
        Self::CLOSE_ANGLE,
        Self::LESS_EQUAL,
        Self::GREATER_EQUAL,
        Self::ARROW,
        Self::EQUAL_EQUAL,
        Self::NOT_EQUAL,
        Self::EQUAL_EQUAL_EQUAL,
        Self::NOT_EQUAL_EQUAL,
        Self::PLUS,
        Self::MINUS,
        Self::STAR,
        Self::STAR_STAR,
        Self::PERCENT,
        Self::PLUS_PLUS,
        Self::MINUS_MINUS,
        Self::LEFT_SHIFT,
        Self::RIGHT_SHIFT,
        Self::UNSIGNED_RIGHT_SHIFT,
        Self::AMPERSAND,
        Self::BAR,
        Self::CARET,
        Self::BANG,
        Self::TILDE,
        Self::AND,
        Self::OR,
        Self::QUESTION,
        Self::QUESTION_QUESTION,
        Self::QUESTION_DOT,
        Self::COLON,
        Self::EQUAL,
        Self::PLUS_EQUAL,
        Self::MINUS_EQUAL,
        Self::STAR_EQUAL,
        Self::STAR_STAR_EQUAL,
        Self::PERCENT_EQUAL,
        Self::LEFT_SHIFT_EQUAL,
        Self::RIGHT_SHIFT_EQUAL,
        Self::UNSIGNED_RIGHT_SHIFT_EQUAL,
        Self::AMPERSAND_EQUAL,
        Self::BAR_EQUAL,
        Self::CARET_EQUAL,
        Self::SLASH,
        Self::SLASH_EQUAL,
        Self::AND_EQUAL,
        Self::OR_EQUAL,
        Self::QUESTION_QUESTION_EQUAL,
        Self::NULL,
        Self::TRUE,
        Self::FALSE,
        Self::NUMBER,
        Self::STRING,
        Self::BIGINT,
        Self::REGULAR_EXPRESSION,
        Self::ELLIPSIS,
        Self::TEMPLATE_HEAD,
        Self::TEMPLATE_MIDDLE,
        Self::TEMPLATE_TAIL,
        Self::NO_SUBSTITUTION_TEMPLATE,
        Self::TYPE,
        Self::DECLARE,
        Self::MODULE,
        Self::NAMESPACE,
    ];
    pub fn value(self) -> Option<JsString> {
        self.data().value
    }
    fn data(self) -> TokenTypeData {
        let value: Option<&str> = match self {
            Self::END_OF_FILE => Some("End of File"),
            Self::ERROR => Some("error"),
            Self::IDENTIFIER => Some("identifier"),
            Self::OPEN_CURLY => Some("{"),
            Self::CLOSE_CURLY => Some("}"),
            Self::OPEN_PAREN => Some("("),
            Self::CLOSE_PAREN => Some(")"),
            Self::OPEN_SQUARE => Some("["),
            Self::CLOSE_SQUARE => Some("]"),
            Self::PERIOD => Some("."),
            Self::SEMI_COLON => Some(";"),
            Self::COMMA => Some(","),
            Self::OPEN_ANGLE => Some("<"),
            Self::CLOSE_ANGLE => Some(">"),
            Self::LESS_EQUAL => Some("<="),
            Self::GREATER_EQUAL => Some(">="),
            Self::ARROW => Some("=>"),
            Self::EQUAL_EQUAL => Some("=="),
            Self::NOT_EQUAL => Some("!="),
            Self::EQUAL_EQUAL_EQUAL => Some("==="),
            Self::NOT_EQUAL_EQUAL => Some("!=="),
            Self::PLUS => Some("+"),
            Self::MINUS => Some("-"),
            Self::STAR => Some("*"),
            Self::STAR_STAR => Some("**"),
            Self::PERCENT => Some("%"),
            Self::PLUS_PLUS => Some("++"),
            Self::MINUS_MINUS => Some("--"),
            Self::LEFT_SHIFT => Some("<<"),
            Self::RIGHT_SHIFT => Some(">>"),
            Self::UNSIGNED_RIGHT_SHIFT => Some(">>>"),
            Self::AMPERSAND => Some("&"),
            Self::BAR => Some("|"),
            Self::CARET => Some("^"),
            Self::BANG => Some("!"),
            Self::TILDE => Some("~"),
            Self::AND => Some("&&"),
            Self::OR => Some("||"),
            Self::QUESTION => Some("?"),
            Self::QUESTION_QUESTION => Some("??"),
            Self::QUESTION_DOT => Some("?."),
            Self::COLON => Some(":"),
            Self::EQUAL => Some("="),
            Self::PLUS_EQUAL => Some("+="),
            Self::MINUS_EQUAL => Some("-="),
            Self::STAR_EQUAL => Some("*="),
            Self::STAR_STAR_EQUAL => Some("**="),
            Self::PERCENT_EQUAL => Some("%="),
            Self::LEFT_SHIFT_EQUAL => Some("<<="),
            Self::RIGHT_SHIFT_EQUAL => Some(">>="),
            Self::UNSIGNED_RIGHT_SHIFT_EQUAL => Some(">>>="),
            Self::AMPERSAND_EQUAL => Some("&="),
            Self::BAR_EQUAL => Some("|="),
            Self::CARET_EQUAL => Some("^="),
            Self::SLASH => Some("/"),
            Self::SLASH_EQUAL => Some("/="),
            Self::AND_EQUAL => Some("&&="),
            Self::OR_EQUAL => Some("||="),
            Self::QUESTION_QUESTION_EQUAL => Some("??="),
            Self::NUMBER => Some("number literal"),
            Self::STRING => Some("string literal"),
            Self::BIGINT => Some("bigint literal"),
            Self::REGULAR_EXPRESSION => Some("regular expression literal"),
            Self::ELLIPSIS => Some("..."),
            Self::TEMPLATE_HEAD => Some("template head"),
            Self::TEMPLATE_MIDDLE => Some("template middle"),
            Self::TEMPLATE_TAIL => Some("template tail"),
            Self::NO_SUBSTITUTION_TEMPLATE => Some("no substitution template"),
            _ => None,
        };
        match value {
            Some(value) => TokenTypeData::new(Some(JsString::from(value))),
            None => TokenTypeData::default(),
        }
    }
    // port: TokenType#toString
    pub fn to_string_utf16(self) -> JsString {
        // TODO: straighten this out
        self.value().unwrap_or_else(|| {
            Keywords::get_by_type(self)
                .expect("keyword token type")
                .value()
        })
    }
}
impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TokenType spellings come only from the fixed ASCII token and keyword tables.
        f.write_str(&self.to_string_utf16().to_string_lossy())
    }
}
