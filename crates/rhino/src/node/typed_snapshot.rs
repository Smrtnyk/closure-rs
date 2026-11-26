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

//! Rust-only (performance divergence): what closure-jscomp's restore of the typed bundled
//! externs (`crates/jscomp/src/typed_externs.rs`) reads of an arena: node handles by slot, the
//! node range of a loaded bundled script, and digests of the node state typing reads that the
//! snapshot watch does not track (properties, and the JSDoc of nodes outside a watched range).
use super::{Ast, NodeId, ObjectProp, PropValue};
use crate::jsdoc_info::{JSDocInfo, PackedPropertyValues, PropertyValue};
use std::num::NonZeroU32;

impl NodeId {
    /// Rust-only: the node in slot `index` of its arena (the inverse of `arena_index`).
    pub fn from_arena_index(index: usize) -> NodeId {
        NodeId(NonZeroU32::new(u32::try_from(index + 1).expect("arena index")).unwrap())
    }
}

/// A digest of 64-bit words that is the same on every platform and in every process (a
/// multiply-xorshift mix; not for adversarial inputs).
#[derive(Clone, Copy, Debug)]
pub struct Digest(pub u64);

impl Default for Digest {
    fn default() -> Self {
        Self(0x9e37_79b9_7f4a_7c15)
    }
}

impl Digest {
    #[inline]
    pub fn feed(&mut self, v: u64) {
        let h = (self.0 ^ v).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        self.0 = h ^ (h >> 31);
    }

    pub fn feed_units(&mut self, units: &[u16]) {
        self.feed(units.len() as u64);
        let mut chunks = units.chunks_exact(4);
        for chunk in &mut chunks {
            self.feed(
                u64::from(chunk[0])
                    | (u64::from(chunk[1]) << 16)
                    | (u64::from(chunk[2]) << 32)
                    | (u64::from(chunk[3]) << 48),
            );
        }
        for &unit in chunks.remainder() {
            self.feed(u64::from(unit));
        }
    }

    fn feed_bytes(&mut self, bytes: &[u8]) {
        self.feed(bytes.len() as u64);
        for &byte in bytes {
            self.feed(u64::from(byte));
        }
    }
}

/// The digests of the names of the last source files and input ids seen (most nodes in a row
/// share one), by address.
#[derive(Default)]
struct NameDigests {
    last: Option<(usize, u64)>,
}

impl NameDigests {
    fn of(&mut self, ptr: usize, name: impl FnOnce() -> Vec<u8>) -> u64 {
        match self.last {
            Some((p, d)) if p == ptr => d,
            _ => {
                let mut digest = Digest::default();
                digest.feed_bytes(&name());
                self.last = Some((ptr, digest.0));
                digest.0
            }
        }
    }
}

impl Ast {
    /// The slots `[start, end)` loaded for this exact script by `watch_parsed_nodes`, whatever
    /// changed since.
    pub fn parsed_script_range(&self, script: NodeId) -> Option<(u32, u32)> {
        let i = script.0.get() - 1;
        let ranges = &self.snapshot_watches.ranges;
        let index = ranges.partition_point(|range| range.end <= i);
        let range = ranges.get(index)?;
        (range.root == script).then_some((range.start, range.end))
    }

    /// Sets the types of many nodes (as `Node#setJSType` would one by one on untyped nodes).
    pub fn set_node_jstypes(
        &mut self,
        types: impl ExactSizeIterator<Item = (NodeId, crate::jstype::TypeId)>,
    ) {
        let mut types = types.peekable();
        let mut max = 0;
        let mut pending = Vec::with_capacity(types.len());
        for (n, t) in types.by_ref() {
            let index = n.arena_index();
            assert!(index < self.nodes.len());
            max = max.max(index + 1);
            pending.push((index, t));
        }
        if self.types.len() < max {
            self.types.resize_with(max, || None);
        }
        for (index, t) in pending {
            let slot = &mut self.types[index];
            crate::check_state!(slot.is_none());
            *slot = Some(super::NodeType::JSType(t));
        }
    }

    /// Whether any node in slots `[start, end)` has a type or color.
    pub fn has_types_in(&self, start: u32, end: u32) -> bool {
        let end = (end as usize).min(self.types.len());
        let start = (start as usize).min(end);
        self.types[start..end].iter().any(Option::is_some)
    }

    /// The node's property presence bits (`prop_masks`) and the identity of its property list.
    pub fn prop_state(&self, n: NodeId) -> (u64, usize) {
        let index = n.arena_index();
        let list = self.nodes[index]
            .prop_list_head
            .as_ref()
            .map_or(0, |head| std::sync::Arc::as_ptr(head) as usize);
        (self.prop_masks[index], list)
    }

    /// The kind of the first item of the node's property list and the identity of the rest.
    pub fn prop_list_head_item(&self, n: NodeId) -> Option<(u8, usize)> {
        let head = self.nodes[n.arena_index()].prop_list_head.as_deref()?;
        let next = head
            .next
            .as_ref()
            .map_or(0, |next| std::sync::Arc::as_ptr(next) as usize);
        Some((head.prop_type, next))
    }

    /// The root node of the arena's implicit template bound, if it was made.
    pub fn implicit_template_bound_root(&self) -> Option<NodeId> {
        self.implicit_template_bound.as_ref().map(|e| e.get_root())
    }

    /// Feeds the properties of the nodes in slots `[start, end)` to `digest` (see
    /// `prop_digest`); a node without properties adds nothing but its place.
    pub fn prop_digest_range(
        &self,
        start: u32,
        end: u32,
        rel: &mut dyn FnMut(NodeId) -> u64,
        digest: &mut Digest,
    ) {
        digest.feed(u64::from(end - start));
        let mut names = NameDigests::default();
        for index in start as usize..end as usize {
            if self.prop_masks[index] != 0 || self.nodes[index].prop_list_head.is_some() {
                digest.feed((index - start as usize) as u64);
                self.prop_digest_cached(NodeId::from_arena_index(index), rel, digest, &mut names);
            }
        }
    }

    /// Feeds the node's properties to `digest`: the presence bits and each item's kind and value,
    /// with node values through `rel`. JSDoc values are left out (see `JSDocInfo::digest`), and so
    /// are values without a stable description (FeatureSet and other opaque values).
    pub fn prop_digest(&self, n: NodeId, rel: &mut dyn FnMut(NodeId) -> u64, digest: &mut Digest) {
        self.prop_digest_cached(n, rel, digest, &mut NameDigests::default());
    }

    fn prop_digest_cached(
        &self,
        n: NodeId,
        rel: &mut dyn FnMut(NodeId) -> u64,
        digest: &mut Digest,
        names: &mut NameDigests,
    ) {
        let index = n.arena_index();
        digest.feed(self.prop_masks[index]);
        let mut item = self.nodes[index].prop_list_head.as_deref();
        while let Some(current) = item {
            digest.feed(u64::from(current.prop_type));
            match &current.value {
                PropValue::Int(v) => digest.feed(*v as u32 as u64),
                PropValue::Object(object) => match object {
                    ObjectProp::NonJSDocComment(comment) => {
                        digest.feed(1);
                        digest.feed_units(comment.get_comment_string().as_units());
                    }
                    ObjectProp::JSDocInfo(_) => digest.feed(2),
                    ObjectProp::StaticSourceFile(file) => {
                        digest.feed(3);
                        let ptr = std::sync::Arc::as_ptr(file) as *const () as usize;
                        digest.feed(names.of(ptr, || file.get_name().as_bytes().to_vec()));
                    }
                    ObjectProp::InputId(id) => {
                        digest.feed(4);
                        let ptr = std::sync::Arc::as_ptr(id) as usize;
                        digest.feed(names.of(ptr, || id.get_id_name().as_bytes().to_vec()));
                    }
                    ObjectProp::Node(node) => {
                        digest.feed(5);
                        let v = rel(*node);
                        digest.feed(v);
                    }
                    ObjectProp::JSType(t) => {
                        digest.feed(6);
                        digest.feed(u64::from(t.0));
                    }
                    ObjectProp::Opaque(_) => digest.feed(7),
                },
            }
            item = current.next.as_deref();
        }
    }
}

impl JSDocInfo {
    /// Feeds this JSDoc's fields to `digest`: its flags and, for each value, its kind and a
    /// description (type expressions by their root node through `rel`, strings by content).
    pub fn digest(&self, rel: &mut dyn FnMut(NodeId) -> u64, digest: &mut Digest) {
        digest.feed(self.property_bits);
        digest.feed(self.property_keys_bitset);
        let mut value = |v: &PropertyValue, digest: &mut Digest| match v {
            PropertyValue::Null => digest.feed(0),
            PropertyValue::Visibility(v) => {
                digest.feed(1);
                digest.feed(*v as u64);
            }
            PropertyValue::TypeExpr(e) => {
                digest.feed(2);
                digest.feed(e.as_ref().map_or(0, |e| rel(e.get_root()) + 1));
            }
            PropertyValue::TypeList(list) => {
                digest.feed(3);
                digest.feed(list.len() as u64);
                for e in list {
                    digest.feed(rel(e.get_root()));
                }
            }
            PropertyValue::TypeMap(map) => {
                digest.feed(4);
                digest.feed(map.len() as u64);
                for (k, e) in map {
                    digest.feed_units(k.as_units());
                    digest.feed(e.as_ref().map_or(0, |e| rel(e.get_root()) + 1));
                }
            }
            PropertyValue::NodeMap(map) => {
                digest.feed(5);
                digest.feed(map.len() as u64);
                for (k, n) in map {
                    digest.feed_units(k.as_units());
                    digest.feed(rel(*n));
                }
            }
            PropertyValue::Int(v) => {
                digest.feed(6);
                digest.feed(*v as u32 as u64);
            }
            PropertyValue::IdGenerator(_) => digest.feed(7),
            PropertyValue::Str(s) => {
                digest.feed(8);
                digest.feed_units(s.as_units());
            }
            PropertyValue::StrList(list) => {
                digest.feed(9);
                digest.feed(list.len() as u64);
                for s in list {
                    digest.feed_units(s.as_units());
                }
            }
            PropertyValue::StrMap(map) => {
                digest.feed(10);
                digest.feed(map.len() as u64);
                for (k, s) in map {
                    digest.feed_units(k.as_units());
                    digest.feed_units(s.as_units());
                }
            }
            PropertyValue::Suppressions(_) => digest.feed(11),
            PropertyValue::StrSet(set) => {
                digest.feed(12);
                digest.feed(set.len() as u64);
                for s in set {
                    digest.feed_units(s.as_units());
                }
            }
            PropertyValue::Markers(markers) => {
                digest.feed(13);
                digest.feed(markers.len() as u64);
            }
            PropertyValue::ClosureUnawareMode(_) => digest.feed(14),
        };
        match &self.property_values {
            PackedPropertyValues::Empty => {}
            PackedPropertyValues::Single(v) => value(v, digest),
            PackedPropertyValues::Multiple(values) => {
                for v in values {
                    value(v, digest);
                }
            }
        }
    }
}
