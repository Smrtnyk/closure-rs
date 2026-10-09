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

//! Strict field-by-field reading of JSON objects into the typed model.
//!
//! Every typed section is read with an [`Obj`]: each known key is taken once, and
//! [`Obj::finish`] fails when a key is left over, so an undocumented key is a load error rather
//! than silently dropped. Errors carry the JSON path (`$.harness.fields.compareAsTree`).

use crate::json::{JsString, JsonNumber, JsonValue};
use closure_rhino::fx_hash::IndexMap;
use std::fmt;

/// A model error: the JSON path and what is wrong there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl std::error::Error for ModelError {}

pub type ModelResult<T> = Result<T, ModelError>;

pub fn err<T>(path: &str, message: impl Into<String>) -> ModelResult<T> {
    Err(ModelError {
        path: path.to_string(),
        message: message.into(),
    })
}

/// `path.key`
pub fn join(path: &str, key: &str) -> String {
    format!("{path}.{key}")
}

/// `path[i]`
pub fn index(path: &str, i: usize) -> String {
    format!("{path}[{i}]")
}

/// A strict reader over one JSON object.
pub struct Obj<'a> {
    pub path: String,
    map: &'a IndexMap<String, JsonValue>,
    taken: Vec<bool>,
}

impl<'a> Obj<'a> {
    pub fn new(v: &'a JsonValue, path: &str) -> ModelResult<Obj<'a>> {
        match v {
            JsonValue::Object(map) => Ok(Obj {
                path: path.to_string(),
                map,
                taken: vec![false; map.len()],
            }),
            other => err(
                path,
                format!("expected an object, found {}", other.type_name()),
            ),
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }

    /// Takes `key` if present.
    pub fn opt(&mut self, key: &str) -> Option<&'a JsonValue> {
        let i = self.map.get_index_of(key)?;
        self.taken[i] = true;
        Some(&self.map[i])
    }

    /// Takes `key`, which must be present.
    pub fn req(&mut self, key: &str) -> ModelResult<&'a JsonValue> {
        match self.opt(key) {
            Some(v) => Ok(v),
            None => err(&self.path, format!("missing required key {key:?}")),
        }
    }

    pub fn sub(&self, key: &str) -> String {
        join(&self.path, key)
    }

    /// Takes every key not yet taken, in recorded order.
    pub fn rest(&mut self) -> IndexMap<String, &'a JsonValue> {
        let mut out = IndexMap::<_, _>::default();
        for (i, (k, v)) in self.map.iter().enumerate() {
            if !self.taken[i] {
                self.taken[i] = true;
                out.insert(k.clone(), v);
            }
        }
        out
    }

    /// Fails when a key was not taken.
    pub fn finish(self) -> ModelResult<()> {
        let left: Vec<&String> = self
            .map
            .keys()
            .zip(&self.taken)
            .filter(|(_, t)| !**t)
            .map(|(k, _)| k)
            .collect();
        if left.is_empty() {
            Ok(())
        } else {
            err(&self.path, format!("unknown key(s) {left:?}"))
        }
    }

    // Typed shorthands.

    pub fn req_string(&mut self, key: &str) -> ModelResult<String> {
        let p = self.sub(key);
        as_string(self.req(key)?, &p)
    }

    pub fn opt_string(&mut self, key: &str) -> ModelResult<Option<String>> {
        let p = self.sub(key);
        self.opt(key).map(|v| as_string(v, &p)).transpose()
    }

    pub fn req_js_string(&mut self, key: &str) -> ModelResult<JsString> {
        let p = self.sub(key);
        as_js_string(self.req(key)?, &p)
    }

    pub fn req_bool(&mut self, key: &str) -> ModelResult<bool> {
        let p = self.sub(key);
        as_bool(self.req(key)?, &p)
    }

    pub fn opt_bool(&mut self, key: &str) -> ModelResult<Option<bool>> {
        let p = self.sub(key);
        self.opt(key).map(|v| as_bool(v, &p)).transpose()
    }

    pub fn req_i32(&mut self, key: &str) -> ModelResult<i32> {
        let p = self.sub(key);
        as_i32(self.req(key)?, &p)
    }

    pub fn opt_i32(&mut self, key: &str) -> ModelResult<Option<i32>> {
        let p = self.sub(key);
        self.opt(key).map(|v| as_i32(v, &p)).transpose()
    }
}

pub fn as_string(v: &JsonValue, path: &str) -> ModelResult<String> {
    match v {
        JsonValue::String(s) => match s.to_string_strict() {
            Some(s) => Ok(s),
            None => err(path, "expected a string without lone surrogates"),
        },
        other => err(
            path,
            format!("expected a string, found {}", other.type_name()),
        ),
    }
}

pub fn as_js_string(v: &JsonValue, path: &str) -> ModelResult<JsString> {
    match v {
        JsonValue::String(s) => Ok(s.clone()),
        other => err(
            path,
            format!("expected a string, found {}", other.type_name()),
        ),
    }
}

/// A string or null.
pub fn as_opt_js_string(v: &JsonValue, path: &str) -> ModelResult<Option<JsString>> {
    match v {
        JsonValue::Null => Ok(None),
        other => as_js_string(other, path).map(Some),
    }
}

/// A string or null.
pub fn as_opt_string(v: &JsonValue, path: &str) -> ModelResult<Option<String>> {
    match v {
        JsonValue::Null => Ok(None),
        other => as_string(other, path).map(Some),
    }
}

pub fn as_bool(v: &JsonValue, path: &str) -> ModelResult<bool> {
    match v {
        JsonValue::Bool(b) => Ok(*b),
        other => err(
            path,
            format!("expected a bool, found {}", other.type_name()),
        ),
    }
}

pub fn as_i32(v: &JsonValue, path: &str) -> ModelResult<i32> {
    match v {
        JsonValue::Number(n) => match n.as_i32() {
            Some(i) => Ok(i),
            None => err(path, format!("expected a 32-bit integer, found {}", n.0)),
        },
        other => err(
            path,
            format!("expected a number, found {}", other.type_name()),
        ),
    }
}

pub fn as_number(v: &JsonValue, path: &str) -> ModelResult<JsonNumber> {
    match v {
        JsonValue::Number(n) => Ok(n.clone()),
        other => err(
            path,
            format!("expected a number, found {}", other.type_name()),
        ),
    }
}

pub fn as_array<'a>(v: &'a JsonValue, path: &str) -> ModelResult<&'a Vec<JsonValue>> {
    match v {
        JsonValue::Array(a) => Ok(a),
        other => err(
            path,
            format!("expected an array, found {}", other.type_name()),
        ),
    }
}

/// Reads every item of an array with `f(item, item_path)`.
pub fn list_of<T>(
    v: &JsonValue,
    path: &str,
    mut f: impl FnMut(&JsonValue, &str) -> ModelResult<T>,
) -> ModelResult<Vec<T>> {
    as_array(v, path)?
        .iter()
        .enumerate()
        .map(|(i, x)| f(x, &index(path, i)))
        .collect()
}

/// Reads every member of an object with `f(value, member_path)`, keeping order.
pub fn map_of<T>(
    v: &JsonValue,
    path: &str,
    mut f: impl FnMut(&JsonValue, &str) -> ModelResult<T>,
) -> ModelResult<IndexMap<String, T>> {
    match v {
        JsonValue::Object(m) => m
            .iter()
            .map(|(k, x)| Ok((k.clone(), f(x, &join(path, k))?)))
            .collect(),
        other => err(
            path,
            format!("expected an object, found {}", other.type_name()),
        ),
    }
}

// Writing helpers.

/// Builds a JSON object in insertion order.
#[derive(Default)]
pub struct ObjOut(pub IndexMap<String, JsonValue>);

impl ObjOut {
    pub fn new() -> ObjOut {
        ObjOut(IndexMap::<_, _>::default())
    }

    pub fn put(&mut self, key: &str, v: JsonValue) -> &mut ObjOut {
        self.0.insert(key.to_string(), v);
        self
    }

    pub fn put_opt(&mut self, key: &str, v: Option<JsonValue>) -> &mut ObjOut {
        if let Some(v) = v {
            self.0.insert(key.to_string(), v);
        }
        self
    }

    pub fn build(&mut self) -> JsonValue {
        JsonValue::Object(std::mem::take(&mut self.0))
    }
}

pub fn js(s: &JsString) -> JsonValue {
    JsonValue::String(s.clone())
}

pub fn st(s: &str) -> JsonValue {
    JsonValue::str(s)
}

pub fn opt_js(s: &Option<JsString>) -> JsonValue {
    s.as_ref().map_or(JsonValue::Null, js)
}

pub fn opt_st(s: &Option<String>) -> JsonValue {
    s.as_deref().map_or(JsonValue::Null, st)
}

pub fn int(i: i32) -> JsonValue {
    JsonValue::int(i64::from(i))
}

pub fn arr<T>(items: &[T], f: impl Fn(&T) -> JsonValue) -> JsonValue {
    JsonValue::Array(items.iter().map(f).collect())
}

pub fn strs(items: &[String]) -> JsonValue {
    arr(items, |s| st(s))
}
