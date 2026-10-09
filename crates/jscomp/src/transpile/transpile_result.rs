/*
 * Copyright 2016 The Closure Compiler Authors.
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
/*
 * Copyright (C) 2008 The Guava Authors
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/transpile/TranspileResult.java,
//   test/com/google/javascript/jscomp/transpile/TranspileResultTest.java.
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/escape/UnicodeEscaper.java, com/google/common/net/PercentEscaper.java.

use base64::{Engine, engine::general_purpose::STANDARD};
use closure_rhino::{
    java_lang::{string::get_bytes_utf8, uri::URI},
    js_string::JsString,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranspileResult {
    data: Arc<ResultData>,
}
#[derive(Debug, PartialEq, Eq)]
struct ResultData {
    path: URI,
    original: JsString,
    transpiled: JsString,
    source_map: JsString,
}
impl TranspileResult {
    // port: TranspileResult#TranspileResult
    pub fn new(
        path: URI,
        original: impl Into<JsString>,
        transpiled: impl Into<JsString>,
        source_map: impl Into<JsString>,
    ) -> Self {
        Self {
            data: Arc::new(ResultData {
                path,
                original: original.into(),
                transpiled: transpiled.into(),
                source_map: source_map.into(),
            }),
        }
    }
    // port: TranspileResult#path
    pub fn path(&self) -> &URI {
        &self.data.path
    }
    // port: TranspileResult#original
    pub fn original(&self) -> &JsString {
        &self.data.original
    }
    // port: TranspileResult#sourceMap
    pub fn source_map(&self) -> &JsString {
        &self.data.source_map
    }
    // port: TranspileResult#embedSourcemapUrl
    pub fn embed_sourcemap_url(&self, url: &str) -> Self {
        if self.source_map().is_empty() {
            return self.clone();
        }
        let embedded = self
            .transpiled()
            .concat(&JsString::from(format!("\n//# sourceMappingURL={url}\n")));
        Self::new(
            self.path().clone(),
            self.original().clone(),
            embedded,
            self.source_map().clone(),
        )
    }
    // port: TranspileResult#embedSourcemap
    pub fn embed_sourcemap(&self) -> Self {
        if self.source_map().is_empty() {
            return self.clone();
        }
        let embedded = self.transpiled().concat(&JsString::from(format!(
            "\n//# sourceMappingURL=data:,{}\n",
            percent_escape(self.source_map())
        )));
        Self::new(self.path().clone(), self.original().clone(), embedded, "")
    }
    // port: TranspileResult#embedSourcemapBase64
    pub fn embed_sourcemap_base64(&self) -> Self {
        if self.source_map().is_empty() {
            return self.clone();
        }
        let embedded = self.transpiled().concat(&JsString::from(format!(
            "\n//# sourceMappingURL=data:application/json;base64,{}\n",
            STANDARD.encode(get_bytes_utf8(self.source_map()))
        )));
        Self::new(self.path().clone(), self.original().clone(), embedded, "")
    }
    // port: TranspileResult#transpiled
    pub fn transpiled(&self) -> &JsString {
        &self.data.transpiled
    }
    // port: TranspileResult#wasTranspiled
    pub fn was_transpiled(&self) -> bool {
        self.transpiled() != self.original()
    }
    // port: TranspileResult#equals (reference identity when methods return this)
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
    // port: TranspileResult#hashCode
    pub fn hash_code(&self) -> i32 {
        [
            self.path().hash_code(),
            self.original().hash_code(),
            self.transpiled().hash_code(),
            self.source_map().hash_code(),
        ]
        .into_iter()
        .fold(1i32, |h, v| h.wrapping_mul(31).wrapping_add(v))
    }
}
impl Hash for TranspileResult {
    // port: TranspileResult#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code());
    }
}
impl fmt::Display for TranspileResult {
    // port: TranspileResult#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TranspileResut{{path={}, original={}, transpiled={}, sourceMapURL={}}}",
            self.path(),
            self.original().to_string_lossy(),
            self.transpiled().to_string_lossy(),
            self.source_map().to_string_lossy()
        )
    }
}
// port: PercentEscaper#escape, PercentEscaper#escape(int) (safeChars="-_.*", plusForSpace=false)
fn percent_escape(string: &JsString) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < string.length() {
        let c = unicode_code_point_at(string, i);
        i += if c > 0xffff { 2 } else { 1 };
        if matches!(c,48..=57|65..=90|97..=122|45|95|46|42) {
            result.push(char::from_u32(c).unwrap());
        } else {
            let mut bytes = [0; 4];
            for b in char::from_u32(c).unwrap().encode_utf8(&mut bytes).bytes() {
                result.push('%');
                result.push(b"0123456789ABCDEF"[(b >> 4) as usize] as char);
                result.push(b"0123456789ABCDEF"[(b & 15) as usize] as char);
            }
        }
    }
    result
}
// port: UnicodeEscaper#codePointAt, UnicodeEscaper#escapeSlow
fn unicode_code_point_at(seq: &JsString, index: usize) -> u32 {
    let c = seq.char_at(index);
    if !(0xd800..=0xdfff).contains(&c) {
        return c as u32;
    }
    if c <= 0xdbff {
        if index + 1 == seq.length() {
            panic!("Trailing high surrogate at end of input");
        }
        let c2 = seq.char_at(index + 1);
        if (0xdc00..=0xdfff).contains(&c2) {
            return 0x10000 + ((c as u32 - 0xd800) << 10) + (c2 as u32 - 0xdc00);
        }
        panic!(
            "Expected low surrogate but got char '{}' with value {} at index {} in '{}'",
            JsString::from_units(vec![c2]).to_string_lossy(),
            c2,
            index + 1,
            seq.to_string_lossy()
        );
    }
    panic!(
        "Unexpected low surrogate character '{}' with value {} at index {} in '{}'",
        JsString::from_units(vec![c]).to_string_lossy(),
        c,
        index,
        seq.to_string_lossy()
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    // port: TranspileResultTest#testEquals
    #[test]
    fn test_equals() {
        let groups = [
            vec![
                TranspileResult::new(URI::new("a").unwrap(), "b", "c", "d"),
                TranspileResult::new(URI::new("a").unwrap(), "b", "c", "d"),
            ],
            vec![TranspileResult::new(URI::new("A").unwrap(), "b", "c", "d")],
            vec![TranspileResult::new(URI::new("a").unwrap(), "B", "c", "d")],
            vec![TranspileResult::new(URI::new("a").unwrap(), "b", "C", "d")],
            vec![TranspileResult::new(URI::new("a").unwrap(), "b", "c", "D")],
        ];
        for (i, g) in groups.iter().enumerate() {
            for a in g {
                for (j, h) in groups.iter().enumerate() {
                    for b in h {
                        assert_eq!(a == b, i == j);
                        if i == j {
                            assert_eq!(a.hash_code(), b.hash_code());
                        }
                    }
                }
            }
        }
    }
    // port: TranspileResultTest#testEmbedSourceMap_noSourceMap
    #[test]
    fn test_embed_source_map_no_source_map() {
        let result = TranspileResult::new(URI::new("a").unwrap(), "b", "c", "");
        assert!(result.embed_sourcemap().ptr_eq(&result));
        assert!(result.embed_sourcemap_url("foo").ptr_eq(&result));
    }
    // port: TranspileResultTest#testEmbedSourceMap
    #[test]
    fn test_embed_source_map() {
        let result = TranspileResult::new(URI::new("a").unwrap(), "b", "c", "{\"version\": 3}");
        assert_eq!(
            result.embed_sourcemap(),
            TranspileResult::new(
                URI::new("a").unwrap(),
                "b",
                "c\n//# sourceMappingURL=data:,%7B%22version%22%3A%203%7D\n",
                ""
            )
        );
        assert_eq!(
            result.embed_sourcemap_url("foo.js.map"),
            TranspileResult::new(
                URI::new("a").unwrap(),
                "b",
                "c\n//# sourceMappingURL=foo.js.map\n",
                "{\"version\": 3}"
            )
        );
    }
}
#[cfg(test)]
mod differential_tests {
    use super::*;
    // port: TranspileResult#embedSourcemap, embedSourcemapBase64, hashCode, toString (Java-generated fixture)
    #[test]
    fn java_transpile_result_differential() {
        for line in include_str!("testdata/transpile.tsv").lines() {
            let f: Vec<_> = line.split('\t').collect();
            let r = TranspileResult::new(
                URI::new("HTTP://HOST/a%2f").unwrap(),
                "original",
                "out",
                decode(f[0]),
            );
            let escaped = std::panic::catch_unwind(|| r.embed_sourcemap());
            if let Some(expected) = f[1].strip_prefix('v') {
                assert_eq!(escaped.unwrap().transpiled(), &decode(expected));
            } else {
                let panic = escaped.unwrap_err();
                let msg = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap();
                assert_eq!(msg, decode(&f[1][1..]).to_string_lossy());
            }
            let base64 = r.embed_sourcemap_base64();
            assert_eq!(base64.transpiled(), &decode(f[2]));
            assert_eq!(r.hash_code(), f[3].parse::<i32>().unwrap());
            assert_eq!(r.to_string(), decode(f[4]).to_string_lossy());
            assert_eq!(r.ptr_eq(&base64).to_string(), f[5]);
            assert!(r.was_transpiled());
        }
    }
    // port: String#charAt (fixture encoding)
    fn decode(s: &str) -> JsString {
        JsString::from_units(
            (0..s.len())
                .step_by(4)
                .map(|i| u16::from_str_radix(&s[i..i + 4], 16).unwrap())
                .collect::<Vec<_>>(),
        )
    }
}
