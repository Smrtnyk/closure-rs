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

//! Build-time snapshot of one parsed, diagnostic-free extern file. Local node ids include
//! detached JSDoc type trees. Indexed tables preserve sharing of property lists, JSDoc,
//! expressions and strings. Only parser-produced values can be captured.

use super::{
    Ast, NodeCold, NodeData, NodeId, NodeKind, NodeLinks, ObjectProp, PreparsedIdMap,
    PresenceSummary, Prop, PropListItem, PropValue, TemplateLiteralStrings,
};
use crate::{
    fast_hash::IndexMap,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{IdGenerator, JSDocInfo, PerFileClosureUnawareMode, PropertyValue, Visibility},
    rhino_string_pool::RhinoStringPool,
    static_source_file::StaticSourceFile,
};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroU32, sync::Arc};

#[derive(Serialize, Deserialize)]
pub struct AstSnapshot<F> {
    root: NodeId,
    bound_first_use: Option<u32>,
    presence: PresenceSummary,
    strings: Vec<(bool, Vec<u16>)>,
    features: Vec<F>,
    expression_roots: Vec<NodeId>,
    docs: Vec<Doc>,
    properties: Vec<Property>,
    nodes: Vec<Node>,
}

#[derive(Serialize, Deserialize)]
struct Node {
    links: NodeLinks,
    lineno_charno: i32,
    length: i32,
    original_name: Option<usize>,
    kind: Kind,
    head: Option<usize>,
}

#[derive(Serialize, Deserialize)]
enum Kind {
    Plain,
    Number(f64),
    BigInt(Vec<u8>),
    String(usize),
    Template { raw: usize, cooked: Option<usize> },
}

/// The built-in externs fit in 16-bit local node, string and property indices. Keep their
/// rows at a fixed stride so the target can read each one directly from the embedded bytes.
/// Zero represents an absent link, property or string. Non-string payloads and original
/// names are sparse. Larger inputs retain the ordinary snapshot encoding.
#[derive(Serialize, Deserialize)]
pub struct BundledAstSnapshot<F, B> {
    arena: AstSnapshot<F>,
    rows: Option<PackedRows<B>>,
}

#[derive(Serialize, Deserialize)]
struct PackedRows<B> {
    bytes: B,
    string_bytes: B,
    string_count: usize,
    tokens: Vec<crate::token::Token>,
    other_kinds: Vec<Kind>,
    original_names: Vec<(usize, usize)>,
}

const ROW_BYTES: usize = 21;
const OTHER_KIND: u16 = u16::MAX;

impl Kind {
    fn restore(self, strings: &[JsString]) -> NodeKind {
        let string = |id: usize| strings[id].clone();
        match self {
            Kind::Plain => NodeKind::Node,
            Kind::Number(number) => NodeKind::Number { number },
            Kind::BigInt(bytes) => NodeKind::BigInt {
                bigint: Arc::new(BigInt::from_signed_bytes_le(&bytes)),
            },
            Kind::String(id) => NodeKind::String { str: string(id) },
            Kind::Template { raw, cooked } => {
                NodeKind::TemplateLiteralSubstring(Box::new(TemplateLiteralStrings {
                    raw: string(raw),
                    cooked: cooked.map(string),
                }))
            }
        }
    }
}

impl<F: super::OpaqueProp + Clone> BundledAstSnapshot<F, &[u8]> {
    pub fn append(self, ast: &mut Ast, source: Arc<dyn StaticSourceFile>) -> NodeId {
        match self.rows {
            Some(rows) => {
                let strings = rows.strings();
                if self.arena.bound_first_use.is_some() && ast.implicit_template_bound.is_none() {
                    // This parse introduces the singleton in the middle of its local node IDs.
                    self.arena.append_rows(ast, source, rows.nodes(), strings)
                } else {
                    rows.append(ast, self.arena, source, strings)
                }
            }
            None => self.arena.append(ast, source),
        }
    }
}

impl PackedRows<&[u8]> {
    /// Decode code units into reusable scratch. Interned entries hit the pool by content
    /// without first allocating a temporary Java string. Non-interned entries remain
    /// separate allocations even if their code units are equal.
    fn strings(&self) -> Vec<JsString> {
        let mut bytes = self.string_bytes;
        let mut units = Vec::new();
        let mut strings = Vec::with_capacity(self.string_count);
        while let Some((header, rest)) = bytes.split_first_chunk::<4>() {
            let header = u32::from_le_bytes(*header);
            let len = (header >> 2) as usize;
            let narrow = header & 2 != 0;
            let (data, rest) = rest.split_at(if narrow { len } else { len * 2 });
            bytes = rest;
            units.clear();
            if narrow {
                units.extend(data.iter().copied().map(u16::from));
            } else {
                units.extend(
                    data.as_chunks::<2>()
                        .0
                        .iter()
                        .copied()
                        .map(u16::from_le_bytes),
                );
            }
            strings.push(if header & 1 != 0 {
                RhinoStringPool::add_or_get_units(&units)
            } else {
                JsString::from_slice(&units)
            });
        }
        assert!(bytes.is_empty() && strings.len() == self.string_count);
        strings
    }

    /// With no implicit bound to insert, each local link has the same relocation. Fill the
    /// arena columns in place and restore each shared property list's summary once.
    fn append<F: super::OpaqueProp + Clone>(
        self,
        ast: &mut Ast,
        arena: AstSnapshot<F>,
        source: Arc<dyn StaticSourceFile>,
        strings: Vec<JsString>,
    ) -> NodeId {
        let (rows, remainder) = self.bytes.as_chunks::<ROW_BYTES>();
        assert!(remainder.is_empty(), "incomplete bundled extern row");
        let base = ast.node_count();
        base.checked_add(u32::try_from(rows.len()).unwrap())
            .expect("Node arena capacity exceeded");
        let map = PreparsedIdMap {
            base,
            bound_at: None,
        };
        let root = map.map(arena.root);
        ast.presence.merge(&arena.presence);
        let source_name = source.get_name().to_owned();
        let lists = arena.property_lists(ast, source, map, &strings);
        // Packed property indices are one-based. Slot zero describes nodes without properties.
        let mut properties = Vec::with_capacity(lists.len() + 1);
        properties.push((None, 0u64, None, 0i32, 0u8));
        properties.extend(
            lists
                .into_iter()
                .map(|head| super::split_inline_from_list(Some(head))),
        );
        let has_any_ct = properties.iter().any(|p| p.3 != 0);
        let has_any_se = properties.iter().any(|p| p.4 != 0);
        let word = |row: &[u8; ROW_BYTES], at| u16::from_le_bytes([row[at], row[at + 1]]);
        let link =
            |local| NonZeroU32::new(u32::from(local)).map(|n| NodeId(n.checked_add(base).unwrap()));
        ast.links.extend(rows.iter().map(|row| NodeLinks {
            subtree_size: 0,
            token: self.tokens[usize::from(row[20])],
            parent: link(word(row, 0)),
            first: link(word(row, 2)),
            next: link(word(row, 4)),
            previous: link(word(row, 6)),
        }));
        ast.cold.extend(rows.iter().map(|row| NodeCold {
            lineno_charno: i32::from_le_bytes(row[8..12].try_into().unwrap()),
            length: i32::from_le_bytes(row[12..16].try_into().unwrap()),
            original_name: None,
        }));
        let mut other_kinds = self.other_kinds.into_iter();
        ast.nodes.extend(rows.iter().map(|row| {
            let kind = match word(row, 18) {
                0 => NodeKind::Node,
                OTHER_KIND => other_kinds
                    .next()
                    .expect("bundled extern payload")
                    .restore(&strings),
                id => NodeKind::String {
                    str: strings[usize::from(id - 1)].clone(),
                },
            };
            NodeData {
                kind,
                prop_list_head: properties[usize::from(word(row, 16))].0.clone(),
            }
        }));
        ast.prop_masks.extend(
            rows.iter()
                .map(|row| properties[usize::from(word(row, 16))].1),
        );
        ast.jsdocs.extend(rows.iter().map(|row| {
            let doc = &properties[usize::from(word(row, 16))].2;
            if let Some(info) = doc {
                // Raw public NodeData replacement can transplant a property list from
                // another arena without adding its flags to the source presence summary.
                ast.presence.note_jsdoc(info);
            }
            doc.clone()
        }));
        for (i, name) in self.original_names {
            NodeId::store_original_name(ast, map.node_at(i), Some(strings[name].clone()));
        }
        if has_any_ct || has_any_se {
            for (i, row) in rows.iter().enumerate() {
                let p = &properties[usize::from(word(row, 16))];
                if p.3 != 0 {
                    ast.set_inline_change_time(map.node_at(i).arena_index(), p.3);
                }
                if p.4 != 0 {
                    ast.set_inline_side_effect_flags(map.node_at(i).arena_index(), p.4);
                }
            }
        }
        // Rust-only (performance divergence): what the nodes were loaded with, for
        // `Ast::prop_digest_range_since_load`.
        ast.snapshot_watches.pending_loaded = Some(Box::new(super::snapshot_watch::LoadedProps {
            start: base,
            lists: properties
                .iter()
                .map(|(list, mask, ..)| (list.clone(), *mask))
                .collect(),
            index: rows.iter().map(|row| word(row, 16)).collect(),
            source_name,
        }));
        root
    }

    fn nodes(self) -> impl ExactSizeIterator<Item = Node> {
        let (rows, remainder) = self.bytes.as_chunks::<ROW_BYTES>();
        assert!(remainder.is_empty(), "incomplete bundled extern row");
        let mut kinds = self.other_kinds.into_iter();
        let mut names = self.original_names.into_iter().peekable();
        rows.iter().enumerate().map(move |(i, row)| {
            let word = |at| u16::from_le_bytes([row[at], row[at + 1]]);
            let link = |at| NonZeroU32::new(u32::from(word(at))).map(NodeId);
            let kind = match word(18) {
                0 => Kind::Plain,
                OTHER_KIND => kinds.next().expect("bundled extern payload"),
                id => Kind::String(usize::from(id - 1)),
            };
            Node {
                links: NodeLinks {
                    subtree_size: 0,
                    token: self.tokens[usize::from(row[20])],
                    parent: link(0),
                    first: link(2),
                    next: link(4),
                    previous: link(6),
                },
                lineno_charno: i32::from_le_bytes(row[8..12].try_into().unwrap()),
                length: i32::from_le_bytes(row[12..16].try_into().unwrap()),
                head: word(16).checked_sub(1).map(usize::from),
                kind,
                original_name: names
                    .next_if(|(index, _)| *index == i)
                    .map(|(_, name)| name),
            }
        })
    }
}

#[derive(Serialize, Deserialize)]
struct Property {
    key: u8,
    next: Option<usize>,
    value: Value,
}

#[derive(Serialize, Deserialize)]
enum Value {
    Int(i32),
    Source,
    Features(usize),
    Doc(usize),
    Node(NodeId),
}

#[derive(Serialize, Deserialize)]
struct Doc {
    bits: u64,
    keys: u64,
    values: Vec<DocValue>,
}

// Expression zero is Java's implicit template bound. Other expression ids are one-based.
#[derive(Serialize, Deserialize)]
enum DocValue {
    Null,
    Visibility(Visibility),
    Type(Option<usize>),
    Types(Vec<usize>),
    TypeMap(Vec<(usize, Option<usize>)>),
    NodeMap(Vec<(usize, NodeId)>),
    Int(i32),
    IdGenerator(IdGenerator),
    String(usize),
    Strings(Vec<usize>),
    StringMap(Vec<(usize, usize)>),
    Suppressions(Vec<(Vec<usize>, usize)>),
    StringSet(Vec<usize>),
    ClosureUnawareMode(PerFileClosureUnawareMode),
}

struct Encoder<'a, F> {
    source: &'a Arc<dyn StaticSourceFile>,
    features: IndexMap<usize, F>,
    bound: &'a Arc<JSTypeExpression>,
    strings: IndexMap<(bool, *const u16, usize), JsString>,
    expressions: IndexMap<usize, NodeId>,
    docs: IndexMap<usize, Doc>,
    properties: IndexMap<usize, Property>,
}

impl<F: super::OpaqueProp + Clone> AstSnapshot<F> {
    /// Capture only a parse whose object graph can be faithfully represented. Files with other
    /// property kinds continue to use the source parser when the bundled externs change.
    pub fn capture(ast: &Ast, root: NodeId, source: &Arc<dyn StaticSourceFile>) -> Option<Self> {
        if !ast.preparse || ast.types.iter().any(Option::is_some) {
            return None;
        }
        let mut enc = Encoder {
            source,
            features: IndexMap::default(),
            bound: ast.implicit_template_bound.as_ref()?,
            strings: IndexMap::default(),
            expressions: IndexMap::default(),
            docs: IndexMap::default(),
            properties: IndexMap::default(),
        };
        let mut nodes = Vec::with_capacity(ast.nodes.len());
        // Keep any newly materialized inline-property heads alive for the entire capture so
        // `Encoder::properties` (keyed by `Arc::as_ptr`) never observes reused heap addresses.
        let mut kept_materialized = Vec::new();
        for (idx, ((data, &links), cold)) in
            ast.nodes.iter().zip(&ast.links).zip(&ast.cold).enumerate()
        {
            let kind = match &data.kind {
                NodeKind::Node => Kind::Plain,
                NodeKind::Number { number } => Kind::Number(*number),
                NodeKind::BigInt { bigint } => Kind::BigInt(bigint.to_signed_bytes_le()),
                NodeKind::String { str } => Kind::String(enc.string(str)),
                NodeKind::TemplateLiteralSubstring(s) => Kind::Template {
                    raw: enc.string(&s.raw),
                    cooked: s.cooked.as_ref().map(|v| enc.string(v)),
                },
            };
            let node_id = NodeId(NonZeroU32::new(idx as u32 + 1).unwrap());
            let full_head = ast.materialize_full_prop_list(node_id);
            let head = match full_head {
                Some(head) => {
                    let id = enc.property(&head)?;
                    if data
                        .prop_list_head
                        .as_ref()
                        .is_none_or(|orig| !Arc::ptr_eq(&head, orig))
                    {
                        kept_materialized.push(head);
                    }
                    Some(id)
                }
                None => None,
            };
            nodes.push(Node {
                links,
                lineno_charno: cold.lineno_charno,
                length: cold.length,
                original_name: cold
                    .original_name
                    .map(|id| enc.string(ast.original_name(id))),
                kind,
                head,
            });
        }
        Some(Self {
            root,
            bound_first_use: ast.preparse_bound_first_use,
            presence: ast.presence,
            strings: enc
                .strings
                .into_values()
                .map(|s| (RhinoStringPool::is_pool_entry(&s), s.as_units().to_vec()))
                .collect(),
            features: enc.features.into_values().collect(),
            expression_roots: enc.expressions.into_values().collect(),
            docs: enc.docs.into_values().collect(),
            properties: enc.properties.into_values().collect(),
            nodes,
        })
    }

    /// Use the compact row and string representation when all local indices fit. Larger
    /// files still carry their complete captured representation.
    pub fn for_bundle(mut self) -> BundledAstSnapshot<F, Vec<u8>> {
        let rows = self.take_packed_rows();
        BundledAstSnapshot { arena: self, rows }
    }

    fn take_packed_rows(&mut self) -> Option<PackedRows<Vec<u8>>> {
        if self.nodes.len() > u16::MAX as usize
            || self.strings.len() >= OTHER_KIND as usize
            || self.properties.len() > u16::MAX as usize
            || self
                .strings
                .iter()
                .any(|(_, units)| units.len() > (u32::MAX >> 2) as usize)
        {
            return None;
        }
        let mut bytes = Vec::with_capacity(self.nodes.len() * ROW_BYTES);
        let string_count = self.strings.len();
        let mut string_bytes = Vec::new();
        for (interned, units) in self.strings.drain(..) {
            let narrow = units.iter().all(|unit| *unit <= u16::from(u8::MAX));
            let header = (units.len() as u32) << 2 | (u32::from(narrow) << 1) | u32::from(interned);
            string_bytes.extend_from_slice(&header.to_le_bytes());
            if narrow {
                string_bytes.extend(units.into_iter().map(|unit| unit as u8));
            } else {
                string_bytes.extend(units.into_iter().flat_map(u16::to_le_bytes));
            }
        }
        let mut tokens = IndexMap::<_, _>::default();
        let mut other_kinds = Vec::new();
        let mut original_names = Vec::new();
        for (i, node) in self.nodes.drain(..).enumerate() {
            let link = node.links;
            for id in [link.parent, link.first, link.next, link.previous] {
                bytes.extend_from_slice(&(id.map_or(0, |n| n.0.get()) as u16).to_le_bytes());
            }
            bytes.extend_from_slice(&node.lineno_charno.to_le_bytes());
            bytes.extend_from_slice(&node.length.to_le_bytes());
            bytes.extend_from_slice(&(node.head.map_or(0, |i| i + 1) as u16).to_le_bytes());
            let kind = match node.kind {
                Kind::Plain => 0,
                Kind::String(i) => (i + 1) as u16,
                kind => {
                    other_kinds.push(kind);
                    OTHER_KIND
                }
            };
            bytes.extend_from_slice(&kind.to_le_bytes());
            let entry = tokens.entry(link.token);
            bytes.push(u8::try_from(entry.index()).expect("number of Rhino tokens"));
            entry.or_insert(());
            if let Some(name) = node.original_name {
                original_names.push((i, name));
            }
        }
        Some(PackedRows {
            bytes,
            string_bytes,
            string_count,
            tokens: tokens.into_keys().collect(),
            other_kinds,
            original_names,
        })
    }

    /// Instantiate in the destination arena at the point where the compiler asks for this file.
    pub fn append(mut self, ast: &mut Ast, source: Arc<dyn StaticSourceFile>) -> NodeId {
        let nodes = std::mem::take(&mut self.nodes);
        let strings = std::mem::take(&mut self.strings)
            .into_iter()
            .map(|(interned, units)| {
                let s = JsString::from_units(units);
                if interned {
                    RhinoStringPool::add_or_get(s)
                } else {
                    s
                }
            })
            .collect();
        self.append_rows(ast, source, nodes.into_iter(), strings)
    }

    fn append_rows(
        self,
        ast: &mut Ast,
        source: Arc<dyn StaticSourceFile>,
        nodes: impl ExactSizeIterator<Item = Node>,
        strings: Vec<JsString>,
    ) -> NodeId {
        let string = |id: usize| strings[id].clone();
        ast.presence.merge(&self.presence);
        // The bundled parses normally carry no original names. Collect only the populated
        // slots so they can be installed with their final node ids after the append.
        let mut original_names = Vec::new();
        let rows = nodes.enumerate().map(|(i, node)| {
            if let Some(name) = node.original_name {
                original_names.push((i, name));
            }
            let kind = node.kind.restore(&strings);
            (
                NodeData {
                    kind,
                    prop_list_head: None,
                },
                node.links,
                NodeCold {
                    lineno_charno: node.lineno_charno,
                    length: node.length,
                    original_name: None,
                },
                node.head,
            )
        });
        let (map, heads) = ast.append_preparsed_nodes(self.bound_first_use, rows);
        for (i, name) in original_names {
            NodeId::store_original_name(ast, map.node_at(i), Some(string(name)));
        }
        let root = map.map(self.root);
        let properties: Vec<_> = self
            .property_lists(ast, source, map, &strings)
            .into_iter()
            .map(|head| super::split_inline_from_list(Some(head)))
            .collect();
        for (i, head) in heads.into_iter().enumerate() {
            if let Some(index) = head {
                let (ref list, mask, ref doc, ct, se) = properties[index];
                let dst = map.node_at(i);
                let slot = dst.arena_index();
                if let Some(info) = doc {
                    ast.presence.note_jsdoc(info);
                }
                ast.nodes[slot].prop_list_head = list.clone();
                ast.prop_masks[slot] = mask;
                ast.jsdocs[slot] = doc.clone();
                if ct != 0 {
                    ast.set_inline_change_time(slot, ct);
                }
                if se != 0 {
                    ast.set_inline_side_effect_flags(slot, se);
                }
            }
        }
        root
    }
    fn property_lists(
        self,
        ast: &Ast,
        source: Arc<dyn StaticSourceFile>,
        map: PreparsedIdMap,
        strings: &[JsString],
    ) -> Vec<Arc<PropListItem>> {
        let string = |id: usize| strings[id].clone();
        let mut expression_source: Option<Arc<str>> = None;
        let expressions: Vec<_> = self
            .expression_roots
            .into_iter()
            .map(|root| {
                let name = expression_source.get_or_insert_with(|| source.get_name().into());
                Arc::new(JSTypeExpression::with_shared_source(
                    map.map(root),
                    name.clone(),
                ))
            })
            .collect();
        let expr = |id: usize| match id.checked_sub(1) {
            Some(i) => expressions[i].clone(),
            None => ast
                .implicit_template_bound
                .clone()
                .expect("parsed template bound"),
        };
        let docs: Vec<_> = self
            .docs
            .into_iter()
            .map(|doc| {
                let values = doc.values.into_iter().map(|v| match v {
                    DocValue::Null => PropertyValue::Null,
                    DocValue::Visibility(v) => PropertyValue::Visibility(v),
                    DocValue::Type(e) => PropertyValue::TypeExpr(e.map(expr)),
                    DocValue::Types(v) => {
                        PropertyValue::TypeList(v.into_iter().map(expr).collect())
                    }
                    DocValue::TypeMap(v) => PropertyValue::TypeMap(
                        v.into_iter()
                            .map(|(k, e)| (string(k), e.map(expr)))
                            .collect(),
                    ),
                    DocValue::NodeMap(v) => PropertyValue::NodeMap(
                        v.into_iter()
                            .map(|(k, n)| (string(k), map.map(n)))
                            .collect(),
                    ),
                    DocValue::Int(n) => PropertyValue::Int(n),
                    DocValue::IdGenerator(g) => PropertyValue::IdGenerator(g),
                    DocValue::String(s) => PropertyValue::Str(string(s)),
                    DocValue::Strings(v) => {
                        PropertyValue::StrList(v.into_iter().map(string).collect())
                    }
                    DocValue::StringMap(v) => PropertyValue::StrMap(
                        v.into_iter().map(|(k, s)| (string(k), string(s))).collect(),
                    ),
                    DocValue::Suppressions(v) => PropertyValue::Suppressions(
                        v.into_iter()
                            .map(|(keys, s)| (keys.into_iter().map(string).collect(), string(s)))
                            .collect(),
                    ),
                    DocValue::StringSet(v) => {
                        PropertyValue::StrSet(v.into_iter().map(string).collect())
                    }
                    DocValue::ClosureUnawareMode(m) => PropertyValue::ClosureUnawareMode(m),
                });
                Arc::new(JSDocInfo::from_snapshot_properties(
                    doc.bits, doc.keys, values,
                ))
            })
            .collect();
        let features: Vec<_> = self
            .features
            .into_iter()
            .map(|f| ObjectProp::Opaque(Arc::new(f)))
            .collect();
        let mut properties: Vec<Arc<PropListItem>> = Vec::with_capacity(self.properties.len());
        for prop in self.properties {
            let value = match prop.value {
                Value::Int(n) => PropValue::Int(n),
                Value::Source => PropValue::Object(ObjectProp::StaticSourceFile(source.clone())),
                Value::Features(i) => PropValue::Object(features[i].clone()),
                Value::Doc(i) => PropValue::Object(ObjectProp::JSDocInfo(docs[i].clone())),
                Value::Node(n) => PropValue::Object(ObjectProp::Node(map.map(n))),
            };
            let next = prop.next.map(|i| properties[i].clone());
            properties.push(Arc::new(PropListItem {
                prop_type: prop.key,
                value,
                next,
            }));
        }
        properties
    }
}

impl<F: super::OpaqueProp + Clone> Encoder<'_, F> {
    fn string(&mut self, s: &JsString) -> usize {
        let units = s.as_units();
        let entry = self
            .strings
            .entry((s.is_interned(), units.as_ptr(), units.len()));
        let index = entry.index();
        entry.or_insert_with(|| s.clone());
        index
    }

    fn expression(&mut self, e: &Arc<JSTypeExpression>) -> Option<usize> {
        if Arc::ptr_eq(e, self.bound) {
            return Some(0);
        }
        if e.get_source_name() != self.source.get_name() {
            return None;
        }
        let key = Arc::as_ptr(e) as usize;
        let entry = self.expressions.entry(key);
        let id = entry.index() + 1;
        entry.or_insert(e.get_root());
        Some(id)
    }

    fn doc(&mut self, info: &Arc<JSDocInfo>) -> Option<usize> {
        let key = Arc::as_ptr(info) as usize;
        if let Some(id) = self.docs.get_index_of(&key) {
            return Some(id);
        }
        let (bits, keys, props) = info.snapshot_properties();
        let values = props
            .iter()
            .map(|v| self.doc_value(v))
            .collect::<Option<_>>()?;
        Some(self.docs.insert_full(key, Doc { bits, keys, values }).0)
    }

    fn optional_expr(&mut self, e: &Option<Arc<JSTypeExpression>>) -> Option<Option<usize>> {
        match e {
            Some(e) => Some(Some(self.expression(e)?)),
            None => Some(None),
        }
    }

    fn doc_value(&mut self, v: &PropertyValue) -> Option<DocValue> {
        Some(match v {
            PropertyValue::Null => DocValue::Null,
            PropertyValue::Visibility(v) => DocValue::Visibility(*v),
            PropertyValue::TypeExpr(e) => DocValue::Type(self.optional_expr(e)?),
            PropertyValue::TypeList(v) => DocValue::Types(
                v.iter()
                    .map(|e| self.expression(e))
                    .collect::<Option<_>>()?,
            ),
            PropertyValue::TypeMap(v) => DocValue::TypeMap(
                v.iter()
                    .map(|(s, e)| Some((self.string(s), self.optional_expr(e)?)))
                    .collect::<Option<_>>()?,
            ),
            PropertyValue::NodeMap(v) => {
                DocValue::NodeMap(v.iter().map(|(s, n)| (self.string(s), *n)).collect())
            }
            PropertyValue::Int(n) => DocValue::Int(*n),
            PropertyValue::IdGenerator(g) => DocValue::IdGenerator(*g),
            PropertyValue::Str(s) => DocValue::String(self.string(s)),
            PropertyValue::StrList(v) => {
                DocValue::Strings(v.iter().map(|s| self.string(s)).collect())
            }
            PropertyValue::StrMap(v) => DocValue::StringMap(
                v.iter()
                    .map(|(k, s)| (self.string(k), self.string(s)))
                    .collect(),
            ),
            PropertyValue::Suppressions(v) => DocValue::Suppressions(
                v.iter()
                    .map(|(keys, s)| {
                        (
                            keys.iter().map(|k| self.string(k)).collect(),
                            self.string(s),
                        )
                    })
                    .collect(),
            ),
            PropertyValue::StrSet(v) => {
                DocValue::StringSet(v.iter().map(|s| self.string(s)).collect())
            }
            PropertyValue::ClosureUnawareMode(m) => DocValue::ClosureUnawareMode(*m),
            PropertyValue::Markers(_) => return None,
        })
    }

    fn property(&mut self, prop: &Arc<PropListItem>) -> Option<usize> {
        let key = Arc::as_ptr(prop) as usize;
        if let Some(id) = self.properties.get_index_of(&key) {
            return Some(id);
        }
        let next = match &prop.next {
            Some(next) => Some(self.property(next)?),
            None => None,
        };
        let value = match &prop.value {
            PropValue::Int(v) => Value::Int(*v),
            PropValue::Object(ObjectProp::StaticSourceFile(f)) if Arc::ptr_eq(f, self.source) => {
                Value::Source
            }
            PropValue::Object(ObjectProp::Opaque(obj))
                if prop.prop_type == Prop::FEATURE_SET as u8 =>
            {
                let value = obj.as_ref().as_any().downcast_ref::<F>()?;
                let entry = self.features.entry(Arc::as_ptr(obj) as *const () as usize);
                let index = entry.index();
                entry.or_insert_with(|| value.clone());
                Value::Features(index)
            }
            PropValue::Object(ObjectProp::JSDocInfo(doc)) => Value::Doc(self.doc(doc)?),
            PropValue::Object(ObjectProp::Node(n)) => Value::Node(*n),
            _ => return None,
        };
        Some(
            self.properties
                .insert_full(
                    key,
                    Property {
                        key: prop.prop_type,
                        next,
                        value,
                    },
                )
                .0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        simple_source_file::SimpleSourceFile, static_source_file::SourceKind, token::Token,
    };

    #[test]
    fn packed_indices_stop_before_a_string_or_link_would_wrap() {
        let source: Arc<dyn StaticSourceFile> =
            Arc::new(SimpleSourceFile::new("test", SourceKind::EXTERN));
        let mut ast = Ast::new_for_preparse();
        let root = ast.new_node(Token::BLOCK);
        let empty = || Node {
            links: NodeLinks {
                subtree_size: 0,
                token: Token::EMPTY,
                parent: None,
                first: None,
                next: None,
                previous: None,
            },
            lineno_charno: -1,
            length: 0,
            original_name: None,
            kind: Kind::Plain,
            head: None,
        };
        let mut snapshot = AstSnapshot::<String>::capture(&ast, root, &source).unwrap();
        snapshot.nodes.resize_with(u16::MAX as usize + 1, empty);
        assert!(snapshot.take_packed_rows().is_none());
        snapshot.nodes.pop();
        snapshot
            .strings
            .resize_with(OTHER_KIND as usize, || (false, Vec::new()));
        assert!(snapshot.take_packed_rows().is_none());
        snapshot.strings.pop();
        let max_id = NodeId(NonZeroU32::new(u32::from(u16::MAX)).unwrap());
        snapshot.nodes[0].links.first = Some(max_id);
        snapshot.nodes[0].kind = Kind::String(snapshot.strings.len() - 1);
        let packed = snapshot.take_packed_rows().unwrap();
        let packed = PackedRows {
            bytes: packed.bytes.as_slice(),
            string_bytes: packed.string_bytes.as_slice(),
            string_count: packed.string_count,
            tokens: packed.tokens,
            other_kinds: packed.other_kinds,
            original_names: packed.original_names,
        };
        let first = packed.nodes().next().unwrap();
        assert_eq!(first.links.first, Some(max_id));
        assert!(matches!(first.kind, Kind::String(i) if i == OTHER_KIND as usize - 2));
    }
}
