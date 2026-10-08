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
//   test/com/google/debugging/sourcemap/Base64VLQTest.java.

use closure_rhino::js_string::JsString;
use closure_sourcemap::base64_vlq::{Base64VLQ, CharIterator};
// port: Base64VLQTest#testValue
fn test_value(value: i32) {
    let mut sb = String::new();
    Base64VLQ::encode(&mut sb, value).unwrap();
    let mut ci = CharIteratorImpl::default();
    ci.set(sb);
    let result = Base64VLQ::decode(&mut ci);
    assert_eq!(result, value);
}
// port: Base64VLQTest#testBase64VLQSelectedValues1
#[test]
fn test_base64_vlq_selected_values1() {
    for i in 0..63 {
        test_value(i);
    }
}
// port: Base64VLQTest#testBase64VLQSelectedValues2
#[test]
fn test_base64_vlq_selected_values2() {
    let mut base = 1;
    for _ in 0..30 {
        test_value(base - 1);
        test_value(base);
        base *= 2;
    }
}
// port: Base64VLQTest#testBase64VLQSelectedSignedValues1
#[test]
fn test_base64_vlq_selected_signed_values1() {
    for i in -(64 * 64 - 1)..(64 * 64 - 1) {
        test_value(i);
    }
}
// port: Base64VLQTest#testBase64VLQSelectedSignedValues2
#[test]
fn test_base64_vlq_selected_signed_values2() {
    let mut base = 1;
    for _ in 0..30 {
        test_value(base - 1);
        test_value(base);
        base *= 2;
    }
    base = -1;
    for _ in 0..30 {
        test_value(base - 1);
        test_value(base);
        base *= 2;
    }
}
// port: Base64VLQTest#testBase64VLQSelectedSignedValues3
#[test]
fn test_base64_vlq_selected_signed_values3() {
    test_value(i32::MAX);
    test_value(i32::MAX - 1);
    test_value(i32::MAX - 2);
    test_value(0x70000000);
    test_value(i32::MIN);
    test_value(i32::MIN + 1);
    test_value(i32::MIN + 2);
}

#[derive(Default)]
struct CharIteratorImpl {
    current: usize,
    length: usize,
    cs: JsString,
}
impl CharIteratorImpl {
    // port: Base64VLQTest.CharIteratorImpl#set
    fn set(&mut self, sb: impl Into<JsString>) {
        let sb = sb.into();
        self.current = 0;
        self.length = sb.length();
        self.cs = sb;
    }
}
impl CharIterator for CharIteratorImpl {
    // port: Base64VLQTest.CharIteratorImpl#hasNext
    fn has_next(&self) -> bool {
        self.current < self.length
    }
    // port: Base64VLQTest.CharIteratorImpl#next
    fn next(&mut self) -> u16 {
        let result = self.cs.char_at(self.current);
        self.current += 1;
        result
    }
}
