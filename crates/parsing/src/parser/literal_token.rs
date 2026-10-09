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
//   src/com/google/javascript/jscomp/parsing/parser/LiteralToken.java.

use super::{
    JsString,
    token::{Token, TokenData},
    token_type::TokenType,
    util::SourceRange,
};

/// A token representing a javascript literal. Includes string, regexp, and number literals.
/// Boolean and null literals are represented as regular keyword tokens.
///
/// The value just includes the raw lexeme. For string literals it includes the beginning and ending
/// delimiters.
///
/// TODO: Regexp literals should have their own token type.
/// TODO: A way to get the processed value, rather than the raw value.
#[derive(Clone, Debug)]
pub struct LiteralToken {
    pub value: JsString,
}

impl LiteralToken {
    // port: LiteralToken#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(type_: TokenType, value: JsString, location: SourceRange) -> Token {
        Token {
            type_,
            location,
            data: TokenData::Literal(Self { value }),
        }
    }

    // port: LiteralToken#toString
    pub fn to_string_utf16(&self) -> JsString {
        self.value.clone()
    }
}
