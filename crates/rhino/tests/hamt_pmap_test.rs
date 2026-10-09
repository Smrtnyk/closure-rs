/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/rhino/HamtPMapTest.java.

use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{hamt_pmap::HamtPMap, java_lang::JavaHashCode, pmap::Reconciler};
use std::{
    collections::BTreeSet,
    fmt::{self, Debug, Display},
    hash::Hash,
};

// port: HamtPMapTest#build
fn build(values: &[i32]) -> HamtPMap<i32, i32> {
    let mut map = HamtPMap::empty();
    for &value in values {
        map = map.plus(value, value);
        map.assert_correct_structure();
    }
    map
}
#[derive(Debug, Clone)]
struct IntegerWithCustomHashCode {
    i: i32,
    hash_code: i32,
}
impl IntegerWithCustomHashCode {
    // port: IntegerWithCustomHashCode#IntegerWithCustomHashCode
    fn new(i: i32, hash_code: i32) -> Self {
        Self { i, hash_code }
    }
}
impl JavaHashCode for IntegerWithCustomHashCode {
    // port: IntegerWithCustomHashCode#hashCode
    fn hash_code(&self) -> i32 {
        self.hash_code
    }
}
impl PartialEq for IntegerWithCustomHashCode {
    // port: IntegerWithCustomHashCode#equals
    fn eq(&self, that: &Self) -> bool {
        self.i == that.i
    }
}
impl Display for IntegerWithCustomHashCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.i, f)
    }
}
// port: HamtPMapTest#buildWithCustomHashCode
fn build_with_custom_hash_code(
    fixed_hash_code: i32,
    values: &[i32],
) -> HamtPMap<IntegerWithCustomHashCode, i32> {
    let mut map = HamtPMap::empty();
    for &value in values {
        map = map.plus(
            IntegerWithCustomHashCode::new(value, fixed_hash_code),
            value,
        );
        map.assert_correct_structure();
    }
    map
}
// port: HamtPMapTest#equalsAndCheckNotNull
fn equals_and_check_not_null<T: PartialEq>(o1: &T, o2: &T) -> bool {
    o1 == o2
} // References enforce Java's checkNotNull.
// port: Truth#containsExactly
fn contains_exactly<T: Ord + Debug>(actual: impl Iterator<Item = T>, mut expected: Vec<T>) {
    let mut actual: Vec<_> = actual.collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
struct JoinExpectations<K, V> {
    expectations: IndexMap<K, [Option<V>; 3]>,
}
impl<K: Eq + Hash + Debug, V: Clone + PartialEq + Debug> Reconciler<K, V>
    for JoinExpectations<K, V>
{
    // port: JoinExpectations#merge
    fn merge(&mut self, key: &K, left: Option<&V>, right: Option<&V>) -> V {
        assert!(self.expectations.contains_key(key));
        let entries = self.expectations.get(key).unwrap();
        assert_eq!(entries[0].as_ref(), left);
        assert_eq!(entries[1].as_ref(), right);
        self.expectations.shift_remove(key).unwrap()[2]
            .take()
            .unwrap()
    }
}
impl<K: Eq + Hash, V> JoinExpectations<K, V> {
    // port: JoinExpectations#JoinExpectations
    fn new() -> Self {
        Self {
            expectations: IndexMap::<_, _>::default(),
        }
    }
    // port: JoinExpectations#expect
    fn expect(mut self, key: K, left: Option<V>, right: Option<V>, result: V) -> Self {
        assert!(!self.expectations.contains_key(&key));
        self.expectations.insert(key, [left, right, Some(result)]);
        self
    }
    // port: JoinExpectations#verify
    fn verify(&self) {
        assert!(self.expectations.is_empty());
    }
}
// port: HamtPMapTest#testEmpty
#[test]
fn test_empty() {
    let map: HamtPMap<String, i32> = HamtPMap::empty();
    assert_eq!(map.get(&"foo".into()), None);
    assert_eq!(map.values().count(), 0);
    assert!(map.is_empty());
}
// port: HamtPMapTest#testPlus
#[test]
fn test_plus() {
    let empty: HamtPMap<String, i32> = HamtPMap::empty();
    let map = empty.plus("foo".into(), 42);
    assert_eq!(map.get(&"foo".into()), Some(&42));
    contains_exactly(map.values().copied(), vec![42]);
    assert!(!map.is_empty());
}
// port: HamtPMapTest#testPlus_alreadyExistsReturnsSame
#[test]
fn test_plus_already_exists_returns_same() {
    let empty: HamtPMap<String, i32> = HamtPMap::empty();
    let map = empty.plus("foo".into(), 42);
    assert!(map.plus("foo".into(), 42).ptr_eq(&map));
}
// port: HamtPMapTest#testPlus_updatesExistingKey
#[test]
fn test_plus_updates_existing_key() {
    let empty: HamtPMap<String, i32> = HamtPMap::empty();
    let map = empty.plus("foo".into(), 42).plus("foo".into(), 23);
    assert_eq!(map.get(&"foo".into()), Some(&23));
    contains_exactly(map.values().copied(), vec![23]);
    assert!(!map.is_empty());
}
// port: HamtPMapTest#testMinus
#[test]
fn test_minus() {
    let map: HamtPMap<String, i32> = HamtPMap::empty().plus("foo".into(), 42);
    let empty = map.minus(&"foo".into());
    assert_eq!(empty.get(&"foo".into()), None);
    assert_eq!(empty.values().count(), 0);
    assert!(empty.is_empty());
}
// port: HamtPMapTest#testMinus_nonexistentKeyReturnsSame
#[test]
fn test_minus_nonexistent_key_returns_same() {
    let empty: HamtPMap<String, i32> = HamtPMap::empty();
    let map = empty.plus("foo".into(), 42);
    assert!(map.minus(&"bar".into()).ptr_eq(&map));
}
// port: HamtPMapTest#testPlusAndMinus
#[test]
fn test_plus_and_minus() {
    let empty: HamtPMap<String, i32> = HamtPMap::empty();
    let mut map = empty
        .plus("foo".into(), 42)
        .plus("bar".into(), 19)
        .plus("baz".into(), 12);
    assert_eq!(map.get(&"foo".into()), Some(&42));
    assert_eq!(map.get(&"bar".into()), Some(&19));
    assert_eq!(map.get(&"baz".into()), Some(&12));
    contains_exactly(map.values().copied(), vec![12, 19, 42]);
    assert!(!map.is_empty());
    map = map.minus(&"bar".into());
    assert_eq!(map.get(&"foo".into()), Some(&42));
    assert_eq!(map.get(&"bar".into()), None);
    assert_eq!(map.get(&"baz".into()), Some(&12));
    contains_exactly(map.values().copied(), vec![12, 42]);
    assert!(!map.is_empty());
    map = map.minus(&"baz".into());
    assert_eq!(map.get(&"foo".into()), Some(&42));
    assert_eq!(map.get(&"baz".into()), None);
    contains_exactly(map.values().copied(), vec![42]);
    assert!(!map.is_empty());
    map = map.minus(&"foo".into());
    assert_eq!(map.get(&"foo".into()), None);
    assert_eq!(map.values().count(), 0);
    assert!(map.is_empty());
}
// port: HamtPMapTest#testReconcile_customJoiner
#[test]
fn test_reconcile_custom_joiner() {
    let empty: HamtPMap<i32, String> = HamtPMap::empty();
    let left = empty
        .plus(1, "1".into())
        .plus(3, "3".into())
        .plus(5, "5".into())
        .plus(7, "7".into())
        .plus(9, "9".into());
    let right = empty
        .plus(3, "C".into())
        .plus(4, "D".into())
        .plus(5, "5".into())
        .plus(6, "F".into())
        .plus(7, "G".into());
    let mut expectations = JoinExpectations::new()
        .expect(1, Some("1".into()), None, "1".into())
        .expect(3, Some("3".into()), Some("C".into()), "c".into())
        .expect(4, None, Some("D".into()), "D".into())
        .expect(6, None, Some("F".into()), "F".into())
        .expect(7, Some("7".into()), Some("G".into()), "g".into())
        .expect(9, Some("9".into()), None, "9".into());
    let joined = left.reconcile(&right, &mut expectations);
    contains_exactly(
        joined.values().map(String::as_str),
        vec!["1", "c", "D", "5", "F", "g", "9"],
    );
    expectations.verify();
}
// port: HamtPMapTest#testReconcile_emptyMaps
#[test]
fn test_reconcile_empty_maps() {
    let empty: HamtPMap<i32, String> = HamtPMap::empty();
    let map = empty
        .plus(1, "1".into())
        .plus(3, "3".into())
        .plus(5, "5".into())
        .plus(7, "7".into())
        .plus(9, "9".into());
    let mut joined = map.reconcile(
        &empty,
        &mut |_: &i32, a: Option<&String>, b: Option<&String>| {
            assert!(b.is_none());
            a.unwrap().clone()
        },
    );
    assert!(joined.equivalent(&map, PartialEq::eq));
    joined = empty.reconcile(
        &map,
        &mut |_: &i32, a: Option<&String>, b: Option<&String>| {
            assert!(a.is_none());
            b.unwrap().clone()
        },
    );
    assert!(joined.equivalent(&map, PartialEq::eq));
}
// port: HamtPMapTest#testReconcile_differentSizes
#[test]
fn test_reconcile_different_sizes() {
    let left = build(&[1, 6, 2, 19, 4, 23, 5, 8, 42, 12, 18, 33]);
    let right = build(&[])
        .plus(3, 1)
        .plus(4, 1)
        .plus(8, 1)
        .plus(19, 1)
        .plus(25, 1)
        .plus(42, 1);
    let expected = left
        .plus(3, 1)
        .plus(4, 5)
        .plus(8, 9)
        .plus(19, 20)
        .plus(25, 1)
        .plus(42, 43);
    let joined = left.reconcile(
        &right,
        &mut |_: &i32, a: Option<&i32>, b: Option<&i32>| match (a, b) {
            (None, Some(b)) => *b,
            (Some(a), None) => *a,
            (Some(a), Some(b)) => a + b,
            _ => unreachable!(),
        },
    );
    assert!(joined.equivalent(&expected, PartialEq::eq));
}
// port: HamtPMapTest#testReconcile_rejectsNullResult
#[test]
fn test_reconcile_rejects_null_result() {
    let left = build(&[1, 3, 5, 7]);
    let right = build(&[2, 4, 6, 8]);
    assert!(std::panic::catch_unwind(|| left.reconcile_nullable(&right, |_, _, _| None)).is_err());
}
// port: HamtPMapTest#testEquivalent_equivalent
#[test]
fn test_equivalent_equivalent() {
    let left = build(&[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024]);
    let right = build(&[2, 512, 32, 8, 1024, 1, 64, 4, 16, 256, 128]);
    assert!(left.equivalent(&right, PartialEq::eq));
    assert!(right.equivalent(&left, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_equivalent_hashCodeCollisions
#[test]
fn test_equivalent_equivalent_hash_code_collisions() {
    let fixed_hash_code = 42;
    let left = build_with_custom_hash_code(
        fixed_hash_code,
        &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024],
    );
    let right = build_with_custom_hash_code(
        fixed_hash_code,
        &[2, 512, 32, 8, 1024, 1, 64, 4, 16, 256, 128],
    );
    assert!(left.equivalent(&right, PartialEq::eq));
    assert!(right.equivalent(&left, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_oneDifferentValue
#[test]
fn test_equivalent_one_different_value() {
    let left1 = build(&[1, 3, 5, 7]);
    let left2 = build(&[7, 3, 5, 1]);
    let right1 = build(&[]).plus(1, 2).plus(3, 3).plus(5, 5).plus(7, 7);
    let right2 = build(&[1, 3, 5]).plus(7, 8);
    assert!(!left1.equivalent(&right1, PartialEq::eq));
    assert!(!right1.equivalent(&left1, PartialEq::eq));
    assert!(!left2.equivalent(&right1, PartialEq::eq));
    assert!(!right1.equivalent(&left2, PartialEq::eq));
    assert!(!left1.equivalent(&right2, PartialEq::eq));
    assert!(!right2.equivalent(&left1, PartialEq::eq));
    assert!(!left2.equivalent(&right2, PartialEq::eq));
    assert!(!right2.equivalent(&left2, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_oneDifferentKey
#[test]
fn test_equivalent_one_different_key() {
    let left1 = build(&[]).plus(2, 1).plus(3, 3).plus(5, 5).plus(7, 7);
    let left2 = build(&[1, 3, 5]).plus(7, 8);
    let right = build(&[1, 3, 5, 7]);
    assert!(!left1.equivalent(&right, PartialEq::eq));
    assert!(!right.equivalent(&left1, PartialEq::eq));
    assert!(!left2.equivalent(&right, PartialEq::eq));
    assert!(!right.equivalent(&left2, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_oneDifferentKey_withHashCodeCollisions
#[test]
fn test_equivalent_one_different_key_with_hash_code_collisions() {
    let fixed_hash_code = 42;
    let left1 = build_with_custom_hash_code(42, &[])
        .plus(IntegerWithCustomHashCode::new(2, fixed_hash_code), 1)
        .plus(IntegerWithCustomHashCode::new(3, fixed_hash_code), 3)
        .plus(IntegerWithCustomHashCode::new(5, fixed_hash_code), 5)
        .plus(IntegerWithCustomHashCode::new(7, fixed_hash_code), 7);
    let left2 = build_with_custom_hash_code(fixed_hash_code, &[1, 3, 5])
        .plus(IntegerWithCustomHashCode::new(7, fixed_hash_code), 8);
    let right = build_with_custom_hash_code(fixed_hash_code, &[1, 3, 5, 7]);
    assert!(!left1.equivalent(&right, PartialEq::eq));
    assert!(!right.equivalent(&left1, PartialEq::eq));
    assert!(!left2.equivalent(&right, PartialEq::eq));
    assert!(!right.equivalent(&left2, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_oneDifferentKey_withHashCodeCollisions_doesNotPassNullToEquivalenceTest
#[test]
fn test_equivalent_one_different_key_with_hash_code_collisions_does_not_pass_null_to_equivalence_test()
 {
    let fixed_hash_code = 42;
    let left1 = build_with_custom_hash_code(42, &[])
        .plus(IntegerWithCustomHashCode::new(2, fixed_hash_code), 1)
        .plus(IntegerWithCustomHashCode::new(3, fixed_hash_code), 3)
        .plus(IntegerWithCustomHashCode::new(5, fixed_hash_code), 5)
        .plus(IntegerWithCustomHashCode::new(7, fixed_hash_code), 7);
    let left2 = build_with_custom_hash_code(fixed_hash_code, &[1, 3, 5])
        .plus(IntegerWithCustomHashCode::new(7, fixed_hash_code), 8);
    let right = build_with_custom_hash_code(fixed_hash_code, &[1, 3, 5, 7]);
    assert!(!left1.equivalent(&right, equals_and_check_not_null));
    assert!(!right.equivalent(&left1, equals_and_check_not_null));
    assert!(!left2.equivalent(&right, equals_and_check_not_null));
    assert!(!right.equivalent(&left2, equals_and_check_not_null));
}
// port: HamtPMapTest#testEquivalent_oneMissingKey
#[test]
fn test_equivalent_one_missing_key() {
    let left = build(&[1, 5, 4, 3, 9]);
    let right = build(&[1, 5, 3, 9]);
    assert!(!left.equivalent(&right, PartialEq::eq));
    assert!(!right.equivalent(&left, PartialEq::eq));
}
// port: HamtPMapTest#testEquivalent_equalsWithCustomEquivalence
#[test]
fn test_equivalent_equals_with_custom_equivalence() {
    let empty: HamtPMap<String, String> = HamtPMap::empty();
    let left = empty
        .plus("abc".into(), "AAa".into())
        .plus("def".into(), "Ddd".into())
        .plus("ghi".into(), "ggG".into());
    let right = empty
        .plus("abc".into(), "aAA".into())
        .plus("def".into(), "ddD".into())
        .plus("ghi".into(), "GGG".into());
    assert!(left.equivalent(&right, |l, r| l.to_lowercase() == r.to_lowercase()));
}
// port: HamtPMapTest#testEquivalent_notEquivalentShortCircuits
#[test]
fn test_equivalent_not_equivalent_short_circuits() {
    let empty: HamtPMap<i32, String> = HamtPMap::empty();
    let left = empty.plus(1, "1".into()).plus(2, "2".into());
    let right = empty.plus(1, "3".into()).plus(2, "4".into());
    let mut calls = 0;
    assert!(!left.equivalent(&right, |_, _| {
        calls += 1;
        false
    }));
    assert_eq!(calls, 1);
}
// port: HamtPMapTest#testKeys_containsExactlyKeys
#[test]
fn test_keys_contains_exactly_keys() {
    let empty: HamtPMap<i32, String> = HamtPMap::empty();
    let actual = empty
        .plus(1, "a".into())
        .plus(2, "b".into())
        .plus(100, "x".into())
        .plus(-847, "h".into())
        .minus(&2);
    contains_exactly(actual.keys().copied(), vec![1, 100, -847]);
}
// port: HamtPMapTest#testValues_containsExactlyValues
#[test]
fn test_values_contains_exactly_values() {
    let empty: HamtPMap<i32, String> = HamtPMap::empty();
    let actual = empty
        .plus(1, "a".into())
        .plus(2, "b".into())
        .plus(3, "kkdmw".into())
        .plus(4, ":::".into())
        .minus(&1);
    contains_exactly(
        actual.values().map(String::as_str),
        vec!["b", "kkdmw", ":::"],
    );
}
// port: HamtPMapTest#testIntegration
#[test]
fn test_integration() {
    let mut reference = BTreeSet::new();
    let mut map: HamtPMap<i32, String> = HamtPMap::empty();
    let mut i = 17;
    while reference.insert(i) {
        map = map.plus(i, i.to_string());
        map.assert_correct_structure();
        assert_eq!(map.is_empty(), reference.is_empty());
        for j in 1..127 {
            assert_eq!(
                map.get(&j),
                reference.contains(&j).then(|| j.to_string()).as_ref()
            );
            if reference.contains(&j) {
                assert!(map.plus(j, j.to_string()).ptr_eq(&map));
            } else {
                assert!(map.minus(&j).ptr_eq(&map));
            }
        }
        contains_exactly(
            map.values().cloned(),
            reference.iter().map(i32::to_string).collect(),
        );
        i = (i * 43) % 127;
    }
    assert_eq!(reference.len(), 126);
    i = 12;
    while reference.remove(&i) {
        map = map.minus(&i);
        map.assert_correct_structure();
        assert_eq!(map.is_empty(), reference.is_empty());
        for j in 1..127 {
            assert_eq!(
                map.get(&j),
                reference.contains(&j).then(|| j.to_string()).as_ref()
            );
            if reference.contains(&j) {
                assert!(map.plus(j, j.to_string()).ptr_eq(&map));
            } else {
                assert!(map.minus(&j).ptr_eq(&map));
            }
        }
        contains_exactly(
            map.values().cloned(),
            reference.iter().map(i32::to_string).collect(),
        );
        i = (i * 39) % 127;
    }
    assert!(reference.is_empty());
    assert!(map.ptr_eq(&HamtPMap::empty()));
}
