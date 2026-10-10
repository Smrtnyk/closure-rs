/*
 * Copyright 2026 The closure-rs Authors.
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

use closure_rhino::js_string::JsString;

/// Java's `String#hashCode` loop.
fn java_hash_code(units: &[u16]) -> i32 {
    units
        .iter()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(*c)))
}

#[test]
fn hash_code_is_java_string_hash_code() {
    assert_eq!(JsString::from("").hash_code(), 0);
    assert_eq!(JsString::from("a").hash_code(), 97);
    assert_eq!(JsString::from("hello").hash_code(), 99162322);
    assert_eq!(JsString::from("prototype").hash_code(), -598792926);
    // Every length up to 40 (all remainders of the four-unit steps), ASCII and not.
    let mut units: Vec<u16> = Vec::new();
    for i in 0..40u16 {
        for s in [
            JsString::from_slice(&units),
            JsString::from_units(units.clone()),
        ] {
            assert_eq!(s.hash_code(), java_hash_code(&units), "{units:?}");
        }
        units.push(i.wrapping_mul(7919) ^ if i % 3 == 0 { 0xd800 } else { 0x41 });
    }
    for text in ["ascii only", "café", "日本語のテキスト", "a\u{1F600}b", "x".repeat(1000).as_str()] {
        let units: Vec<u16> = text.encode_utf16().collect();
        let s = JsString::from(text);
        assert_eq!(s.as_units(), units.as_slice());
        assert_eq!(s.hash_code(), java_hash_code(&units), "{text}");
    }
}

#[test]
fn empty_strings_are_equal() {
    assert_eq!(JsString::default(), JsString::from(""));
    assert!(JsString::default().is_empty());
    assert_eq!(JsString::default().hash_code(), 0);
}
