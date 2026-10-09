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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/base/LinkedIdentityHashMap.java.

use crate::fast_hash::IndexMap;
use crate::{js_string::JsString, node::NodeId};
use std::hash::Hash;
use std::sync::Arc;

/// Java `==` keys. Arena handles implement this with their handle value; heap
/// objects use their allocation address, even when their contents compare equal.
pub trait JavaIdentity: Clone {
    type Identity: Eq + Hash;
    fn java_identity(&self) -> Self::Identity;
}
impl<T: ?Sized> JavaIdentity for Arc<T> {
    type Identity = usize;
    fn java_identity(&self) -> usize {
        Arc::as_ptr(self).cast::<()>() as usize
    }
}
impl JavaIdentity for JsString {
    type Identity = usize;
    fn java_identity(&self) -> usize {
        self.as_units().as_ptr() as usize
    }
}
impl JavaIdentity for NodeId {
    type Identity = Self;
    fn java_identity(&self) -> Self {
        *self
    }
}
impl JavaIdentity for crate::jstype::TypeId {
    type Identity = Self;
    fn java_identity(&self) -> Self {
        *self
    }
}
impl<K: JavaIdentity> JavaIdentity for Option<K> {
    type Identity = Option<K::Identity>;
    fn java_identity(&self) -> Self::Identity {
        self.as_ref().map(JavaIdentity::java_identity)
    }
}

pub struct LinkedIdentityHashMap<K: JavaIdentity, V> {
    inner_map: IndexMap<K::Identity, Entry<K, V>>,
}
struct Entry<K, V> {
    key: K,
    value: Option<V>,
}
impl<K, V> Entry<K, V> {
    // port: LinkedIdentityHashMap.Entry#Entry
    fn new(key: K, value: Option<V>) -> Self {
        Self { key, value }
    }
}
impl<K: JavaIdentity, V> Default for LinkedIdentityHashMap<K, V> {
    // port: LinkedIdentityHashMap#LinkedIdentityHashMap
    fn default() -> Self {
        Self {
            inner_map: IndexMap::<_, _>::default(),
        }
    }
}
impl<K: JavaIdentity, V> LinkedIdentityHashMap<K, V> {
    // port: LinkedIdentityHashMap#get
    pub fn get(&self, key: &K) -> Option<&V> {
        self.inner_map
            .get(&key.java_identity())
            .and_then(|entry| entry.value.as_ref())
    }
    // port: LinkedIdentityHashMap#put
    pub fn put(&mut self, key: K, value: Option<V>) -> Option<V> {
        match self.inner_map.entry(key.java_identity()) {
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(Entry::new(key, value));
                None
            }
            indexmap::map::Entry::Occupied(mut entry) => {
                std::mem::replace(&mut entry.get_mut().value, value)
            }
        }
    }
    // port: LinkedIdentityHashMap#computeIfAbsent
    pub fn compute_if_absent(&mut self, key: K, f: impl FnOnce(&K) -> Option<V>) -> Option<&V> {
        if self.get(&key).is_some() {
            return self.get(&key);
        }
        let new_value = f(&key);
        self.put(key.clone(), new_value);
        self.get(&key)
    }
    // port: LinkedIdentityHashMap#merge
    pub fn merge(
        &mut self,
        key: K,
        value: Option<V>,
        f: impl FnOnce(&V, Option<&V>) -> Option<V>,
    ) -> Option<&V> {
        let merged = match self.get(&key) {
            None => value,
            Some(old) => f(old, value.as_ref()),
        };
        self.put(key.clone(), merged);
        self.get(&key)
    }
    // port: LinkedIdentityHashMap#forEach
    pub fn for_each(&self, mut f: impl FnMut(&K, Option<&V>)) {
        for entry in self.inner_map.values() {
            f(&entry.key, entry.value.as_ref());
        }
    }
}
