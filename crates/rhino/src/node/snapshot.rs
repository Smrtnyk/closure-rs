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
    Ast, NodeCold, NodeData, NodeId, NodeKind, NodeLinks, ObjectProp, PresenceSummary, Prop,
    PropListItem, PropValue, TemplateLiteralStrings,
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
use std::sync::Arc;

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
        for ((data, &links), cold) in ast.nodes.iter().zip(&ast.links).zip(&ast.cold) {
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
            nodes.push(Node {
                links,
                lineno_charno: cold.lineno_charno,
                length: cold.length,
                original_name: cold
                    .original_name
                    .map(|id| enc.string(ast.original_name(id))),
                kind,
                head: match &data.prop_list_head {
                    Some(head) => Some(enc.property(head)?),
                    None => None,
                },
            });
        }
        Some(Self {
            root,
            bound_first_use: ast.preparse_bound_first_use,
            presence: ast.presence,
            strings: enc
                .strings
                .into_values()
                .map(|s| (s.is_interned(), s.as_units().to_vec()))
                .collect(),
            features: enc.features.into_values().collect(),
            expression_roots: enc.expressions.into_values().collect(),
            docs: enc.docs.into_values().collect(),
            properties: enc.properties.into_values().collect(),
            nodes,
        })
    }

    /// Instantiate in the destination arena at the point where the compiler asks for this file.
    pub fn append(self, ast: &mut Ast, source: Arc<dyn StaticSourceFile>) -> NodeId {
        let strings: Vec<_> = self
            .strings
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
        let string = |id: usize| strings[id].clone();
        ast.presence.merge(&self.presence);
        // The bundled parses normally carry no original names. Collect only the populated
        // slots so they can be installed with their final node ids after the append.
        let mut original_names = Vec::new();
        let rows = self.nodes.into_iter().enumerate().map(|(i, node)| {
            if let Some(name) = node.original_name {
                original_names.push((i, name));
            }
            let kind = match node.kind {
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
            };
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
        let expressions: Vec<_> = self
            .expression_roots
            .into_iter()
            .map(|root| Arc::new(JSTypeExpression::new(map.map(root), source.get_name())))
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
                let values = doc
                    .values
                    .into_iter()
                    .map(|v| match v {
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
                                .map(|(keys, s)| {
                                    (keys.into_iter().map(string).collect(), string(s))
                                })
                                .collect(),
                        ),
                        DocValue::StringSet(v) => {
                            PropertyValue::StrSet(v.into_iter().map(string).collect())
                        }
                        DocValue::ClosureUnawareMode(m) => PropertyValue::ClosureUnawareMode(m),
                    })
                    .collect();
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
        for (i, head) in heads.into_iter().enumerate() {
            if let Some(index) = head {
                ast.set_props(map.node_at(i), Some(properties[index].clone()));
            }
        }
        map.map(self.root)
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
