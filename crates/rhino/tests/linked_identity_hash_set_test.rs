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
//   test/com/google/javascript/jscomp/base/LinkedIdentityHashSetTest.java.

use closure_rhino::jscomp_base::LinkedIdentityHashSet;
use std::sync::Arc;
#[derive(Debug)]
struct Key;
impl PartialEq for Key {
    // port: LinkedIdentityHashSetTest.Key#equals
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Key {}
impl Key {
    // port: LinkedIdentityHashSetTest.Key#hashCode
    fn hash_code(&self) -> i32 {
        0
    }
}
// port: LinkedIdentityHashSetTest#validateKeys
fn validate_keys() -> [Arc<Key>; 2] {
    let keys = [Arc::new(Key), Arc::new(Key)];
    assert_eq!(keys[0], keys[1]);
    assert!(!Arc::ptr_eq(&keys[0], &keys[1]));
    assert_eq!(keys[0].hash_code(), 0);
    assert_eq!(keys[1].hash_code(), 0);
    keys
}
// port: LinkedIdentityHashSetTest#entries_keyedByIdentity
#[test]
fn entries_keyed_by_identity() {
    let [k0, k1] = validate_keys();
    let mut set = LinkedIdentityHashSet::default();
    assert!(!set.contains(&k0));
    assert!(!set.contains(&k1));
    set.add(k0.clone());
    assert!(set.contains(&k0));
    assert!(!set.contains(&k1));
    set.add(k1.clone());
    assert!(set.contains(&k0));
    assert!(set.contains(&k1));
}
// port: LinkedIdentityHashSetTest#add_returnsPrevioulyPresent
#[test]
fn add_returns_previouly_present() {
    let [k0, _] = validate_keys();
    let mut set = LinkedIdentityHashSet::default();
    assert!(!set.contains(&k0));
    assert!(set.add(k0.clone()));
    assert!(!set.add(k0));
}
// port: LinkedIdentityHashSetTest#remove_returnsPrevioulyPresent
#[test]
fn remove_returns_previouly_present() {
    let [k0, _] = validate_keys();
    let mut set = LinkedIdentityHashSet::default();
    assert!(!set.contains(&k0));
    set.add(k0.clone());
    assert!(set.contains(&k0));
    assert!(set.remove(k0.clone()));
    assert!(!set.remove(k0));
}
