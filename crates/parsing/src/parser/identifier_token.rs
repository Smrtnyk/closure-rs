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
//   src/com/google/javascript/jscomp/parsing/parser/IdentifierToken.java.

use super::{
    JsString,
    keywords::Keywords,
    token::{Token, TokenData},
    token_type::TokenType,
    util::SourceRange,
};

/// A token representing an identifier.
#[derive(Clone, Debug)]
pub struct IdentifierToken {
    value: JsString,
    private_identifier: bool,
}

impl IdentifierToken {
    // Constructors return the complete token, including the inherited fields.
    // port: IdentifierToken#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(location: SourceRange, value: JsString) -> Token {
        let private_identifier = value.starts_with("#");
        Token {
            type_: TokenType::IDENTIFIER,
            location,
            data: TokenData::Identifier(Self {
                value,
                private_identifier,
            }),
        }
    }

    // port: IdentifierToken#toString
    pub fn to_string_utf16(&self) -> JsString {
        self.value.clone()
    }

    // port: IdentifierToken#isKeyword
    pub fn is_keyword(&self) -> bool {
        Keywords::is_keyword(&self.value)
    }

    // port: IdentifierToken#valueEquals
    pub fn value_equals(&self, string: &JsString) -> bool {
        &self.value == string
    }

    /// Gets the value of the identifier assuring that it is not a private identifier.
    ///
    /// <p>You must verify privateIdentifier is false (and presumably error if it is true) before
    /// calling this method.
    ///
    /// <p>Prefer calling {@link #isKeyword()} or {@link #valueEquals(String)} if those methods meet
    /// your needs.
    // port: IdentifierToken#getValue
    pub fn get_value(&self) -> &JsString {
        assert!(!self.private_identifier);
        &self.value
    }

    /// Gets the value of the identifier, allowing it to be a private identifier.
    // port: IdentifierToken#getMaybePrivateValue
    pub fn get_maybe_private_value(&self) -> &JsString {
        &self.value
    }

    /// Whether the value starts with a #.
    // port: IdentifierToken#isPrivateIdentifier
    pub fn is_private_identifier(&self) -> bool {
        self.private_identifier
    }
}
