/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/StringLiteralToken.java.

use super::{
    JsString,
    literal_token::LiteralToken,
    token::{Token, TokenData},
    token_type::TokenType,
    util::SourceRange,
};

/// A single or double quoted JavaScript string literal.
#[derive(Clone, Debug)]
pub struct StringLiteralToken {
    pub literal: LiteralToken,
    has_unescaped_unicode_line_or_paragraph_separator: bool,
}

impl StringLiteralToken {
    // port: StringLiteralToken#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        value: JsString,
        location: SourceRange,
        has_unescaped_unicode_line_or_paragraph_separator: bool,
    ) -> Token {
        Token {
            type_: TokenType::STRING,
            location,
            data: TokenData::StringLiteral(Self {
                literal: LiteralToken { value },
                has_unescaped_unicode_line_or_paragraph_separator,
            }),
        }
    }

    // port: StringLiteralToken#hasUnescapedUnicodeLineOrParagraphSeparator
    pub fn has_unescaped_unicode_line_or_paragraph_separator(&self) -> bool {
        self.has_unescaped_unicode_line_or_paragraph_separator
    }
}
