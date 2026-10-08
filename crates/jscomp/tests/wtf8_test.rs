/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/serialization/Wtf8Test.java.

//! Port of serialization/Wtf8Test.java.
use closure_jscomp::serialization::wtf8::Wtf8;
use closure_rhino::js_string::JsString;

fn roundtrip(s: JsString) {
    let mut decoder = Wtf8::decoder(256);
    let serialized = Wtf8::encode_to_wtf8(&s);
    let roundtripped = decoder.decode(&serialized);
    assert_eq!(roundtripped, s);
}

fn units(u: &[u16]) -> JsString {
    JsString::from_units(u.to_vec())
}

// port: Wtf8Test#testAscii
#[test]
fn test_ascii() {
    roundtrip(JsString::from("hello world"));
}

// port: Wtf8Test#testTwoByteLatin1SupplementCharacter
#[test]
fn test_two_byte_latin1_supplement_character() {
    // Two-bytes in UTF-8: https://www.compart.com/en/unicode/U+00BC
    roundtrip(JsString::from("¼"));
}

// port: Wtf8Test#testMultibyteBMPCharacters
#[test]
fn test_multibyte_bmp_characters() {
    // Korean characters are 3 bytes in UTF-8. e.g. https://www.compart.com/en/unicode/U+C138
    roundtrip(JsString::from("세계님 안녕"));
}

// port: Wtf8Test#testEscapedPairedSurrogate
#[test]
fn test_escaped_paired_surrogate() {
    roundtrip(units(&[0xd83d, 0xdc85]));
}

// port: Wtf8Test#testUnpairedSurrogates
#[test]
fn test_unpaired_surrogates() {
    let mut hello = "Hello ".encode_utf16().collect::<Vec<u16>>();
    hello.push(0xd800);
    roundtrip(units(&hello));
    let mut second = vec![0xd800];
    second.extend(", hello".encode_utf16());
    roundtrip(units(&second));
}

// port: Wtf8Test#testInvalidSurrogatePairings
#[test]
fn test_invalid_surrogate_pairings() {
    // This is an invalid pair: low-high
    roundtrip(units(&[0xdc37, 0xd801]));
    // This is an invalid pair: low-low
    roundtrip(units(&[0xd802, 0xd801]));
    // This is an invalid pair: high-high
    roundtrip(units(&[0xdc37, 0xdc39]));
}

// port: Wtf8Test#testUnescapedPairedSurrogates
#[test]
fn test_unescaped_paired_surrogates() {
    // Emojis are always 4 bytes in UTF-8 and thus represented as paired surrogates in UTF-16.
    roundtrip(JsString::from("💅🏽"));
}
