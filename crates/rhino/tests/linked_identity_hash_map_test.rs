/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/base/LinkedIdentityHashMapTest.java.

use closure_rhino::jscomp_base::LinkedIdentityHashMap;
use std::{
    fmt::Display,
    hash::{Hash, Hasher},
    sync::Arc,
};
#[derive(Debug)]
struct Key;
impl PartialEq for Key {
    // port: LinkedIdentityHashMapTest.Key#equals
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Key {}
impl Key {
    // port: LinkedIdentityHashMapTest.Key#hashCode
    fn hash_code(&self) -> i32 {
        0
    }
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_i32(self.hash_code());
    }
}
// port: LinkedIdentityHashMapTest#validateKeys
fn validate_keys() -> [Arc<Key>; 3] {
    let keys = [Arc::new(Key), Arc::new(Key), Arc::new(Key)];
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        assert_eq!(keys[a], keys[b]);
        assert!(!Arc::ptr_eq(&keys[a], &keys[b]));
    }
    assert_eq!(keys[0].hash_code(), 0);
    assert_eq!(keys[1].hash_code(), 0);
    assert_eq!(keys[2].hash_code(), 0);
    keys
}
impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("com.google.javascript.jscomp.base.LinkedIdentityHashMapTest$Key@0")
    }
}
// port: LinkedIdentityHashMapTest#throwOnCall(P)
fn throw_on_call<T: Display, R>(arg: T) -> R {
    panic!("Unexpected call with arg '{arg}'")
}
// port: LinkedIdentityHashMapTest#throwOnCall(P0,P1)
fn throw_on_call2<T: Display, U: Display, R>(arg0: T, arg1: Option<U>) -> R {
    let second = arg1.map_or_else(|| "null".to_owned(), |a| a.to_string());
    panic!("Unexpected call with args '{arg0}', '{second}'")
}
type Map = LinkedIdentityHashMap<Arc<Key>, String>;
// port: LinkedIdentityHashMapTest#entries_keyedByIdentity
#[test]
fn entries_keyed_by_identity() {
    let [k0, k1, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    assert_eq!(map.get(&k1), None);
    map.put(k0.clone(), Some("hello".into()));
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
    assert_eq!(map.get(&k1), None);
    map.put(k1.clone(), Some("world".into()));
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
    assert_eq!(map.get(&k1).map(String::as_str), Some("world"));
}
// port: LinkedIdentityHashMapTest#entries_allowNullValues
#[test]
fn entries_allow_null_values() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), None);
    assert_eq!(map.get(&k0), None);
}
// port: LinkedIdentityHashMapTest#entries_canBeOverwritten
#[test]
fn entries_can_be_overwritten() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), Some("hello".into()));
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
    map.put(k0.clone(), Some("world".into()));
    assert_eq!(map.get(&k0).map(String::as_str), Some("world"));
    map.put(k0.clone(), None);
    assert_eq!(map.get(&k0), None);
}
// port: LinkedIdentityHashMapTest#put_returnsPreviousValue
#[test]
fn put_returns_previous_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    assert_eq!(map.put(k0.clone(), Some("hello".into())), None);
    assert_eq!(
        map.put(k0.clone(), Some("world".into())),
        Some("hello".into())
    );
    assert_eq!(map.put(k0.clone(), None), Some("world".into()));
    assert_eq!(map.put(k0, Some("hello".into())), None);
}
// port: LinkedIdentityHashMapTest#computeIfAbsent_setsWhenNoEntry_returnsCurrentValue
#[test]
fn compute_if_absent_sets_when_no_entry_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    assert_eq!(
        map.compute_if_absent(k0.clone(), |_| Some("hello".into()))
            .map(String::as_str),
        Some("hello")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
}
// port: LinkedIdentityHashMapTest#computeIfAbsent_setsWhenNullEntry_returnsCurrentValue
#[test]
fn compute_if_absent_sets_when_null_entry_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), Some("world".into()));
    map.put(k0.clone(), None);
    assert_eq!(
        map.compute_if_absent(k0.clone(), |_| Some("hello".into()))
            .map(String::as_str),
        Some("hello")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
}
// port: LinkedIdentityHashMapTest#computeIfAbsent_noopWhenEntry_returnsCurrentValue
#[test]
fn compute_if_absent_noop_when_entry_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), Some("world".into()));
    assert_eq!(
        map.compute_if_absent(k0.clone(), |k| throw_on_call(k))
            .map(String::as_str),
        Some("world")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("world"));
}
// port: LinkedIdentityHashMapTest#computeIfAbsent_passesKey
#[test]
fn compute_if_absent_passes_key() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.compute_if_absent(k0.clone(), |k| {
        assert!(Arc::ptr_eq(k, &k0));
        Some("hello".into())
    });
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
}
// port: LinkedIdentityHashMapTest#merge_setsWhenNoEntry_returnsCurrentValue
#[test]
fn merge_sets_when_no_entry_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    assert_eq!(
        map.merge(k0.clone(), Some("hello".into()), |a, b| throw_on_call2(
            a, b
        ))
        .map(String::as_str),
        Some("hello")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
}
// port: LinkedIdentityHashMapTest#merge_setsWhenNullEntry_returnsCurrentValue
#[test]
fn merge_sets_when_null_entry_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), Some("world".into()));
    map.put(k0.clone(), None);
    assert_eq!(
        map.merge(k0.clone(), Some("hello".into()), |a, b| throw_on_call2(
            a, b
        ))
        .map(String::as_str),
        Some("hello")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello"));
}
// port: LinkedIdentityHashMapTest#merge_mergesExistingValue_returnsCurrentValue
#[test]
fn merge_merges_existing_value_returns_current_value() {
    let [k0, _, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    map.put(k0.clone(), Some("hello".into()));
    assert_eq!(
        map.merge(k0.clone(), Some("world".into()), |v0, v1| Some(format!(
            "{} {}",
            v0,
            v1.unwrap()
        )))
        .map(String::as_str),
        Some("hello world")
    );
    assert_eq!(map.get(&k0).map(String::as_str), Some("hello world"));
}
// port: LinkedIdentityHashMapTest#forEach_noopOnEmpty
#[test]
fn for_each_noop_on_empty() {
    Map::default().for_each(|a, b| throw_on_call2(a, b));
}
// port: LinkedIdentityHashMapTest#forEach_iteratesInInsertionOrder
#[test]
fn for_each_iterates_in_insertion_order() {
    let [k0, k1, k2] = validate_keys();
    let mut map = Map::default();
    for k in [&k0, &k1, &k2] {
        assert_eq!(map.get(k), None);
    }
    map.put(k1.clone(), Some("hello".into()));
    map.put(k0.clone(), Some("world".into()));
    map.put(k2.clone(), Some("great".into()));
    let mut keys = Vec::new();
    let mut values = Vec::new();
    map.for_each(|k, v| {
        keys.push(k.clone());
        values.push(v.cloned());
    });
    for (a, b) in keys.iter().zip([k1, k0, k2]) {
        assert!(Arc::ptr_eq(a, &b));
    }
    assert_eq!(keys.len(), 3);
    assert_eq!(
        values,
        vec![
            Some("hello".into()),
            Some("world".into()),
            Some("great".into())
        ]
    );
}
// port: LinkedIdentityHashMapTest#forEach_iteratesInInsertionOrder_afterOverwrite
#[test]
fn for_each_iterates_in_insertion_order_after_overwrite() {
    let [k0, k1, k2] = validate_keys();
    let mut map = Map::default();
    for k in [&k0, &k1, &k2] {
        assert_eq!(map.get(k), None);
    }
    map.put(k1.clone(), Some("hello".into()));
    map.put(k0.clone(), Some("world".into()));
    map.put(k2.clone(), Some("great".into()));
    map.put(k2.clone(), Some("hello".into()));
    map.put(k0.clone(), Some("great".into()));
    map.put(k1.clone(), Some("world".into()));
    let mut keys = Vec::new();
    let mut values = Vec::new();
    map.for_each(|k, v| {
        keys.push(k.clone());
        values.push(v.cloned());
    });
    for (a, b) in keys.iter().zip([k1, k0, k2]) {
        assert!(Arc::ptr_eq(a, &b));
    }
    assert_eq!(keys.len(), 3);
    assert_eq!(
        values,
        vec![
            Some("world".into()),
            Some("great".into()),
            Some("hello".into())
        ]
    );
}
// port: LinkedIdentityHashMapTest#forEach_includesNullValues
#[test]
fn for_each_includes_null_values() {
    let [k0, k1, _] = validate_keys();
    let mut map = Map::default();
    assert_eq!(map.get(&k0), None);
    assert_eq!(map.get(&k1), None);
    map.put(k0.clone(), None);
    map.put(k1.clone(), Some("hello".into()));
    map.put(k1.clone(), None);
    let mut keys = Vec::new();
    let mut values = Vec::new();
    map.for_each(|k, v| {
        keys.push(k.clone());
        values.push(v.cloned());
    });
    for (a, b) in keys.iter().zip([k0, k1]) {
        assert!(Arc::ptr_eq(a, &b));
    }
    assert_eq!(keys.len(), 2);
    assert_eq!(values, vec![None, None]);
}
