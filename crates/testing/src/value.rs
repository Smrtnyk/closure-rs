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

//! FORMAT.md "Value encoding", "Neutral encodings" and "Referenceable values": the tagged value
//! dumps found in `options`, `testFields`, `testFieldsAfter`, `harness.fields`, the processor
//! dump, `expected.postconditionValues` and `postCall`.

use crate::json::{JsString, JsonNumber, JsonValue};
use crate::reader::{
    ModelResult, Obj, ObjOut, arr, as_array, as_bool, as_i32, as_js_string, as_opt_string,
    as_string, err, index, int, join, js, list_of, map_of, opt_st, st, strs,
};
use closure_rhino::fx_hash::IndexMap;

/// A reflective field dump: field name (FORMAT.md "Value encoding": inherited fields prefixed
/// with the declaring class's simple name) to value, in recorded order.
pub type FieldDump = IndexMap<String, Value>;

/// FORMAT.md inputs: `SourceFile` `{name, code, kind}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub name: String,
    pub code: JsString,
    pub kind: SourceKind,
}

/// `SourceKind` (`com.google.javascript.jscomp.SourceFile.SourceKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum SourceKind {
    STRONG,
    WEAK,
    EXTERN,
    NON_CODE,
}

impl SourceKind {
    pub fn name(self) -> &'static str {
        match self {
            SourceKind::STRONG => "STRONG",
            SourceKind::WEAK => "WEAK",
            SourceKind::EXTERN => "EXTERN",
            SourceKind::NON_CODE => "NON_CODE",
        }
    }

    pub fn from_name(s: &str) -> Option<SourceKind> {
        Some(match s {
            "STRONG" => SourceKind::STRONG,
            "WEAK" => SourceKind::WEAK,
            "EXTERN" => SourceKind::EXTERN,
            "NON_CODE" => SourceKind::NON_CODE,
            _ => return None,
        })
    }
}

impl SourceFile {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<SourceFile> {
        let mut o = Obj::new(v, path)?;
        let name = o.req_string("name")?;
        let code = o.req_js_string("code")?;
        let kind_s = o.req_string("kind")?;
        let Some(kind) = SourceKind::from_name(&kind_s) else {
            return err(&o.sub("kind"), format!("unknown SourceKind {kind_s:?}"));
        };
        o.finish()?;
        Ok(SourceFile { name, code, kind })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("name", st(&self.name))
            .put("code", js(&self.code))
            .put("kind", st(self.kind.name()))
            .build()
    }

    pub fn list_from_json(v: &JsonValue, path: &str) -> ModelResult<Vec<SourceFile>> {
        list_of(v, path, SourceFile::from_json)
    }

    pub fn list_to_json(files: &[SourceFile]) -> JsonValue {
        arr(files, SourceFile::to_json)
    }
}

/// `{key, level}` of a `DiagnosticType` (`level` = its default level).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticTypeRef {
    pub key: String,
    pub level: String,
}

impl DiagnosticTypeRef {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<DiagnosticTypeRef> {
        let mut o = Obj::new(v, path)?;
        let key = o.req_string("key")?;
        let level = o.req_string("level")?;
        o.finish()?;
        Ok(DiagnosticTypeRef { key, level })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("key", st(&self.key))
            .put("level", st(&self.level))
            .build()
    }
}

/// `{group, types}` of a `DiagnosticGroup`: `group` = registered name or
/// `DiagnosticGroups.FIELD`, else null; `types` = the keys of its diagnostic types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticGroupRef {
    pub group: Option<String>,
    pub types: Vec<String>,
}

impl DiagnosticGroupRef {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<DiagnosticGroupRef> {
        let mut o = Obj::new(v, path)?;
        let p = o.sub("group");
        let group = as_opt_string(o.req("group")?, &p)?;
        let p = o.sub("types");
        let types = list_of(o.req("types")?, &p, as_string)?;
        o.finish()?;
        Ok(DiagnosticGroupRef { group, types })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("group", opt_st(&self.group))
            .put("types", strs(&self.types))
            .build()
    }
}

/// `{"typedScope": {rootToken, depth, typeOfThis, vars}}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedScope {
    pub root_token: String,
    pub depth: i32,
    pub type_of_this: Option<JsString>,
    /// `[name, JSType.toString() or null, isTypeInferred]` in the scope's iteration order.
    pub vars: Vec<TypedScopeVar>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypedScopeVar {
    pub name: JsString,
    pub type_string: Option<JsString>,
    pub is_type_inferred: bool,
}

impl TypedScope {
    fn from_json(v: &JsonValue, path: &str) -> ModelResult<TypedScope> {
        let mut o = Obj::new(v, path)?;
        let root_token = o.req_string("rootToken")?;
        let depth = o.req_i32("depth")?;
        let p = o.sub("typeOfThis");
        let type_of_this = crate::reader::as_opt_js_string(o.req("typeOfThis")?, &p)?;
        let p = o.sub("vars");
        let vars = list_of(o.req("vars")?, &p, |x, xp| {
            let a = as_array(x, xp)?;
            if a.len() != 3 {
                return err(xp, "expected [name, type, isTypeInferred]");
            }
            Ok(TypedScopeVar {
                name: as_js_string(&a[0], &index(xp, 0))?,
                type_string: crate::reader::as_opt_js_string(&a[1], &index(xp, 1))?,
                is_type_inferred: as_bool(&a[2], &index(xp, 2))?,
            })
        })?;
        o.finish()?;
        Ok(TypedScope {
            root_token,
            depth,
            type_of_this,
            vars,
        })
    }

    fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("rootToken", st(&self.root_token))
            .put("depth", int(self.depth))
            .put("typeOfThis", crate::reader::opt_js(&self.type_of_this))
            .put(
                "vars",
                arr(&self.vars, |v| {
                    JsonValue::Array(vec![
                        js(&v.name),
                        crate::reader::opt_js(&v.type_string),
                        JsonValue::Bool(v.is_type_inferred),
                    ])
                }),
            )
            .build()
    }
}

/// A protobuf message in the neutral encoding (FORMAT.md "Neutral encodings"):
/// `{"proto": <descriptor full name>, "fields": {<field name>: value}}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtoMessage {
    pub name: String,
    pub fields: IndexMap<String, ProtoValue>,
}

/// A protobuf field value (FORMAT.md "Neutral encodings").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProtoValue {
    /// `int32`/`uint32`/`sint32`/`fixed32`.
    Number(JsonNumber),
    /// 64-bit integers: `{"long": "<decimal>"}`.
    Long(String),
    /// `float`/`double`: `{"double": Double.toString}`.
    Double(String),
    Bool(bool),
    /// `string`, an enum value's name, or a `StringPoolProto.strings` entry.
    String(JsString),
    /// `bytes`: `{"bytes": "<base64>"}`.
    Bytes(String),
    /// `LazyAst.script`: `{"astNode": <the bytes parsed as jscomp.AstNode>}`.
    AstNode(Box<ProtoMessage>),
    Message(Box<ProtoMessage>),
    /// A repeated field (map fields as their entry messages), in wire order.
    Repeated(Vec<ProtoValue>),
}

impl ProtoMessage {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<ProtoMessage> {
        let mut o = Obj::new(v, path)?;
        let name = o.req_string("proto")?;
        let p = o.sub("fields");
        let fields = map_of(o.req("fields")?, &p, ProtoValue::from_json)?;
        o.finish()?;
        Ok(ProtoMessage { name, fields })
    }

    pub fn to_json(&self) -> JsonValue {
        let mut f = ObjOut::new();
        for (k, v) in &self.fields {
            f.put(k, v.to_json());
        }
        ObjOut::new()
            .put("proto", st(&self.name))
            .put("fields", f.build())
            .build()
    }
}

impl ProtoValue {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<ProtoValue> {
        Ok(match v {
            JsonValue::Number(n) => ProtoValue::Number(n.clone()),
            JsonValue::Bool(b) => ProtoValue::Bool(*b),
            JsonValue::String(s) => ProtoValue::String(s.clone()),
            JsonValue::Array(_) => ProtoValue::Repeated(list_of(v, path, ProtoValue::from_json)?),
            JsonValue::Object(m) => {
                if m.contains_key("proto") {
                    ProtoValue::Message(Box::new(ProtoMessage::from_json(v, path)?))
                } else {
                    let mut o = Obj::new(v, path)?;
                    let r = if o.has("long") {
                        ProtoValue::Long(o.req_string("long")?)
                    } else if o.has("double") {
                        ProtoValue::Double(o.req_string("double")?)
                    } else if o.has("bytes") {
                        ProtoValue::Bytes(o.req_string("bytes")?)
                    } else if o.has("astNode") {
                        let p = o.sub("astNode");
                        ProtoValue::AstNode(Box::new(ProtoMessage::from_json(
                            o.req("astNode")?,
                            &p,
                        )?))
                    } else {
                        return err(path, "unknown protobuf value tag");
                    };
                    o.finish()?;
                    r
                }
            }
            JsonValue::Null => return err(path, "null protobuf value"),
        })
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            ProtoValue::Number(n) => JsonValue::Number(n.clone()),
            ProtoValue::Bool(b) => JsonValue::Bool(*b),
            ProtoValue::String(s) => js(s),
            ProtoValue::Long(s) => ObjOut::new().put("long", st(s)).build(),
            ProtoValue::Double(s) => ObjOut::new().put("double", st(s)).build(),
            ProtoValue::Bytes(s) => ObjOut::new().put("bytes", st(s)).build(),
            ProtoValue::AstNode(m) => ObjOut::new().put("astNode", m.to_json()).build(),
            ProtoValue::Message(m) => m.to_json(),
            ProtoValue::Repeated(items) => arr(items, ProtoValue::to_json),
        }
    }
}

/// One dumped value (FORMAT.md "Value encoding").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    /// A JSON number (`int`, `short`, `byte`).
    Int(JsonNumber),
    String(JsString),
    /// `{"long": "123"}`
    Long(String),
    /// `{"double": "1.5"}` (Java `Double.toString`; also `float`).
    Double(String),
    /// `{"char": "x"}` (one UTF-16 unit, which may be a lone surrogate).
    Char(JsString),
    /// `{"enum": FQCN, "name": N}`
    Enum {
        class: String,
        name: String,
    },
    /// `{"diagnosticType": {key, level}}`
    DiagnosticType(DiagnosticTypeRef),
    /// `{"diagnosticGroup": {group, types}}`
    DiagnosticGroup(DiagnosticGroupRef),
    /// `{"classRef": FQCN}`
    ClassRef(String),
    /// `{"regex": p, "flags": n}`
    Regex {
        pattern: JsString,
        flags: i32,
    },
    /// `{"sourceFile": SourceFile}`
    SourceFile(SourceFile),
    /// `{"optional": v}` (`java.util.Optional`; `None` when empty, written as null).
    Optional(Option<Box<Value>>),
    /// `{"list": [...], "impl": FQCN}`
    List {
        items: Vec<Value>,
        impl_class: String,
    },
    /// `{"set": [...], "impl": FQCN}`
    Set {
        items: Vec<Value>,
        impl_class: String,
    },
    /// `{"map": [[k, v], ...], "impl": FQCN}`
    Map {
        entries: Vec<(Value, Value)>,
        impl_class: String,
    },
    /// `{"arrayOf": FQCN, "items": [...]}`
    ArrayOf {
        class: String,
        items: Vec<Value>,
    },
    /// `{"composeWarningsGuard": {"guards": [...]}}` (guards in effective order).
    ComposeWarningsGuard {
        guards: Vec<Value>,
    },
    /// `{"ref": FQCN}` (+ `captures` for a lambda in referenceable mode).
    Ref {
        class: String,
        captures: Option<FieldDump>,
    },
    /// `{"object": FQCN, "fields": {...}}`
    Object {
        class: String,
        fields: FieldDump,
    },
    /// `{"unrepresentable": FQCN}` (`cycle:FQCN` for a cycle; + `captures` for a lambda).
    Unrepresentable {
        class: String,
        captures: Option<FieldDump>,
    },
    /// `{"typedScope": {...}}`
    TypedScope(TypedScope),
    /// `{"table": [[row, column, value], ...], "impl": FQCN}` (Guava `Table`).
    Table {
        cells: Vec<(Value, Value, Value)>,
        impl_class: String,
    },
    /// `{"proto": name, "fields": {...}}` (protobuf message or builder).
    Proto(ProtoMessage),
}

/// The tag keys of [`Value`], each with the companion keys it may carry.
const TAGS: &[&str] = &[
    "long",
    "double",
    "char",
    "enum",
    "diagnosticType",
    "diagnosticGroup",
    "classRef",
    "regex",
    "sourceFile",
    "optional",
    "list",
    "set",
    "map",
    "arrayOf",
    "composeWarningsGuard",
    "ref",
    "object",
    "unrepresentable",
    "typedScope",
    "table",
    "proto",
];

impl Value {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<Value> {
        let m = match v {
            JsonValue::Null => return Ok(Value::Null),
            JsonValue::Bool(b) => return Ok(Value::Bool(*b)),
            JsonValue::Number(n) => return Ok(Value::Int(n.clone())),
            JsonValue::String(s) => return Ok(Value::String(s.clone())),
            JsonValue::Array(_) => return err(path, "untagged array in a value dump"),
            JsonValue::Object(m) => m,
        };
        let present: Vec<&str> = TAGS
            .iter()
            .copied()
            .filter(|t| m.contains_key(*t))
            .collect();
        if present.len() != 1 {
            return err(
                path,
                format!(
                    "expected exactly one value tag, found {present:?} among keys {:?}",
                    m.keys().collect::<Vec<_>>()
                ),
            );
        }
        let tag = present[0];
        let mut o = Obj::new(v, path)?;
        let tp = o.sub(tag);
        let r = match tag {
            "long" => Value::Long(o.req_string("long")?),
            "double" => Value::Double(o.req_string("double")?),
            "char" => Value::Char(o.req_js_string("char")?),
            "enum" => Value::Enum {
                class: o.req_string("enum")?,
                name: o.req_string("name")?,
            },
            "diagnosticType" => {
                Value::DiagnosticType(DiagnosticTypeRef::from_json(o.req(tag)?, &tp)?)
            }
            "diagnosticGroup" => {
                Value::DiagnosticGroup(DiagnosticGroupRef::from_json(o.req(tag)?, &tp)?)
            }
            "classRef" => Value::ClassRef(o.req_string("classRef")?),
            "regex" => Value::Regex {
                pattern: o.req_js_string("regex")?,
                flags: o.req_i32("flags")?,
            },
            "sourceFile" => Value::SourceFile(SourceFile::from_json(o.req(tag)?, &tp)?),
            "optional" => {
                let x = o.req(tag)?;
                Value::Optional(match x {
                    JsonValue::Null => None,
                    x => Some(Box::new(Value::from_json(x, &join(&tp, "get"))?)),
                })
            }
            "list" | "set" => {
                let items = list_of(o.req(tag)?, &tp, Value::from_json)?;
                let impl_class = o.req_string("impl")?;
                if tag == "list" {
                    Value::List { items, impl_class }
                } else {
                    Value::Set { items, impl_class }
                }
            }
            "map" => {
                let entries = list_of(o.req(tag)?, &tp, |e, ep| {
                    let a = as_array(e, ep)?;
                    if a.len() != 2 {
                        return err(ep, "expected a [key, value] pair");
                    }
                    Ok((
                        Value::from_json(&a[0], &index(ep, 0))?,
                        Value::from_json(&a[1], &index(ep, 1))?,
                    ))
                })?;
                Value::Map {
                    entries,
                    impl_class: o.req_string("impl")?,
                }
            }
            "arrayOf" => {
                let class = o.req_string("arrayOf")?;
                let p = o.sub("items");
                let items = list_of(o.req("items")?, &p, Value::from_json)?;
                Value::ArrayOf { class, items }
            }
            "composeWarningsGuard" => {
                let mut g = Obj::new(o.req(tag)?, &tp)?;
                let p = g.sub("guards");
                let guards = list_of(g.req("guards")?, &p, Value::from_json)?;
                g.finish()?;
                Value::ComposeWarningsGuard { guards }
            }
            "ref" | "unrepresentable" => {
                let class = o.req_string(tag)?;
                let p = o.sub("captures");
                let captures = o
                    .opt("captures")
                    .map(|c| field_dump_from_json(c, &p))
                    .transpose()?;
                if tag == "ref" {
                    Value::Ref { class, captures }
                } else {
                    Value::Unrepresentable { class, captures }
                }
            }
            "object" => {
                let class = o.req_string("object")?;
                let p = o.sub("fields");
                let fields = field_dump_from_json(o.req("fields")?, &p)?;
                Value::Object { class, fields }
            }
            "typedScope" => Value::TypedScope(TypedScope::from_json(o.req(tag)?, &tp)?),
            "table" => {
                let cells = list_of(o.req(tag)?, &tp, |c, cp| {
                    let a = as_array(c, cp)?;
                    if a.len() != 3 {
                        return err(cp, "expected a [row, column, value] cell");
                    }
                    Ok((
                        Value::from_json(&a[0], &index(cp, 0))?,
                        Value::from_json(&a[1], &index(cp, 1))?,
                        Value::from_json(&a[2], &index(cp, 2))?,
                    ))
                })?;
                Value::Table {
                    cells,
                    impl_class: o.req_string("impl")?,
                }
            }
            "proto" => return Ok(Value::Proto(ProtoMessage::from_json(v, path)?)),
            _ => unreachable!("tag list and match agree"),
        };
        o.finish()?;
        Ok(r)
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            Value::Null => JsonValue::Null,
            Value::Bool(b) => JsonValue::Bool(*b),
            Value::Int(n) => JsonValue::Number(n.clone()),
            Value::String(s) => js(s),
            Value::Long(s) => ObjOut::new().put("long", st(s)).build(),
            Value::Double(s) => ObjOut::new().put("double", st(s)).build(),
            Value::Char(s) => ObjOut::new().put("char", js(s)).build(),
            Value::Enum { class, name } => ObjOut::new()
                .put("enum", st(class))
                .put("name", st(name))
                .build(),
            Value::DiagnosticType(d) => ObjOut::new().put("diagnosticType", d.to_json()).build(),
            Value::DiagnosticGroup(g) => ObjOut::new().put("diagnosticGroup", g.to_json()).build(),
            Value::ClassRef(c) => ObjOut::new().put("classRef", st(c)).build(),
            Value::Regex { pattern, flags } => ObjOut::new()
                .put("regex", js(pattern))
                .put("flags", int(*flags))
                .build(),
            Value::SourceFile(f) => ObjOut::new().put("sourceFile", f.to_json()).build(),
            Value::Optional(x) => ObjOut::new()
                .put(
                    "optional",
                    x.as_ref().map_or(JsonValue::Null, |x| x.to_json()),
                )
                .build(),
            Value::List { items, impl_class } => ObjOut::new()
                .put("list", arr(items, Value::to_json))
                .put("impl", st(impl_class))
                .build(),
            Value::Set { items, impl_class } => ObjOut::new()
                .put("set", arr(items, Value::to_json))
                .put("impl", st(impl_class))
                .build(),
            Value::Map {
                entries,
                impl_class,
            } => ObjOut::new()
                .put(
                    "map",
                    arr(entries, |(k, v)| {
                        JsonValue::Array(vec![k.to_json(), v.to_json()])
                    }),
                )
                .put("impl", st(impl_class))
                .build(),
            Value::ArrayOf { class, items } => ObjOut::new()
                .put("arrayOf", st(class))
                .put("items", arr(items, Value::to_json))
                .build(),
            Value::ComposeWarningsGuard { guards } => ObjOut::new()
                .put(
                    "composeWarningsGuard",
                    ObjOut::new()
                        .put("guards", arr(guards, Value::to_json))
                        .build(),
                )
                .build(),
            Value::Ref { class, captures } => ObjOut::new()
                .put("ref", st(class))
                .put_opt("captures", captures.as_ref().map(field_dump_to_json))
                .build(),
            Value::Object { class, fields } => ObjOut::new()
                .put("object", st(class))
                .put("fields", field_dump_to_json(fields))
                .build(),
            Value::Unrepresentable { class, captures } => ObjOut::new()
                .put("unrepresentable", st(class))
                .put_opt("captures", captures.as_ref().map(field_dump_to_json))
                .build(),
            Value::TypedScope(s) => ObjOut::new().put("typedScope", s.to_json()).build(),
            Value::Table { cells, impl_class } => ObjOut::new()
                .put(
                    "table",
                    arr(cells, |(r, c, v)| {
                        JsonValue::Array(vec![r.to_json(), c.to_json(), v.to_json()])
                    }),
                )
                .put("impl", st(impl_class))
                .build(),
            Value::Proto(m) => m.to_json(),
        }
    }

    /// The JSON-number value as an `i32`, if this is one.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::Int(n) => n.as_i32(),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The `long` value parsed, if this is one.
    pub fn as_long(&self) -> Option<i64> {
        match self {
            Value::Long(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// The `double` value parsed (Java `Double.toString` text, including `NaN`/`Infinity`).
    pub fn as_double(&self) -> Option<f64> {
        match self {
            Value::Double(s) => match s.as_str() {
                "NaN" => Some(f64::NAN),
                "Infinity" => Some(f64::INFINITY),
                "-Infinity" => Some(f64::NEG_INFINITY),
                s => s.parse().ok(),
            },
            _ => None,
        }
    }
}

pub fn field_dump_from_json(v: &JsonValue, path: &str) -> ModelResult<FieldDump> {
    map_of(v, path, Value::from_json)
}

pub fn field_dump_to_json(d: &FieldDump) -> JsonValue {
    let mut o = ObjOut::new();
    for (k, v) in d {
        o.put(k, v.to_json());
    }
    o.build()
}

/// A JSON number that must be an `int` (used by typed sections).
pub fn int_from_json(v: &JsonValue, path: &str) -> ModelResult<i32> {
    as_i32(v, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_json;

    fn rt(text: &str) -> Value {
        let j = parse_json(text).unwrap();
        let v = Value::from_json(&j, "$").unwrap_or_else(|e| panic!("{text}: {e}"));
        assert_eq!(v.to_json(), j, "round trip of {text}");
        v
    }

    #[test]
    fn decodes_every_tag() {
        assert_eq!(rt("null"), Value::Null);
        assert_eq!(rt("true"), Value::Bool(true));
        assert_eq!(rt("7").as_i32(), Some(7));
        assert_eq!(rt(r#""s""#), Value::String(JsString::from_rust_str("s")));
        assert_eq!(
            rt(r#"{"long":"-4631228533296558972"}"#).as_long(),
            Some(-4631228533296558972)
        );
        assert_eq!(rt(r#"{"double":"5.0"}"#).as_double(), Some(5.0));
        assert!(rt(r#"{"double":"NaN"}"#).as_double().unwrap().is_nan());
        assert_eq!(
            rt(r#"{"char":"\ud800"}"#),
            Value::Char(JsString(vec![0xd800]))
        );
        assert_eq!(
            rt(r#"{"enum":"a.B$C","name":"X"}"#),
            Value::Enum {
                class: "a.B$C".into(),
                name: "X".into()
            }
        );
        rt(r#"{"diagnosticType":{"key":"JSC_X","level":"WARNING"}}"#);
        rt(r#"{"diagnosticGroup":{"group":null,"types":["JSC_A","JSC_B"]}}"#);
        rt(r#"{"classRef":"a.B"}"#);
        rt(r#"{"regex":"(code$)","flags":0}"#);
        rt(r#"{"sourceFile":{"name":"f.js","code":"x","kind":"EXTERN"}}"#);
        assert_eq!(rt(r#"{"optional":null}"#), Value::Optional(None));
        rt(r#"{"optional":{"long":"1"}}"#);
        rt(r#"{"list":[1,"a"],"impl":"java.util.ArrayList"}"#);
        rt(r#"{"set":[],"impl":"java.util.LinkedHashSet"}"#);
        rt(r#"{"map":[["k",{"char":"c"}]],"impl":"java.util.LinkedHashMap"}"#);
        rt(r#"{"arrayOf":"a.B","items":[{"object":"a.B","fields":{"x":1}}]}"#);
        rt(r#"{"composeWarningsGuard":{"guards":[{"object":"a.G","fields":{}}]}}"#);
        rt(r#"{"ref":"a.B"}"#);
        rt(r#"{"ref":"a.B$$Lambda","captures":{"arg$1":"x"}}"#);
        rt(r#"{"unrepresentable":"cycle:a.B"}"#);
        rt(r#"{"unrepresentable":"a.B$$Lambda","captures":{}}"#);
        rt(
            r#"{"typedScope":{"rootToken":"ROOT","depth":0,"typeOfThis":null,"vars":[["x","number",false],["y",null,true]]}}"#,
        );
        rt(
            r#"{"table":[[{"ref":"a.N"},"c",1]],"impl":"com.google.common.collect.HashBasedTable"}"#,
        );
        rt(
            r#"{"proto":"jscomp.X","fields":{"a":1,"b":[{"long":"2"}],"c":{"bytes":"AA=="},"d":{"astNode":{"proto":"jscomp.AstNode","fields":{"kind":"SOURCE_FILE"}}},"e":{"proto":"jscomp.Y","fields":{}},"f":{"double":"1.0"},"g":true,"object":{"proto":"jscomp.Z","fields":{}}}}"#,
        );
    }

    #[test]
    fn rejects_unknown_or_ambiguous_tags() {
        for bad in [
            r#"{"what":1}"#,
            r#"{"long":"1","double":"2"}"#,
            r#"{"list":[]}"#,
            r#"{"enum":"a.B","name":"X","extra":1}"#,
            r#"[1,2]"#,
            r#"{"sourceFile":{"name":"f","code":"","kind":"BOGUS"}}"#,
        ] {
            let j = parse_json(bad).unwrap();
            assert!(Value::from_json(&j, "$").is_err(), "accepted {bad}");
        }
    }
}
