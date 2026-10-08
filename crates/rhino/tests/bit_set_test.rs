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

use closure_rhino::java_util::bit_set::BitSet;

#[test]
fn java_strings_and_hash_codes() {
    for (bits, text, hash, size) in [
        (vec![], "{}", 1234, 0),
        (vec![0, 2, 5], "{0, 2, 5}", 1271, 64),
        (vec![63, 64, 129], "{63, 64, 129}", -2147482410, 256),
        (vec![31, 32, 127], "{31, 32, 127}", -2147482413, 128),
    ] {
        let mut b = BitSet::new(0);
        for bit in &bits {
            b.set(*bit);
        }
        assert_eq!(b.to_string(), text);
        assert_eq!(b.hash_code(), hash);
        assert_eq!(b.size(), size);
        assert_eq!(b.cardinality(), bits.len() as i32);
        assert_eq!(b.is_empty(), bits.is_empty());
        let c = b.clone();
        assert_eq!(b, c);
        b.and_not(&c);
        assert!(b.is_empty());
        b.or(&c);
        assert_eq!(b, c);
        b.clear();
        assert!(b.is_empty());
    }
}
#[test]
fn java_capacity_clone_and_next_set_bit() {
    let mut b = BitSet::new(0);
    b.set(129);
    assert_eq!(b.size(), 192);
    b.set(193);
    assert_eq!(b.size(), 384);
    b.clear_bit(193);
    let c = b.clone();
    assert_eq!(b.size(), 192);
    assert_eq!(c.size(), 192);
    assert_eq!(b.next_set_bit(0), 129);
    assert_eq!(b.next_set_bit(129), 129);
    assert_eq!(b.next_set_bit(130), -1);
    assert!(!b.get(128));
    assert!(b.get(129));
    assert!(!b.get(512));
    let sticky = BitSet::new(500);
    assert_eq!(sticky.clone().size(), 512);
}
