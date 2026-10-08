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
//   test/com/google/debugging/sourcemap/Base64Test.java.

use closure_sourcemap::base64::Base64;
// port: Base64Test#testBase64
#[test]
fn test_base64() {
    for i in 0..64 {
        test_value(i);
    }
}
// port: Base64Test#testBase64EncodeInt
#[test]
fn test_base64_encode_int() {
    assert_eq!(Base64::base64_encode_int(0), "AAAAAA");
    assert_eq!(Base64::base64_encode_int(1), "AAAAAQ");
    assert_eq!(Base64::base64_encode_int(42), "AAAAKg");
    assert_eq!(Base64::base64_encode_int(-100), "////nA");
    assert_eq!(Base64::base64_encode_int(0xffffffffu32 as i32), "/////w");
}
// port: Base64Test#testValue
fn test_value(value: i32) {
    assert_eq!(Base64::from_base64(Base64::to_base64(value)), value);
}
