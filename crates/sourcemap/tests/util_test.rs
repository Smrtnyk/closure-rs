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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/debugging/sourcemap/UtilTest.java.

use closure_sourcemap::util::Util;
// port: UtilTest#testAppendHexJavaScriptRepresentation(char,String)
fn test_append_hex_java_script_representation_with_expected(ch: u16, expected_out: &str) {
    let mut sb = String::new();
    Util::append_hex_java_script_representation(&mut sb, ch);
    assert_eq!(sb, expected_out);
}
// port: UtilTest#testAppendHexJavaScriptRepresentation
#[test]
fn test_append_hex_java_script_representation() {
    test_append_hex_java_script_representation_with_expected(b'a' as u16, "\\u0061");
    test_append_hex_java_script_representation_with_expected(b'z' as u16, "\\u007a");
    test_append_hex_java_script_representation_with_expected(b'\0' as u16, "\\u0000");
    test_append_hex_java_script_representation_with_expected('¡' as u16, "\\u00a1");
}
