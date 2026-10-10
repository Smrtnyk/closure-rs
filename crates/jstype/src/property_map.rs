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
 *   Nick Santos
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/jstype/PropertyMap.java.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use crate::{
    TypeId,
    function_type::FunctionType,
    js_type::JSType,
    js_type_registry::JSTypeRegistry,
    object_type::ObjectType,
    property::{OwnedProperty, Property, PropertyId, PropertyKey},
};
use closure_rhino::{check_state, fast_hash::IndexMap, js_string::JsString, node::Ast};

#[derive(Debug, Default)]
struct KeyCache {
    counter: i32,
    string_keys: Option<Arc<BTreeSet<JsString>>>,
    symbol_keys: Option<Arc<Vec<TypeId>>>,
}

/// Rust-only (D-025): Java's `TreeMap<String, Property>` of a property map, kept as a hash map
/// since lookups by name (by the string's cached hash) are far more frequent than walks in key
/// order. It offers no walk in any other order: `sorted_values` and `key_set` give Java's.
#[derive(Clone, Debug, Default)]
pub(crate) struct PropertyTable(IndexMap<JsString, PropertyId>);

impl PropertyTable {
    pub(crate) fn get(&self, name: &JsString) -> Option<PropertyId> {
        self.0.get(name).copied()
    }
    pub(crate) fn contains_key(&self, name: &JsString) -> bool {
        self.0.contains_key(name)
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn insert(&mut self, name: JsString, prop: PropertyId) {
        self.0.insert(name, prop);
    }
    /// The values in key order.
    fn sorted_values(&self) -> Vec<PropertyId> {
        let mut entries: Vec<(&JsString, PropertyId)> =
            self.0.iter().map(|(k, v)| (k, *v)).collect();
        entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
        entries.into_iter().map(|(_, v)| v).collect()
    }
    /// The keys, sorted.
    fn key_set(&self) -> BTreeSet<JsString> {
        self.0.keys().cloned().collect()
    }
    /// The sum of the keys' hash codes (Java `AbstractSet#hashCode`, order-free).
    fn key_hash_sum(&self) -> i32 {
        self.0
            .keys()
            .fold(0_i32, |hash, key| hash.wrapping_add(key.hash_code()))
    }
}

#[derive(Clone, Debug)]
pub struct PropertyMap {
    pub(crate) parent_source: Option<TypeId>,
    // Java's PropertyMap is a reference type; callers here clone it to release the registry
    // borrow. The maps are shared copy-on-write (Arc::make_mut in put_property_raw) so a clone
    // is O(1) instead of a deep copy; the snapshot semantics are unchanged.
    pub(crate) properties: Arc<PropertyTable>,
    pub(crate) known_symbols: Option<Arc<Vec<(TypeId, PropertyId)>>>,
    cache: Arc<Mutex<KeyCache>>,
    immutable_empty: bool,
}
impl Default for PropertyMap {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllKeys {
    pub string_keys: Arc<BTreeSet<JsString>>,
    pub known_symbol_keys: Arc<Vec<TypeId>>,
}
impl AllKeys {
    // port: PropertyMap#AllKeys
    pub fn new(string_keys: Arc<BTreeSet<JsString>>, known_symbol_keys: Arc<Vec<TypeId>>) -> Self {
        Self {
            string_keys,
            known_symbol_keys,
        }
    }
    pub fn string_keys(&self) -> Arc<BTreeSet<JsString>> {
        self.string_keys.clone()
    }
    pub fn known_symbol_keys(&self) -> Arc<Vec<TypeId>> {
        self.known_symbol_keys.clone()
    }
}

impl PropertyMap {
    // port: PropertyMap#PropertyMap
    pub fn new() -> Self {
        Self::from_maps(Arc::new(PropertyTable::default()), None, false)
    }
    // port: PropertyMap#PropertyMap
    fn from_maps(
        properties: Arc<PropertyTable>,
        known_symbols: Option<Arc<Vec<(TypeId, PropertyId)>>>,
        immutable_empty: bool,
    ) -> Self {
        Self {
            parent_source: None,
            properties,
            known_symbols,
            cache: Arc::new(Mutex::new(KeyCache::default())),
            immutable_empty,
        }
    }
    // port: PropertyMap#immutableEmptyMap
    pub fn immutable_empty_map() -> &'static Self {
        static EMPTY: std::sync::OnceLock<PropertyMap> = std::sync::OnceLock::new();
        EMPTY.get_or_init(|| {
            Self::from_maps(
                Arc::new(PropertyTable::default()),
                Some(Arc::new(Vec::new())),
                true,
            )
        })
    }
    // port: PropertyMap#setParentSource
    pub fn set_parent_source(&mut self, owner_type: TypeId) {
        if self.immutable_empty {
            return;
        }
        self.parent_source = Some(owner_type);
        self.increment_cached_key_set_counter();
    }
    // port: PropertyMap#setParentForTesting
    pub fn set_parent_for_testing(&mut self, parent: &PropertyMap) {
        self.parent_source = parent.parent_source;
        self.increment_cached_key_set_counter();
    }
    // port: PropertyMap#getPrimaryParent
    pub fn get_primary_parent(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<PropertyMap> {
        self.parent_source
            .and_then(|t| t.get_implicit_prototype(reg, ast))
            .map(|t| t.get_property_map(reg).clone())
    }
    #[allow(clippy::collapsible_if)] // Keep Java's constructor and abstract checks.
    // port: PropertyMap#getSecondaryParentObjects
    fn get_secondary_parent_objects(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> Vec<TypeId> {
        Self::secondary_parent_objects_of(self.parent_source, reg, ast)
    }
    // port: PropertyMap#getSecondaryParentObjects
    /// Rust-only: `getSecondaryParentObjects` of the map whose parent source is `parent_source`.
    #[allow(clippy::collapsible_if)] // Keep Java's constructor and abstract checks.
    fn secondary_parent_objects_of(
        parent_source: Option<TypeId>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> Vec<TypeId> {
        let Some(parent_source) = parent_source else {
            return Vec::new();
        };
        if let Some(ctor) = parent_source.get_constructor(reg) {
            if ctor.is_abstract(reg) {
                return ctor.get_own_implemented_interfaces(reg);
            }
        }
        parent_source.get_ctor_extended_interfaces(reg, ast)
    }
    // port: PropertyMap#findClosest
    pub fn find_closest(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
    ) -> Option<OwnedProperty> {
        let name = name.into();
        if let Some(prop) = self.get_own_property(reg, ast, &name) {
            return Some(OwnedProperty::new(self.parent_source, prop));
        }
        Self::find_closest_after_own(self.parent_source, reg, ast, &name)
    }
    /// Rust-only: `findClosest` on the property map of `t`, without copying that map.
    pub fn find_closest_of_type(
        t: TypeId,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &PropertyKey,
    ) -> Option<OwnedProperty> {
        let (prop, parent_source) = Self::own_property_of_type(t, reg, ast, name);
        if let Some(prop) = prop {
            return Some(OwnedProperty::new(parent_source, prop));
        }
        Self::find_closest_after_own(parent_source, reg, ast, name)
    }
    // port: PropertyMap#findClosest
    /// Rust-only: the rest of `findClosest` once the start map (whose parent source is
    /// `parent_source`) lacks the property. Java walks PropertyMap objects; the walk only needs
    /// each map's parent source and own lookup, so it reads the maps in place instead of copying
    /// each one (same calls in the same order, D-025).
    fn find_closest_after_own(
        parent_source: Option<TypeId>,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &PropertyKey,
    ) -> Option<OwnedProperty> {
        let mut source = parent_source;
        while let Some(next) = source.and_then(|t| t.get_implicit_prototype(reg, ast)) {
            let (prop, next_source) = Self::own_property_of_type(next, reg, ast, name);
            if let Some(prop) = prop {
                return Some(OwnedProperty::new(next_source, prop));
            }
            source = next_source;
        }
        let mut source = parent_source;
        loop {
            for o in Self::secondary_parent_objects_of(source, reg, ast) {
                let parent = o.get_property_map(reg).clone();
                if let Some(e) = parent.find_closest(reg, ast, name.clone()) {
                    return Some(e);
                }
            }
            match source.and_then(|t| t.get_implicit_prototype(reg, ast)) {
                Some(next) => source = next.get_property_map(reg).parent_source,
                None => return None,
            }
        }
    }
    /// Rust-only: `getOwnProperty` on the property map of `t` and that map's parent source.
    fn own_property_of_type(
        t: TypeId,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &PropertyKey,
    ) -> (Option<PropertyId>, Option<TypeId>) {
        let map = t.get_property_map(reg);
        let parent_source = map.parent_source;
        match name {
            PropertyKey::String(n) => (map.properties.get(n), parent_source),
            PropertyKey::Symbol(_) => {
                let map = map.clone();
                (map.get_own_property(reg, ast, name), parent_source)
            }
        }
    }
    // port: PropertyMap#getOwnProperty
    pub fn get_own_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: &PropertyKey,
    ) -> Option<PropertyId> {
        match name {
            PropertyKey::String(n) => self.properties.get(n),
            PropertyKey::Symbol(s) => self.known_symbols.as_ref().and_then(|items| {
                items
                    .iter()
                    .find(|(key, _)| {
                        key.hash_code(reg) == s.hash_code(reg) && key.equals(reg, ast, *s)
                    })
                    .map(|(_, p)| *p)
            }),
        }
    }
    // port: PropertyMap#getPropertiesCount
    pub fn get_properties_count(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> usize {
        if self.get_primary_parent(reg, ast).is_none() {
            self.properties.len()
        } else {
            self.get_all_keys(reg, ast).string_keys.len()
        }
    }
    // port: PropertyMap#getOwnPropertyNames
    pub fn get_own_property_names(&self) -> BTreeSet<JsString> {
        self.properties.key_set()
    }
    // port: PropertyMap#getOwnKnownSymbols
    pub fn get_own_known_symbols(&self) -> Vec<TypeId> {
        self.known_symbols
            .as_ref()
            .map_or_else(Vec::new, |symbols| {
                symbols.iter().map(|(s, _)| *s).collect()
            })
    }
    // port: PropertyMap#getAllKeys
    pub fn get_all_keys(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> AllKeys {
        let mut ancestors = Vec::new();
        self.collect_all_ancestors(reg, ast, &mut ancestors);
        let max_ancestor_counter = ancestors
            .iter()
            .map(|m| m.cache.lock().unwrap().counter)
            .max()
            .unwrap_or(0);
        let invalid = {
            let cache = self.cache.lock().unwrap();
            max_ancestor_counter != cache.counter || cache.string_keys.is_none()
        };
        if invalid {
            let mut keys = BTreeSet::new();
            let mut known_symbol_keys = Vec::new();
            for ancestor in &ancestors {
                {
                    let mut cache = ancestor.cache.lock().unwrap();
                    cache.counter = max_ancestor_counter;
                    cache.string_keys = None;
                    cache.symbol_keys = None;
                }
                keys.extend(ancestor.get_own_property_names());
                for symbol in ancestor.get_own_known_symbols() {
                    if !known_symbol_keys.iter().any(|&s: &TypeId| {
                        s.hash_code(reg) == symbol.hash_code(reg) && s.equals(reg, ast, symbol)
                    }) {
                        known_symbol_keys.push(symbol);
                    }
                }
            }
            let mut cache = self.cache.lock().unwrap();
            cache.string_keys = Some(Arc::new(keys));
            cache.symbol_keys = Some(Arc::new(known_symbol_keys));
        }
        let cache = self.cache.lock().unwrap();
        AllKeys::new(
            cache.string_keys.clone().unwrap(),
            cache.symbol_keys.clone().unwrap(),
        )
    }
    // port: PropertyMap#collectAllAncestors
    fn collect_all_ancestors(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        ancestors: &mut Vec<PropertyMap>,
    ) {
        if ancestors.iter().any(|m| Arc::ptr_eq(&m.cache, &self.cache)) {
            return;
        }
        ancestors.push(self.clone());
        if let Some(primary_parent) = self.get_primary_parent(reg, ast) {
            primary_parent.collect_all_ancestors(reg, ast, ancestors);
        }
        for parent_type in self.get_secondary_parent_objects(reg, ast) {
            let parent_map = parent_type.get_property_map(reg).clone();
            parent_map.collect_all_ancestors(reg, ast, ancestors);
        }
    }
    // port: PropertyMap#putProperty
    pub fn put_property(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<PropertyKey>,
        new_prop: PropertyId,
    ) {
        let name = name.into();
        let old_prop = self.get_own_property(reg, ast, &name);
        if let (PropertyKey::String(_), Some(old_prop)) = (&name, old_prop) {
            let info = old_prop.get_jsdoc_info(reg);
            new_prop.set_jsdoc_info(reg, info);
        }
        self.put_property_raw(reg, ast, name, new_prop);
    }
    pub(crate) fn put_property_raw(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: PropertyKey,
        new_prop: PropertyId,
    ) {
        check_state!(!self.immutable_empty);
        match name {
            PropertyKey::String(n) => {
                if !self.properties.contains_key(&n) {
                    self.increment_cached_key_set_counter();
                }
                Arc::make_mut(&mut self.properties).insert(n, new_prop);
            }
            PropertyKey::Symbol(symbol) => {
                let found = self.known_symbols.as_ref().and_then(|items| {
                    items.iter().position(|(key, _)| {
                        key.hash_code(reg) == symbol.hash_code(reg) && key.equals(reg, ast, symbol)
                    })
                });
                if found.is_none() {
                    self.increment_cached_key_set_counter();
                }
                let symbols = Arc::make_mut(
                    self.known_symbols
                        .get_or_insert_with(|| Arc::new(Vec::new())),
                );
                if let Some(index) = found {
                    symbols[index].1 = new_prop;
                } else {
                    symbols.push((symbol, new_prop));
                }
            }
        }
    }
    // port: PropertyMap#values
    pub fn values(&self) -> Vec<PropertyId> {
        self.properties.sorted_values()
    }
    // port: PropertyMap#hashCode
    pub fn hash_code(&self, reg: &JSTypeRegistry) -> i32 {
        let strings = self.properties.key_hash_sum();
        match &self.known_symbols {
            None => strings,
            Some(symbols) => {
                let symbol_hash = symbols.iter().fold(0_i32, |hash, (key, _)| {
                    hash.wrapping_add(key.hash_code(reg))
                });
                31_i32
                    .wrapping_add(strings)
                    .wrapping_mul(31)
                    .wrapping_add(symbol_hash)
            }
        }
    }
    // port: PropertyMap#incrementCachedKeySetCounter
    fn increment_cached_key_set_counter(&mut self) {
        let mut cache = self.cache.lock().unwrap();
        cache.counter = cache.counter.wrapping_add(1);
        cache.string_keys = None;
        check_state!(cache.counter >= 0);
    }
}
