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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! DSL.md "Descriptor file": `corpus/unit/descriptors/<TestClassSimpleName>.json`, and the case
//! selection of `ReplayDsl#selectCase`.
//!
//! The keys replay and gate 0.2 read are typed. Every other top-level or case key is a note for
//! humans ("ignored by replay") and is kept verbatim in `notes`, so the file round-trips.

use crate::dsl::Expr;
use crate::json::JsonValue;
use crate::reader::{
    ModelError, ModelResult, Obj, ObjOut, arr, as_string, err, int, list_of, map_of, st, strs,
};
use indexmap::IndexMap;

/// One descriptor file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptor {
    /// `source`: where `getProcessor` lives.
    pub source: String,
    /// `classMap`: recorded FQCN -> helper FQCN.
    pub class_map: Option<IndexMap<String, String>>,
    pub cases: Vec<Case>,
    /// `unrepresentable`: records excluded from gate (a), counted in gate (e).
    pub unrepresentable: Option<Vec<UnrepresentableRecord>>,
    /// Descriptor-level `testFieldsAfterUnchecked` reason.
    pub test_fields_after_unchecked: Option<String>,
    /// `postCallAssertions` (superseded by corpus/unit/postcall/, listed only).
    pub post_call_assertions: Option<Vec<PostCallAssertion>>,
    /// `vacuityNoopClass`: an FQCN or a list of FQCNs.
    pub vacuity_noop_class: Option<VacuityNoopClass>,
    /// Every other top-level key (`note`, `unrepresentableAudit`, ...): notes, ignored by replay.
    pub notes: IndexMap<String, JsonValue>,
}

/// One entry of the descriptor's `unrepresentable` list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnrepresentableRecord {
    /// 0-based line of the record in the class's record file.
    pub index: i32,
    pub method: Option<String>,
    pub call: Option<i32>,
    pub reason: String,
}

/// One entry of `postCallAssertions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostCallAssertion {
    /// `"*"` (`None`) or the method names.
    pub methods: Option<Vec<String>>,
    pub status: String,
    pub reason: String,
}

/// `vacuityNoopClass`, in the form it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VacuityNoopClass {
    One(String),
    Many(Vec<String>),
}

impl VacuityNoopClass {
    pub fn classes(&self) -> Vec<&str> {
        match self {
            VacuityNoopClass::One(c) => vec![c.as_str()],
            VacuityNoopClass::Many(cs) => cs.iter().map(String::as_str).collect(),
        }
    }
}

/// A `when` test of one dotted path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhenTest {
    /// `{"isNull": true|false}`: (non-)null or missing.
    IsNull(bool),
    /// Any other JSON value: JSON equality (a missing path equals only `null`).
    Equals(JsonValue),
}

/// `options` case key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseOptions {
    pub skip: Option<Vec<String>>,
    pub then: Option<Vec<Expr>>,
}

/// One case (DSL.md "Case keys").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// `when` (absent: always matches, like `{}`).
    pub when: Option<IndexMap<String, WhenTest>>,
    pub processor: Option<Expr>,
    pub compiler_setup: Option<Vec<Expr>>,
    pub options: Option<CaseOptions>,
    pub postconditions: Option<Vec<Expr>>,
    pub test_fields_after: Option<Expr>,
    pub test_fields_after_also: Option<Vec<String>>,
    /// Field -> reason (a reason must be non-empty; the harness checks it at replay).
    pub test_fields_after_skip: Option<IndexMap<String, String>>,
    pub test_fields_after_unchecked: Option<String>,
    /// Every other case key (`note`, ...): notes, ignored by replay.
    pub notes: IndexMap<String, JsonValue>,
}

fn exprs(v: &JsonValue, path: &str) -> ModelResult<Vec<Expr>> {
    list_of(v, path, Expr::from_json)
}

fn exprs_to_json(items: &[Expr]) -> JsonValue {
    arr(items, Expr::to_json)
}

fn opt_exprs(o: &mut Obj<'_>, key: &str) -> ModelResult<Option<Vec<Expr>>> {
    let p = o.sub(key);
    o.opt(key).map(|v| exprs(v, &p)).transpose()
}

fn opt_strs(o: &mut Obj<'_>, key: &str) -> ModelResult<Option<Vec<String>>> {
    let p = o.sub(key);
    o.opt(key).map(|v| list_of(v, &p, as_string)).transpose()
}

fn string_map(m: &IndexMap<String, String>) -> JsonValue {
    JsonValue::Object(m.iter().map(|(k, v)| (k.clone(), st(v))).collect())
}

fn notes_rest(o: &mut Obj<'_>) -> IndexMap<String, JsonValue> {
    o.rest().into_iter().map(|(k, v)| (k, v.clone())).collect()
}

impl WhenTest {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<WhenTest> {
        match v.as_object() {
            Some(m) if m.contains_key("isNull") => {
                let mut o = Obj::new(v, path)?;
                let b = o.req_bool("isNull")?;
                o.finish()?;
                Ok(WhenTest::IsNull(b))
            }
            _ => Ok(WhenTest::Equals(v.clone())),
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            WhenTest::IsNull(b) => ObjOut::new().put("isNull", JsonValue::Bool(*b)).build(),
            WhenTest::Equals(v) => v.clone(),
        }
    }

    // port: ReplayDsl#matches
    pub fn matches(&self, actual: Option<&JsonValue>) -> bool {
        match self {
            WhenTest::IsNull(want_null) => {
                let is_null = actual.is_none_or(JsonValue::is_null);
                is_null == *want_null
            }
            WhenTest::Equals(want) => match actual {
                None => want.is_null(),
                Some(a) => a.gson_equals(want),
            },
        }
    }
}

impl CaseOptions {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<CaseOptions> {
        let mut o = Obj::new(v, path)?;
        let skip = opt_strs(&mut o, "skip")?;
        let then = opt_exprs(&mut o, "then")?;
        o.finish()?;
        Ok(CaseOptions { skip, then })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put_opt("skip", self.skip.as_deref().map(strs))
            .put_opt("then", self.then.as_deref().map(exprs_to_json))
            .build()
    }
}

impl Case {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<Case> {
        let mut o = Obj::new(v, path)?;
        let wp = o.sub("when");
        let when = o
            .opt("when")
            .map(|w| map_of(w, &wp, WhenTest::from_json))
            .transpose()?;
        let pp = o.sub("processor");
        let processor = o
            .opt("processor")
            .map(|x| Expr::from_json(x, &pp))
            .transpose()?;
        let compiler_setup = opt_exprs(&mut o, "compilerSetup")?;
        let op = o.sub("options");
        let options = o
            .opt("options")
            .map(|x| CaseOptions::from_json(x, &op))
            .transpose()?;
        let postconditions = opt_exprs(&mut o, "postconditions")?;
        let tp = o.sub("testFieldsAfter");
        let test_fields_after = o
            .opt("testFieldsAfter")
            .map(|x| Expr::from_json(x, &tp))
            .transpose()?;
        let test_fields_after_also = opt_strs(&mut o, "testFieldsAfterAlso")?;
        let sp = o.sub("testFieldsAfterSkip");
        let test_fields_after_skip = o
            .opt("testFieldsAfterSkip")
            .map(|x| map_of(x, &sp, as_string))
            .transpose()?;
        let test_fields_after_unchecked = o.opt_string("testFieldsAfterUnchecked")?;
        let notes = notes_rest(&mut o);
        o.finish()?;
        Ok(Case {
            when,
            processor,
            compiler_setup,
            options,
            postconditions,
            test_fields_after,
            test_fields_after_also,
            test_fields_after_skip,
            test_fields_after_unchecked,
            notes,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        let mut o = ObjOut::new();
        o.put_opt(
            "when",
            self.when.as_ref().map(|w| {
                JsonValue::Object(w.iter().map(|(k, t)| (k.clone(), t.to_json())).collect())
            }),
        )
        .put_opt("processor", self.processor.as_ref().map(Expr::to_json))
        .put_opt(
            "compilerSetup",
            self.compiler_setup.as_deref().map(exprs_to_json),
        )
        .put_opt("options", self.options.as_ref().map(CaseOptions::to_json))
        .put_opt(
            "postconditions",
            self.postconditions.as_deref().map(exprs_to_json),
        )
        .put_opt(
            "testFieldsAfter",
            self.test_fields_after.as_ref().map(Expr::to_json),
        )
        .put_opt(
            "testFieldsAfterAlso",
            self.test_fields_after_also.as_deref().map(strs),
        )
        .put_opt(
            "testFieldsAfterSkip",
            self.test_fields_after_skip.as_ref().map(string_map),
        )
        .put_opt(
            "testFieldsAfterUnchecked",
            self.test_fields_after_unchecked.as_deref().map(st),
        );
        for (k, v) in &self.notes {
            o.put(k, v.clone());
        }
        o.build()
    }

    /// Every `when` entry matches `record` (the raw record JSON).
    pub fn matches(&self, record: &JsonValue) -> Result<bool, ModelError> {
        if let Some(when) = &self.when {
            for (p, want) in when {
                let actual = path(record, p)?;
                if !want.matches(actual) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

impl UnrepresentableRecord {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<UnrepresentableRecord> {
        let mut o = Obj::new(v, path)?;
        let index = o.req_i32("index")?;
        let method = o.opt_string("method")?;
        let call = o.opt_i32("call")?;
        let reason = o.req_string("reason")?;
        o.finish()?;
        Ok(UnrepresentableRecord {
            index,
            method,
            call,
            reason,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put("index", int(self.index))
            .put_opt("method", self.method.as_deref().map(st))
            .put_opt("call", self.call.map(int))
            .put("reason", st(&self.reason))
            .build()
    }
}

impl PostCallAssertion {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<PostCallAssertion> {
        let mut o = Obj::new(v, path)?;
        let mp = o.sub("methods");
        let methods = match o.req("methods")? {
            JsonValue::String(s) if s.eq_str("*") => None,
            m => Some(list_of(m, &mp, as_string)?),
        };
        let status = o.req_string("status")?;
        let reason = o.req_string("reason")?;
        o.finish()?;
        Ok(PostCallAssertion {
            methods,
            status,
            reason,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        ObjOut::new()
            .put(
                "methods",
                self.methods.as_deref().map_or_else(|| st("*"), strs),
            )
            .put("status", st(&self.status))
            .put("reason", st(&self.reason))
            .build()
    }
}

impl Descriptor {
    pub fn from_json(v: &JsonValue, path: &str) -> ModelResult<Descriptor> {
        let mut o = Obj::new(v, path)?;
        let source = o.req_string("source")?;
        let cp = o.sub("classMap");
        let class_map = o
            .opt("classMap")
            .map(|x| map_of(x, &cp, as_string))
            .transpose()?;
        let csp = o.sub("cases");
        let cases = list_of(o.req("cases")?, &csp, Case::from_json)?;
        let up = o.sub("unrepresentable");
        let unrepresentable = o
            .opt("unrepresentable")
            .map(|x| list_of(x, &up, UnrepresentableRecord::from_json))
            .transpose()?;
        let test_fields_after_unchecked = o.opt_string("testFieldsAfterUnchecked")?;
        let pp = o.sub("postCallAssertions");
        let post_call_assertions = o
            .opt("postCallAssertions")
            .map(|x| list_of(x, &pp, PostCallAssertion::from_json))
            .transpose()?;
        let vp = o.sub("vacuityNoopClass");
        let vacuity_noop_class = o
            .opt("vacuityNoopClass")
            .map(|x| match x {
                JsonValue::String(_) => as_string(x, &vp).map(VacuityNoopClass::One),
                _ => list_of(x, &vp, as_string).map(VacuityNoopClass::Many),
            })
            .transpose()?;
        let notes = notes_rest(&mut o);
        o.finish()?;
        Ok(Descriptor {
            source,
            class_map,
            cases,
            unrepresentable,
            test_fields_after_unchecked,
            post_call_assertions,
            vacuity_noop_class,
            notes,
        })
    }

    pub fn to_json(&self) -> JsonValue {
        let mut o = ObjOut::new();
        o.put("source", st(&self.source))
            .put_opt("classMap", self.class_map.as_ref().map(string_map))
            .put("cases", arr(&self.cases, Case::to_json))
            .put_opt(
                "unrepresentable",
                self.unrepresentable
                    .as_deref()
                    .map(|u| arr(u, UnrepresentableRecord::to_json)),
            )
            .put_opt(
                "testFieldsAfterUnchecked",
                self.test_fields_after_unchecked.as_deref().map(st),
            )
            .put_opt(
                "postCallAssertions",
                self.post_call_assertions
                    .as_deref()
                    .map(|p| arr(p, PostCallAssertion::to_json)),
            )
            .put_opt(
                "vacuityNoopClass",
                self.vacuity_noop_class.as_ref().map(|v| match v {
                    VacuityNoopClass::One(c) => st(c),
                    VacuityNoopClass::Many(cs) => strs(cs),
                }),
            );
        for (k, v) in &self.notes {
            o.put(k, v.clone());
        }
        o.build()
    }

    /// The class name a recorded name maps to under `classMap` (DSL.md "classMap").
    pub fn map_class<'a>(&'a self, name: &'a str) -> &'a str {
        self.class_map
            .as_ref()
            .and_then(|m| m.get(name))
            .map_or(name, String::as_str)
    }
}

/// Java `String.split("\\.")`: split at every `.`, then drop trailing empty strings (an input
/// without `.` is returned whole, even when empty).
pub fn java_split_dot(p: &str) -> Vec<&str> {
    if !p.contains('.') {
        return vec![p];
    }
    let mut parts: Vec<&str> = p.split('.').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// Dotted path into a JSON object ("testFields.late"); a decimal segment indexes an array.
/// `None` is Java's `null` (missing); an explicit JSON null is `Some(JsonValue::Null)`.
// port: ReplayDsl#path
pub fn path<'a>(root: &'a JsonValue, p: &str) -> Result<Option<&'a JsonValue>, ModelError> {
    let mut cur = Some(root);
    for part in java_split_dot(p) {
        if let Some(JsonValue::Array(a)) = cur
            && !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
        {
            let i: i32 = match part.parse() {
                Ok(i) => i,
                Err(_) => {
                    return err(
                        p,
                        format!("NumberFormatException: For input string: \"{part}\""),
                    );
                }
            };
            cur = usize::try_from(i).ok().and_then(|i| a.get(i));
            continue;
        }
        match cur {
            Some(JsonValue::Object(m)) => cur = m.get(part),
            _ => return Ok(None),
        }
    }
    Ok(cur)
}

/// The first case whose `when` matches, or `None` (integration and type_check records).
// port: ReplayDsl#selectCaseOrNull
pub fn select_case_or_null<'d>(
    descriptor: Option<&'d Descriptor>,
    record: &JsonValue,
) -> Result<Option<&'d Case>, ModelError> {
    let Some(descriptor) = descriptor else {
        return Ok(None);
    };
    for c in &descriptor.cases {
        if c.matches(record)? {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// As [`select_case_or_null`], but no match is an error (compiler_test_case records).
// port: ReplayDsl#selectCase
pub fn select_case<'d>(
    descriptor: Option<&'d Descriptor>,
    record: &JsonValue,
) -> Result<&'d Case, ModelError> {
    match select_case_or_null(descriptor, record)? {
        Some(c) => Ok(c),
        None => err("$", "no descriptor case matches the record"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_json;

    #[test]
    fn split_like_java() {
        assert_eq!(java_split_dot(""), vec![""]);
        assert_eq!(java_split_dot("a"), vec!["a"]);
        assert_eq!(java_split_dot("a.b."), vec!["a", "b"]);
        assert_eq!(java_split_dot("a..b"), vec!["a", "", "b"]);
        assert_eq!(java_split_dot(".a"), vec!["", "a"]);
        assert!(java_split_dot(".").is_empty());
    }

    #[test]
    fn paths_and_selection() {
        let rec = parse_json(r#"{"a":{"b":[10,{"c":null}]},"n":1}"#).unwrap();
        assert_eq!(path(&rec, "a.b.0").unwrap(), Some(&JsonValue::int(10)));
        assert_eq!(path(&rec, "a.b.1.c").unwrap(), Some(&JsonValue::Null));
        assert_eq!(path(&rec, "a.b.5").unwrap(), None);
        assert_eq!(path(&rec, "a.b.x").unwrap(), None);
        assert_eq!(path(&rec, "a.zz.q").unwrap(), None);
        assert!(path(&rec, "a.b.99999999999").is_err());
        let d = Descriptor::from_json(
            &parse_json(
                r#"{"source":"S","cases":[
                {"when":{"n":2},"processor":{"null":true}},
                {"when":{"a.b.1.c":{"isNull":true},"missing":null,"n":1.0},"note":"x"},
                {"when":{}}]}"#,
            )
            .unwrap(),
            "$",
        )
        .unwrap();
        let c = select_case(Some(&d), &rec).unwrap();
        assert_eq!(c.notes["note"], JsonValue::str("x"));
        let other = parse_json(r#"{"n":3}"#).unwrap();
        assert!(select_case(Some(&d), &other).unwrap().notes.is_empty());
        assert!(select_case(None, &rec).is_err());
    }
}
