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

#[path = "support/whitespace_compiler.rs"]
mod whitespace_compiler;

#[test]
fn whitespace_compile_matches_reference_candidates() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/compiler_whitespace.json")).unwrap();
    assert_eq!(cases.len(), 32);
    assert!(
        cases
            .iter()
            .map(|c| c["inputs"].as_array().unwrap().len())
            .sum::<usize>()
            >= 30
    );
    for case in cases {
        assert_eq!(
            whitespace_compiler::compile(&case),
            case["expected"],
            "{}",
            case["id"]
        );
    }
}
