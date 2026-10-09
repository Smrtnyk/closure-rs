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
//   src/com/google/javascript/jscomp/base/LinkedIdentityHashSet.java.

use super::linked_identity_hash_map::{JavaIdentity, LinkedIdentityHashMap};
struct Sentinel;
pub struct LinkedIdentityHashSet<E: JavaIdentity> {
    inner_map: LinkedIdentityHashMap<E, Sentinel>,
}
impl<E: JavaIdentity> Default for LinkedIdentityHashSet<E> {
    // port: LinkedIdentityHashSet#LinkedIdentityHashSet
    fn default() -> Self {
        Self {
            inner_map: LinkedIdentityHashMap::default(),
        }
    }
}
impl<E: JavaIdentity> LinkedIdentityHashSet<E> {
    // port: LinkedIdentityHashSet#contains
    pub fn contains(&self, element: &E) -> bool {
        self.inner_map.get(element).is_some()
    }
    // port: LinkedIdentityHashSet#add
    pub fn add(&mut self, element: E) -> bool {
        self.inner_map.put(element, Some(Sentinel)).is_none()
    }
    // port: LinkedIdentityHashSet#remove
    pub fn remove(&mut self, element: E) -> bool {
        self.inner_map.put(element, None).is_some()
    }
}
