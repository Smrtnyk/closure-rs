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

//! Every record of corpus/unit/records loads with zero errors and round-trips losslessly.

use closure_rhino::fast_hash::IndexMap;
use closure_testing::corpus::{file_stem, map_files_parallel, record_files, records};
use closure_testing::corpus_counts::{assert_count, assert_counts_with_prefix};
use closure_testing::record::{Api, RecordKind};

/// (kind, api, "class#method", call) of one record.
type RecordKey = (RecordKind, Api, String, i32);
/// Per file: its stem and its record keys, or the load error.
type FileResult = (String, Result<Vec<RecordKey>, String>);

#[test]
fn all_records_load_and_round_trip() {
    let files = record_files().unwrap();
    assert_count("record_files", files.len());
    // Record by record: each is checked and dropped before the next line is read.
    let results: Vec<FileResult> = map_files_parallel(&files, 4, |f| {
        let r = records(f).map_err(|e| e.to_string()).and_then(|recs| {
            let mut out = Vec::new();
            for lr in recs {
                let lr = lr.map_err(|e| e.to_string())?;
                let back = lr.record.to_json();
                if back != lr.raw {
                    return Err(format!(
                        "{}:{}: round trip differs",
                        f.display(),
                        lr.index + 1
                    ));
                }
                let r = lr.record;
                out.push((r.kind, r.api, format!("{}#{}", r.class, r.method), r.call));
            }
            Ok(out)
        });
        (file_stem(f), r)
    });
    let mut errors = Vec::new();
    let mut total = 0usize;
    let mut by_kind: IndexMap<&'static str, usize> = IndexMap::<_, _>::default();
    let mut by_api: IndexMap<&'static str, usize> = IndexMap::<_, _>::default();
    let mut nonempty = 0usize;
    let mut table = String::from("stem\ttotal\tcompiler_test_case\tintegration\ttype_check\n");
    println!("stem\ttotal\tcompiler_test_case\tintegration\ttype_check");
    for (stem, r) in results {
        match r {
            Err(e) => errors.push(e),
            Ok(recs) => {
                let mut per = [0usize; 3];
                let mut calls: IndexMap<String, i32> = IndexMap::<_, _>::default();
                for (kind, api, cm, call) in &recs {
                    per[*kind as usize] += 1;
                    *by_kind.entry(kind.name()).or_default() += 1;
                    *by_api.entry(api.name()).or_default() += 1;
                    let next = calls.entry(cm.clone()).or_insert(0);
                    assert_eq!(*call, *next, "{stem}: call numbering of {cm}");
                    *next += 1;
                }
                total += recs.len();
                if !recs.is_empty() {
                    nonempty += 1;
                }
                let row = format!("{stem}\t{}\t{}\t{}\t{}", recs.len(), per[0], per[1], per[2]);
                println!("{row}");
                table.push_str(&row);
                table.push('\n');
            }
        }
    }
    for e in errors.iter().take(20) {
        eprintln!("{e}");
    }
    assert!(errors.is_empty(), "{} files failed to load", errors.len());
    // Per-class counts and totals, checked against the counts scripts/unit_corpus_counts.py
    // writes independently with Python's json module (tests/data/record_counts.tsv and
    // tests/data/corpus_counts.tsv).
    let expected = include_str!("data/record_counts.tsv");
    for (got, want) in table.lines().zip(expected.lines()) {
        assert_eq!(
            got, want,
            "per-class record counts (tests/data/record_counts.tsv; after a corpus change, \
             `python3 scripts/unit_corpus_counts.py` rewrites it)"
        );
    }
    assert_eq!(
        table.lines().count(),
        expected.lines().count(),
        "record_counts.tsv rows (after a corpus change, `python3 scripts/unit_corpus_counts.py` \
         rewrites it)"
    );
    println!("records: {total}, non-empty files: {nonempty}, kinds: {by_kind:?}, apis: {by_api:?}");
    assert_count("records", total);
    assert_count("recorded_files", nonempty);
    let pairs =
        |m: &IndexMap<&'static str, usize>| m.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>();
    assert_counts_with_prefix("kind.", &pairs(&by_kind));
    assert_counts_with_prefix("api.", &pairs(&by_api));
}
