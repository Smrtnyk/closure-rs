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

//! Rust-only, not in Java (DECISIONS.md D-025): the names declared in a scope, in declaration
//! order (Java: `AbstractScope#vars`, a `LinkedHashMap`).
//!
//! Most scopes declare one or two names. Those are kept in the scope itself and found by
//! comparing the strings' cached hashes (an interned name then compares by identity), so a name
//! lookup that walks the scope chain reads no hash table for them; a scope that declares more
//! moves its names to an insertion-ordered hash map.

use closure_rhino::{fast_hash::IndexMap, js_string::JsString};

/// The most names kept in the scope itself.
const INLINE: usize = 2;

#[derive(Debug, Clone)]
pub(crate) enum VarMap<V> {
    /// The first `n` entries are `Some`, in declaration order.
    Inline([Option<(JsString, V)>; INLINE]),
    Map(IndexMap<JsString, V>),
}

impl<V> Default for VarMap<V> {
    fn default() -> Self {
        Self::Inline(std::array::from_fn(|_| None))
    }
}

impl<V> VarMap<V> {
    fn inline_entries(
        entries: &[Option<(JsString, V)>; INLINE],
    ) -> impl Iterator<Item = &(JsString, V)> {
        entries.iter().map_while(Option::as_ref)
    }

    pub fn get(&self, name: &JsString) -> Option<&V> {
        match self {
            Self::Inline(entries) => Self::inline_entries(entries)
                .find(|(key, _)| key == name)
                .map(|(_, value)| value),
            Self::Map(map) => map.get(name),
        }
    }

    pub fn contains_key(&self, name: &JsString) -> bool {
        self.get(name).is_some()
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Inline(entries) => Self::inline_entries(entries).count(),
            Self::Map(map) => map.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Inline(entries) => entries[0].is_none(),
            Self::Map(map) => map.is_empty(),
        }
    }

    /// `LinkedHashMap#put`: a new name goes last, a known one keeps its place.
    pub fn insert(&mut self, name: JsString, value: V) -> Option<V> {
        match self {
            Self::Inline(entries) => {
                for entry in entries.iter_mut() {
                    match entry {
                        Some((key, old)) if *key == name => {
                            return Some(std::mem::replace(old, value));
                        }
                        Some(_) => {}
                        None => {
                            *entry = Some((name, value));
                            return None;
                        }
                    }
                }
                let mut map = IndexMap::with_capacity_and_hasher(INLINE * 2, Default::default());
                for entry in entries.iter_mut() {
                    let (key, old) = entry.take().unwrap();
                    map.insert(key, old);
                }
                map.insert(name, value);
                *self = Self::Map(map);
                None
            }
            Self::Map(map) => map.insert(name, value),
        }
    }

    /// `LinkedHashMap#remove`: the names after it keep their order.
    pub fn shift_remove(&mut self, name: &JsString) -> Option<V> {
        match self {
            Self::Inline(entries) => {
                let index = Self::inline_entries(entries).position(|(key, _)| key == name)?;
                let (_, value) = entries[index].take().unwrap();
                entries[index..].rotate_left(1);
                Some(value)
            }
            Self::Map(map) => map.shift_remove(name),
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&JsString, &V)> {
        let (inline, map) = match self {
            Self::Inline(entries) => (Some(Self::inline_entries(entries)), None),
            Self::Map(map) => (None, Some(map.iter())),
        };
        inline
            .into_iter()
            .flatten()
            .map(|(key, value)| (key, value))
            .chain(map.into_iter().flatten())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use super::VarMap;
    use closure_rhino::js_string::JsString;

    fn names(map: &VarMap<i32>) -> Vec<(String, i32)> {
        map.iter().map(|(k, v)| (k.to_string_lossy(), *v)).collect()
    }

    #[test]
    fn keeps_declaration_order_inline_and_spilled() {
        let mut map = VarMap::default();
        assert!(map.is_empty());
        assert_eq!(map.insert(JsString::from("a"), 1), None);
        assert_eq!(map.insert(JsString::from("b"), 2), None);
        assert!(matches!(map, VarMap::Inline(_)));
        assert_eq!(map.insert(JsString::from("a"), 3), Some(1));
        assert_eq!(names(&map), [("a".into(), 3), ("b".into(), 2)]);
        assert_eq!(map.shift_remove(&JsString::from("a")), Some(3));
        assert_eq!(names(&map), [("b".into(), 2)]);
        assert_eq!(map.insert(JsString::from("c"), 4), None);
        assert_eq!(map.insert(JsString::from("d"), 5), None);
        assert!(matches!(map, VarMap::Map(_)));
        assert_eq!(
            names(&map),
            [("b".into(), 2), ("c".into(), 4), ("d".into(), 5)]
        );
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&JsString::from("c")), Some(&4));
        assert!(!map.contains_key(&JsString::from("a")));
        assert_eq!(map.shift_remove(&JsString::from("c")), Some(4));
        assert_eq!(names(&map), [("b".into(), 2), ("d".into(), 5)]);
        assert_eq!(map.values().copied().collect::<Vec<_>>(), [2, 5]);
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
        assert_eq!(map.shift_remove(&JsString::from("b")), None);
    }
}
