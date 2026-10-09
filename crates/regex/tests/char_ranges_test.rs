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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/regex/CharRangesTest.java.

//! Port of `com.google.javascript.jscomp.regex.CharRangesTest`.

use std::panic;
use std::time::{SystemTime, UNIX_EPOCH};

use closure_regex::char_ranges::{self, CharRanges};

// `java.util.Random`, ported from OpenJDK (GPL-2.0 with the Classpath exception), is in its own
// file.
#[path = "jdk/java_util_random.rs"]
mod java_util_random;
use java_util_random::Random;

/// `java.util.BitSet`, the subset the test uses.
struct BitSet {
    bits: Vec<bool>,
}

impl BitSet {
    fn new() -> BitSet {
        BitSet { bits: Vec::new() }
    }

    fn set(&mut self, i: usize) {
        if i >= self.bits.len() {
            self.bits.resize(i + 1, false);
        }
        self.bits[i] = true;
    }

    fn get(&self, i: usize) -> bool {
        i < self.bits.len() && self.bits[i]
    }

    fn cardinality(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }

    fn next_set_bit(&self, from: usize) -> i32 {
        (from..self.bits.len())
            .find(|i| self.bits[*i])
            .map_or(-1, |i| i as i32)
    }
}

/// `CharRangesTest.SEED`: `junit.random.seed` (here the `JUNIT_RANDOM_SEED` environment variable)
/// or the current time in milliseconds.
fn seed() -> i64 {
    std::env::var("JUNIT_RANDOM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
        })
}

/// Guava `EqualsTester().addEqualityGroup(items...).testEquals()` for one group: every item
/// equals itself and every other item of the group, with equal hash codes.
#[allow(clippy::eq_op)] // EqualsTester checks reflexivity on purpose.
fn test_equals_group(group: &[&CharRanges]) {
    for a in group {
        assert_eq!(*a, *a);
        assert_eq!(a.hash_code(), a.hash_code());
        for b in group {
            assert_eq!(*a, *b, "{a} must equal {b}");
            assert_eq!(a.hash_code(), b.hash_code(), "hashCode of {a} and {b}");
        }
    }
}

fn assert_illegal_argument(f: impl FnOnce() -> CharRanges + panic::UnwindSafe, what: &str) {
    let result = panic::catch_unwind(f);
    match result {
        Ok(r) => panic!("{what}: expected IllegalArgumentException, got {r}"),
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            assert!(
                msg.starts_with("java.lang.IllegalArgumentException"),
                "{what}: unexpected panic {msg}"
            );
        }
    }
}

// port: CharRangesTest#testAgainstRegularImplementation
#[test]
fn test_against_regular_implementation() {
    let seed = seed();
    let mut rnd = Random::new(seed);

    let mut run = 10;
    loop {
        run -= 1;
        if run < 0 {
            break;
        }
        // Fill with bits in the range [0x1000, 0x3000).
        let mut bs = BitSet::new();
        let mut i = 0x1000;
        loop {
            i -= 1;
            if i < 0 {
                break;
            }
            bs.set((0x1000 + rnd.next_int(0x3000)) as usize);
        }

        // Create an equivalent sparse bit set
        let mut members = vec![0i32; bs.cardinality()];
        let mut i: i32 = -1;
        for member in members.iter_mut() {
            i = bs.next_set_bit((i + 1) as usize);
            *member = i;
        }
        let sbs = CharRanges::with_members(&members);

        // Check all bits including past the min/max bit
        for i in 0..0x5000 {
            if bs.get(i as usize) != sbs.contains(i) {
                panic!("seed={seed}, sbs={sbs}, difference at bit {i}");
            }
        }
    }
}

// port: CharRangesTest#testEmptyCharRanges
#[test]
fn test_empty_char_ranges() {
    let sbs = &*char_ranges::EMPTY;
    for i in -1000..1000 {
        assert!(!sbs.contains(i));
    }
    assert_eq!(sbs.to_string(), "[]");
}

// port: CharRangesTest#testCharRangesFactories
#[test]
fn test_char_ranges_factories() {
    let isbs = CharRanges::with_members(&[0, 1, 4, 9]);
    let isbs2 = CharRanges::with_members(&[0, 1, 4, 9]);
    assert_eq!(isbs.to_string(), "[0x0-0x1 0x4 0x9]");

    let esbs = CharRanges::with_members(&[]);

    assert_eq!(isbs2, isbs);
    assert_ne!(isbs, esbs);
    // assertThat(isbs).isNotNull() and isNotEqualTo(new Object()): a Rust CharRanges is never
    // null and only compares with CharRanges, so both hold by construction.

    assert_eq!(isbs2.hash_code(), isbs.hash_code());
    assert_ne!(isbs.hash_code(), esbs.hash_code());
}

// port: CharRangesTest#testRangeConstructor
#[test]
fn test_range_constructor() {
    assert_illegal_argument(|| CharRanges::with_ranges(&[1]), "Mismatched ranges");

    assert_illegal_argument(
        || CharRanges::with_ranges(&[1, 4, 4, 5]),
        "Discontiguous ranges",
    );

    assert_illegal_argument(
        || CharRanges::with_ranges(&[4, 5, 1, 3]),
        "Misordered ranges",
    );

    assert_illegal_argument(|| CharRanges::with_ranges(&[0, 0]), "Empty range");
}

// port: CharRangesTest#testDupeMembers
#[test]
fn test_dupe_members() {
    let sbs1 = CharRanges::with_members(&[0, 1, 4, 9]);
    assert_eq!(sbs1.to_string(), "[0x0-0x1 0x4 0x9]", "{sbs1}");

    let sbs2 = CharRanges::with_members(&[9, 1, 4, 1, 0]);
    assert_eq!(sbs2.to_string(), "[0x0-0x1 0x4 0x9]", "{sbs2}");

    test_equals_group(&[&sbs1, &sbs2]);

    for i in -10..20 {
        assert_eq!(sbs2.contains(i), sbs1.contains(i), "{i}");
    }
}

// port: CharRangesTest#testDifference
#[test]
fn test_difference() {
    //                     1               2               3
    //     0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0
    // b-a  DD         DD DDD        D      DDD
    // a      AAAAAAAAA      A A A A   A AAA   AAA A A
    // b    BBB  BBB  BBB BBB        B B    BBB
    // a-b     DD   DD       D D D D     DDD   DDD D D
    let a = CharRanges::with_ranges(&[
        0x03, 0x0C, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1C, 0x1D, 0x1E, 0x21, 0x24,
        0x27, 0x28, 0x29, 0x2A, 0x2B,
    ]);
    let b = CharRanges::with_ranges(&[
        0x01, 0x04, 0x06, 0x09, 0x0B, 0x0E, 0x0F, 0x12, 0x1A, 0x1B, 0x1C, 0x1D, 0x21, 0x24,
    ]);
    let empty = CharRanges::with_members(&[]);

    assert_eq!(empty.union(&empty), empty);
    assert_eq!(a.union(&empty), a);
    assert_eq!(empty.union(&b), b);

    let a_sb = a.difference(&b);
    assert_eq!(
        a_sb.to_string(),
        "[0x4-0x5 0x9-0xa 0x12 0x14 0x16 0x18 0x1e-0x20 0x24-0x26 0x28 0x2a]"
    );
    assert!(a.contains_all(&a_sb));
    assert!(!a_sb.contains_all(&a));
    assert!(!a_sb.contains_all(&b));

    let b_sa = b.difference(&a);
    assert_eq!(
        b_sa.to_string(),
        "[0x1-0x2 0xc-0xd 0xf-0x11 0x1a 0x21-0x23]"
    );
    assert!(b.contains_all(&b_sa));
    assert!(!b_sa.contains_all(&a));
    assert!(!b_sa.contains_all(&b));

    // Check that a and b not changed by operation
    assert_eq!(
        a.to_string(),
        "[0x3-0xb 0x12 0x14 0x16 0x18 0x1c 0x1e-0x20 0x24-0x26 0x28 0x2a]"
    );
    assert_eq!(
        b.to_string(),
        "[0x1-0x3 0x6-0x8 0xb-0xd 0xf-0x11 0x1a 0x1c 0x21-0x23]"
    );

    //    0 1 2 3 4 5 6 7 8 9 a b c d e f
    // m: * * * *     *     * *       * *
    // s:     *     * * *     * *   * *
    // d: * *   *           *           *
    let m = CharRanges::with_members(&[0, 1, 2, 3, 6, 9, 0xa, 0xe, 0xf]);
    let s = CharRanges::with_members(&[2, 5, 6, 7, 0xa, 0xb, 0xd, 0xe]);
    let d = m.difference(&s);
    assert_eq!(d.to_string(), "[0x0-0x1 0x3 0x9 0xf]");
    assert!(m.contains_all(&d));
    assert!(!d.contains_all(&m));
    assert!(!d.contains_all(&s));
    assert!(!s.contains_all(&d));
    assert!(d.contains_all(&d));
}

// port: CharRangesTest#testUnion
#[test]
fn test_union() {
    //                 1               2               3
    // 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0
    //    AAAAAAAAA      A A A A   A AAA   AAA A A
    //  BBB  BBB  BBB BBB        B B    BBB
    //  UUUUUUUUUUUUU UUUU U U U U U UUUUUUUUU U U
    let a = CharRanges::with_ranges(&[
        0x03, 0x0C, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1C, 0x1D, 0x1E, 0x21, 0x24,
        0x27, 0x28, 0x29, 0x2A, 0x2B,
    ]);
    let b = CharRanges::with_ranges(&[
        0x01, 0x04, 0x06, 0x09, 0x0B, 0x0E, 0x0F, 0x12, 0x1A, 0x1B, 0x1C, 0x1D, 0x21, 0x24,
    ]);
    let empty = CharRanges::with_members(&[]);

    assert_eq!(empty.union(&empty), empty);
    assert_eq!(a.union(&empty), a);
    assert_eq!(empty.union(&b), b);

    let a_ub = a.union(&b);
    assert_eq!(
        a_ub.to_string(),
        "[0x1-0xd 0xf-0x12 0x14 0x16 0x18 0x1a 0x1c 0x1e-0x26 0x28 0x2a]"
    );
    assert_eq!(b.union(&a), a_ub);
    assert!(a_ub.contains_all(&a));
    assert!(a_ub.contains_all(&b));
    assert!(!a.contains_all(&b));
    assert!(!b.contains_all(&a));
    assert!(a.contains_all(&a));
    assert!(b.contains_all(&b));
    assert!(a_ub.contains_all(&a_ub));

    // Check that a and b not changed by operation
    assert_eq!(
        a.to_string(),
        "[0x3-0xb 0x12 0x14 0x16 0x18 0x1c 0x1e-0x20 0x24-0x26 0x28 0x2a]"
    );
    assert_eq!(
        b.to_string(),
        "[0x1-0x3 0x6-0x8 0xb-0xd 0xf-0x11 0x1a 0x1c 0x21-0x23]"
    );
}
