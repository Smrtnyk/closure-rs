/*
 * Copyright (C) 2010 The Guava Authors
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
// Ported from Guava 33.4.6-jre (https://github.com/google/guava):
//   com/google/common/base/Ascii.java.

//! The `com.google.common.base.Ascii` members Closure's checks import statically
//! (ClosureCheckModule, CheckClosureImports). ASCII only, unlike `java.lang.Character`.

// port: com.google.common.base.Ascii#CASE_MASK
const CASE_MASK: u16 = 0x20;

// port: com.google.common.base.Ascii#isLowerCase(char)
pub fn is_lower_case(c: u16) -> bool {
    // Note: This was benchmarked against the alternate expression "(char)(c - 'a') < 26" (Nov
    // '13) and found to perform at least as well, or better.
    (c >= b'a' as u16) && (c <= b'z' as u16)
}

// port: com.google.common.base.Ascii#isUpperCase(char)
pub fn is_upper_case(c: u16) -> bool {
    (c >= b'A' as u16) && (c <= b'Z' as u16)
}

// port: com.google.common.base.Ascii#toLowerCase(char)
pub fn to_lower_case(c: u16) -> u16 {
    if is_upper_case(c) { c ^ CASE_MASK } else { c }
}

// port: com.google.common.base.Ascii#toUpperCase(char)
pub fn to_upper_case(c: u16) -> u16 {
    if is_lower_case(c) { c ^ CASE_MASK } else { c }
}
