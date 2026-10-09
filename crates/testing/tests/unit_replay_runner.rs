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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

use closure_testing::{
    corpus,
    json::parse_json,
    replay::replay_main::{Report, Runner},
};
use indexmap::IndexSet;

const SAMPLE: usize = 16;

// Rust-only: the number of records the sampled run must classify (every SAMPLE-th of each class)
fn sampled_record_count() -> usize {
    let mut files = std::fs::read_dir(corpus::corpus_unit_dir().join("records"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    files.sort();
    let (mut all, mut sampled) = (0, 0);
    // Streamed (one record in memory per thread) and 4 files at a time.
    let counts = corpus::map_files_parallel(&files, 4, |file| {
        corpus::records(file).unwrap().map(Result::unwrap).count()
    });
    for n in counts {
        all += n;
        sampled += n.div_ceil(SAMPLE);
    }
    assert_eq!(all, 24768);
    sampled
}

// port: ReplayMain#main / compare (structural corpus coverage)
#[test]
fn every_record_is_classified_once_and_the_report_round_trips() {
    let mut identities = IndexSet::new();
    let mut classes = 0;
    let report = Runner {
        corpus: corpus::corpus_unit_dir(),
        classes: None,
        // every 16th record: the structure is checked here; gates/unit_rust.sh replays all records
        // in a release build and fails on any harness error
        sample: Some(SAMPLE),
    }
    // The classes on 4 threads; the sinks below still see them, and their records, in order.
    .run_on(
        4,
        |r| {
            assert!(
                identities.insert((r.class.clone(), r.index)),
                "duplicate record"
            );
            assert!(matches!(r.status.as_str(), "pass" | "fail" | "unported"));
            assert_eq!(r.status == "unported", r.unported_by.is_some());
            Ok(())
        },
        |_, counts| {
            classes += 1;
            assert_eq!(counts.records, counts.pass + counts.fail + counts.unported);
            assert_eq!(counts.unported, counts.unported_by.values().sum::<usize>());
        },
    )
    .unwrap();
    assert_eq!(classes, 432);
    assert_eq!(identities.len(), sampled_record_count());
    assert_eq!(report.totals.records, identities.len());
    assert_eq!(
        report.totals.records,
        report.totals.pass + report.totals.fail + report.totals.unported
    );
    assert_eq!(
        report.totals.records,
        report.classes.values().map(|c| c.records).sum::<usize>()
    );
    assert_eq!(report.harness_errors, 0);
    let encoded = report.to_json();
    let decoded = Report::from_json(&parse_json(&encoded.to_json_string()).unwrap()).unwrap();
    assert!(decoded.to_json().gson_equals(&encoded));
    assert_eq!(decoded, report);
}
