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
//   src/com/google/javascript/jscomp/parsing/parser/Keywords.java.

use super::{JsString, token_type::TokenType};

/// The JavaScript keywords.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keywords {
    // 7.6.1.1 Keywords
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
    // 7.6.1.2 Future Reserved Words
    CLASS,
    CONST,
    ENUM,
    EXPORT,
    EXTENDS,
    IMPORT,
    SUPER,
    // Future Reserved Words in a strict context
    IMPLEMENTS,
    INTERFACE,
    LET,
    PACKAGE,
    PRIVATE,
    PROTECTED,
    PUBLIC,
    STATIC,
    YIELD,
    // 7.8 Literals
    NULL,
    TRUE,
    FALSE,
}
// Java enum singleton fields, constructed in declaration order.
struct KeywordsData {
    value: JsString,
    type_: TokenType,
}
impl KeywordsData {
    // port: Keywords#<init>
    fn new(value: JsString, type_: TokenType) -> Self {
        Self { value, type_ }
    }
}
impl Keywords {
    fn data(self) -> KeywordsData {
        let value: &str = match self {
            Self::BREAK => "break",
            Self::CASE => "case",
            Self::CATCH => "catch",
            Self::CONTINUE => "continue",
            Self::DEBUGGER => "debugger",
            Self::DEFAULT => "default",
            Self::DELETE => "delete",
            Self::DO => "do",
            Self::ELSE => "else",
            Self::FINALLY => "finally",
            Self::FOR => "for",
            Self::FUNCTION => "function",
            Self::IF => "if",
            Self::IN => "in",
            Self::INSTANCEOF => "instanceof",
            Self::NEW => "new",
            Self::RETURN => "return",
            Self::SWITCH => "switch",
            Self::THIS => "this",
            Self::THROW => "throw",
            Self::TRY => "try",
            Self::TYPEOF => "typeof",
            Self::VAR => "var",
            Self::VOID => "void",
            Self::WHILE => "while",
            Self::WITH => "with",
            Self::CLASS => "class",
            Self::CONST => "const",
            Self::ENUM => "enum",
            Self::EXPORT => "export",
            Self::EXTENDS => "extends",
            Self::IMPORT => "import",
            Self::SUPER => "super",
            Self::IMPLEMENTS => "implements",
            Self::INTERFACE => "interface",
            Self::LET => "let",
            Self::PACKAGE => "package",
            Self::PRIVATE => "private",
            Self::PROTECTED => "protected",
            Self::PUBLIC => "public",
            Self::STATIC => "static",
            Self::YIELD => "yield",
            Self::NULL => "null",
            Self::TRUE => "true",
            Self::FALSE => "false",
        };
        let type_ = match self {
            Self::BREAK => TokenType::BREAK,
            Self::CASE => TokenType::CASE,
            Self::CATCH => TokenType::CATCH,
            Self::CONTINUE => TokenType::CONTINUE,
            Self::DEBUGGER => TokenType::DEBUGGER,
            Self::DEFAULT => TokenType::DEFAULT,
            Self::DELETE => TokenType::DELETE,
            Self::DO => TokenType::DO,
            Self::ELSE => TokenType::ELSE,
            Self::FINALLY => TokenType::FINALLY,
            Self::FOR => TokenType::FOR,
            Self::FUNCTION => TokenType::FUNCTION,
            Self::IF => TokenType::IF,
            Self::IN => TokenType::IN,
            Self::INSTANCEOF => TokenType::INSTANCEOF,
            Self::NEW => TokenType::NEW,
            Self::RETURN => TokenType::RETURN,
            Self::SWITCH => TokenType::SWITCH,
            Self::THIS => TokenType::THIS,
            Self::THROW => TokenType::THROW,
            Self::TRY => TokenType::TRY,
            Self::TYPEOF => TokenType::TYPEOF,
            Self::VAR => TokenType::VAR,
            Self::VOID => TokenType::VOID,
            Self::WHILE => TokenType::WHILE,
            Self::WITH => TokenType::WITH,
            Self::CLASS => TokenType::CLASS,
            Self::CONST => TokenType::CONST,
            Self::ENUM => TokenType::ENUM,
            Self::EXPORT => TokenType::EXPORT,
            Self::EXTENDS => TokenType::EXTENDS,
            Self::IMPORT => TokenType::IMPORT,
            Self::SUPER => TokenType::SUPER,
            Self::IMPLEMENTS => TokenType::IMPLEMENTS,
            Self::INTERFACE => TokenType::INTERFACE,
            Self::LET => TokenType::LET,
            Self::PACKAGE => TokenType::PACKAGE,
            Self::PRIVATE => TokenType::PRIVATE,
            Self::PROTECTED => TokenType::PROTECTED,
            Self::PUBLIC => TokenType::PUBLIC,
            Self::STATIC => TokenType::STATIC,
            Self::YIELD => TokenType::YIELD,
            Self::NULL => TokenType::NULL,
            Self::TRUE => TokenType::TRUE,
            Self::FALSE => TokenType::FALSE,
        };
        KeywordsData::new(JsString::from(value), type_)
    }
    pub fn value(self) -> JsString {
        self.data().value
    }
    pub fn type_(self) -> TokenType {
        self.data().type_
    }

    // port: Keywords#toString
    pub fn to_string_utf16(self) -> super::JsString {
        self.value()
    }
    // port: Keywords#isKeyword
    pub fn is_keyword(value: &JsString) -> bool {
        Self::get_by_name(value).is_some()
    }
    // port: Keywords#isKeyword
    pub fn is_keyword_type(token: TokenType) -> bool {
        Self::get_by_type(token).is_some()
    }
    /// Returns true if {@code token} is a "future reserved word" which can
    /// be used as a variable identifier, but only in non-strict mode.
    // port: Keywords#isStrictKeyword
    pub fn is_strict_keyword(token: TokenType) -> bool {
        matches!(
            token,
            TokenType::IMPLEMENTS
                | TokenType::INTERFACE
                | TokenType::LET
                | TokenType::PACKAGE
                | TokenType::PRIVATE
                | TokenType::PROTECTED
                | TokenType::PUBLIC
                | TokenType::STATIC
                | TokenType::YIELD
        )
    }
    // port: Keywords#getTokenType
    pub fn get_token_type(value: &JsString) -> TokenType {
        Self::get_by_name(value).expect("keyword").type_()
    }
    // port: Keywords#get
    pub fn get_by_name(value: &JsString) -> Option<Self> {
        match value {
            value if value == "break" => Some(Self::BREAK),
            value if value == "case" => Some(Self::CASE),
            value if value == "catch" => Some(Self::CATCH),
            value if value == "continue" => Some(Self::CONTINUE),
            value if value == "debugger" => Some(Self::DEBUGGER),
            value if value == "default" => Some(Self::DEFAULT),
            value if value == "delete" => Some(Self::DELETE),
            value if value == "do" => Some(Self::DO),
            value if value == "else" => Some(Self::ELSE),
            value if value == "finally" => Some(Self::FINALLY),
            value if value == "for" => Some(Self::FOR),
            value if value == "function" => Some(Self::FUNCTION),
            value if value == "if" => Some(Self::IF),
            value if value == "in" => Some(Self::IN),
            value if value == "instanceof" => Some(Self::INSTANCEOF),
            value if value == "new" => Some(Self::NEW),
            value if value == "return" => Some(Self::RETURN),
            value if value == "switch" => Some(Self::SWITCH),
            value if value == "this" => Some(Self::THIS),
            value if value == "throw" => Some(Self::THROW),
            value if value == "try" => Some(Self::TRY),
            value if value == "typeof" => Some(Self::TYPEOF),
            value if value == "var" => Some(Self::VAR),
            value if value == "void" => Some(Self::VOID),
            value if value == "while" => Some(Self::WHILE),
            value if value == "with" => Some(Self::WITH),
            value if value == "class" => Some(Self::CLASS),
            value if value == "const" => Some(Self::CONST),
            value if value == "enum" => Some(Self::ENUM),
            value if value == "export" => Some(Self::EXPORT),
            value if value == "extends" => Some(Self::EXTENDS),
            value if value == "import" => Some(Self::IMPORT),
            value if value == "super" => Some(Self::SUPER),
            value if value == "implements" => Some(Self::IMPLEMENTS),
            value if value == "interface" => Some(Self::INTERFACE),
            value if value == "let" => Some(Self::LET),
            value if value == "package" => Some(Self::PACKAGE),
            value if value == "private" => Some(Self::PRIVATE),
            value if value == "protected" => Some(Self::PROTECTED),
            value if value == "public" => Some(Self::PUBLIC),
            value if value == "static" => Some(Self::STATIC),
            value if value == "yield" => Some(Self::YIELD),
            value if value == "null" => Some(Self::NULL),
            value if value == "true" => Some(Self::TRUE),
            value if value == "false" => Some(Self::FALSE),
            _ => None,
        }
    }
    // port: Keywords#get
    pub fn get_by_type(token: TokenType) -> Option<Self> {
        match token {
            TokenType::BREAK => Some(Self::BREAK),
            TokenType::CASE => Some(Self::CASE),
            TokenType::CATCH => Some(Self::CATCH),
            TokenType::CONTINUE => Some(Self::CONTINUE),
            TokenType::DEBUGGER => Some(Self::DEBUGGER),
            TokenType::DEFAULT => Some(Self::DEFAULT),
            TokenType::DELETE => Some(Self::DELETE),
            TokenType::DO => Some(Self::DO),
            TokenType::ELSE => Some(Self::ELSE),
            TokenType::FINALLY => Some(Self::FINALLY),
            TokenType::FOR => Some(Self::FOR),
            TokenType::FUNCTION => Some(Self::FUNCTION),
            TokenType::IF => Some(Self::IF),
            TokenType::IN => Some(Self::IN),
            TokenType::INSTANCEOF => Some(Self::INSTANCEOF),
            TokenType::NEW => Some(Self::NEW),
            TokenType::RETURN => Some(Self::RETURN),
            TokenType::SWITCH => Some(Self::SWITCH),
            TokenType::THIS => Some(Self::THIS),
            TokenType::THROW => Some(Self::THROW),
            TokenType::TRY => Some(Self::TRY),
            TokenType::TYPEOF => Some(Self::TYPEOF),
            TokenType::VAR => Some(Self::VAR),
            TokenType::VOID => Some(Self::VOID),
            TokenType::WHILE => Some(Self::WHILE),
            TokenType::WITH => Some(Self::WITH),
            TokenType::CLASS => Some(Self::CLASS),
            TokenType::CONST => Some(Self::CONST),
            TokenType::ENUM => Some(Self::ENUM),
            TokenType::EXPORT => Some(Self::EXPORT),
            TokenType::EXTENDS => Some(Self::EXTENDS),
            TokenType::IMPORT => Some(Self::IMPORT),
            TokenType::SUPER => Some(Self::SUPER),
            TokenType::IMPLEMENTS => Some(Self::IMPLEMENTS),
            TokenType::INTERFACE => Some(Self::INTERFACE),
            TokenType::LET => Some(Self::LET),
            TokenType::PACKAGE => Some(Self::PACKAGE),
            TokenType::PRIVATE => Some(Self::PRIVATE),
            TokenType::PROTECTED => Some(Self::PROTECTED),
            TokenType::PUBLIC => Some(Self::PUBLIC),
            TokenType::STATIC => Some(Self::STATIC),
            TokenType::YIELD => Some(Self::YIELD),
            TokenType::NULL => Some(Self::NULL),
            TokenType::TRUE => Some(Self::TRUE),
            TokenType::FALSE => Some(Self::FALSE),
            _ => None,
        }
    }
}
