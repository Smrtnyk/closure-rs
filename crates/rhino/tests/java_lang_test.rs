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

use closure_rhino::{java_lang::double_to_string, js_string::JsString};
#[test]
fn jdk21_double_samples() {
    for line in include_str!("double_samples.txt").lines() {
        let (bits, expected) = line.split_once(' ').unwrap();
        let value = f64::from_bits(u64::from_str_radix(bits, 16).unwrap());
        assert_eq!(double_to_string(value), expected, "bits={bits}");
    }
}
#[test]
fn utf16_code_units_survive() {
    let s = JsString::from_units(vec![0xd800, 0x0061, 0xdc00, 0xd83d, 0xde00]);
    assert_eq!(s.length(), 5);
    assert_eq!(s.char_at(0), 0xd800);
    assert_eq!(s.code_point_at(3), 0x1f600);
    assert_eq!(s.code_point_at(0), 0xd800);
    assert_eq!(s.substring(0, 3).as_units(), &[0xd800, 0x0061, 0xdc00]);
    assert_eq!(
        s.hash_code(),
        s.as_units()
            .iter()
            .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(*c as i32))
    );
}
