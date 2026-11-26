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

//! Rust-only (performance divergence): a byte encoding of the type registry's state, used by
//! closure-jscomp to restore the typing of the bundled externs from data prepared when closure-rs
//! is built (`crates/jscomp/src/typed_externs.rs`), and to compare that data with the state an
//! ordinary compile computes. Java has no counterpart.
//!
//! The encoding is complete: every field of the registry and of every type and property is
//! written, by exhaustive destructuring, so that a new field fails to compile here until it is
//! encoded. The registry fields that are inputs of a compile rather than results of typing
//! (`scope_storage`, `reporter`, `closure_namespaces`, `name_table_global_root`,
//! `non_nullable_type_names`, `forward_declared_types`) are not written; the caller guards them.
//! Shared values (`Arc`s) are written once and referred to afterwards, so a restore reproduces
//! which types share a template map, a property table or a scope.
//!
//! Nodes, JSDoc, scopes of the compiler and the compiler's validators are written by a
//! `SnapshotEnv`, which relocates node ids and knows the compiler's objects.

use super::{ClosureNamespace, JSTypeRegistry, SyntheticTemplateScope};
use crate::{
    TypeId,
    all_type::AllTypeData,
    arrow_type::ArrowTypeData,
    big_int_type::BigIntTypeData,
    boolean_type::BooleanTypeData,
    enum_element_type::EnumElementTypeData,
    enum_type::EnumTypeData,
    function_type::{ConstructorAmbiguity, FunctionTypeData, Kind, Parameter, PropAccess},
    instance_object_type::InstanceObjectTypeData,
    js_type::{JSTypeData, JSTypeKind, TypeValidator},
    js_type_resolver::{JSTypeResolver, State},
    known_symbol_type::KnownSymbolTypeData,
    named_type::{NamedTypeData, PropertyContinuation, ResolutionKind},
    no_object_type::NoObjectTypeData,
    no_resolved_type::NoResolvedTypeData,
    no_type::NoTypeData,
    null_type::NullTypeData,
    number_type::NumberTypeData,
    object_type::ObjectTypeData,
    property::{PropertyAccessAnnotations, PropertyData, PropertyId, PropertyKey},
    property_map::{KeyCache, PropertyMap, PropertyTable},
    prototype_object_type::PrototypeObjectTypeData,
    proxy_object_type::ProxyObjectTypeData,
    record_type::RecordTypeData,
    static_typed_scope::StaticTypedScope,
    string_type::StringTypeData,
    symbol_type::SymbolTypeData,
    template_type::TemplateTypeData,
    template_type_map::TemplateTypeMap,
    template_type_replacer::TemplateTypeReplacer,
    templatized_type::TemplatizedTypeData,
    union_type::UnionTypeData,
    unknown_type::UnknownTypeData,
    value_type::ValueTypeData,
    void_type::VoidTypeData,
};
use closure_rhino::{
    closure_primitive::ClosurePrimitive,
    fast_hash::{IndexMap, IndexSet},
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    rhino_string_pool::RhinoStringPool,
};
use std::{
    cell::Cell,
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex},
};

/// Why a state cannot be written: it holds something the encoding does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotError(pub String);

pub type SnapshotResult<T> = Result<T, SnapshotError>;

fn unsupported<T>(what: impl Into<String>) -> SnapshotResult<T> {
    Err(SnapshotError(what.into()))
}

/// The kinds of shared values, each with its own table of back references.
#[derive(Clone, Copy)]
enum Shared {
    TemplateMap,
    Alternates,
    PropertyTable,
    KnownSymbols,
    KeyCache,
    KeySet,
    SymbolKeys,
    Scope,
    Validator,
}
const SHARED_KINDS: usize = 9;

/// Writes a snapshot: LEB128 numbers, strings as indexes into a table written in front of the
/// body, shared values once.
pub struct Writer {
    body: Vec<u8>,
    strings: IndexSet<JsString>,
    rust_strings: IndexSet<String>,
    shared: IndexMap<(u8, usize), u32>,
    shared_counts: [u32; SHARED_KINDS],
    labels: Option<Vec<(usize, String)>>,
    /// Type ids and property ids are written as the difference to the last one written.
    last_type: u32,
    last_prop: u32,
    /// The last Java hash written for each type (`indexed_by_property` entries repeat them).
    type_hashes: IndexMap<TypeId, i32>,
    /// The synthetic externs' entries to leave out (see `SyntheticEntries`), and whether a left
    /// out property was referred to elsewhere.
    synthetic: Option<SyntheticNorm>,
    bad_reference: bool,
}

/// What `Writer` leaves out and renumbers for `SyntheticEntries`.
#[derive(Clone)]
struct SyntheticNorm {
    props_start: u32,
    props_count: u32,
    names: IndexSet<JsString>,
    global_this: TypeId,
}

impl SyntheticNorm {
    fn is_synthetic_entry(&self, key: &JsString, t: TypeId) -> bool {
        t == self.global_this && self.names.contains(key)
    }

    fn is_synthetic_prop(&self, p: PropertyId) -> bool {
        (self.props_start..self.props_start + self.props_count).contains(&p.0)
    }
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            strings: IndexSet::default(),
            rust_strings: IndexSet::default(),
            shared: IndexMap::default(),
            shared_counts: [0; SHARED_KINDS],
            labels: None,
            last_type: 0,
            last_prop: 0,
            type_hashes: IndexMap::default(),
            synthetic: None,
            bad_reference: false,
        }
    }

    /// A writer that also remembers where each labelled part starts (for reports of a difference).
    pub fn with_labels() -> Self {
        let mut w = Self::new();
        w.labels = Some(Vec::new());
        w
    }

    /// Marks the start of a part, if this writer keeps labels.
    #[inline]
    pub fn label(&mut self, label: impl FnOnce() -> String) {
        if let Some(labels) = &mut self.labels {
            labels.push((self.body.len(), label()));
        }
    }

    /// The label of the part that holds body offset `offset`.
    pub fn label_at(&self, offset: usize) -> Option<&str> {
        let labels = self.labels.as_ref()?;
        let i = labels.partition_point(|(start, _)| *start <= offset);
        (i > 0).then(|| labels[i - 1].1.as_str())
    }

    pub fn body_len(&self) -> usize {
        self.body.len()
    }

    #[inline]
    pub fn u(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.body.push(byte);
                return;
            }
            self.body.push(byte | 0x80);
        }
    }

    #[inline]
    pub fn i(&mut self, v: i64) {
        self.u(((v << 1) ^ (v >> 63)) as u64);
    }

    #[inline]
    pub fn b(&mut self, v: bool) {
        self.body.push(u8::from(v));
    }

    #[inline]
    pub fn js(&mut self, s: &JsString) {
        let (index, _) = self.strings.insert_full(s.clone());
        self.u(index as u64);
    }

    pub fn opt_js(&mut self, s: Option<&JsString>) {
        match s {
            None => self.u(0),
            Some(s) => {
                let (index, _) = self.strings.insert_full(s.clone());
                self.u(index as u64 + 1);
            }
        }
    }

    pub fn rs(&mut self, s: &str) {
        let index = match self.rust_strings.get_index_of(s) {
            Some(index) => index,
            None => self.rust_strings.insert_full(s.to_owned()).0,
        };
        self.u(index as u64);
    }

    /// Writes several flags as one number (the first flag is the lowest bit).
    #[inline]
    pub fn flags(&mut self, flags: &[bool]) {
        let mut v = 0u64;
        for (i, &flag) in flags.iter().enumerate() {
            v |= u64::from(flag) << i;
        }
        self.u(v);
    }

    /// Makes `t` the base of the next type id (the id of the type being written).
    #[inline]
    pub fn type_base(&mut self, t: u32) {
        self.last_type = t;
    }

    #[inline]
    pub fn ty(&mut self, t: TypeId) {
        self.i(i64::from(t.0) - i64::from(self.last_type));
        self.last_type = t.0;
    }

    #[inline]
    pub fn oty(&mut self, t: Option<TypeId>) {
        match t {
            None => self.u(0),
            Some(t) => {
                let delta = i64::from(t.0) - i64::from(self.last_type);
                self.u((((delta << 1) ^ (delta >> 63)) as u64) + 1);
                self.last_type = t.0;
            }
        }
    }

    pub fn tys(&mut self, types: &[TypeId]) {
        self.u(types.len() as u64);
        for &t in types {
            self.ty(t);
        }
    }

    #[inline]
    pub fn prop(&mut self, p: PropertyId) {
        let mut id = p.0;
        if let Some(norm) = &self.synthetic
            && id >= norm.props_start
        {
            if id < norm.props_start + norm.props_count {
                self.bad_reference = true;
            }
            id -= norm.props_count;
        }
        self.i(i64::from(id) - i64::from(self.last_prop));
        self.last_prop = id;
    }

    /// A Java hash of `t`: by reference to the last one written for `t` when it is the same.
    fn type_hash(&mut self, t: TypeId, hash: i32) {
        if self.type_hashes.get(&t) == Some(&hash) {
            self.u(0);
        } else {
            self.type_hashes.insert(t, hash);
            self.u(1);
            self.i(i64::from(hash));
        }
    }

    /// Writes a reference to a shared value seen before (and returns false), or marks a new one
    /// whose content the caller writes next, followed by `shared_end` (and returns true).
    fn shared_begin(&mut self, kind: Shared, ptr: usize) -> bool {
        if let Some(&index) = self.shared.get(&(kind as u8, ptr)) {
            self.u(u64::from(index) + 1);
            false
        } else {
            self.u(0);
            true
        }
    }

    /// Numbers a new shared value after its content (so values nested in it come first).
    fn shared_end(&mut self, kind: Shared, ptr: usize) {
        let count = &mut self.shared_counts[kind as usize];
        self.shared.insert((kind as u8, ptr), *count);
        *count += 1;
    }

    /// The snapshot: the string tables, then the body.
    pub fn finish(self) -> Vec<u8> {
        self.bytes()
    }

    /// `finish` without consuming the writer (its labels stay readable).
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes_with(&mut |_| None)
    }

    /// `bytes`, writing each string that `node_string` finds held by a node (as a pair of
    /// numbers the reader's resolver turns back into the node's string) by reference instead of
    /// by content: reading it then costs no interning.
    pub fn bytes_with(
        &self,
        node_string: &mut dyn FnMut(&JsString) -> Option<(u64, u64)>,
    ) -> Vec<u8> {
        let mut out = Writer::new();
        out.u(self.strings.len() as u64);
        for s in &self.strings {
            match node_string(s) {
                Some((a, b)) => {
                    out.u(0);
                    out.u(a);
                    out.u(b);
                }
                None => {
                    let units = s.as_units();
                    out.u(units.len() as u64 + 1);
                    for &unit in units {
                        out.u(u64::from(unit));
                    }
                }
            }
        }
        out.u(self.rust_strings.len() as u64);
        for s in &self.rust_strings {
            out.u(s.len() as u64);
            out.body.extend_from_slice(s.as_bytes());
        }
        let header = out.body.len();
        let mut bytes = out.body;
        bytes.reserve(self.body.len());
        bytes.extend_from_slice(&self.body);
        debug_assert!(header <= bytes.len());
        bytes
    }

    /// The length of the string tables `bytes` writes in front of the body (strings by content).
    pub fn header_len(&self) -> usize {
        self.bytes().len() - self.body.len()
    }
}

/// Reads a snapshot `Writer::finish` wrote.
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    strings: Vec<JsString>,
    rust_strings: Vec<Arc<str>>,
    template_maps: Vec<Arc<TemplateTypeMap>>,
    alternates: Vec<Arc<Vec<TypeId>>>,
    property_tables: Vec<Arc<IndexMap<JsString, PropertyId>>>,
    known_symbols: Vec<Arc<Vec<(TypeId, PropertyId)>>>,
    key_caches: Vec<Arc<Mutex<KeyCache>>>,
    key_sets: Vec<Arc<BTreeSet<JsString>>>,
    symbol_keys: Vec<Arc<Vec<TypeId>>>,
    scopes: Vec<Arc<dyn StaticTypedScope>>,
    validators: Vec<TypeValidator>,
    last_type: u32,
    last_prop: u32,
    type_hashes: IndexMap<TypeId, i32>,
    /// Restoring with synthetic externs: property ids from here on are moved up by this much.
    prop_shift: Option<(u32, u32)>,
}

impl<'a> Reader<'a> {
    /// Reads the string tables (interning each string once) and stops at the body.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self::new_with(bytes, &mut |_, _| {
            panic!("a node string without a resolver")
        })
    }

    /// `new`, resolving strings written by reference (`Writer::bytes_with`) with `node_string`.
    pub fn new_with(bytes: &'a [u8], node_string: &mut dyn FnMut(u64, u64) -> JsString) -> Self {
        let mut r = Self {
            bytes,
            pos: 0,
            strings: Vec::new(),
            rust_strings: Vec::new(),
            template_maps: Vec::new(),
            alternates: Vec::new(),
            property_tables: Vec::new(),
            known_symbols: Vec::new(),
            key_caches: Vec::new(),
            key_sets: Vec::new(),
            symbol_keys: Vec::new(),
            scopes: Vec::new(),
            validators: Vec::new(),
            last_type: 0,
            last_prop: 0,
            type_hashes: IndexMap::default(),
            prop_shift: None,
        };
        let count = r.u() as usize;
        r.strings.reserve(count);
        let mut units = Vec::new();
        for _ in 0..count {
            let len = r.u() as usize;
            if len == 0 {
                let a = r.u();
                let b = r.u();
                r.strings.push(node_string(a, b));
                continue;
            }
            units.clear();
            for _ in 0..len - 1 {
                units.push(r.u() as u16);
            }
            r.strings
                .push(RhinoStringPool::add_or_get(JsString::from_slice(&units)));
        }
        let count = r.u() as usize;
        for _ in 0..count {
            let len = r.u() as usize;
            let s = std::str::from_utf8(&r.bytes[r.pos..r.pos + len]).expect("snapshot string");
            r.rust_strings.push(Arc::from(s));
            r.pos += len;
        }
        r
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.bytes.len()
    }

    #[inline]
    pub fn u(&mut self) -> u64 {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.bytes[self.pos];
            self.pos += 1;
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return v;
            }
            shift += 7;
        }
    }

    #[inline]
    pub fn i(&mut self) -> i64 {
        let v = self.u();
        ((v >> 1) as i64) ^ -((v & 1) as i64)
    }

    #[inline]
    pub fn b(&mut self) -> bool {
        let v = self.bytes[self.pos];
        self.pos += 1;
        v != 0
    }

    #[inline]
    pub fn js(&mut self) -> JsString {
        let index = self.u() as usize;
        self.strings[index].clone()
    }

    pub fn opt_js(&mut self) -> Option<JsString> {
        match self.u() as usize {
            0 => None,
            index => Some(self.strings[index - 1].clone()),
        }
    }

    pub fn rs(&mut self) -> Arc<str> {
        let index = self.u() as usize;
        self.rust_strings[index].clone()
    }

    #[inline]
    pub fn flags(&mut self) -> u64 {
        self.u()
    }

    #[inline]
    pub fn type_base(&mut self, t: u32) {
        self.last_type = t;
    }

    #[inline]
    pub fn ty(&mut self) -> TypeId {
        let t = (i64::from(self.last_type) + self.i()) as u32;
        self.last_type = t;
        TypeId(t)
    }

    #[inline]
    pub fn oty(&mut self) -> Option<TypeId> {
        match self.u() {
            0 => None,
            v => {
                let v = v - 1;
                let delta = ((v >> 1) as i64) ^ -((v & 1) as i64);
                let t = (i64::from(self.last_type) + delta) as u32;
                self.last_type = t;
                Some(TypeId(t))
            }
        }
    }

    pub fn tys(&mut self) -> Vec<TypeId> {
        let len = self.u() as usize;
        (0..len).map(|_| self.ty()).collect()
    }

    #[inline]
    pub fn prop(&mut self) -> PropertyId {
        let p = (i64::from(self.last_prop) + self.i()) as u32;
        self.last_prop = p;
        match self.prop_shift {
            Some((start, count)) if p >= start => PropertyId(p + count),
            _ => PropertyId(p),
        }
    }

    fn type_hash(&mut self, t: TypeId) -> i32 {
        if self.u() == 0 {
            self.type_hashes[&t]
        } else {
            let hash = self.i() as i32;
            self.type_hashes.insert(t, hash);
            hash
        }
    }
}

/// Writes and reads what the registry cannot: the compiler's nodes (relocated), its JSDoc, its
/// scopes and its validators.
pub trait SnapshotEnv {
    fn write_node(&mut self, w: &mut Writer, n: NodeId) -> SnapshotResult<()>;
    fn read_node(&mut self, r: &mut Reader<'_>) -> NodeId;
    fn write_jsdoc(&mut self, w: &mut Writer, doc: &Arc<JSDocInfo>) -> SnapshotResult<()>;
    fn read_jsdoc(&mut self, r: &mut Reader<'_>, ast: &Ast) -> Arc<JSDocInfo>;
    /// A scope that is not a `SyntheticTemplateScope`.
    fn write_scope(
        &mut self,
        w: &mut Writer,
        scope: &Arc<dyn StaticTypedScope>,
    ) -> SnapshotResult<()>;
    fn read_scope(&mut self, r: &mut Reader<'_>) -> Arc<dyn StaticTypedScope>;
    fn write_validator(&mut self, w: &mut Writer, validator: &TypeValidator) -> SnapshotResult<()>;
    fn read_validator(&mut self, r: &mut Reader<'_>) -> TypeValidator;
}

fn write_opt_node(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    n: Option<NodeId>,
) -> SnapshotResult<()> {
    match n {
        None => {
            w.b(false);
            Ok(())
        }
        Some(n) => {
            w.b(true);
            env.write_node(w, n)
        }
    }
}

fn read_opt_node(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv) -> Option<NodeId> {
    r.b().then(|| env.read_node(r))
}

fn ptr<T: ?Sized>(arc: &Arc<T>) -> usize {
    Arc::as_ptr(arc) as *const () as usize
}

fn write_template_map(w: &mut Writer, map: &Arc<TemplateTypeMap>) {
    if w.shared_begin(Shared::TemplateMap, ptr(map)) {
        let TemplateTypeMap {
            template_keys,
            template_values,
            resolved_template_values,
            sub_map_starts,
        } = &**map;
        w.tys(template_keys);
        w.tys(template_values);
        w.tys(resolved_template_values);
        w.u(sub_map_starts.len() as u64);
        for &start in sub_map_starts {
            w.b(start);
        }
        w.shared_end(Shared::TemplateMap, ptr(map));
    }
}

fn read_template_map(r: &mut Reader<'_>) -> Arc<TemplateTypeMap> {
    match r.u() as usize {
        0 => {
            let template_keys = r.tys();
            let template_values = r.tys();
            let resolved_template_values = r.tys();
            let len = r.u() as usize;
            let sub_map_starts = (0..len).map(|_| r.b()).collect();
            let map = Arc::new(TemplateTypeMap {
                template_keys,
                template_values,
                resolved_template_values,
                sub_map_starts,
            });
            r.template_maps.push(map.clone());
            map
        }
        index => r.template_maps[index - 1].clone(),
    }
}

fn write_scope_arc(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    scope: &Arc<dyn StaticTypedScope>,
) -> SnapshotResult<()> {
    if w.shared_begin(Shared::Scope, ptr(scope)) {
        if let Some((delegate, types)) = scope.snapshot_template_scope() {
            w.b(true);
            write_scope_arc(w, env, delegate)?;
            w.u(types.len() as u64);
            for (name, &t) in types {
                w.js(name);
                w.ty(t);
            }
        } else {
            w.b(false);
            env.write_scope(w, scope)?;
        }
        w.shared_end(Shared::Scope, ptr(scope));
    }
    Ok(())
}

fn read_scope_arc(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv) -> Arc<dyn StaticTypedScope> {
    match r.u() as usize {
        0 => {
            let scope: Arc<dyn StaticTypedScope> = if r.b() {
                let delegate = read_scope_arc(r, env);
                let len = r.u() as usize;
                let mut types = IndexMap::with_capacity_and_hasher(len, Default::default());
                for _ in 0..len {
                    let name = r.js();
                    types.insert(name, r.ty());
                }
                Arc::new(SyntheticTemplateScope { delegate, types })
            } else {
                env.read_scope(r)
            };
            r.scopes.push(scope.clone());
            scope
        }
        index => r.scopes[index - 1].clone(),
    }
}

fn write_validator_arc(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    validator: &TypeValidator,
) -> SnapshotResult<()> {
    if w.shared_begin(Shared::Validator, ptr(validator)) {
        env.write_validator(w, validator)?;
        w.shared_end(Shared::Validator, ptr(validator));
    }
    Ok(())
}

fn read_validator_arc(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv) -> TypeValidator {
    match r.u() as usize {
        0 => {
            let validator = env.read_validator(r);
            r.validators.push(validator.clone());
            validator
        }
        index => r.validators[index - 1].clone(),
    }
}

fn write_object(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    o: &ObjectTypeData,
) -> SnapshotResult<()> {
    let ObjectTypeData {
        visited,
        doc_info,
        unknown,
    } = o;
    w.flags(&[*visited, doc_info.is_some(), *unknown]);
    if let Some(doc) = doc_info {
        env.write_jsdoc(w, doc)?;
    }
    Ok(())
}

fn read_object(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> ObjectTypeData {
    let flags = r.flags();
    let visited = flags & 1 != 0;
    let doc_info = (flags & 2 != 0).then(|| env.read_jsdoc(r, ast));
    let unknown = flags & 4 != 0;
    ObjectTypeData {
        visited,
        doc_info,
        unknown,
    }
}

fn write_property_map(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    m: &PropertyMap,
) -> SnapshotResult<()> {
    let _ = env;
    let PropertyMap {
        parent_source,
        properties,
        known_symbols,
        cache,
        immutable_empty,
    } = m;
    w.oty(*parent_source);
    let PropertyTable(table) = properties;
    // The cache of the process-wide immutable empty map is shared by every clone of that map.
    let static_cache = Arc::ptr_eq(cache, &PropertyMap::immutable_empty_map().cache);
    w.flags(&[
        table.is_some(),
        known_symbols.is_some(),
        static_cache,
        *immutable_empty,
    ]);
    // The synthetic externs' properties are left out, and so are the counts they added.
    let synthetic = table.as_ref().map_or(0, |table| {
        w.synthetic.as_ref().map_or(0, |norm| {
            table
                .values()
                .filter(|&&p| norm.is_synthetic_prop(p))
                .count()
        })
    });
    if let Some(table) = table
        && w.shared_begin(Shared::PropertyTable, ptr(table))
    {
        w.u((table.len() - synthetic) as u64);
        for (name, &p) in table.iter() {
            if w.synthetic
                .as_ref()
                .is_some_and(|norm| norm.is_synthetic_prop(p))
            {
                continue;
            }
            w.js(name);
            w.prop(p);
        }
        w.shared_end(Shared::PropertyTable, ptr(table));
    }
    if let Some(symbols) = known_symbols
        && w.shared_begin(Shared::KnownSymbols, ptr(symbols))
    {
        w.u(symbols.len() as u64);
        for &(t, p) in symbols.iter() {
            w.ty(t);
            w.prop(p);
        }
        w.shared_end(Shared::KnownSymbols, ptr(symbols));
    }
    if !static_cache {
        if w.shared_begin(Shared::KeyCache, ptr(cache)) {
            let guard = cache.lock().unwrap();
            let KeyCache {
                counter,
                string_keys,
                symbol_keys,
            } = &*guard;
            w.i(i64::from(*counter) - synthetic as i64);
            w.flags(&[string_keys.is_some(), symbol_keys.is_some()]);
            if let Some(keys) = string_keys
                && w.shared_begin(Shared::KeySet, ptr(keys))
            {
                w.u(keys.len() as u64);
                for key in keys.iter() {
                    w.js(key);
                }
                w.shared_end(Shared::KeySet, ptr(keys));
            }
            if let Some(keys) = symbol_keys
                && w.shared_begin(Shared::SymbolKeys, ptr(keys))
            {
                w.tys(keys);
                w.shared_end(Shared::SymbolKeys, ptr(keys));
            }
            drop(guard);
            w.shared_end(Shared::KeyCache, ptr(cache));
        }
    }
    Ok(())
}

fn read_property_map(r: &mut Reader<'_>) -> PropertyMap {
    let parent_source = r.oty();
    let flags = r.flags();
    let table = if flags & 1 != 0 {
        Some(match r.u() as usize {
            0 => {
                let len = r.u() as usize;
                let mut table = IndexMap::with_capacity_and_hasher(len, Default::default());
                for _ in 0..len {
                    let name = r.js();
                    table.insert(name, r.prop());
                }
                let table = Arc::new(table);
                r.property_tables.push(table.clone());
                table
            }
            index => r.property_tables[index - 1].clone(),
        })
    } else {
        None
    };
    let known_symbols = if flags & 2 != 0 {
        Some(match r.u() as usize {
            0 => {
                let len = r.u() as usize;
                let symbols: Vec<(TypeId, PropertyId)> =
                    (0..len).map(|_| (r.ty(), r.prop())).collect();
                let symbols = Arc::new(symbols);
                r.known_symbols.push(symbols.clone());
                symbols
            }
            index => r.known_symbols[index - 1].clone(),
        })
    } else {
        None
    };
    let cache = if flags & 4 != 0 {
        PropertyMap::immutable_empty_map().cache.clone()
    } else {
        match r.u() as usize {
            0 => {
                let counter = r.i() as i32;
                let key_flags = r.flags();
                let string_keys = if key_flags & 1 != 0 {
                    Some(match r.u() as usize {
                        0 => {
                            let len = r.u() as usize;
                            let keys: BTreeSet<JsString> = (0..len).map(|_| r.js()).collect();
                            let keys = Arc::new(keys);
                            r.key_sets.push(keys.clone());
                            keys
                        }
                        index => r.key_sets[index - 1].clone(),
                    })
                } else {
                    None
                };
                let symbol_keys = if key_flags & 2 != 0 {
                    Some(match r.u() as usize {
                        0 => {
                            let keys = Arc::new(r.tys());
                            r.symbol_keys.push(keys.clone());
                            keys
                        }
                        index => r.symbol_keys[index - 1].clone(),
                    })
                } else {
                    None
                };
                let cache = Arc::new(Mutex::new(KeyCache {
                    counter,
                    string_keys,
                    symbol_keys,
                }));
                r.key_caches.push(cache.clone());
                cache
            }
            index => r.key_caches[index - 1].clone(),
        }
    };
    let immutable_empty = flags & 8 != 0;
    PropertyMap {
        parent_source,
        properties: PropertyTable(table),
        known_symbols,
        cache,
        immutable_empty,
    }
}

fn write_prototype(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    p: &PrototypeObjectTypeData,
) -> SnapshotResult<()> {
    let PrototypeObjectTypeData {
        object,
        class_name,
        template_param_count,
        properties,
        native_type,
        anonymous_type,
        implicit_prototype_fallback,
        owner_function,
        pretty_print,
    } = p;
    write_object(w, env, object)?;
    w.opt_js(class_name.as_ref());
    w.u(*template_param_count as u64);
    write_property_map(w, env, properties)?;
    w.flags(&[*native_type, *anonymous_type, *pretty_print]);
    w.oty(*implicit_prototype_fallback);
    w.oty(*owner_function);
    Ok(())
}

fn read_prototype(
    r: &mut Reader<'_>,
    env: &mut dyn SnapshotEnv,
    ast: &Ast,
) -> PrototypeObjectTypeData {
    let object = read_object(r, env, ast);
    let class_name = r.opt_js();
    let template_param_count = r.u() as usize;
    let properties = read_property_map(r);
    let flags = r.flags();
    let native_type = flags & 1 != 0;
    let anonymous_type = flags & 2 != 0;
    let pretty_print = flags & 4 != 0;
    let implicit_prototype_fallback = r.oty();
    let owner_function = r.oty();
    PrototypeObjectTypeData {
        object,
        class_name,
        template_param_count,
        properties,
        native_type,
        anonymous_type,
        implicit_prototype_fallback,
        owner_function,
        pretty_print,
    }
}

fn write_proxy(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    p: &ProxyObjectTypeData,
) -> SnapshotResult<()> {
    let ProxyObjectTypeData {
        object,
        referenced_type,
        referenced_obj_type,
    } = p;
    write_object(w, env, object)?;
    w.ty(*referenced_type);
    w.oty(*referenced_obj_type);
    Ok(())
}

fn read_proxy(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> ProxyObjectTypeData {
    let object = read_object(r, env, ast);
    let referenced_type = r.ty();
    let referenced_obj_type = r.oty();
    ProxyObjectTypeData {
        object,
        referenced_type,
        referenced_obj_type,
    }
}

fn write_function(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    f: &FunctionTypeData,
) -> SnapshotResult<()> {
    let FunctionTypeData {
        prototype,
        constructor_ambiguity,
        call,
        prototype_slot,
        kind,
        prop_access,
        type_of_this,
        source,
        goog_module_id,
        is_structural_interface,
        is_abstract,
        implemented_interfaces,
        extended_interfaces,
        closure_primitive,
        canonical_representation,
    } = f;
    write_prototype(w, env, prototype)?;
    let ambiguity: u64 = match constructor_ambiguity {
        ConstructorAmbiguity::UNKNOWN => 0,
        ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR => 1,
        ConstructorAmbiguity::IS_UNAMBIGUOUS_CONSTRUCTOR => 2,
    };
    let kind: u64 = match kind {
        Kind::ORDINARY => 0,
        Kind::CONSTRUCTOR => 1,
        Kind::INTERFACE => 2,
        Kind::NONE => 3,
    };
    let access: u64 = match prop_access {
        None => 0,
        Some(PropAccess::ANY) => 1,
        Some(PropAccess::ANY_EXPLICIT) => 2,
        Some(PropAccess::STRUCT) => 3,
        Some(PropAccess::DICT) => 4,
    };
    let primitive: u64 = match closure_primitive {
        None => 0,
        Some(ClosurePrimitive::ASSERTS_FAIL) => 1,
        Some(ClosurePrimitive::ASSERTS_MATCHES_RETURN) => 2,
        Some(ClosurePrimitive::ASSERTS_TRUTHY) => 3,
    };
    w.u(ambiguity
        | kind << 2
        | access << 4
        | primitive << 7
        | u64::from(*is_structural_interface) << 9
        | u64::from(*is_abstract) << 10
        | u64::from(prototype_slot.is_some()) << 11
        | u64::from(source.is_some()) << 12
        | u64::from(!implemented_interfaces.is_empty()) << 13
        | u64::from(!extended_interfaces.is_empty()) << 14);
    w.oty(*call);
    if let Some(slot) = prototype_slot {
        w.prop(*slot);
    }
    w.oty(*type_of_this);
    if let Some(source) = source {
        env.write_node(w, *source)?;
    }
    w.opt_js(goog_module_id.as_ref());
    if !implemented_interfaces.is_empty() {
        w.tys(implemented_interfaces);
    }
    if !extended_interfaces.is_empty() {
        w.tys(extended_interfaces);
    }
    w.oty(*canonical_representation);
    Ok(())
}

fn read_function(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> FunctionTypeData {
    let prototype = read_prototype(r, env, ast);
    let packed = r.u();
    let constructor_ambiguity = match packed & 3 {
        0 => ConstructorAmbiguity::UNKNOWN,
        1 => ConstructorAmbiguity::IS_AMBIGUOUS_CONSTRUCTOR,
        _ => ConstructorAmbiguity::IS_UNAMBIGUOUS_CONSTRUCTOR,
    };
    let kind = match (packed >> 2) & 3 {
        0 => Kind::ORDINARY,
        1 => Kind::CONSTRUCTOR,
        2 => Kind::INTERFACE,
        _ => Kind::NONE,
    };
    let prop_access = match (packed >> 4) & 7 {
        0 => None,
        1 => Some(PropAccess::ANY),
        2 => Some(PropAccess::ANY_EXPLICIT),
        3 => Some(PropAccess::STRUCT),
        _ => Some(PropAccess::DICT),
    };
    let closure_primitive = match (packed >> 7) & 3 {
        0 => None,
        1 => Some(ClosurePrimitive::ASSERTS_FAIL),
        2 => Some(ClosurePrimitive::ASSERTS_MATCHES_RETURN),
        _ => Some(ClosurePrimitive::ASSERTS_TRUTHY),
    };
    let is_structural_interface = packed & (1 << 9) != 0;
    let is_abstract = packed & (1 << 10) != 0;
    let call = r.oty();
    let prototype_slot = (packed & (1 << 11) != 0).then(|| r.prop());
    let type_of_this = r.oty();
    let source = (packed & (1 << 12) != 0).then(|| env.read_node(r));
    let goog_module_id = r.opt_js();
    let implemented_interfaces = if packed & (1 << 13) != 0 {
        r.tys()
    } else {
        Vec::new()
    };
    let extended_interfaces = if packed & (1 << 14) != 0 {
        r.tys()
    } else {
        Vec::new()
    };
    let canonical_representation = r.oty();
    FunctionTypeData {
        prototype,
        constructor_ambiguity,
        call,
        prototype_slot,
        kind,
        prop_access,
        type_of_this,
        source,
        goog_module_id,
        is_structural_interface,
        is_abstract,
        implemented_interfaces,
        extended_interfaces,
        closure_primitive,
        canonical_representation,
    }
}

fn write_property_key(w: &mut Writer, key: &PropertyKey) {
    match key {
        PropertyKey::String(s) => {
            w.u(0);
            w.js(s);
        }
        PropertyKey::Symbol(t) => {
            w.u(1);
            w.ty(*t);
        }
    }
}

fn read_property_key(r: &mut Reader<'_>) -> PropertyKey {
    match r.u() {
        0 => PropertyKey::String(r.js()),
        _ => PropertyKey::Symbol(r.ty()),
    }
}

fn write_named(w: &mut Writer, env: &mut dyn SnapshotEnv, n: &NamedTypeData) -> SnapshotResult<()> {
    let NamedTypeData {
        proxy,
        reference,
        source_name,
        lineno,
        charno,
        resolution_kind,
        resolution_scope,
        validator,
        property_continuations,
        template_types,
        restrict_by_null,
    } = n;
    write_proxy(w, env, proxy)?;
    w.js(reference);
    w.rs(source_name);
    w.i(i64::from(*lineno));
    w.i(i64::from(*charno));
    w.u(match resolution_kind {
        ResolutionKind::NONE => 0,
        ResolutionKind::TYPE_NAME => 1,
        ResolutionKind::TYPEOF => 2,
    });
    match resolution_scope {
        None => w.b(false),
        Some(scope) => {
            w.b(true);
            write_scope_arc(w, env, scope)?;
        }
    }
    match validator {
        None => w.b(false),
        Some(validator) => {
            w.b(true);
            write_validator_arc(w, env, validator)?;
        }
    }
    match property_continuations {
        None => w.b(false),
        Some(continuations) => {
            w.b(true);
            w.u(continuations.len() as u64);
            for c in continuations {
                let PropertyContinuation {
                    property_name,
                    type_,
                    inferred,
                    property_node,
                } = c;
                write_property_key(w, property_name);
                w.ty(*type_);
                w.b(*inferred);
                write_opt_node(w, env, *property_node)?;
            }
        }
    }
    w.tys(template_types);
    w.b(*restrict_by_null);
    Ok(())
}

fn read_named(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> NamedTypeData {
    let proxy = read_proxy(r, env, ast);
    let reference = r.js();
    let source_name = r.rs().to_string();
    let lineno = r.i() as i32;
    let charno = r.i() as i32;
    let resolution_kind = match r.u() {
        0 => ResolutionKind::NONE,
        1 => ResolutionKind::TYPE_NAME,
        _ => ResolutionKind::TYPEOF,
    };
    let resolution_scope = r.b().then(|| read_scope_arc(r, env));
    let validator = r.b().then(|| read_validator_arc(r, env));
    let property_continuations = r.b().then(|| {
        let len = r.u() as usize;
        (0..len)
            .map(|_| {
                let property_name = read_property_key(r);
                let type_ = r.ty();
                let inferred = r.b();
                let property_node = read_opt_node(r, env);
                PropertyContinuation {
                    property_name,
                    type_,
                    inferred,
                    property_node,
                }
            })
            .collect()
    });
    let template_types = r.tys();
    let restrict_by_null = r.b();
    NamedTypeData {
        proxy,
        reference,
        source_name,
        lineno,
        charno,
        resolution_kind,
        resolution_scope,
        validator,
        property_continuations,
        template_types,
        restrict_by_null,
    }
}

fn write_type_set(w: &mut Writer, set: &IndexSet<TypeId>) {
    w.u(set.len() as u64);
    for &t in set {
        w.ty(t);
    }
}

fn read_type_set(r: &mut Reader<'_>) -> IndexSet<TypeId> {
    let len = r.u() as usize;
    let mut set = IndexSet::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        set.insert(r.ty());
    }
    set
}

fn write_replacer(w: &mut Writer, t: &TemplateTypeReplacer) {
    let TemplateTypeReplacer {
        bindings,
        visit_properties,
        use_unknown_for_missing_keys,
        use_unknown_for_missing_values,
        has_made_replacement,
        key_type,
        own_sub_map_boundary,
        seen_types,
        visited_object_types,
    } = t;
    write_template_map(w, bindings);
    w.b(*visit_properties);
    w.b(*use_unknown_for_missing_keys);
    w.b(*use_unknown_for_missing_values);
    w.b(*has_made_replacement);
    w.oty(*key_type);
    w.i(i64::from(*own_sub_map_boundary));
    match seen_types {
        None => w.b(false),
        Some(seen) => {
            w.b(true);
            write_type_set(w, seen);
        }
    }
    match visited_object_types {
        None => w.b(false),
        Some(visited) => {
            w.b(true);
            w.u(visited.len() as u64);
            for (&k, &v) in visited {
                w.ty(k);
                w.ty(v);
            }
        }
    }
}

fn read_replacer(r: &mut Reader<'_>) -> TemplateTypeReplacer {
    let bindings = read_template_map(r);
    let visit_properties = r.b();
    let use_unknown_for_missing_keys = r.b();
    let use_unknown_for_missing_values = r.b();
    let has_made_replacement = r.b();
    let key_type = r.oty();
    let own_sub_map_boundary = r.i() as i32;
    let seen_types = r.b().then(|| read_type_set(r));
    let visited_object_types = r.b().then(|| {
        let len = r.u() as usize;
        let mut map = IndexMap::with_capacity_and_hasher(len, Default::default());
        for _ in 0..len {
            let k = r.ty();
            map.insert(k, r.ty());
        }
        map
    });
    TemplateTypeReplacer {
        bindings,
        visit_properties,
        use_unknown_for_missing_keys,
        use_unknown_for_missing_values,
        has_made_replacement,
        key_type,
        own_sub_map_boundary,
        seen_types,
        visited_object_types,
    }
}

fn write_value_type(w: &mut Writer, v: &ValueTypeData) {
    let ValueTypeData = v;
    let _ = w;
}

/// The code of a type's kind (written with the type's flags, see `write_type`).
fn kind_code(kind: &JSTypeKind) -> u64 {
    match kind {
        JSTypeKind::All(_) => 0,
        JSTypeKind::Arrow(_) => 1,
        JSTypeKind::Boolean(_) => 2,
        JSTypeKind::BigInt(_) => 3,
        JSTypeKind::Enum(_) => 4,
        JSTypeKind::EnumElement(_) => 5,
        JSTypeKind::Function(_) => 6,
        JSTypeKind::InstanceObject(_) => 7,
        JSTypeKind::Named(_) => 8,
        JSTypeKind::No(_) => 9,
        JSTypeKind::NoObject(_) => 10,
        JSTypeKind::NoResolved(_) => 11,
        JSTypeKind::Null(_) => 12,
        JSTypeKind::Number(_) => 13,
        JSTypeKind::PrototypeObject(_) => 14,
        JSTypeKind::ProxyObject(_) => 15,
        JSTypeKind::Record(_) => 16,
        JSTypeKind::String(_) => 17,
        JSTypeKind::Symbol(_) => 18,
        JSTypeKind::KnownSymbol(_) => 19,
        JSTypeKind::Template(_) => 20,
        JSTypeKind::Templatized(_) => 21,
        JSTypeKind::Union(_) => 22,
        JSTypeKind::Unknown(_) => 23,
        JSTypeKind::Void(_) => 24,
        JSTypeKind::UnitTesting(_) => 31,
    }
}

fn write_kind(w: &mut Writer, env: &mut dyn SnapshotEnv, kind: &JSTypeKind) -> SnapshotResult<()> {
    match kind {
        JSTypeKind::All(d) => {
            let AllTypeData = d;
        }
        JSTypeKind::Arrow(d) => {
            let ArrowTypeData {
                parameter_list,
                return_type,
                return_type_inferred,
            } = d;
            w.u(parameter_list.len() as u64);
            for p in parameter_list {
                w.ty(p.get_jstype());
                w.b(p.is_optional());
                w.b(p.is_variadic());
            }
            w.ty(*return_type);
            w.b(*return_type_inferred);
        }
        JSTypeKind::Boolean(d) => {
            let BooleanTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::BigInt(d) => {
            let BigIntTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::Enum(d) => {
            let EnumTypeData {
                prototype,
                element_type,
                elements,
                source,
                goog_module_id,
            } = &**d;
            write_prototype(w, env, prototype)?;
            w.oty(*element_type);
            w.u(elements.len() as u64);
            for e in elements {
                w.js(e);
            }
            write_opt_node(w, env, *source)?;
            w.opt_js(goog_module_id.as_ref());
        }
        JSTypeKind::EnumElement(d) => {
            let EnumElementTypeData {
                object,
                primitive_type,
                primitive_object_type,
                name,
                enum_type,
            } = d;
            write_object(w, env, object)?;
            w.ty(*primitive_type);
            w.oty(*primitive_object_type);
            w.opt_js(name.as_ref());
            w.ty(*enum_type);
        }
        JSTypeKind::Function(d) => {
            write_function(w, env, d)?;
        }
        JSTypeKind::InstanceObject(d) => {
            let InstanceObjectTypeData {
                prototype,
                constructor,
            } = d;
            write_prototype(w, env, prototype)?;
            w.ty(*constructor);
        }
        JSTypeKind::Named(d) => {
            write_named(w, env, d)?;
        }
        JSTypeKind::No(d) => {
            let NoTypeData { no_object } = &**d;
            let NoObjectTypeData { function } = no_object;
            write_function(w, env, function)?;
        }
        JSTypeKind::NoObject(d) => {
            let NoObjectTypeData { function } = &**d;
            write_function(w, env, function)?;
        }
        JSTypeKind::NoResolved(d) => {
            let NoResolvedTypeData {
                object,
                reference_name,
                template_types,
            } = d;
            write_object(w, env, object)?;
            w.js(reference_name);
            match template_types {
                None => w.b(false),
                Some(types) => {
                    w.b(true);
                    w.tys(types);
                }
            }
        }
        JSTypeKind::Null(d) => {
            let NullTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::Number(d) => {
            let NumberTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::PrototypeObject(d) => {
            write_prototype(w, env, d)?;
        }
        JSTypeKind::ProxyObject(d) => {
            write_proxy(w, env, d)?;
        }
        JSTypeKind::Record(d) => {
            let RecordTypeData {
                prototype,
                declared,
                is_frozen,
            } = d;
            write_prototype(w, env, prototype)?;
            w.b(*declared);
            w.b(*is_frozen);
        }
        JSTypeKind::String(d) => {
            let StringTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::Symbol(d) => {
            let SymbolTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::KnownSymbol(d) => {
            let KnownSymbolTypeData { symbol, name } = d;
            let SymbolTypeData { value } = symbol;
            write_value_type(w, value);
            w.js(name);
        }
        JSTypeKind::Template(d) => {
            let TemplateTypeData {
                proxy,
                name,
                bound,
                type_transformation,
            } = d;
            write_proxy(w, env, proxy)?;
            w.js(name);
            w.ty(*bound);
            write_opt_node(w, env, *type_transformation)?;
        }
        JSTypeKind::Templatized(d) => {
            let TemplatizedTypeData {
                proxy,
                template_types,
                is_specialized_only_with_unknown,
                replacer,
            } = &**d;
            write_proxy(w, env, proxy)?;
            w.tys(template_types);
            w.b(*is_specialized_only_with_unknown);
            write_replacer(w, replacer);
        }
        JSTypeKind::Union(d) => {
            let UnionTypeData {
                alternates,
                alternates_resolved_before_build,
                resolved_alternates_at_last_build,
            } = d;
            if w.shared_begin(Shared::Alternates, ptr(alternates)) {
                w.tys(alternates);
                w.shared_end(Shared::Alternates, ptr(alternates));
            }
            w.b(*alternates_resolved_before_build);
            w.i(i64::from(*resolved_alternates_at_last_build));
        }
        JSTypeKind::Unknown(d) => {
            let UnknownTypeData { object, is_checked } = d;
            write_object(w, env, object)?;
            w.b(*is_checked);
        }
        JSTypeKind::Void(d) => {
            let VoidTypeData { value } = d;
            write_value_type(w, value);
        }
        JSTypeKind::UnitTesting(_) => return unsupported("a unit-testing type"),
    }
    Ok(())
}

fn read_kind(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast, code: u64) -> JSTypeKind {
    let value = || ValueTypeData;
    match code {
        0 => JSTypeKind::All(AllTypeData),
        1 => {
            let len = r.u() as usize;
            let parameter_list = (0..len)
                .map(|_| {
                    let t = r.ty();
                    let optional = r.b();
                    Parameter::create(t, optional, r.b())
                })
                .collect();
            let return_type = r.ty();
            let return_type_inferred = r.b();
            JSTypeKind::Arrow(ArrowTypeData {
                parameter_list,
                return_type,
                return_type_inferred,
            })
        }
        2 => JSTypeKind::Boolean(BooleanTypeData { value: value() }),
        3 => JSTypeKind::BigInt(BigIntTypeData { value: value() }),
        4 => {
            let prototype = read_prototype(r, env, ast);
            let element_type = r.oty();
            let len = r.u() as usize;
            let mut elements = IndexSet::with_capacity_and_hasher(len, Default::default());
            for _ in 0..len {
                elements.insert(r.js());
            }
            let source = read_opt_node(r, env);
            let goog_module_id = r.opt_js();
            JSTypeKind::Enum(Box::new(EnumTypeData {
                prototype,
                element_type,
                elements,
                source,
                goog_module_id,
            }))
        }
        5 => {
            let object = read_object(r, env, ast);
            let primitive_type = r.ty();
            let primitive_object_type = r.oty();
            let name = r.opt_js();
            let enum_type = r.ty();
            JSTypeKind::EnumElement(EnumElementTypeData {
                object,
                primitive_type,
                primitive_object_type,
                name,
                enum_type,
            })
        }
        6 => JSTypeKind::Function(Box::new(read_function(r, env, ast))),
        7 => {
            let prototype = read_prototype(r, env, ast);
            let constructor = r.ty();
            JSTypeKind::InstanceObject(InstanceObjectTypeData {
                prototype,
                constructor,
            })
        }
        8 => JSTypeKind::Named(Box::new(read_named(r, env, ast))),
        9 => JSTypeKind::No(Box::new(NoTypeData {
            no_object: NoObjectTypeData {
                function: read_function(r, env, ast),
            },
        })),
        10 => JSTypeKind::NoObject(Box::new(NoObjectTypeData {
            function: read_function(r, env, ast),
        })),
        11 => {
            let object = read_object(r, env, ast);
            let reference_name = r.js();
            let template_types = r.b().then(|| r.tys());
            JSTypeKind::NoResolved(NoResolvedTypeData {
                object,
                reference_name,
                template_types,
            })
        }
        12 => JSTypeKind::Null(NullTypeData { value: value() }),
        13 => JSTypeKind::Number(NumberTypeData { value: value() }),
        14 => JSTypeKind::PrototypeObject(read_prototype(r, env, ast)),
        15 => JSTypeKind::ProxyObject(read_proxy(r, env, ast)),
        16 => {
            let prototype = read_prototype(r, env, ast);
            let declared = r.b();
            let is_frozen = r.b();
            JSTypeKind::Record(RecordTypeData {
                prototype,
                declared,
                is_frozen,
            })
        }
        17 => JSTypeKind::String(StringTypeData { value: value() }),
        18 => JSTypeKind::Symbol(SymbolTypeData { value: value() }),
        19 => JSTypeKind::KnownSymbol(KnownSymbolTypeData {
            symbol: SymbolTypeData { value: value() },
            name: r.js(),
        }),
        20 => {
            let proxy = read_proxy(r, env, ast);
            let name = r.js();
            let bound = r.ty();
            let type_transformation = read_opt_node(r, env);
            JSTypeKind::Template(TemplateTypeData {
                proxy,
                name,
                bound,
                type_transformation,
            })
        }
        21 => {
            let proxy = read_proxy(r, env, ast);
            let template_types = r.tys();
            let is_specialized_only_with_unknown = r.b();
            let replacer = read_replacer(r);
            JSTypeKind::Templatized(Box::new(TemplatizedTypeData {
                proxy,
                template_types,
                is_specialized_only_with_unknown,
                replacer,
            }))
        }
        22 => {
            let alternates = match r.u() as usize {
                0 => {
                    let alternates = Arc::new(r.tys());
                    r.alternates.push(alternates.clone());
                    alternates
                }
                index => r.alternates[index - 1].clone(),
            };
            let alternates_resolved_before_build = r.b();
            let resolved_alternates_at_last_build = r.i() as i32;
            JSTypeKind::Union(UnionTypeData {
                alternates,
                alternates_resolved_before_build,
                resolved_alternates_at_last_build,
            })
        }
        23 => {
            let object = read_object(r, env, ast);
            let is_checked = r.b();
            JSTypeKind::Unknown(UnknownTypeData { object, is_checked })
        }
        _ => JSTypeKind::Void(VoidTypeData { value: value() }),
    }
}

fn write_type(w: &mut Writer, env: &mut dyn SnapshotEnv, d: &JSTypeData) -> SnapshotResult<()> {
    let JSTypeData {
        resolve_result,
        template_type_map,
        loosen_typechecking_due_to_forward_referenced_supertype,
        hash_code_in_progress,
        in_templated_check_visit,
        template_check_result,
        kind,
    } = d;
    let check: u64 = match template_check_result.get() {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    };
    // The kind and the flags in one number.
    w.u(kind_code(kind)
        | u64::from(*loosen_typechecking_due_to_forward_referenced_supertype) << 5
        | u64::from(hash_code_in_progress.get()) << 6
        | u64::from(in_templated_check_visit.get()) << 7
        | check << 8);
    w.oty(*resolve_result);
    write_template_map(w, template_type_map);
    write_kind(w, env, kind)
}

fn read_type(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> JSTypeData {
    let packed = r.u();
    let loosen_typechecking_due_to_forward_referenced_supertype = packed & (1 << 5) != 0;
    let hash_code_in_progress = Cell::new(packed & (1 << 6) != 0);
    let in_templated_check_visit = Cell::new(packed & (1 << 7) != 0);
    let template_check_result = Cell::new(match (packed >> 8) & 3 {
        0 => None,
        1 => Some(false),
        _ => Some(true),
    });
    let resolve_result = r.oty();
    let template_type_map = read_template_map(r);
    let kind = read_kind(r, env, ast, packed & 31);
    JSTypeData {
        resolve_result,
        template_type_map,
        loosen_typechecking_due_to_forward_referenced_supertype,
        hash_code_in_progress,
        in_templated_check_visit,
        template_check_result,
        kind,
    }
}

fn write_property(
    w: &mut Writer,
    env: &mut dyn SnapshotEnv,
    p: &PropertyData,
) -> SnapshotResult<()> {
    let PropertyData {
        name,
        type_,
        inferred,
        property_node,
        doc_info,
    } = p;
    w.flags(&[
        matches!(name, PropertyKey::Symbol(_)),
        *inferred,
        property_node.is_some(),
        doc_info.is_some(),
    ]);
    match name {
        PropertyKey::String(s) => w.js(s),
        PropertyKey::Symbol(t) => w.ty(*t),
    }
    w.ty(*type_);
    if let Some(n) = property_node {
        env.write_node(w, *n)?;
    }
    if let Some(doc) = doc_info {
        env.write_jsdoc(w, doc)?;
    }
    Ok(())
}

fn read_property(r: &mut Reader<'_>, env: &mut dyn SnapshotEnv, ast: &Ast) -> PropertyData {
    let flags = r.flags();
    let name = if flags & 1 != 0 {
        PropertyKey::Symbol(r.ty())
    } else {
        PropertyKey::String(r.js())
    };
    let type_ = r.ty();
    let inferred = flags & 2 != 0;
    let property_node = (flags & 4 != 0).then(|| env.read_node(r));
    let doc_info = (flags & 8 != 0).then(|| env.read_jsdoc(r, ast));
    PropertyData {
        name,
        type_,
        inferred,
        property_node,
        doc_info,
    }
}

/// A key of an index by property name and the entries kept under it.
type KeptEntries<'m> = (&'m JsString, Vec<(i32, TypeId)>);

/// The entries of an index by property name without the synthetic externs' ones (a key left with
/// none is left out), and how many were left out.
fn kept_entries<'m>(
    w: &Writer,
    map: &'m IndexMap<JsString, Vec<(i32, TypeId)>>,
) -> (Vec<KeptEntries<'m>>, u64) {
    let mut kept = Vec::with_capacity(map.len());
    let mut left_out = 0;
    for (name, entries) in map {
        let entries: Vec<(i32, TypeId)> = match &w.synthetic {
            Some(norm) => entries
                .iter()
                .copied()
                .filter(|&(_, t)| !norm.is_synthetic_entry(name, t))
                .collect(),
            None => entries.clone(),
        };
        left_out += (map[name].len() - entries.len()) as u64;
        if !entries.is_empty() {
            kept.push((name, entries));
        }
    }
    (kept, left_out)
}

/// An index by property name, its keys in order: the index is only looked up (Java's `HashMap`),
/// so the order in which a compile added the keys is no part of its state. The entries of a key
/// keep their order.
fn write_indexed_by_property(w: &mut Writer, map: &IndexMap<JsString, Vec<(i32, TypeId)>>) {
    let (mut kept, _) = kept_entries(w, map);
    kept.sort_unstable_by(|a, b| a.0.cmp(b.0));
    w.u(kept.len() as u64);
    for (name, entries) in kept {
        w.js(name);
        w.u(entries.len() as u64);
        for (hash, t) in entries {
            w.ty(t);
            w.type_hash(t, hash);
        }
    }
}

fn read_indexed_by_property(r: &mut Reader<'_>) -> IndexMap<JsString, Vec<(i32, TypeId)>> {
    let len = r.u() as usize;
    let mut map = IndexMap::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        let name = r.js();
        let count = r.u() as usize;
        let entries = (0..count)
            .map(|_| {
                let t = r.ty();
                (r.type_hash(t), t)
            })
            .collect();
        map.insert(name, entries);
    }
    map
}

fn write_string_set(w: &mut Writer, set: &IndexSet<JsString>) {
    w.u(set.len() as u64);
    for s in set {
        w.js(s);
    }
}

fn read_string_set(r: &mut Reader<'_>) -> IndexSet<JsString> {
    let len = r.u() as usize;
    let mut set = IndexSet::with_capacity_and_hasher(len, Default::default());
    for _ in 0..len {
        set.insert(r.js());
    }
    set
}

fn write_resolver(w: &mut Writer, resolver: &JSTypeResolver) {
    let JSTypeResolver {
        capture_stack,
        resolution_queue,
        state,
    } = resolver;
    w.u(capture_stack.len() as u64);
    for &t in capture_stack {
        w.ty(t);
    }
    w.u(resolution_queue.len() as u64);
    for &t in resolution_queue {
        w.ty(t);
    }
    w.u(match state {
        State::CLOSED => 0,
        State::OPEN => 1,
        State::CLOSING => 2,
    });
}

fn read_resolver(r: &mut Reader<'_>) -> JSTypeResolver {
    let len = r.u() as usize;
    let capture_stack: VecDeque<TypeId> = (0..len).map(|_| r.ty()).collect();
    let len = r.u() as usize;
    let resolution_queue: VecDeque<TypeId> = (0..len).map(|_| r.ty()).collect();
    let state = match r.u() {
        0 => State::CLOSED,
        1 => State::OPEN,
        _ => State::CLOSING,
    };
    JSTypeResolver {
        capture_stack,
        resolution_queue,
        state,
    }
}

/// An entry of an index by property name: the key's position, the key, the entry's position in
/// its list and the entry.
type IndexEntry = (usize, JsString, usize, (i32, TypeId));

/// What a compile's synthetic externs (the `var x;` statements VarCheck adds for undeclared
/// names, typed before the bundled externs) added to the registry: for each name a property of
/// the global this type, its entry in that type's property table (and one more in the table's
/// key counter), and its registration in the indexes by property name (and one more in the
/// registration epoch). A snapshot made with them leaves them out and numbers the properties
/// after them as if they were not there, so that it applies whatever the program's undeclared
/// names are; a restore puts them back where they were.
#[derive(Clone, Debug)]
pub struct SyntheticEntries {
    names: IndexSet<JsString>,
    global_this: TypeId,
    props_start: u32,
    properties: Vec<PropertyData>,
    table_position: usize,
    table_entries: Vec<(JsString, PropertyId)>,
    /// Per index (non-ref, each-ref): the key's position, the key, the entry's position in its
    /// list and the entry, in key order.
    index_entries: [Vec<IndexEntry>; 2],
}

impl SyntheticEntries {
    fn norm(&self) -> SyntheticNorm {
        SyntheticNorm {
            props_start: self.props_start,
            props_count: self.properties.len() as u32,
            names: self.names.clone(),
            global_this: self.global_this,
        }
    }

    /// Puts the entries back into a restored registry (whose ids already leave room for them).
    fn put_back(&self, reg: &mut JSTypeRegistry) {
        let start = self.props_start as usize;
        reg.properties
            .splice(start..start, self.properties.iter().cloned());
        let k = self.table_entries.len();
        let map = &mut crate::js_type::prototype_data_mut(self.global_this, reg).properties;
        let table = Arc::make_mut(map.properties.0.get_or_insert_with(Arc::default));
        for (i, (name, p)) in self.table_entries.iter().enumerate() {
            table.shift_insert(self.table_position + i, name.clone(), *p);
        }
        map.cache.lock().unwrap().counter += k as i32;
        let mut epoch = 0;
        for (index, entries) in [
            &mut reg.non_ref_types_indexed_by_property,
            &mut reg.each_ref_type_indexed_by_property,
        ]
        .into_iter()
        .zip(&self.index_entries)
        {
            for (_, key, list_index, entry) in entries {
                match index.get_mut(key) {
                    Some(list) => list.insert(*list_index, *entry),
                    None => {
                        index.insert(key.clone(), vec![*entry]);
                    }
                }
                epoch += 1;
            }
        }
        reg.property_registration_epoch += epoch;
    }
}

impl JSTypeRegistry {
    /// The entries the synthetic externs declaring `names` (with these NAME nodes) added, which
    /// must be the last properties made and the last entries of the global this type's table.
    pub fn snapshot_synthetic_entries(
        &self,
        names: &[JsString],
        nodes: &[NodeId],
    ) -> Result<SyntheticEntries, String> {
        let k = names.len();
        let total = self.properties.len();
        if total < k {
            return Err("fewer properties than synthetic externs".into());
        }
        let props_start = total - k;
        for (j, (name, &node)) in names.iter().zip(nodes).enumerate() {
            let p = &self.properties[props_start + j];
            if !matches!(&p.name, PropertyKey::String(n) if n == name)
                || p.property_node != Some(node)
            {
                return Err(format!("the property of synthetic extern {name}"));
            }
        }
        let global_this = self.get_native_type(crate::JSTypeNative::GLOBAL_THIS);
        let map = &crate::js_type::prototype_data(global_this, self).properties;
        let entries = map.properties.entries();
        if entries.len() < k {
            return Err("the global this table".into());
        }
        let table_position = entries.len() - k;
        let mut table_entries = Vec::with_capacity(k);
        for (j, (name, &p)) in entries[table_position..].iter().enumerate() {
            if *name != names[j] || p.0 as usize != props_start + j {
                return Err(format!(
                    "the global this entry of synthetic extern {}",
                    names[j]
                ));
            }
            table_entries.push((name.clone(), p));
        }
        let names: IndexSet<JsString> = names.iter().cloned().collect();
        let mut index_entries: [Vec<_>; 2] = [Vec::new(), Vec::new()];
        let mut count = 0;
        for (out, index) in index_entries.iter_mut().zip([
            &self.non_ref_types_indexed_by_property,
            &self.each_ref_type_indexed_by_property,
        ]) {
            for (key_index, (key, list)) in index.iter().enumerate() {
                if !names.contains(key) {
                    continue;
                }
                for (list_index, &(hash, t)) in list.iter().enumerate() {
                    if t == global_this {
                        out.push((key_index, key.clone(), list_index, (hash, t)));
                        count += 1;
                    }
                }
            }
        }
        if count != k {
            return Err("the index entries of the synthetic externs".into());
        }
        Ok(SyntheticEntries {
            names,
            global_this,
            props_start: props_start as u32,
            properties: self.properties[props_start..].to_vec(),
            table_position,
            table_entries,
            index_entries,
        })
    }
}

impl JSTypeRegistry {
    /// The names of the global this type's own properties.
    pub fn snapshot_global_this_names(&self) -> IndexSet<JsString> {
        let global_this = self.get_native_type(crate::JSTypeNative::GLOBAL_THIS);
        crate::js_type::prototype_data(global_this, self)
            .properties
            .properties
            .entries()
            .keys()
            .cloned()
            .collect()
    }
}

/// The names the registry was asked about while a lookup log was kept (see
/// `JSTypeRegistry::snapshot_start_lookup_log`): the inputs of a compile that typing reads.
#[derive(Debug, Default, Clone)]
pub struct LookupLog {
    /// `isNonNullableName`: the name-table root of the lookup and the name.
    pub non_nullable: IndexSet<(NodeId, JsString)>,
    /// `isForwardDeclaredType`.
    pub forward_declared: IndexSet<JsString>,
    /// `resolveViaClosureNamespace` while a namespace was registered.
    pub closure_namespace: IndexSet<JsString>,
}

impl JSTypeRegistry {
    /// Writes the registry's state (see the module comment for what is left out).
    pub fn snapshot_encode(
        &self,
        w: &mut Writer,
        env: &mut dyn SnapshotEnv,
        synthetic: Option<&SyntheticEntries>,
    ) -> SnapshotResult<()> {
        w.synthetic = synthetic.map(SyntheticEntries::norm);
        w.bad_reference = false;
        let result = self.snapshot_encode_fields(w, env);
        w.synthetic = None;
        result?;
        if w.bad_reference {
            return unsupported("a reference to a synthetic extern's property");
        }
        Ok(())
    }

    fn snapshot_encode_fields(
        &self,
        w: &mut Writer,
        env: &mut dyn SnapshotEnv,
    ) -> SnapshotResult<()> {
        let JSTypeRegistry {
            scope_storage: _,
            types,
            properties,
            property_access_annotations,
            resolver,
            empty_template_type_map,
            reporter: _,
            native_types,
            scoped_name_table,
            qualified_type_roots,
            has_this_qualified_type_root,
            closure_namespaces: _,
            name_table_global_root: _,
            non_nullable_type_names: _,
            forward_declared_types: _,
            non_ref_types_indexed_by_property,
            sentinel_object_literal,
            properties_of_supertypes_in_unions,
            dropped_properties_of_unions,
            each_ref_type_indexed_by_property,
            property_registration_epoch,
            snapshot_lookups: _,
            i_object_index_template_key,
            i_object_element_template_key,
            iterable_value_template,
            iterable_return_template,
            iterator_iterable_value_template,
            iterator_iterable_return_template,
            iterator_like_value_template,
            iterator_like_return_template,
            iterator_value_template,
            iterator_return_template,
            iiterable_result_value_template,
            async_iterable_value_template,
            async_iterable_return_template,
            async_iterator_value_template,
            async_iterator_return_template,
            generator_value_template,
            generator_return_template,
            i_thenable_template_key,
            promise_template_key,
            array_element_template_key,
            readonly_array_element_template_key,
            readonly_map_key_template_key,
            readonly_map_value_template_key,
            map_key_template_key,
            map_value_template_key,
        } = self;
        w.label(|| "registry: empty template map".into());
        match empty_template_type_map {
            None => w.b(false),
            Some(map) => {
                w.b(true);
                write_template_map(w, map);
            }
        }
        w.u(types.len() as u64);
        for (i, t) in types.iter().enumerate() {
            w.label(|| format!("type {i}"));
            w.type_base(i as u32);
            write_type(w, env, t).map_err(|e| SnapshotError(format!("type {i}: {}", e.0)))?;
        }
        let synthetic_props = w.synthetic.as_ref().map_or(0..0, |norm| {
            norm.props_start as usize..(norm.props_start + norm.props_count) as usize
        });
        w.u((properties.len() - synthetic_props.len()) as u64);
        for (i, p) in properties.iter().enumerate() {
            if synthetic_props.contains(&i) {
                continue;
            }
            w.label(|| format!("property {i}"));
            write_property(w, env, p)
                .map_err(|e| SnapshotError(format!("property {i}: {}", e.0)))?;
        }
        w.label(|| "registry: property access annotations".into());
        w.u(property_access_annotations.len() as u64);
        for (name, annotations) in property_access_annotations {
            let PropertyAccessAnnotations {
                visibility,
                constant,
                deprecated,
            } = annotations;
            w.js(name);
            w.b(*visibility);
            w.b(*constant);
            w.b(*deprecated);
        }
        w.label(|| "registry: resolver".into());
        write_resolver(w, resolver);
        w.label(|| "registry: native types".into());
        w.u(native_types.len() as u64);
        for &t in native_types {
            w.oty(t);
        }
        w.label(|| "registry: scoped name table".into());
        w.u(scoped_name_table.len() as u64);
        for ((root, name), &t) in scoped_name_table {
            env.write_node(w, *root)?;
            w.js(name);
            w.ty(t);
        }
        w.label(|| "registry: qualified type roots".into());
        write_string_set(w, qualified_type_roots);
        w.b(*has_this_qualified_type_root);
        w.label(|| "registry: non-ref types by property".into());
        write_indexed_by_property(w, non_ref_types_indexed_by_property);
        w.oty(*sentinel_object_literal);
        w.label(|| "registry: union property sets".into());
        write_string_set(w, properties_of_supertypes_in_unions);
        write_string_set(w, dropped_properties_of_unions);
        w.label(|| "registry: ref types by property".into());
        write_indexed_by_property(w, each_ref_type_indexed_by_property);
        // Each entry of an index added one to the epoch.
        let left_out = kept_entries(w, non_ref_types_indexed_by_property).1
            + kept_entries(w, each_ref_type_indexed_by_property).1;
        w.u(*property_registration_epoch - left_out);
        w.label(|| "registry: template keys".into());
        for key in [
            i_object_index_template_key,
            i_object_element_template_key,
            iterable_value_template,
            iterable_return_template,
            iterator_iterable_value_template,
            iterator_iterable_return_template,
            iterator_like_value_template,
            iterator_like_return_template,
            iterator_value_template,
            iterator_return_template,
            iiterable_result_value_template,
            async_iterable_value_template,
            async_iterable_return_template,
            async_iterator_value_template,
            async_iterator_return_template,
            generator_value_template,
            generator_return_template,
            i_thenable_template_key,
            promise_template_key,
            array_element_template_key,
            readonly_array_element_template_key,
            readonly_map_key_template_key,
            readonly_map_value_template_key,
            map_key_template_key,
            map_value_template_key,
        ] {
            w.oty(*key);
        }
        Ok(())
    }

    /// Replaces the state `snapshot_encode` writes with the state in `r`.
    pub fn snapshot_restore(
        &mut self,
        r: &mut Reader<'_>,
        env: &mut dyn SnapshotEnv,
        ast: &Ast,
        synthetic: Option<&SyntheticEntries>,
    ) {
        r.prop_shift = synthetic.map(|s| (s.props_start, s.properties.len() as u32));
        self.snapshot_restore_fields(r, env, ast);
        r.prop_shift = None;
        if let Some(synthetic) = synthetic {
            synthetic.put_back(self);
        }
    }

    fn snapshot_restore_fields(
        &mut self,
        r: &mut Reader<'_>,
        env: &mut dyn SnapshotEnv,
        ast: &Ast,
    ) {
        self.empty_template_type_map = r.b().then(|| read_template_map(r));
        let len = r.u() as usize;
        let mut types = Vec::with_capacity(len.max(self.types.capacity()));
        for i in 0..len {
            r.type_base(i as u32);
            types.push(read_type(r, env, ast));
        }
        self.types = types;
        let len = r.u() as usize;
        let mut properties = Vec::with_capacity(len.max(self.properties.capacity()));
        for _ in 0..len {
            properties.push(read_property(r, env, ast));
        }
        self.properties = properties;
        let len = r.u() as usize;
        let mut annotations = IndexMap::with_capacity_and_hasher(len, Default::default());
        for _ in 0..len {
            let name = r.js();
            let visibility = r.b();
            let constant = r.b();
            let deprecated = r.b();
            annotations.insert(
                name,
                PropertyAccessAnnotations {
                    visibility,
                    constant,
                    deprecated,
                },
            );
        }
        self.property_access_annotations = annotations;
        self.resolver = read_resolver(r);
        let len = r.u() as usize;
        self.native_types = (0..len).map(|_| r.oty()).collect();
        let len = r.u() as usize;
        let mut table = IndexMap::with_capacity_and_hasher(len, Default::default());
        for _ in 0..len {
            let root = env.read_node(r);
            let name = r.js();
            table.insert((root, name), r.ty());
        }
        self.scoped_name_table = table;
        self.qualified_type_roots = read_string_set(r);
        self.has_this_qualified_type_root = r.b();
        self.non_ref_types_indexed_by_property = read_indexed_by_property(r);
        self.sentinel_object_literal = r.oty();
        self.properties_of_supertypes_in_unions = read_string_set(r);
        self.dropped_properties_of_unions = read_string_set(r);
        self.each_ref_type_indexed_by_property = read_indexed_by_property(r);
        self.property_registration_epoch = r.u();
        for key in [
            &mut self.i_object_index_template_key,
            &mut self.i_object_element_template_key,
            &mut self.iterable_value_template,
            &mut self.iterable_return_template,
            &mut self.iterator_iterable_value_template,
            &mut self.iterator_iterable_return_template,
            &mut self.iterator_like_value_template,
            &mut self.iterator_like_return_template,
            &mut self.iterator_value_template,
            &mut self.iterator_return_template,
            &mut self.iiterable_result_value_template,
            &mut self.async_iterable_value_template,
            &mut self.async_iterable_return_template,
            &mut self.async_iterator_value_template,
            &mut self.async_iterator_return_template,
            &mut self.generator_value_template,
            &mut self.generator_return_template,
            &mut self.i_thenable_template_key,
            &mut self.promise_template_key,
            &mut self.array_element_template_key,
            &mut self.readonly_array_element_template_key,
            &mut self.readonly_map_key_template_key,
            &mut self.readonly_map_value_template_key,
            &mut self.map_key_template_key,
            &mut self.map_value_template_key,
        ] {
            *key = r.oty();
        }
    }

    /// Starts recording the names the inputs of a compile are asked about (`LookupLog`).
    pub fn snapshot_start_lookup_log(&mut self) {
        self.snapshot_lookups = Some(Box::new(Mutex::new(LookupLog::default())));
    }

    /// Stops recording and returns the names recorded since `snapshot_start_lookup_log`.
    pub fn snapshot_take_lookup_log(&mut self) -> Option<LookupLog> {
        self.snapshot_lookups
            .take()
            .map(|log| log.into_inner().unwrap())
    }

    /// The names registered as non-nullable for the name-table root `root`.
    pub fn snapshot_non_nullable_names(&self, root: NodeId) -> Option<&IndexSet<JsString>> {
        self.non_nullable_type_names.get(&root)
    }

    /// The roots that have non-nullable names.
    pub fn snapshot_non_nullable_roots(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.non_nullable_type_names.keys().copied()
    }

    /// Whether every registered Closure namespace is a legacy one (`goog.provide`): then
    /// `resolveViaClosureNamespace` finds no type, as with no namespace at all.
    pub fn snapshot_closure_namespaces_are_legacy(&self) -> bool {
        self.closure_namespaces
            .values()
            .all(ClosureNamespace::is_legacy)
    }

    /// The root of the global name table (made with the registry).
    pub fn snapshot_name_table_global_root(&self) -> NodeId {
        self.name_table_global_root
    }

    /// A digest of the registry fields `snapshot_encode` leaves out, to check that typing did not
    /// change them.
    pub fn snapshot_input_digest(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, namespace) in &self.closure_namespaces {
            out.push(format!(
                "namespace {name} {} {:?} {:?}",
                namespace.is_legacy(),
                namespace.definition_node(),
                namespace.type_()
            ));
        }
        for (root, names) in &self.non_nullable_type_names {
            for name in names {
                out.push(format!("non-nullable {root:?} {name}"));
            }
        }
        out.push(format!("global root {:?}", self.name_table_global_root));
        out
    }

    /// Rust-only: records a lookup while a log is kept.
    #[inline]
    pub(crate) fn note_lookup(&self, record: impl FnOnce(&mut LookupLog)) {
        if let Some(log) = &self.snapshot_lookups {
            record(&mut log.lock().unwrap());
        }
    }
}
