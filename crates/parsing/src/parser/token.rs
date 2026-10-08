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
//   src/com/google/javascript/jscomp/parsing/parser/Token.java.

use super::{
    JsString,
    identifier_token::IdentifierToken,
    literal_token::LiteralToken,
    string_literal_token::StringLiteralToken,
    template_literal_token::TemplateLiteralToken,
    token_type::TokenType,
    util::{SourcePosition, SourceRange},
};

#[derive(Clone, Debug)]
pub enum TokenData {
    Plain,
    Identifier(IdentifierToken),
    Literal(LiteralToken),
    StringLiteral(StringLiteralToken),
    TemplateLiteral(TemplateLiteralToken),
}

/// A Token in a javascript file.
/// Immutable.
/// A plain old data structure. Should contain data members and simple accessors only.
#[derive(Clone, Debug)]
pub struct Token {
    pub location: SourceRange,
    pub type_: TokenType,
    pub data: TokenData,
}

impl Token {
    // port: Token#<init>
    pub fn new(type_: TokenType, location: SourceRange) -> Self {
        Self {
            type_,
            location,
            data: TokenData::Plain,
        }
    }

    // port: Token#getStart
    pub fn get_start(&self) -> SourcePosition {
        self.location.start.clone()
    }

    // port: Token#toString
    pub fn to_string_utf16(&self) -> JsString {
        match &self.data {
            TokenData::Plain => self.type_.to_string_utf16(),
            TokenData::Identifier(value) => value.to_string_utf16(),
            TokenData::Literal(value) => value.to_string_utf16(),
            TokenData::StringLiteral(value) => value.literal.to_string_utf16(),
            TokenData::TemplateLiteral(value) => value.to_string_utf16(),
        }
    }

    // port: Token#asIdentifier
    pub fn as_identifier(&self) -> &IdentifierToken {
        match &self.data {
            TokenData::Identifier(value) => value,
            _ => panic!("ClassCastException: IdentifierToken"),
        }
    }

    // port: Token#asLiteral
    pub fn as_literal(&self) -> &LiteralToken {
        match &self.data {
            TokenData::Literal(value) => value,
            TokenData::StringLiteral(value) => &value.literal,
            TokenData::TemplateLiteral(value) => &value.literal,
            _ => panic!("ClassCastException: LiteralToken"),
        }
    }

    // port: Token#asTemplateLiteral
    pub fn as_template_literal(&self) -> &TemplateLiteralToken {
        match &self.data {
            TokenData::TemplateLiteral(value) => value,
            _ => panic!("ClassCastException: TemplateLiteralToken"),
        }
    }

    pub fn value_equals(&self, string: &JsString) -> bool {
        self.as_identifier().value_equals(string)
    }
    pub fn is_keyword(&self) -> bool {
        self.as_identifier().is_keyword()
    }
    pub fn is_private_identifier(&self) -> bool {
        self.as_identifier().is_private_identifier()
    }
    pub fn get_value(&self) -> &JsString {
        self.as_identifier().get_value()
    }
    pub fn get_maybe_private_value(&self) -> &JsString {
        self.as_identifier().get_maybe_private_value()
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display is Java UTF-8 output; parser messages use to_string_utf16() directly.
        f.write_str(&closure_rhino::java_lang::charset::utf8_encoded_text(
            self.to_string_utf16().as_units(),
        ))
    }
}
