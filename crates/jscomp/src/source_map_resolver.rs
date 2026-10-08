/*
 * Copyright (C) 2012 The Guava Authors
 *
 * Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
 * in compliance with the License. You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software distributed under the License
 * is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
 * or implied. See the License for the specific language governing permissions and limitations under
 * the License.
 */
/*
 * Copyright 2017 The Closure Compiler Authors.
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
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/io/BaseEncoding.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/SourceMapResolver.java.

use crate::source_file::SourceFile;
use closure_rhino::{
    java_lang::{charset::Charset, unix_path::UnixPath, uri::URI},
    js_string::JsString,
    static_source_file::{SourceKind, StaticSourceFile},
};

/// Utility class for resolving source maps and files referenced in source maps.
pub struct SourceMapResolver;
impl SourceMapResolver {
    const BASE64_URL_PREFIX: &str = "data:";
    const BASE64_START: &str = "base64,";

    // For now only accept UTF-8. We could use the actual charset information in the future.
    const ACCEPTED_MEDIA_TYPES: [&str; 2] = [
        "application/json;charset=utf-8;",
        // default is UTF-8 if unspecified.
        "application/json;",
    ];

    /// For a given //# sourceMappingUrl, this locates the appropriate sourcemap on disk. This is
    /// use for sourcemap merging (--apply_input_source_maps) and for error resolution.
    // port: SourceMapResolver#extractSourceMap
    pub fn extract_source_map(
        js_file: &SourceFile,
        source_map_url: &str,
        parse_inline_source_maps: bool,
    ) -> Option<SourceFile> {
        if parse_inline_source_maps && source_map_url.starts_with(Self::BASE64_URL_PREFIX) {
            let extracted_string = Self::extract_base64_string(source_map_url);
            if let Some(extracted_string) = extracted_string {
                return Some(SourceFile::from_code_with_kind(
                    &format!("{}.inline.map", js_file.get_name()),
                    extracted_string,
                    SourceKind::NON_CODE,
                ));
            }
            return None;
        }
        // TODO(tdeegan): Handle absolute urls here.  The compiler needs to come up with a scheme
        // for properly resolving absolute urls from http:// or the root /some/abs/path/... See
        // b/62544959.
        if Self::is_absolute(source_map_url) {
            return None;
        }
        // If not absolute, its relative.
        // TODO(tdeegan): Handle urls relative to //# sourceURL. See the sourcemap spec.
        Some(Self::get_relative_path(js_file.get_name(), source_map_url))
    }

    /// Based on https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URIs
    // port: SourceMapResolver#extractBase64String
    fn extract_base64_string(url: &str) -> Option<JsString> {
        if url.starts_with(Self::BASE64_URL_PREFIX) && url.contains(Self::BASE64_START) {
            let base64_start_index = url.find(Self::BASE64_START).unwrap();
            let media_type = &url[Self::BASE64_URL_PREFIX.len()..base64_start_index];
            if Self::ACCEPTED_MEDIA_TYPES.contains(&media_type) {
                let data = base_encoding::base64_decode(
                    &url[base64_start_index + Self::BASE64_START.len()..],
                );
                return Some(Charset::UTF_8.decode(&data, true).unwrap());
            }
        }
        None
    }

    // port: SourceMapResolver#isAbsolute
    fn is_absolute(url: &str) -> bool {
        match URI::new(url) {
            Ok(uri) => uri.is_absolute() || url.starts_with('/'),
            Err(e) => panic!("java.lang.RuntimeException: Sourcemap url was invalid: {url}: {e}"),
        }
    }

    /// Returns the relative path, resolved relative to the base path, where the base path is
    /// interpreted as a filename rather than a directory. E.g.: getRelativeTo("../foo/bar.js",
    /// "baz/bam/qux.js") --> "baz/foo/bar.js"
    // port: SourceMapResolver#getRelativePath
    pub fn get_relative_path(base_file_path: &str, relative_path: &str) -> SourceFile {
        SourceFile::builder()
            .with_path(
                UnixPath::of(base_file_path)
                    .resolve_sibling(&UnixPath::of(relative_path))
                    .normalize()
                    .to_string(),
            )
            .with_kind(SourceKind::NON_CODE)
            .build()
    }

    /// Returns the encoded source map with the base64 prefix attached. (i.e. prefix with
    /// "data:application/json;base64,")
    ///
    /// E.g. turns "eyJ2ZXJzaW9uI..." into "data:application/json;base64,eyJ2ZXJzaW9uI..."
    // port: SourceMapResolver#addBase64PrefixToEncodedSourceMap
    pub fn add_base64_prefix_to_encoded_source_map(encoded: &str) -> String {
        format!(
            "{}application/json;{}{encoded}",
            Self::BASE64_URL_PREFIX,
            Self::BASE64_START
        )
    }
}

/// Guava's BaseEncoding.base64() decoding (com.google.common.io.BaseEncoding).
mod base_encoding {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const PAD_CHAR: char = '=';
    const BITS_PER_CHAR: usize = 6;
    const CHARS_PER_CHUNK: usize = 4;
    const BYTES_PER_CHUNK: usize = 3;

    struct Alphabet {
        decodabet: [i8; 128],
        valid_padding: [bool; CHARS_PER_CHUNK],
    }
    impl Alphabet {
        // port: com.google.common.io.BaseEncoding.Alphabet#Alphabet
        fn new() -> Self {
            let mut decodabet = [-1i8; 128];
            for (i, &c) in CHARS.iter().enumerate() {
                decodabet[c as usize] = i as i8;
            }
            let mut valid_padding = [false; CHARS_PER_CHUNK];
            for i in 0..BYTES_PER_CHUNK {
                valid_padding[(i * 8).div_ceil(BITS_PER_CHAR)] = true;
            }
            Self {
                decodabet,
                valid_padding,
            }
        }
        // port: com.google.common.io.BaseEncoding.Alphabet#isValidPaddingStartPosition
        fn is_valid_padding_start_position(&self, index: usize) -> bool {
            self.valid_padding[index % CHARS_PER_CHUNK]
        }
        // port: com.google.common.io.BaseEncoding.Alphabet#decode
        fn decode(&self, ch: u16) -> Result<i32, String> {
            if ch > 0x7F || self.decodabet[ch as usize] == -1 {
                return Err(if ch <= u16::from(b' ') || ch == 0x7F {
                    format!("Unrecognized character: 0x{ch:x}")
                } else {
                    format!(
                        "Unrecognized character: {}",
                        String::from_utf16_lossy(&[ch])
                    )
                });
            }
            Ok(i32::from(self.decodabet[ch as usize]))
        }
    }

    // port: com.google.common.io.BaseEncoding#decode
    pub(super) fn base64_decode(chars: &str) -> Vec<u8> {
        match decode_checked(chars) {
            Ok(bytes) => bytes,
            Err(bad_input) => panic!(
                "java.lang.IllegalArgumentException: com.google.common.io.BaseEncoding$DecodingException: {bad_input}"
            ),
        }
    }

    // port: com.google.common.io.BaseEncoding#decodeChecked
    fn decode_checked(chars: &str) -> Result<Vec<u8>, String> {
        let chars = trim_trailing_padding(chars);
        let mut tmp = Vec::with_capacity(chars.len() * 6 / 8 + 1);
        decode_to(&mut tmp, &chars)?;
        Ok(tmp)
    }

    // port: com.google.common.io.BaseEncoding.StandardBaseEncoding#trimTrailingPadding
    fn trim_trailing_padding(chars: &str) -> Vec<u16> {
        let mut units: Vec<u16> = chars.encode_utf16().collect();
        while units.last() == Some(&(PAD_CHAR as u16)) {
            units.pop();
        }
        units
    }

    // port: com.google.common.io.BaseEncoding.Base64Encoding#decodeTo
    fn decode_to(target: &mut Vec<u8>, chars: &[u16]) -> Result<usize, String> {
        let alphabet = Alphabet::new();
        if !alphabet.is_valid_padding_start_position(chars.len()) {
            return Err(format!("Invalid input length {}", chars.len()));
        }
        let mut bytes_written = 0;
        let mut i = 0;
        while i < chars.len() {
            let mut chunk = alphabet.decode(chars[i])? << 18;
            i += 1;
            chunk |= alphabet.decode(chars[i])? << 12;
            i += 1;
            target.push((chunk as u32 >> 16) as u8);
            bytes_written += 1;
            if i < chars.len() {
                chunk |= alphabet.decode(chars[i])? << 6;
                i += 1;
                target.push(((chunk as u32 >> 8) & 0xFF) as u8);
                bytes_written += 1;
                if i < chars.len() {
                    chunk |= alphabet.decode(chars[i])?;
                    i += 1;
                    target.push((chunk & 0xFF) as u8);
                    bytes_written += 1;
                }
            }
        }
        Ok(bytes_written)
    }
}
