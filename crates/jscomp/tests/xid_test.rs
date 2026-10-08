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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/XidTest.java.

use closure_jscomp::xid::Xid;
use indexmap::IndexMap;

// port: XidTest#helpTestUniqueness
fn help_test_uniqueness(map: &mut IndexMap<String, i32>, lo: i32, hi: i32) {
    for i in lo..=hi {
        let key = Xid::to_string(i);
        assert!(
            map.get(&key).is_none(),
            "Both {:?} and {i} map to: {key}",
            map.get(&key)
        );
        map.insert(key, i);
    }
}
// port: XidTest#testUniqueness
#[test]
fn test_uniqueness() {
    let mut map = IndexMap::new();
    help_test_uniqueness(&mut map, -1000, 1000);
    help_test_uniqueness(&mut map, i32::MIN, i32::MIN + 1000);
    help_test_uniqueness(&mut map, i32::MAX, i32::MAX - 1000);
}
// port: XidTest#helpTestLength
fn help_test_length(i: i32) {
    assert!((1..=6).contains(&Xid::to_string(i).len()));
}
// port: XidTest#testLength
#[test]
fn test_length() {
    for i in [0, 1, -1, i32::MIN, i32::MAX] {
        help_test_length(i);
    }
    // java.util.Random's 48-bit LCG, with a repeatable seed for this randomized test.
    let mut seed = (20261007u64 ^ 0x5deece66d) & ((1u64 << 48) - 1);
    for _ in 0..=10000 {
        seed = seed.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1u64 << 48) - 1);
        help_test_length((seed >> 16) as i32);
    }
}
// port: XidTest#testToString
#[test]
fn test_to_string() {
    for (i, s) in [
        (1, "z6ArXc"),
        (2, "A6ArXc"),
        (-1, "x6ArXc"),
        (10000, "OcErXc"),
        (-1951591049, "tTaYp"),
    ] {
        assert_eq!(Xid::to_string(i), s);
    }
}
// port: XidTest#testGet
#[test]
fn test_get() {
    let dummy_map = Xid::new();
    for (key, s) in [
        ("today", "nZzm6c"),
        ("tomorrow", "fkPKBb"),
        ("value", "b6Lt6c"),
        ("foo", "QB6rXc"),
        ("foo.Bar", "RW4o4b"),
        ("prop1", "MiB45c"),
        ("prop2", "NiB45c"),
    ] {
        assert_eq!(dummy_map.get(key), s);
    }
}
