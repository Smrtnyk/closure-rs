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

//! The unit records of the six transpile-es2015 test classes (Es6RewriteClassTest,
//! Es6ConvertSuperTest, Es6RewriteArrowFunctionTest, Es6RewriteDestructuringTest,
//! Es6RewriteRestParametersTest, Es6RewriteSpreadExpressionsTest) through the record replay:
//! every record passes.
use closure_testing::{corpus, replay::replay_main::Runner};

const CLASSES: [&str; 6] = [
    "Es6RewriteClassTest",
    "Es6ConvertSuperTest",
    "Es6RewriteArrowFunctionTest",
    "Es6RewriteDestructuringTest",
    "Es6RewriteRestParametersTest",
    "Es6RewriteSpreadExpressionsTest",
];

// port: ReplayMain#main (the records of the transpile-es2015 test classes)
#[test]
fn transpile_es2015_records_pass() {
    let mut failures = vec![];
    let mut records = 0;
    let report = Runner {
        corpus: corpus::corpus_unit_dir(),
        classes: Some(CLASSES.iter().map(|c| c.to_string()).collect()),
        sample: None,
    }
    // On 4 threads; the sinks still see the records in order.
    .run_on(
        4,
        |r| {
            records += 1;
            if r.status != "pass" {
                failures.push(format!(
                    "{}#{} [{}]: {} {:?} {:?}",
                    r.class, r.method, r.index, r.status, r.why, r.unported_by
                ));
            }
            Ok(())
        },
        |_, _| {},
    )
    .unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!(records, 388);
    assert_eq!(report.harness_errors, 0);
    for class in CLASSES {
        let counts = &report.classes[class];
        assert_eq!(counts.pass, counts.records, "{class}");
        assert_eq!(counts.pass_with_omitted_passes, 0, "{class}");
    }
}
