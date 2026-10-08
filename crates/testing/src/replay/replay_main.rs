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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayCompilerTest.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! ReplayMain comparison, classification and corpus runner.
#![allow(clippy::collapsible_if)] // Retain the Java branches.
use crate::{
    corpus::{self, LoadedRecord},
    derived::ExpectedPipeline,
    descriptor::{self, Descriptor},
    json::JsonValue,
    record::{Api, ComparisonMode, Expected, RecordKind},
    replay::{
        registry::Registry,
        replay_compiler_test::{ReplayCompilerTest, expected_test_fields_after},
        replay_integration_test, replay_type_check_test,
        replay_values::object,
    },
    throwable::Throwable,
};
use indexmap::IndexMap;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct ReplayResult {
    pub outcome: JsonValue,
    pub observed: JsonValue,
    pub post_call: Option<JsonValue>,
    pub test_fields_after_want: Option<JsonValue>,
    pub exception_mapped: bool,
    /// Rust-only: the first unported pass factory (Java class name) whose execution the
    /// compile omitted (Compiler#getOmittedUnportedPasses). A record that fails with it set is
    /// reported `unported`, attributed to that class; one that passes stays `pass`.
    pub omitted_unported: Option<String>,
}
impl ReplayResult {
    // port: ReplayCompilerTest#run (exception outcome)
    pub fn from_run(result: Result<(), Throwable>) -> Result<Self, Throwable> {
        let (outcome, exception_mapped) = match result {
            Ok(()) => (object([("status", JsonValue::str("normal"))]), false),
            Err(e @ Throwable::Unported(_)) | Err(e @ Throwable::HarnessError(_)) => return Err(e),
            Err(Throwable::Assertion { message }) => (
                object([
                    ("status", JsonValue::str("exception")),
                    ("message", JsonValue::str(&message)),
                    ("assertion", JsonValue::Bool(true)),
                ]),
                false,
            ),
            Err(Throwable::Exception { class, message }) => (
                object([
                    ("status", JsonValue::str("exception")),
                    ("exceptionClass", JsonValue::str(&class)),
                    (
                        "message",
                        message.as_deref().map_or(JsonValue::Null, JsonValue::str),
                    ),
                    ("assertion", JsonValue::Bool(false)),
                ]),
                true,
            ),
        };
        Ok(Self {
            outcome,
            observed: object([]),
            post_call: None,
            test_fields_after_want: None,
            exception_mapped,
            omitted_unported: None,
        })
    }
    // port: ReplayMain#replayOne (DESIGN.md precondition exception representation)
    pub fn catch_precondition_run(
        record: &JsonValue,
        action: impl FnOnce() -> Result<(), Throwable>,
    ) -> Result<Self, Throwable> {
        match catch_unwind(AssertUnwindSafe(action)) {
            Ok(run) => Self::from_run(run),
            // A typed Java exception thrown through the compiler (Compiler#parseForCompilation).
            Err(payload)
                if payload
                    .downcast_ref::<closure_jscomp::compiler_options_preprocessor::InvalidOptionsException>()
                    .is_some() =>
            {
                let e = payload
                    .downcast_ref::<closure_jscomp::compiler_options_preprocessor::InvalidOptionsException>()
                    .unwrap();
                Self::from_run(Err(Throwable::Exception {
                    class: "com.google.javascript.jscomp.CompilerOptionsPreprocessor$InvalidOptionsException"
                        .into(),
                    message: Some(e.get_message().into()),
                }))
            }
            Err(payload) => {
                let result = Self::from_panic(payload.as_ref());
                if reproduces_precondition(record, &result) {
                    // Keep the fixture alive so diagnostics/post-state are checked after the throw.
                    Ok(result)
                } else {
                    std::panic::resume_unwind(payload)
                }
            }
        }
    }
    // port: ReplayMain#replayOne (native panic payload)
    pub fn from_panic(payload: &(dyn std::any::Any + Send)) -> Self {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied());
        let mut result = Self::panic(message.unwrap_or("non-string panic"));
        if message.is_none() {
            // The fallback is a report label, never a Java exception message.
            if let JsonValue::Object(outcome) = &mut result.outcome {
                outcome.insert("panicMessageAvailable".into(), JsonValue::Bool(false));
            }
        }
        result
    }
    // port: ReplayMain#replayOne (panics are exceptions, never Unported)
    pub fn panic(message: &str) -> Self {
        Self {
            outcome: object([
                ("status", JsonValue::str("exception")),
                ("panic", JsonValue::Bool(true)),
                ("message", JsonValue::str(message)),
                ("assertion", JsonValue::Bool(false)),
            ]),
            observed: object([]),
            post_call: None,
            test_fields_after_want: None,
            exception_mapped: false,
            omitted_unported: None,
        }
    }
}
const STACK_OVERFLOW_ERROR: &str = "java.lang.StackOverflowError";
// port: ReplayMain#replayOne
pub fn replay_one(
    stem: &str,
    record: &LoadedRecord,
    descriptor: Option<&Descriptor>,
    registry: Registry,
    pipeline: Option<&ExpectedPipeline>,
) -> Result<ReplayResult, Throwable> {
    // FORMAT.md outcome contract and D-010: `java.lang.StackOverflowError` records are
    // depth-sensitive and reported separately. Java catches the overflow; a native run cannot (the
    // whole process aborts), so the record is not run.
    if let Some(JsonValue::String(class)) = record
        .raw
        .get("outcome")
        .and_then(|outcome| outcome.get("exceptionClass"))
        && class.eq_str(STACK_OVERFLOW_ERROR)
    {
        return Err(Throwable::Unported(format!(
            "{STACK_OVERFLOW_ERROR} (depth-sensitive, D-010)"
        )));
    }
    let class_map = descriptor
        .and_then(|d| d.class_map.clone())
        .unwrap_or_default();
    match record.record.kind {
        RecordKind::CompilerTestCase => {
            let case = descriptor::select_case(descriptor, &record.raw)
                .map_err(|e| Throwable::HarnessError(e.to_string()))?;
            ReplayCompilerTest::new(stem, record, case, class_map, registry).run(pipeline)
        }
        RecordKind::Integration => {
            let case = descriptor::select_case_or_null(descriptor, &record.raw)
                .map_err(|e| Throwable::HarnessError(e.to_string()))?;
            replay_integration_test::replay(stem, record, case, class_map, registry)
        }
        RecordKind::TypeCheck => {
            let case = descriptor::select_case_or_null(descriptor, &record.raw)
                .map_err(|e| Throwable::HarnessError(e.to_string()))?;
            replay_type_check_test::replay(stem, record, case, class_map, registry)
        }
    }
}
// port: ReplayMain#compare (FORMAT.md non-Java outcome mapping and neutral equality)
pub fn compare(record: &JsonValue, result: &ReplayResult) -> Option<String> {
    if result.outcome.get("panic") == Some(&JsonValue::Bool(true))
        && !reproduces_precondition(record, result)
    {
        return Some(format!(
            "panic (outcome exception): {}",
            result
                .outcome
                .get("message")
                .unwrap_or(&JsonValue::Null)
                .to_json_string()
        ));
    }
    let want = record.get("outcome")?;
    let ws = want.get("status");
    let gs = result.outcome.get("status");
    if ws != gs {
        return Some(format!(
            "outcome {} != recorded {}",
            result.outcome.to_json_string(),
            want.to_json_string()
        ));
    }
    if ws == Some(&JsonValue::str("exception")) {
        if want.get("assertion") == Some(&JsonValue::Bool(true))
            && result.outcome.get("assertion") != Some(&JsonValue::Bool(true))
        {
            return Some("recorded assertion was not reproduced".into());
        }
        if result.exception_mapped {
            if want.get("exceptionClass") != result.outcome.get("exceptionClass") {
                return Some("exception class differs".into());
            }
            let got = truncate_message(
                result.outcome.get("message").unwrap_or(&JsonValue::Null),
                4000,
            );
            if !got.gson_equals(want.get("message").unwrap_or(&JsonValue::Null)) {
                return Some("exception message differs".into());
            }
        }
    }
    if let Some(wp) = record.get("postCall") {
        if result
            .post_call
            .as_ref()
            .is_none_or(|gp| !neutral_equal(wp, gp))
        {
            return Some("postCall differs".into());
        }
    }
    if let Some(got) = result.observed.get("testFieldsAfter") {
        if result
            .test_fields_after_want
            .as_ref()
            .is_none_or(|want| !neutral_equal(want, got))
        {
            return Some("testFieldsAfter differs".into());
        }
    }
    if let Some(observed) = record.get("observed").and_then(JsonValue::as_object) {
        for (key, want) in observed {
            if key == "compilerClass" {
                continue;
            }
            let Some(got) = result.observed.get(key) else {
                return Some(format!("observed.{key} missing in replay"));
            };
            if !want.gson_equals(got) {
                return Some(format!("observed.{key} differs"));
            }
        }
    }
    None
}
// port: ReplayMain#compare (DESIGN.md preconditions panic with Java message text)
fn reproduces_precondition(record: &JsonValue, result: &ReplayResult) -> bool {
    let Some(want) = record.get("outcome") else {
        return false;
    };
    result.outcome.get("panicMessageAvailable") != Some(&JsonValue::Bool(false))
        && want.get("status") == Some(&JsonValue::str("exception"))
        && want.get("assertion") != Some(&JsonValue::Bool(true))
        && [
            "java.lang.IllegalStateException",
            "java.lang.IllegalArgumentException",
            "java.lang.NullPointerException",
            "java.lang.ClassCastException",
            // NodeTraversal#throwInternalError wraps its cause in a RuntimeException whose
            // message ("INTERNAL COMPILER ERROR.\nPlease report this problem.\n\n" plus the
            // wrapped message) the Rust panic reproduces; the exact comparison below applies.
            "java.lang.RuntimeException",
        ]
        .iter()
        .any(|class| want.get("exceptionClass") == Some(&JsonValue::str(class)))
        && truncate_message(
            result.outcome.get("message").unwrap_or(&JsonValue::Null),
            4000,
        )
        .gson_equals(want.get("message").unwrap_or(&JsonValue::Null))
}
// port: ReplayMain#truncate
fn truncate_message(value: &JsonValue, n: usize) -> JsonValue {
    let Some(s) = value.as_js_string() else {
        return value.clone();
    };
    if s.0.len() <= n {
        return value.clone();
    }
    let mut units = s.0[..n].to_vec();
    units.extend(format!("...<truncated {} chars>", s.0.len() - n).encode_utf16());
    JsonValue::String(crate::json::JsString(units))
}
// port: ReplayMain#postconditionCounts
pub fn postcondition_counts(
    record: &LoadedRecord,
    descriptor: Option<&Descriptor>,
) -> Option<(i32, i32)> {
    if record.record.kind != RecordKind::CompilerTestCase {
        return None;
    }
    let recorded = match &record.record.expected {
        Some(Expected::CompilerTestCase { postconditions, .. }) => *postconditions,
        _ => 0,
    };
    let replayed = descriptor::select_case(descriptor, &record.raw).map_or(-1, |case| {
        case.postconditions.as_ref().map_or(0, |p| p.len() as i32)
    });
    Some((recorded, replayed))
}
// port: ReplayMain#postState
pub fn post_state(record: &LoadedRecord, descriptor: Option<&Descriptor>) -> Option<JsonValue> {
    let r = &record.record;
    let mut state = match r.kind {
        RecordKind::TypeCheck => {
            if r.comparison.mode == ComparisonMode::ObservedOnly {
                Some(object([
                    ("postState", JsonValue::str("notCaptured")),
                    (
                        "reason",
                        JsonValue::str(
                            "type_check observed_only: the test's assertions on the returned TypeCheckResult are not recorded",
                        ),
                    ),
                ]))
            } else {
                None
            }
        }
        RecordKind::Integration => {
            if r.api == Api::Compile {
                Some(object([
                    ("postState", JsonValue::str("notCaptured")),
                    ("integrationCompile", JsonValue::Bool(true)),
                    (
                        "reason",
                        JsonValue::str(
                            "integration compile: the test's assertions on the returned compiler are not recorded",
                        ),
                    ),
                ]))
            } else {
                None
            }
        }
        RecordKind::CompilerTestCase => compiler_post_state(record, descriptor),
    };
    if let Some((recorded, replayed)) = postcondition_counts(record, descriptor) {
        let o = state.get_or_insert_with(|| object([]));
        if let JsonValue::Object(o) = o {
            o.insert(
                "postconditionsRecorded".into(),
                JsonValue::int(i64::from(recorded)),
            );
            o.insert(
                "postconditionsReplayed".into(),
                JsonValue::int(i64::from(replayed)),
            );
        }
    }
    state
}
// port: ReplayMain#postState (compiler_test_case branch)
fn compiler_post_state(
    record: &LoadedRecord,
    descriptor: Option<&Descriptor>,
) -> Option<JsonValue> {
    let (recorded, replayed) = postcondition_counts(record, descriptor).unwrap();
    if recorded != replayed {
        return Some(object([
            ("postState", JsonValue::str("notCaptured")),
            (
                "reason",
                JsonValue::str(&format!(
                    "postconditions: the original call ran {recorded}, the selected case replays {replayed}"
                )),
            ),
        ]));
    }
    let case = descriptor::select_case(descriptor, &record.raw).ok();
    let after = record.record.test_fields_after.as_ref();
    let changed = after.is_some_and(|f| !f.is_empty());
    if let Some(case) = case.filter(|c| {
        c.test_fields_after
            .as_ref()
            .is_some_and(|e| !matches!(e, crate::dsl::Expr::JsonNull))
    }) {
        let want = expected_test_fields_after(&record.raw, case);
        if let Ok(want) = &want {
            let mut fields = want
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            fields.sort();
            if !fields.is_empty() {
                return Some(object([
                    ("postState", JsonValue::str("checked")),
                    (
                        "comparedFields",
                        JsonValue::Array(fields.iter().map(|f| JsonValue::str(f)).collect()),
                    ),
                ]));
            }
        }
        if changed {
            return Some(object([
                ("postState", JsonValue::str("unclassified")),
                (
                    "reason",
                    JsonValue::str(&format!(
                        "the case opts in to testFieldsAfter but compares no field{}; testFieldsAfter changed {}",
                        want.err().map_or(String::new(), |e| format!(" ({e})")),
                        field_names(after)
                    )),
                ),
            ]));
        }
        return None;
    }
    if !changed {
        return None;
    }
    let why = case
        .and_then(|c| c.test_fields_after_unchecked.as_deref())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            descriptor
                .and_then(|d| d.test_fields_after_unchecked.as_deref())
                .filter(|s| !s.trim().is_empty())
        });
    Some(match why {
        Some(reason) => object([
            ("postState", JsonValue::str("notCaptured")),
            ("reason", JsonValue::str(reason)),
        ]),
        None => object([
            ("postState", JsonValue::str("unclassified")),
            (
                "reason",
                JsonValue::str(&format!(
                    "testFieldsAfter changed {} but the case neither checks it nor gives testFieldsAfterUnchecked",
                    field_names(after)
                )),
            ),
        ]),
    })
}
// port: ReplayMain#postState (key-set display)
fn field_names(fields: Option<&crate::value::FieldDump>) -> String {
    format!(
        "[{}]",
        fields
            .map(|f| f.keys().cloned().collect::<Vec<_>>().join(", "))
            .unwrap_or_default()
    )
}
const UNORDERED: &[&str] = &[
    "java.util.HashMap",
    "java.util.HashSet",
    "com.google.common.collect.HashBiMap",
    "com.google.common.collect.HashBiMap$Inverse",
    "com.google.common.collect.AbstractMapBasedMultimap$WrappedSet",
    "com.google.common.collect.AbstractMapBasedMultimap$AsMap",
    "com.google.common.collect.HashBasedTable",
];
// port: ReplayMain#compare (FORMAT.md neutral_equal, scripts/unit_postcall_impls.py)
pub fn neutral_equal(a: &JsonValue, b: &JsonValue) -> bool {
    neutral(a).gson_equals(&neutral(b))
}
// port: ReplayMain#compare (FORMAT.md neutral projection)
fn neutral(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Array(a) => JsonValue::Array(a.iter().map(neutral).collect()),
        JsonValue::Object(o) => {
            let tag = ["list", "set", "map", "table"]
                .into_iter()
                .find(|tag| o.contains_key(*tag));
            if let Some(tag) = tag {
                let Some(items) = o[tag].as_array() else {
                    return value.clone();
                };
                let mut items = items.iter().map(neutral).collect::<Vec<_>>();
                let unordered = tag != "list"
                    && o.get("impl")
                        .and_then(JsonValue::as_js_string)
                        .is_some_and(|s| UNORDERED.iter().any(|c| s.eq_str(c)));
                if unordered {
                    items.sort_by_cached_key(|v| {
                        let key = if tag == "map" {
                            v.as_array()
                                .and_then(|v| v.first())
                                .cloned()
                                .unwrap_or(JsonValue::Null)
                        } else if tag == "table" {
                            JsonValue::Array(
                                v.as_array()
                                    .map(|v| v.iter().take(2).cloned().collect())
                                    .unwrap_or_default(),
                            )
                        } else {
                            v.clone()
                        };
                        canonical_json(&key)
                    });
                }
                object([(tag, JsonValue::Array(items))])
            } else {
                JsonValue::Object(o.iter().map(|(k, v)| (k.clone(), neutral(v))).collect())
            }
        }
        _ => value.clone(),
    }
}
// port: ReplayMain#compare (neutral canonical key)
fn canonical_json(value: &JsonValue) -> String {
    // port: ReplayMain#compare (neutral canonical object-key ordering)
    fn sorted(v: &JsonValue) -> JsonValue {
        match v {
            JsonValue::Object(o) => {
                let mut fields = o.iter().collect::<Vec<_>>();
                fields.sort_by_key(|(a, _)| *a);
                JsonValue::Object(
                    fields
                        .into_iter()
                        .map(|(k, v)| (k.clone(), sorted(v)))
                        .collect(),
                )
            }
            JsonValue::Array(a) => JsonValue::Array(a.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    sorted(value).to_json_string()
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub records: usize,
    pub pass: usize,
    pub fail: usize,
    pub unported: usize,
    pub unported_by: IndexMap<String, usize>,
    /// Rust-only: passing records whose compile omitted an unported pass.
    pub pass_with_omitted_passes: usize,
}
impl Counts {
    // port: ReplayMain#main (per-class accumulation)
    fn add(&mut self, r: &RecordResult) {
        self.records += 1;
        match r.status.as_str() {
            "pass" => {
                self.pass += 1;
                if r.omitted_unported.is_some() {
                    self.pass_with_omitted_passes += 1;
                }
            }
            "unported" => {
                self.unported += 1;
                *self
                    .unported_by
                    .entry(r.unported_by.clone().unwrap())
                    .or_default() += 1;
            }
            _ => self.fail += 1,
        }
    }
    // port: ReplayMain#main (machine-readable counts)
    pub fn to_json(&self, with_items: bool) -> JsonValue {
        let mut o = object([
            ("records", JsonValue::int(self.records as i64)),
            ("pass", JsonValue::int(self.pass as i64)),
            ("fail", JsonValue::int(self.fail as i64)),
            ("unported", JsonValue::int(self.unported as i64)),
            (
                "passWithOmittedPasses",
                JsonValue::int(self.pass_with_omitted_passes as i64),
            ),
        ]);
        if with_items {
            if let JsonValue::Object(o) = &mut o {
                o.insert(
                    "unportedBy".into(),
                    JsonValue::Object(
                        self.unported_by
                            .iter()
                            .map(|(k, v)| (k.clone(), JsonValue::int(*v as i64)))
                            .collect(),
                    ),
                );
            }
        }
        o
    }
}
#[derive(Clone, Debug)]
pub struct RecordResult {
    pub class: String,
    pub index: usize,
    pub method: String,
    pub call: i32,
    pub kind: String,
    pub api: String,
    pub status: String,
    pub why: Option<String>,
    pub unported_by: Option<String>,
    /// Rust-only: see ReplayResult#omitted_unported.
    pub omitted_unported: Option<String>,
    pub post_state: Option<JsonValue>,
    pub unrepresentable: Option<String>,
}
impl RecordResult {
    // port: ReplayMain#main (record classification output)
    pub fn to_json(&self) -> JsonValue {
        object([
            ("class", JsonValue::str(&self.class)),
            ("index", JsonValue::int(self.index as i64)),
            ("method", JsonValue::str(&self.method)),
            ("call", JsonValue::int(i64::from(self.call))),
            ("kind", JsonValue::str(&self.kind)),
            ("api", JsonValue::str(&self.api)),
            ("status", JsonValue::str(&self.status)),
            (
                "why",
                self.why.as_deref().map_or(JsonValue::Null, JsonValue::str),
            ),
            (
                "unportedBy",
                self.unported_by
                    .as_deref()
                    .map_or(JsonValue::Null, JsonValue::str),
            ),
            (
                "omittedUnported",
                self.omitted_unported
                    .as_deref()
                    .map_or(JsonValue::Null, JsonValue::str),
            ),
            (
                "postState",
                self.post_state.clone().unwrap_or(JsonValue::Null),
            ),
            (
                "unrepresentable",
                self.unrepresentable
                    .as_deref()
                    .map_or(JsonValue::Null, JsonValue::str),
            ),
        ])
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub totals: Counts,
    pub classes: IndexMap<String, Counts>,
    pub harness_errors: usize,
}
impl Report {
    // port: ReplayMain#main (report output)
    pub fn to_json(&self) -> JsonValue {
        object([
            ("v", JsonValue::int(1)),
            ("totals", self.totals.to_json(false)),
            (
                "classes",
                JsonValue::Object(
                    self.classes
                        .iter()
                        .map(|(k, v)| (k.clone(), v.to_json(true)))
                        .collect(),
                ),
            ),
        ])
    }
    // port: ReplayMain#main (report read-back verification)
    pub fn from_json(v: &JsonValue) -> Result<Self, Throwable> {
        // port: ReplayMain#main (JSON count decoding)
        fn counts(v: &JsonValue) -> Result<Counts, Throwable> {
            let num = |k: &str| {
                v.get(k)
                    .and_then(JsonValue::as_number)
                    .and_then(|n| n.as_i64())
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| Throwable::HarnessError(format!("invalid report count {k}")))
            };
            let mut out = Counts {
                records: num("records")?,
                pass: num("pass")?,
                fail: num("fail")?,
                unported: num("unported")?,
                unported_by: IndexMap::new(),
                pass_with_omitted_passes: if v.get("passWithOmittedPasses").is_some() {
                    num("passWithOmittedPasses")?
                } else {
                    0
                },
            };
            if let Some(m) = v.get("unportedBy").and_then(JsonValue::as_object) {
                for (k, v) in m {
                    out.unported_by.insert(
                        k.clone(),
                        v.as_number()
                            .and_then(|n| n.as_i64())
                            .and_then(|n| usize::try_from(n).ok())
                            .ok_or_else(|| Throwable::HarnessError("invalid item count".into()))?,
                    );
                }
            }
            Ok(out)
        }
        if v.get("v") != Some(&JsonValue::int(1)) {
            return Err(Throwable::HarnessError("invalid report version".into()));
        }
        let mut totals = counts(
            v.get("totals")
                .ok_or_else(|| Throwable::HarnessError("report totals missing".into()))?,
        )?;
        let classes: IndexMap<String, Counts> = v
            .get("classes")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| Throwable::HarnessError("report classes missing".into()))?
            .iter()
            .map(|(k, v)| Ok((k.clone(), counts(v)?)))
            .collect::<Result<_, Throwable>>()?;
        for counts in classes.values() {
            for (item, count) in &counts.unported_by {
                *totals.unported_by.entry(item.clone()).or_default() += count;
            }
        }
        Ok(Self {
            totals,
            classes,
            harness_errors: 0,
        })
    }
}
pub struct Runner {
    pub corpus: PathBuf,
    pub classes: Option<Vec<String>>,
    /// Rust-only: replay only the records whose index is a multiple of `n`. The debug-built
    /// structural test samples (a full replay in a debug build takes over 10 minutes); the gate's
    /// release run (`gates/unit_rust.sh`, `unit_replay --all`) replays every record.
    pub sample: Option<usize>,
}
impl Runner {
    // port: ReplayMain#main
    pub fn run(
        &self,
        mut record_sink: impl FnMut(&RecordResult) -> Result<(), Throwable>,
        mut class_sink: impl FnMut(&str, &Counts),
    ) -> Result<Report, Throwable> {
        let registry = Registry::load(&self.corpus)?;
        let path = self.corpus.join("derived/expected_pipeline.jsonl.gz");
        let text = corpus::read_gz(&path).map_err(load_error)?;
        let pipelines = corpus::parse_jsonl(&path, &text)
            .map_err(load_error)?
            .iter()
            .map(|v| {
                ExpectedPipeline::from_json(v, "$")
                    .map_err(|e| Throwable::HarnessError(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|p| ((p.file.clone(), p.index as usize), p))
            .collect::<IndexMap<_, _>>();
        let mut files = std::fs::read_dir(self.corpus.join("records"))
            .map_err(|e| Throwable::HarnessError(e.to_string()))?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Throwable::HarnessError(e.to_string()))?;
        files.retain(|p| p.to_string_lossy().ends_with(".jsonl.gz"));
        files.sort();
        let mut report = Report::default();
        if let Some(classes) = &self.classes {
            for class in classes {
                if !files.iter().any(|p| corpus::file_stem(p) == *class) {
                    return Err(Throwable::HarnessError(format!(
                        "unknown record class {class}"
                    )));
                }
            }
        }
        for file in files {
            let stem = corpus::file_stem(&file);
            if self.classes.as_ref().is_some_and(|cs| !cs.contains(&stem)) {
                continue;
            }
            let descriptor_path = self.corpus.join("descriptors").join(format!("{stem}.json"));
            let d = if descriptor_path.exists() {
                Some(corpus::load_descriptor(&descriptor_path).map_err(load_error)?)
            } else {
                None
            };
            let descriptor = d.as_ref().map(|d| &d.descriptor);
            let mut counts = Counts::default();
            for record in corpus::load_records(&file).map_err(load_error)? {
                if self.sample.is_some_and(|n| record.index % n != 0) {
                    continue;
                }
                let pipeline = pipelines
                    .get(&(format!("{stem}.jsonl.gz"), record.index))
                    .or_else(|| pipelines.get(&(stem.clone(), record.index)));
                if record.record.kind == RecordKind::CompilerTestCase && pipeline.is_none() {
                    return Err(Throwable::HarnessError(format!(
                        "expected pipeline missing for {stem}:{}",
                        record.index
                    )));
                }
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    replay_one(&stem, &record, descriptor, registry.clone(), pipeline)
                }));
                let outcome = match outcome {
                    Ok(r) => r,
                    Err(p) => Ok(ReplayResult::from_panic(p.as_ref())),
                };
                let mut omitted_unported = None;
                let (status, why, unported_by) = match outcome {
                    Err(Throwable::Unported(item)) => ("unported", None, Some(item)),
                    Err(e) => {
                        if matches!(e, Throwable::HarnessError(_)) {
                            report.harness_errors += 1;
                        }
                        ("fail", Some(e.to_string()), None)
                    }
                    Ok(result) => {
                        omitted_unported = result.omitted_unported.clone();
                        match compare(&record.raw, &result) {
                            // Rust-only: classify by outcome.
                            Some(_) if omitted_unported.is_some() => {
                                ("unported", None, omitted_unported.clone())
                            }
                            Some(why) => ("fail", Some(why), None),
                            None => ("pass", None, None),
                        }
                    }
                };
                let rr = RecordResult {
                    class: stem.clone(),
                    index: record.index,
                    method: record.record.method.clone(),
                    call: record.record.call,
                    kind: record.record.kind.name().into(),
                    api: record.record.api.name().into(),
                    status: status.into(),
                    why,
                    unported_by,
                    omitted_unported,
                    post_state: post_state(&record, descriptor),
                    unrepresentable: descriptor
                        .and_then(|d| d.unrepresentable.as_ref())
                        .and_then(|us| us.iter().find(|u| u.index as usize == record.index))
                        .map(|u| u.reason.clone()),
                };
                record_sink(&rr)?;
                counts.add(&rr);
                report.totals.add(&rr);
            }
            class_sink(&stem, &counts);
            report.classes.insert(stem, counts);
        }
        Ok(report)
    }
}
// port: ReplayMain#main (loader failures)
fn load_error(error: impl std::fmt::Display) -> Throwable {
    Throwable::HarnessError(error.to_string())
}
// port: ReplayMain#main (default corpus path)
pub fn default_corpus() -> PathBuf {
    corpus::corpus_unit_dir()
}
// port: ReplayMain#main (external report file)
pub fn write_report(path: &Path, report: &Report) -> Result<(), Throwable> {
    std::fs::write(path, format!("{}\n", report.to_json().to_json_string())).map_err(load_error)
}
