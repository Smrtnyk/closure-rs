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

use closure_cli::gson::{json_reader::parse_json_files, json_writer::to_json};
use closure_rhino::java_lang::charset::Charset;
use serde_json::{Value, json};
#[test]
fn json_reader_writer_parity() {
    let rows: Vec<Value> = serde_json::from_str(include_str!("data/json_golden.json")).unwrap();
    let mut failures = Vec::new();
    for row in rows {
        let input = row["input"].as_str().unwrap();
        let expected = &row["result"];
        match parse_json_files(input) {
            Ok(files) => {
                let values:Vec<Value>=files.iter().map(|f|f.as_ref().map_or(Value::Null,|f|json!({"src":f.src.as_ref().map(|s|s.as_units()),"path":f.path.as_ref().map(|s|s.as_units()),"source_map":f.source_map.as_ref().map(|s|s.as_units()),"webpack_id":f.webpack_id.as_ref().map(|s|s.as_units())}))).collect();
                let bytes = closure_cli::java_io::encode(&to_json(&files), Charset::UTF_8, true);
                let actual = json!({"files":values,"output":bytes});
                if actual != *expected {
                    failures.push(format!(
                        "input={input:?}, actual={actual}, expected={expected}"
                    ));
                }
            }
            Err(error) => {
                if expected.get("error").is_none()
                    || expected["error"] != error.0
                    || expected["error_class"] != error.1
                {
                    failures.push(format!(
                        "input={input:?}, actual_error={error:?}, expected={expected}"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
