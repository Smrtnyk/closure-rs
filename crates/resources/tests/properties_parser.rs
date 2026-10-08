/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/resources/PropertiesParser.java.

//! `PropertiesParser#parse` against Java's results (fixture properties_parser.tsv).

mod common;

use closure_resources::resources::properties_parser::PropertiesParser;
use common::{fixture_rows, panic_message, unescape};

// port: PropertiesParser#parse (fixture properties_parser.tsv)
#[test]
fn parse_matches_java() {
    let rows = fixture_rows("properties_parser.tsv");
    assert_eq!(rows.len(), 38);
    for row in rows {
        let input = unescape(&row[0]);
        let expected = row.get(1).cloned().unwrap_or_default();
        if let Some(exception) = expected.strip_prefix("EXC:") {
            let message = exception
                .strip_prefix("java.lang.IllegalArgumentException:")
                .unwrap();
            let source = input.clone();
            let actual = panic_message(move || PropertiesParser::parse(&source));
            assert_eq!(actual, unescape(message), "input {input:?}");
        } else {
            let actual: Vec<String> = PropertiesParser::parse(&input)
                .iter()
                .map(|(k, v)| format!("{k}=>{v}"))
                .collect();
            let expected: Vec<String> = if expected.is_empty() {
                Vec::new()
            } else {
                expected.split('\u{1}').map(unescape).collect()
            };
            assert_eq!(actual, expected, "input {input:?}");
        }
    }
}
