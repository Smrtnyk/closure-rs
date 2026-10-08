/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/OutputCharsetEncoder.java.

#![allow(clippy::unnecessary_unwrap)] // Retain Java null branch.
use closure_rhino::java_lang::charset::{Charset, CharsetEncoder};
/// Output charset encoder for {@code CodeGenerator} that delegates to a CharsetEncoder.
pub struct OutputCharsetEncoder {
    encoder: Option<CharsetEncoder>,
}
impl OutputCharsetEncoder {
    // port: OutputCharsetEncoder#OutputCharsetEncoder
    pub fn new(output_charset: Option<Charset>) -> Self {
        if output_charset.is_none() || output_charset == Some(Charset::US_ASCII) {
            // If we want our default (pretending to be UTF-8, but escaping anything
            // outside of straight ASCII), then don't use the encoder, but
            // just special-case the code.  This keeps the normal path through
            // the code identical to how it's been for years.
            Self { encoder: None }
        } else {
            Self {
                encoder: Some(output_charset.unwrap().new_encoder()),
            }
        }
    }
    // port: OutputCharsetEncoder#canEncode
    pub fn can_encode(&self, c: u16) -> bool {
        self.encoder
            .as_ref()
            .is_some_and(|encoder| encoder.can_encode(c))
    }
}
