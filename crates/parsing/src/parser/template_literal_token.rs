/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/TemplateLiteralToken.java.

use super::{
    JsString,
    literal_token::LiteralToken,
    token::{Token, TokenData},
    token_type::TokenType,
    util::{SourcePosition, SourceRange},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorLevel {
    WARNING,
    ERROR,
}

/// A token representing a javascript template literal substring.
///
/// <p>The value of the Token is the raw string. The token also stores whether this token contains
/// any error messages that should be passed to the parser due to invalid escapes or unnecessary
/// escapes. The parser, not the scanner, reports these errors because the errors are suppressed in
/// tagged template literals. The scanner does not know if it's in a tagged or untagged template lit.
#[derive(Clone, Debug)]
pub struct TemplateLiteralToken {
    pub literal: LiteralToken,
    pub error_message: Option<JsString>,
    pub error_position: Option<SourcePosition>,
    pub error_level: Option<ErrorLevel>,
}

impl TemplateLiteralToken {
    // port: TemplateLiteralToken#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        type_: TokenType,
        value: JsString,
        error_msg: Option<JsString>,
        error_level: Option<ErrorLevel>,
        position: Option<SourcePosition>,
        location: SourceRange,
    ) -> Token {
        Token {
            type_,
            location,
            data: TokenData::TemplateLiteral(Self {
                literal: LiteralToken { value },
                error_message: error_msg,
                error_level,
                error_position: position,
            }),
        }
    }

    // port: TemplateLiteralToken#toString
    pub fn to_string_utf16(&self) -> JsString {
        self.literal.value.clone()
    }

    // port: TemplateLiteralToken#hasError
    pub fn has_error(&self) -> bool {
        self.error_level == Some(ErrorLevel::ERROR)
    }
}
