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

//! Cross-file checks over the whole unit corpus:
//! - DSL.md case selection: every compiler_test_case record selects a descriptor case;
//! - descriptor `unrepresentable` entries name existing records;
//! - derived/expected_pipeline.jsonl.gz has exactly one entry per compiler_test_case record, keyed
//!   by (file, index), with the record's class, method and call;
//! - the 9 record lines with lone-surrogate escapes keep them through the reader and writer.

use closure_rhino::fast_hash::IndexMap;
use closure_testing::corpus::{
    LoadedDescriptor, file_stem, jsonl_lines, load_descriptor_for, load_expected_pipeline,
    map_files_parallel, record_files, records,
};
use closure_testing::descriptor::{select_case, select_case_or_null};
use closure_testing::json::JsonValue;
use std::sync::OnceLock;

/// The case DSL.md selects for a record.
enum Selection {
    Case,
    /// No case, which only a non-compiler_test_case record may have.
    NoCase,
    Error(String),
}

/// What the checks below read from one raw record line.
struct RawRecord {
    kind: String,
    class: String,
    method: String,
    call: Option<i64>,
    /// `None` when the file's descriptor did not load.
    selection: Option<Selection>,
    lone_surrogate: bool,
}

/// One record file (sorted): its stem, its descriptor (loaded when the file has records) and its
/// records.
struct RawFile {
    stem: String,
    descriptor: Option<Result<Option<LoadedDescriptor>, String>>,
    records: Vec<RawRecord>,
}

/// Every record file, read in one pass: each line is parsed, its case selected and its fields
/// taken, and its JSON dropped before the next line is read (the whole corpus's JSON values would
/// take several GB).
fn raw_records() -> &'static Vec<RawFile> {
    static RAW: OnceLock<Vec<RawFile>> = OnceLock::new();
    RAW.get_or_init(|| {
        map_files_parallel(&record_files().unwrap(), 4, |f| {
            let stem = file_stem(f);
            let mut descriptor = None;
            let mut records = Vec::new();
            for line in jsonl_lines(f).unwrap() {
                let raw = line.unwrap().parse(f).unwrap();
                let d = descriptor
                    .get_or_insert_with(|| load_descriptor_for(&stem).map_err(|e| e.to_string()));
                let kind = string(&raw, "kind");
                let selection = match d {
                    Ok(Some(ld)) => Some(if kind == "compiler_test_case" {
                        match select_case(Some(&ld.descriptor), &raw) {
                            Ok(_) => Selection::Case,
                            Err(e) => Selection::Error(e.to_string()),
                        }
                    } else {
                        match select_case_or_null(Some(&ld.descriptor), &raw) {
                            Ok(Some(_)) => Selection::Case,
                            Ok(None) => Selection::NoCase,
                            Err(e) => Selection::Error(e.to_string()),
                        }
                    }),
                    _ => None,
                };
                records.push(RawRecord {
                    class: string(&raw, "class"),
                    method: string(&raw, "method"),
                    call: int(&raw, "call"),
                    kind,
                    selection,
                    lone_surrogate: has_lone_surrogate(&raw),
                });
            }
            RawFile {
                stem,
                descriptor,
                records,
            }
        })
    })
}

fn string(raw: &JsonValue, key: &str) -> String {
    raw.get(key)
        .and_then(JsonValue::as_js_string)
        .and_then(|v| v.to_string_strict())
        .unwrap_or_default()
}

fn int(raw: &JsonValue, key: &str) -> Option<i64> {
    raw.get(key)
        .and_then(JsonValue::as_number)
        .and_then(|n| n.as_i64())
}

#[test]
fn case_selection_and_unrepresentable_entries() {
    let mut selected: IndexMap<String, usize> = IndexMap::<_, _>::default();
    let mut no_match_optional: IndexMap<String, usize> = IndexMap::<_, _>::default();
    let mut errors = Vec::new();
    let mut ctc = 0usize;
    for RawFile {
        stem,
        descriptor,
        records: recs,
    } in raw_records()
    {
        if recs.is_empty() {
            continue;
        }
        let ld = descriptor
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap_or_else(|| panic!("{stem}: no descriptor"));
        let d = &ld.descriptor;
        for (i, raw) in recs.iter().enumerate() {
            let kind = raw.kind.clone();
            if kind == "compiler_test_case" {
                ctc += 1;
            }
            match raw.selection.as_ref().unwrap() {
                Selection::Case => *selected.entry(kind).or_default() += 1,
                Selection::NoCase => *no_match_optional.entry(kind).or_default() += 1,
                Selection::Error(e) => errors.push(format!("{stem}:{}: {e}", i + 1)),
            }
        }
        for u in d.unrepresentable.iter().flatten() {
            let raw = usize::try_from(u.index)
                .ok()
                .and_then(|i| recs.get(i))
                .unwrap_or_else(|| panic!("{stem}: unrepresentable index {}", u.index));
            if let Some(m) = &u.method {
                assert_eq!(&raw.method, m, "{stem}: unrepresentable {}", u.index);
            }
            if let Some(c) = u.call {
                assert_eq!(
                    raw.call.unwrap(),
                    i64::from(c),
                    "{stem}: unrepresentable {}",
                    u.index
                );
            }
        }
    }
    for e in errors.iter().take(20) {
        eprintln!("{e}");
    }
    assert!(errors.is_empty(), "{} records without a case", errors.len());
    println!("selected a case: {selected:?}; no case (optional): {no_match_optional:?}");
    assert_eq!(ctc, 21_308);
    assert_eq!(selected["compiler_test_case"], 21_308);
}

#[test]
fn expected_pipeline_matches_records() {
    let entries = load_expected_pipeline().unwrap();
    let mut by_key: IndexMap<(String, i64), &closure_testing::derived::ExpectedPipeline> =
        IndexMap::<_, _>::default();
    for (e, _) in &entries {
        assert!(
            by_key
                .insert((e.file.clone(), i64::from(e.index)), e)
                .is_none(),
            "duplicate expected_pipeline key {}:{}",
            e.file,
            e.index
        );
    }
    let mut matched = 0usize;
    for RawFile {
        stem,
        records: recs,
        ..
    } in raw_records()
    {
        for (i, raw) in recs.iter().enumerate() {
            if raw.kind != "compiler_test_case" {
                continue;
            }
            let i = i64::try_from(i).unwrap();
            let e = by_key
                .get(&(stem.clone(), i))
                .unwrap_or_else(|| panic!("{stem}:{i}: no expected_pipeline entry"));
            assert_eq!(e.class, raw.class, "{stem}:{i} class");
            assert_eq!(e.method, raw.method, "{stem}:{i} method");
            assert_eq!(i64::from(e.call), raw.call.unwrap(), "{stem}:{i} call");
            matched += 1;
        }
    }
    assert_eq!(matched, entries.len());
    assert_eq!(matched, 21_308);
}

fn has_lone_surrogate(v: &JsonValue) -> bool {
    match v {
        JsonValue::String(s) => s.has_lone_surrogate(),
        JsonValue::Array(a) => a.iter().any(has_lone_surrogate),
        JsonValue::Object(o) => o
            .iter()
            .any(|(k, x)| has_lone_surrogate(&JsonValue::str(k)) || has_lone_surrogate(x)),
        _ => false,
    }
}

#[test]
fn lone_surrogates_survive() {
    let mut found = Vec::new();
    for RawFile {
        stem,
        records: recs,
        ..
    } in raw_records()
    {
        for (i, raw) in recs.iter().enumerate() {
            if raw.lone_surrogate {
                found.push(format!("{stem}:{i}"));
            }
        }
    }
    let want: Vec<String> = [
        ("PeepholeReplaceKnownMethodsTest", 38),
        ("PeepholeReplaceKnownMethodsTest", 39),
        ("PeepholeReplaceKnownMethodsTest", 303),
        ("PeepholeReplaceKnownMethodsTest", 304),
        ("PeepholeReplaceKnownMethodsTest", 589),
        ("PeepholeReplaceKnownMethodsTest", 590),
        ("PeepholeReplaceKnownMethodsTest", 591),
        ("PeepholeReplaceKnownMethodsTest", 592),
        ("SerializeAndDeserializeAstTest", 62),
    ]
    .iter()
    .map(|(s, i)| format!("{s}:{i}"))
    .collect();
    assert_eq!(found, want, "record lines with lone surrogates");

    // The typed records keep them: written back, each line still has a lone-surrogate escape
    // and re-reads to the same JSON.
    let dir = closure_testing::corpus::corpus_unit_dir().join("records");
    for stem in [
        "PeepholeReplaceKnownMethodsTest",
        "SerializeAndDeserializeAstTest",
    ] {
        let recs = records(&dir.join(format!("{stem}.jsonl.gz"))).unwrap();
        for lr in recs
            .map(Result::unwrap)
            .filter(|lr| has_lone_surrogate(&lr.raw))
        {
            let back = lr.record.to_json();
            assert!(has_lone_surrogate(&back), "{stem}:{}", lr.index);
            let text = back.to_json_string();
            let reread = closure_testing::json::parse_json(&text).unwrap();
            assert_eq!(reread, lr.raw, "{stem}:{}", lr.index);
            assert!(
                text.to_ascii_lowercase().contains("\\ud"),
                "{stem}:{}: escape written",
                lr.index
            );
        }
    }
}
