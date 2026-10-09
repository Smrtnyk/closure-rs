/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/LightweightMessageFormatter.java,
//   src/com/google/javascript/jscomp/SortingErrorManager.java.

#[path = "support/d2_replay.rs"]
mod d2_replay;
use closure_rhino::fx_hash::IndexMap;
use serde_json::Value;
// port: SortingErrorManager#generateReport / LightweightMessageFormatter#format (JVM D2 fixture)
#[test]
fn d2_stderr_fixture() {
    let mut types = IndexMap::<_, _>::default();
    let mut rows = 0;
    for line in include_str!("data/d2_stderr_fixture.jsonl").lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        assert_eq!(d2_replay::exclusion(&row), None);
        let actual = d2_replay::render(&row, &mut types);
        for field in ["stderr", "golden_stderr", "report_stderr"] {
            let expected = row[field].as_str().unwrap();
            assert_eq!(
                actual,
                expected,
                "{} / {} {field}\n{}",
                row["case_id"],
                row["profile"],
                d2_replay::unified_diff(expected, &actual)
            );
        }
        rows += 1;
    }
    assert_eq!(rows, 40);
}
