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
//   scripts/unit_options_defaults/OptionsDefaultsDump.java.

mod support;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_testing::{corpus::load_options_defaults, json::JsonValue};
// port: OptionsDefaultsDump#main
#[test]
fn all_compiler_options_defaults_match_java() {
    let (defaults, _) = load_options_defaults().unwrap();
    assert_eq!(defaults.field_types.len(), 205);
    let expected = defaults.to_json();
    let mut expected = expected.get("fields").unwrap().clone();
    let mut actual = support::options_fields(&CompilerOptions::new());
    support::normalize_standins(&mut expected);
    support::normalize_standins(&mut actual);
    assert_eq!(actual.as_object().unwrap().len(), 205);
    for (name, value) in actual.as_object().unwrap() {
        assert!(
            defaults.field_types.contains_key(name),
            "Missing Java field schema for {name}"
        );
        assert_eq!(
            value,
            expected.get(name).unwrap(),
            "CompilerOptions default {name}"
        );
    }
    assert!(matches!(actual, JsonValue::Object(_)));
}
