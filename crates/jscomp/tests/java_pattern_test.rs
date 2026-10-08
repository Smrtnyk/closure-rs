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

use closure_rhino::java_lang::pattern::Pattern;
use serde_json::Value;
// port: PatternExpectationsDump#main
#[test]
fn java_line_terminators_match_find_groups_and_replacements() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("data/pattern_expectations.json")).unwrap();
    assert_eq!(rows.len(), 224);
    for row in rows {
        let source = row["pattern"].as_str().unwrap();
        let input = row["input"].as_str().unwrap();
        let pattern = Pattern::compile(source);
        assert_eq!(pattern.pattern(), source);
        assert_eq!(pattern.flags(), 0);
        assert_eq!(pattern.to_string(), source);
        assert_eq!(
            pattern.matcher(input).matches(),
            row["matches"].as_bool().unwrap(),
            "{row}"
        );
        let mut matcher = pattern.matcher(input);
        for groups in row["finds"].as_array().unwrap() {
            assert!(matcher.find(), "{row}");
            for (i, expected) in groups.as_array().unwrap().iter().enumerate() {
                assert_eq!(matcher.group(i), expected.as_str(), "group {i}: {row}");
            }
        }
        assert!(!matcher.find(), "{row}");
        assert_eq!(
            pattern.matcher(input).replace_all(""),
            row["replaceEmpty"].as_str().unwrap(),
            "{row}"
        );
        assert_eq!(
            pattern.matcher(input).replace_all("<$0>"),
            row["replaceGroup"].as_str().unwrap(),
            "{row}"
        );
    }
}
