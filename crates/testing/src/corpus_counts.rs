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

//! The corpus counts the tests pin: `tests/data/corpus_counts.tsv` (record files, records per
//! kind and per API, descriptors and their cases, `expected_pipeline` lines, CompilerOptions
//! fields and Java configurations) and `tests/data/record_counts.tsv` (the records of each test
//! class). `scripts/unit_corpus_counts.py` writes both from the corpus with Python's `json`
//! module; the unit-install step of a sync runs it and `gates/ci.sh` checks them with `--check`.
//!
//! A test compares a count it computes through the Rust reader (or the Rust CompilerOptions) with
//! the committed one. A reader that drops or invents an entry, or a field the Rust side forgets,
//! fails the test; a changed corpus shows as a diff of the counts files.

const CORPUS_COUNTS: &str = include_str!("../tests/data/corpus_counts.tsv");
const RECORD_COUNTS: &str = include_str!("../tests/data/record_counts.tsv");
const REWRITE: &str = "python3 scripts/unit_corpus_counts.py";

/// The rows after the column names, split at tabs.
fn rows(text: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    text.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').collect())
}

/// The committed count `name` (a row of `tests/data/corpus_counts.tsv`).
pub fn count(name: &str) -> usize {
    rows(CORPUS_COUNTS)
        .find(|r| r[0] == name)
        .map(|r| r[1].parse().unwrap())
        .unwrap_or_else(|| {
            panic!("tests/data/corpus_counts.tsv has no count {name:?}; `{REWRITE}` writes it")
        })
}

/// The committed counts whose name starts with `prefix` (for example `api.`), keyed by the rest
/// of the name, in the file's order.
pub fn counts_with_prefix(prefix: &str) -> Vec<(String, usize)> {
    rows(CORPUS_COUNTS)
        .filter_map(|r| {
            Some((
                r[0].strip_prefix(prefix)?.to_string(),
                r[1].parse().unwrap(),
            ))
        })
        .collect()
}

/// The number of records of `class` (the `total` column of `tests/data/record_counts.tsv`).
pub fn record_count(class: &str) -> usize {
    rows(RECORD_COUNTS)
        .find(|r| r[0] == class)
        .map(|r| r[1].parse().unwrap())
        .unwrap_or_else(|| {
            panic!("tests/data/record_counts.tsv has no class {class:?}; `{REWRITE}` writes it")
        })
}

fn stale(what: &str, actual: &str, expected: &str) -> String {
    format!(
        "{what}: the Rust side counts {actual}, tests/data/corpus_counts.tsv has {expected}. \
         After a corpus change, `{REWRITE}` rewrites the counts (`--check` lists the stale ones); \
         if the corpus did not change, the Rust side dropped or invented entries."
    )
}

/// Asserts that `actual`, a count the Rust side computed, equals the committed count `name`.
#[track_caller]
pub fn assert_count(name: &str, actual: usize) {
    let expected = count(name);
    assert!(
        actual == expected,
        "{}",
        stale(name, &actual.to_string(), &expected.to_string())
    );
}

/// Asserts that `actual`, counts the Rust side computed, equals the committed counts whose name
/// starts with `prefix`; both are compared sorted by name.
#[track_caller]
pub fn assert_counts_with_prefix(prefix: &str, actual: &[(&str, usize)]) {
    let mut expected = counts_with_prefix(prefix);
    expected.sort();
    let mut actual: Vec<(String, usize)> =
        actual.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    actual.sort();
    assert!(
        actual == expected,
        "{}",
        stale(
            &format!("{prefix}*"),
            &format!("{actual:?}"),
            &format!("{expected:?}")
        )
    );
}

/// Asserts that `actual` is the number of records of `class` in `tests/data/record_counts.tsv`.
#[track_caller]
pub fn assert_record_count(class: &str, actual: usize) {
    let expected = record_count(class);
    assert!(
        actual == expected,
        "{class}: the Rust side replays {actual} records, tests/data/record_counts.tsv has \
         {expected}. After a corpus change, `{REWRITE}` rewrites the counts (`--check` lists the \
         stale ones); if the corpus did not change, the replay dropped or invented records."
    );
}
