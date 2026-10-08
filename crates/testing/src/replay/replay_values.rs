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
/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CheckLevel.java.

//! ReplayValues decoder and the recorder's referenceable value/error projection.
#![allow(clippy::collapsible_if)] // Retain the Java branches.
use crate::{
    jscomp_api::{
        CheckLevel, ComposeWarningsGuard, DiagnosticGroup, DiagnosticGroupWarningsGuard,
        DiagnosticType, JSChunk, JSError, SourceFile, SourceKind,
    },
    json::{JsString as JsonString, JsonValue},
    replay::replay_dsl::{DslValue, Object, parse_double},
    throwable::Throwable,
    value::{DiagnosticGroupRef, Value},
};
use closure_rhino::js_string::JsString;
use indexmap::IndexMap;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
};
// port: ReplayValues#classForName
pub fn class_for_name(name: &str, class_map: &IndexMap<String, String>) -> String {
    class_map.get(name).cloned().unwrap_or_else(|| name.into())
}
// port: ReplayValues#enumValue
pub fn enum_value(class: &str, name: &str, class_map: &IndexMap<String, String>) -> DslValue {
    DslValue::Enum {
        class: class_for_name(class, class_map),
        name: name.into(),
    }
}
// port: ReplayValues#decode (JsonPrimitive.getAsInt / BigDecimal.intValue)
fn json_number_int_value(literal: &str) -> Result<i32, Throwable> {
    if let Ok(n) = literal.parse::<i64>() {
        return Ok(n as i32);
    }
    let (mantissa, exponent) = match literal.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (
            mantissa,
            exponent.parse::<i32>().map_err(|_| Throwable::Exception {
                class: "java.lang.NumberFormatException".into(),
                message: Some("Exponent overflow.".into()),
            })?,
        ),
        None => (literal, 0),
    };
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.strip_prefix('-').unwrap_or(mantissa);
    let fractional_digits = mantissa
        .split_once('.')
        .map_or(0, |(_, fractional)| fractional.len());
    let scale =
        i64::try_from(fractional_digits).map_err(|_| bad("invalid number"))? - i64::from(exponent);
    if i32::try_from(scale).is_err() {
        return Err(Throwable::Exception {
            class: "java.lang.NumberFormatException".into(),
            message: Some("Scale out of range.".into()),
        });
    }
    let digits: Vec<u8> = mantissa.bytes().filter(|&b| b != b'.').collect();
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(bad("invalid number"));
    }
    let integer_digits = i64::try_from(digits.len()).map_err(|_| bad("invalid number"))? - scale;
    if integer_digits <= 0 {
        return Ok(0);
    }
    let mut result = 0i32;
    for &digit in digits.iter().take(integer_digits as usize) {
        result = result
            .wrapping_mul(10)
            .wrapping_add(i32::from(digit - b'0'));
    }
    let extra_zeroes = integer_digits - digits.len() as i64;
    if extra_zeroes > 0 {
        // 10^32 contains 2^32, so its low 32 bits (and those of larger powers) are zero.
        result = if extra_zeroes >= 32 {
            0
        } else {
            result.wrapping_mul(10i32.wrapping_pow(extra_zeroes as u32))
        };
    }
    Ok(if negative {
        result.wrapping_neg()
    } else {
        result
    })
}
// port: ReplayValues#decode
pub fn decode(
    value: &Value,
    target: &str,
    class_map: &IndexMap<String, String>,
) -> Result<DslValue, Throwable> {
    let decoded = match value {
        Value::Null => DslValue::Null,
        Value::Bool(b) => DslValue::Bool(*b),
        Value::String(s) => DslValue::String(JsString::from_units(s.0.clone())),
        Value::Int(n) => {
            let i = json_number_int_value(n.as_str())?;
            match target {
                "short" | "java.lang.Short" => DslValue::Int(i as i16 as i32),
                "byte" | "java.lang.Byte" => DslValue::Int(i as i8 as i32),
                "long" | "java.lang.Long" => DslValue::Long(i64::from(i)),
                "double" | "java.lang.Double" => DslValue::Double(f64::from(i)),
                _ => DslValue::Int(i),
            }
        }
        Value::Long(n) => DslValue::Long(n.parse().map_err(|_| bad("invalid long"))?),
        Value::Double(s) => {
            let d = parse_double(s)?;
            DslValue::Double(if target == "float" || target == "java.lang.Float" {
                f64::from(d as f32)
            } else {
                d
            })
        }
        Value::Char(s) => DslValue::Char(*s.0.first().ok_or_else(|| bad("empty char"))?),
        Value::Enum { class, name } => enum_value(class, name, class_map),
        Value::ClassRef(c) => DslValue::Class(class_for_name(c, class_map)),
        Value::DiagnosticType(t) => {
            DslValue::DiagnosticType(diagnostic_type(&t.key, level(&t.level)?))
        }
        Value::DiagnosticGroup(g) => DslValue::DiagnosticGroup(diagnostic_group(g)),
        Value::Regex { pattern, flags } => DslValue::Regex {
            pattern: JsString::from_units(pattern.0.clone()),
            flags: *flags,
        },
        Value::SourceFile(f) => DslValue::SourceFile(source_file(f)),
        Value::Optional(v) => DslValue::Optional(
            v.as_deref()
                .map(|v| decode(v, "java.lang.Object", class_map).map(Box::new))
                .transpose()?,
        ),
        Value::List { items, .. } => DslValue::List(decode_items(items, class_map)?),
        Value::Set { items, .. } => {
            let mut out: Vec<DslValue> = Vec::new();
            for v in decode_items(items, class_map)? {
                if !out.iter().any(|x| x.equals(&v)) {
                    out.push(v);
                }
            }
            DslValue::Set(out)
        }
        Value::Map { entries, .. } => {
            let mut out: Vec<(DslValue, DslValue)> = Vec::new();
            for (k, v) in entries {
                let k = decode(k, "java.lang.Object", class_map)?;
                let v = decode(v, "java.lang.Object", class_map)?;
                if let Some((_, old)) = out.iter_mut().find(|(x, _)| x.equals(&k)) {
                    *old = v;
                } else {
                    out.push((k, v));
                }
            }
            DslValue::Map(out)
        }
        Value::ArrayOf { class, items } => {
            let component = class_for_name(class, class_map);
            DslValue::Array {
                component: component.clone(),
                items: items
                    .iter()
                    .map(|v| decode(v, &component, class_map))
                    .collect::<Result<_, _>>()?,
            }
        }
        Value::ComposeWarningsGuard { guards } => {
            let compose = ComposeWarningsGuard::new(vec![]);
            for g in guards.iter().rev() {
                let v = decode(g, "com.google.javascript.jscomp.WarningsGuard", class_map)?;
                let DslValue::WarningsGuard { guard, .. } = v else {
                    return Err(bad("not a WarningsGuard"));
                };
                compose.add_guard(guard);
            }
            DslValue::WarningsGuard {
                guard: Arc::new(compose),
                encoding: value.to_json(),
            }
        }
        Value::Object { class, fields } => {
            let class = class_for_name(class, class_map);
            if class == "com.google.common.base.Absent" {
                DslValue::Optional(None)
            } else if class == "com.google.common.base.Present" {
                let v = fields
                    .get("reference")
                    .ok_or_else(|| bad("Present has no reference"))?;
                DslValue::Optional(Some(Box::new(decode(v, "java.lang.Object", class_map)?)))
            } else if class == "com.google.javascript.jscomp.DiagnosticGroupWarningsGuard" {
                let group = match fields.get("group") {
                    Some(Value::DiagnosticGroup(g)) => diagnostic_group(g),
                    _ => return Err(bad("guard has no diagnostic group")),
                };
                let l = match fields.get("level") {
                    Some(Value::Enum { name, .. }) => level(name)?,
                    _ => return Err(bad("guard has no level")),
                };
                DslValue::WarningsGuard {
                    guard: Arc::new(DiagnosticGroupWarningsGuard::new(group, l)),
                    encoding: value.to_json(),
                }
            } else if matches!(
                class.as_str(),
                "com.google.javascript.jscomp.StrictWarningsGuard"
                    | "com.google.javascript.jscomp.ByPathWarningsGuard"
                    | "com.google.javascript.jscomp.ShowByPathWarningsGuard"
                    | crate::replay::disambiguate_test_helpers::SilenceNoiseGuard::CLASS
                    | crate::replay::disambiguate_test_helpers::SilenceChecksWarningsGuard::CLASS
            ) {
                let fields = fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), decode(v, "java.lang.Object", class_map)?)))
                    .collect::<Result<IndexMap<_, _>, Throwable>>()?;
                let object = DslValue::Object(Rc::new(RefCell::new(Object {
                    class,
                    fields,
                    field_types: IndexMap::new(),
                })));
                DslValue::WarningsGuard {
                    guard: crate::replay::options_values::decode_guard(&object)?,
                    encoding: value.to_json(),
                }
            } else {
                if target == "com.google.javascript.jscomp.WarningsGuard" {
                    return Err(Throwable::Unported(class));
                }
                let fields = fields
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), decode(v, "java.lang.Object", class_map)?)))
                    .collect::<Result<IndexMap<_, _>, Throwable>>()?;
                DslValue::Object(Rc::new(RefCell::new(Object {
                    class,
                    fields,
                    field_types: IndexMap::new(),
                })))
            }
        }
        Value::Proto(p) => DslValue::Proto(p.clone()),
        Value::Ref { class, .. } => return Err(bad(&format!("reference value {class}"))),
        Value::Unrepresentable { class, .. } => {
            return Err(bad(&format!("unrepresentable value {class}")));
        }
        Value::TypedScope(_) => return Err(bad("typedScope never decodes")),
        Value::Table { .. } => return Err(bad("table never decodes")),
    };
    let decoded = match value {
        Value::List { impl_class, .. }
        | Value::Set { impl_class, .. }
        | Value::Map { impl_class, .. } => DslValue::Typed {
            class: class_for_name(impl_class, class_map),
            value: Box::new(decoded),
        },
        _ => decoded,
    };
    adapt(decoded, target)
}
// port: ReplayValues#decode (JSON boundary)
pub fn decode_json(
    raw: &JsonValue,
    target: &str,
    class_map: &IndexMap<String, String>,
) -> Result<DslValue, Throwable> {
    if let JsonValue::Array(items) = raw {
        return Ok(DslValue::List(
            items
                .iter()
                .map(|v| decode_json(v, "java.lang.Object", class_map))
                .collect::<Result<_, _>>()?,
        ));
    }
    decode(
        &Value::from_json(raw, "$").map_err(|e| bad(&e.to_string()))?,
        target,
        class_map,
    )
}
// port: ReplayValues#collection
fn decode_items(
    items: &[Value],
    cm: &IndexMap<String, String>,
) -> Result<Vec<DslValue>, Throwable> {
    items
        .iter()
        .map(|v| decode(v, "java.lang.Object", cm))
        .collect()
}
// port: ReplayValues#adapt
pub fn adapt(value: DslValue, target: &str) -> Result<DslValue, Throwable> {
    if matches!(value, DslValue::Null) {
        return if matches!(
            target,
            "boolean" | "int" | "long" | "double" | "float" | "short" | "byte" | "char"
        ) {
            Err(bad("null in primitive slot"))
        } else {
            Ok(value)
        };
    }
    match (target, value) {
        ("long" | "java.lang.Long", DslValue::Int(i)) => Ok(DslValue::Long(i64::from(i))),
        ("double" | "java.lang.Double", DslValue::Int(i)) => Ok(DslValue::Double(f64::from(i))),
        ("java.util.Set", DslValue::List(items)) => {
            let mut out: Vec<DslValue> = Vec::new();
            for item in items {
                if !out.iter().any(|x| x.equals(&item)) {
                    out.push(item);
                }
            }
            Ok(DslValue::Set(out))
        }
        ("boolean" | "java.lang.Boolean", v @ DslValue::Bool(_))
        | ("int" | "java.lang.Integer" | "short" | "byte", v @ DslValue::Int(_))
        | ("long" | "java.lang.Long", v @ DslValue::Long(_))
        | ("double" | "java.lang.Double" | "float" | "java.lang.Float", v @ DslValue::Double(_))
        | ("char" | "java.lang.Character", v @ DslValue::Char(_))
        | ("java.lang.String", v @ DslValue::String(_)) => Ok(v),
        (
            "boolean" | "int" | "long" | "double" | "float" | "short" | "byte" | "char"
            | "java.lang.String",
            _,
        ) => Err(bad(&format!("value does not fit {target}"))),
        (_, v) => Ok(v),
    }
}
// port: ReplayValues#diagnosticGroup
pub fn diagnostic_group(g: &DiagnosticGroupRef) -> Arc<DiagnosticGroup> {
    let types = g
        .types
        .iter()
        .map(|k| diagnostic_type(k, CheckLevel::ERROR))
        .collect::<Vec<_>>();
    Arc::new(match &g.group {
        Some(name) => DiagnosticGroup::new_named(name, &types),
        None => DiagnosticGroup::new(&types),
    })
}
// port: ReplayValues#decode (DiagnosticType)
pub fn diagnostic_type(key: &str, l: CheckLevel) -> &'static DiagnosticType {
    static TYPES: LazyLock<Mutex<IndexMap<(String, CheckLevel), &'static DiagnosticType>>> =
        LazyLock::new(|| Mutex::new(IndexMap::new()));
    let mut types = TYPES.lock().unwrap();
    types.entry((key.into(), l)).or_insert_with(|| {
        Box::leak(Box::new(DiagnosticType::make(
            Box::leak(key.to_string().into_boxed_str()),
            l,
            "{0}",
        )))
    })
}
// port: CheckLevel#valueOf
pub fn level(name: &str) -> Result<CheckLevel, Throwable> {
    match name {
        "ERROR" => Ok(CheckLevel::ERROR),
        "WARNING" => Ok(CheckLevel::WARNING),
        "OFF" => Ok(CheckLevel::OFF),
        _ => Err(bad("unknown CheckLevel")),
    }
}
// port: ReplayValues#sourceFile
pub fn source_file(f: &crate::value::SourceFile) -> Arc<SourceFile> {
    let kind = match f.kind {
        crate::value::SourceKind::STRONG => SourceKind::STRONG,
        crate::value::SourceKind::WEAK => SourceKind::WEAK,
        crate::value::SourceKind::EXTERN => SourceKind::EXTERN,
        crate::value::SourceKind::NON_CODE => SourceKind::NON_CODE,
    };
    Arc::new(SourceFile::from_code_with_kind(
        &f.name,
        JsString::from_units(f.code.0.clone()),
        kind,
    ))
}
// port: ReplayValues#sourceFiles
pub fn source_files(files: &[crate::value::SourceFile]) -> Vec<Arc<SourceFile>> {
    files.iter().map(source_file).collect()
}
// port: ReplayValues#chunks
pub fn chunks(records: &[crate::record::Chunk]) -> Result<Vec<JSChunk>, Throwable> {
    let mut by_name: IndexMap<String, JSChunk> = IndexMap::new();
    let mut out = Vec::new();
    for c in records {
        let chunk = JSChunk::new(&c.name);
        for dep in &c.deps {
            chunk.add_dependency(
                by_name
                    .get(dep)
                    .ok_or_else(|| bad("chunk dependency not yet constructed"))?,
            );
        }
        for f in &c.inputs {
            chunk.add_source_file(source_file(f));
        }
        by_name.insert(c.name.clone(), chunk.clone());
        out.push(chunk);
    }
    Ok(out)
}
// port: UnitRecorder#errors
pub fn errors(errs: &[JSError]) -> JsonValue {
    JsonValue::Array(
        errs.iter()
            .map(|e| {
                object([
                    ("key", JsonValue::str(e.get_type().key)),
                    (
                        "defaultLevel",
                        JsonValue::str(&e.default_level().to_string()),
                    ),
                    ("description", JsonValue::str(e.description())),
                    (
                        "source",
                        e.source_name().map_or(JsonValue::Null, JsonValue::str),
                    ),
                    ("line", JsonValue::int(i64::from(e.lineno()))),
                    ("charno", JsonValue::int(i64::from(e.charno()))),
                    ("length", JsonValue::int(i64::from(e.length()))),
                ])
            })
            .collect(),
    )
}
// port: UnitRecorder#refValue
pub fn encode(v: &DslValue) -> Result<JsonValue, Throwable> {
    Ok(match v {
        DslValue::Typed { class, value } => {
            let mut out = encode(value)?;
            if let JsonValue::Object(o) = &mut out {
                if o.contains_key("impl") {
                    o.insert("impl".into(), JsonValue::str(class));
                }
                // A Node is written as a reference to its runtime class (Node$StringNode, ...).
                if matches!(value.untyped(), DslValue::Node(_)) && o.contains_key("ref") {
                    o.insert("ref".into(), JsonValue::str(class));
                }
            }
            out
        }
        DslValue::Null => JsonValue::Null,
        DslValue::Int(i) => JsonValue::int(i64::from(*i)),
        DslValue::Bool(b) => JsonValue::Bool(*b),
        DslValue::String(s) => JsonValue::String(JsonString(s.as_units().to_vec())),
        DslValue::Long(i) => object([("long", JsonValue::str(&i.to_string()))]),
        DslValue::Double(d) => object([(
            "double",
            JsonValue::str(&closure_rhino::java_lang::double_to_string(*d)),
        )]),
        DslValue::Char(c) => object([("char", JsonValue::String(JsonString(vec![*c])))]),
        DslValue::Enum { class, name } => object([
            ("enum", JsonValue::str(class)),
            ("name", JsonValue::str(name)),
        ]),
        DslValue::Class(c) => object([("classRef", JsonValue::str(c))]),
        DslValue::List(items) | DslValue::Set(items) => {
            let tag = if matches!(v, DslValue::List(_)) {
                "list"
            } else {
                "set"
            };
            object([
                (
                    tag,
                    JsonValue::Array(items.iter().map(encode).collect::<Result<_, _>>()?),
                ),
                (
                    "impl",
                    JsonValue::str(if tag == "list" {
                        "java.util.ArrayList"
                    } else {
                        "java.util.LinkedHashSet"
                    }),
                ),
            ])
        }
        DslValue::Map(items) => object([
            (
                "map",
                JsonValue::Array(
                    items
                        .iter()
                        .map(|(k, v)| Ok(JsonValue::Array(vec![encode(k)?, encode(v)?])))
                        .collect::<Result<_, Throwable>>()?,
                ),
            ),
            ("impl", JsonValue::str("java.util.LinkedHashMap")),
        ]),
        DslValue::Array { component, items } => object([
            ("arrayOf", JsonValue::str(component)),
            (
                "items",
                JsonValue::Array(items.iter().map(encode).collect::<Result<_, _>>()?),
            ),
        ]),
        DslValue::Optional(v) => object([(
            "optional",
            v.as_deref().map_or(Ok(JsonValue::Null), encode)?,
        )]),
        DslValue::Object(o) => {
            let o = o.borrow();
            object([
                ("object", JsonValue::str(&o.class)),
                (
                    "fields",
                    JsonValue::Object(
                        o.fields
                            .iter()
                            .map(|(k, v)| Ok((k.clone(), encode(v)?)))
                            .collect::<Result<_, Throwable>>()?,
                    ),
                ),
            ])
        }
        DslValue::DiagnosticType(t) => object([(
            "diagnosticType",
            object([
                ("key", JsonValue::str(t.key)),
                ("level", JsonValue::str(&t.level.to_string())),
            ]),
        )]),
        DslValue::DiagnosticGroup(g) => object([(
            "diagnosticGroup",
            object([
                (
                    "group",
                    g.get_name().map_or(JsonValue::Null, JsonValue::str),
                ),
                (
                    "types",
                    JsonValue::Array(
                        g.get_types()
                            .iter()
                            .map(|t| JsonValue::str(t.key))
                            .collect(),
                    ),
                ),
            ]),
        )]),
        DslValue::CodingConvention(c) => {
            crate::replay::options_values::encode_coding_convention(c.as_ref())?
        }
        DslValue::NameGenerator(n) => encode(
            &crate::replay::options_values::encode_name_generator(n.as_ref())?,
        )?,
        DslValue::WarningsGuard { encoding, .. } => encoding.clone(),
        DslValue::Regex { pattern, flags } => object([
            (
                "regex",
                JsonValue::String(JsonString(pattern.as_units().to_vec())),
            ),
            ("flags", JsonValue::int(i64::from(*flags))),
        ]),
        DslValue::Proto(p) => p.to_json(),
        DslValue::SourceFile(f) => object([(
            "sourceFile",
            object([
                (
                    "name",
                    JsonValue::str(crate::jscomp_api::StaticSourceFile::get_name(f.as_ref())),
                ),
                (
                    "code",
                    JsonValue::String(JsonString(
                        f.get_code()
                            .map_err(|e| bad(&e.to_string()))?
                            .as_units()
                            .to_vec(),
                    )),
                ),
                (
                    "kind",
                    JsonValue::str(&format!(
                        "{:?}",
                        crate::jscomp_api::StaticSourceFile::get_kind(f.as_ref())
                    )),
                ),
            ]),
        )]),
        _ => object([("ref", JsonValue::str(&v.class_name()))]),
    })
}
// port: UnitRecorder#refValue (container implementation projection)
pub fn encode_with_template(
    v: &DslValue,
    template: Option<&JsonValue>,
) -> Result<JsonValue, Throwable> {
    let encoded = encode(v)?;
    Ok(template.map_or_else(|| encoded.clone(), |t| restore_impls(&encoded, t)))
}
// port: UnitRecorder#dump (recorded implementation names)
fn restore_impls(v: &JsonValue, t: &JsonValue) -> JsonValue {
    if let (Some(a), Some(b)) = (v.as_object(), t.as_object()) {
        if a.contains_key("optional")
            && b.get("object")
                .and_then(JsonValue::as_js_string)
                .is_some_and(|s| {
                    s.eq_str("com.google.common.base.Absent")
                        || s.eq_str("com.google.common.base.Present")
                })
        {
            if a["optional"].is_null() {
                return object([
                    ("object", JsonValue::str("com.google.common.base.Absent")),
                    ("fields", object([])),
                ]);
            }
            let bt = b
                .get("fields")
                .and_then(|f| f.get("reference"))
                .unwrap_or(&JsonValue::Null);
            return object([
                ("object", JsonValue::str("com.google.common.base.Present")),
                (
                    "fields",
                    object([("reference", restore_impls(&a["optional"], bt))]),
                ),
            ]);
        }
        let mut out = IndexMap::new();
        for (k, x) in a {
            out.insert(
                k.clone(),
                if k == "impl" {
                    b.get(k).cloned().unwrap_or_else(|| x.clone())
                } else {
                    b.get(k).map_or_else(|| x.clone(), |y| restore_impls(x, y))
                },
            );
        }
        JsonValue::Object(out)
    } else if let (Some(a), Some(b)) = (v.as_array(), t.as_array()) {
        JsonValue::Array(
            a.iter()
                .enumerate()
                .map(|(i, x)| b.get(i).map_or_else(|| x.clone(), |y| restore_impls(x, y)))
                .collect(),
        )
    } else {
        v.clone()
    }
}
// port: UnitRecorder#dump (JSON object construction)
pub fn object<const N: usize>(fields: [(&str, JsonValue); N]) -> JsonValue {
    JsonValue::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
// port: ReplayValues.Undecodable#Undecodable
fn bad(message: &str) -> Throwable {
    Throwable::HarnessError(message.into())
}
