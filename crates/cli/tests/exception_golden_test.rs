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

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn java_runtime_exception_boundaries() {
    // The recording's input files lived in a directory of the recording machine, which the golden
    // names {tmp}: this run creates them in a directory of its own and substitutes it everywhere.
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("exception-golden-{}", std::process::id()));
    let tmp = serde_json::to_string(tmp.to_str().unwrap()).unwrap(); // escaped for the JSON text
    let golden =
        include_str!("data/exception_golden.json").replace("{tmp}", &tmp[1..tmp.len() - 1]);
    let cases: serde_json::Value = serde_json::from_str(&golden).unwrap();
    for case in cases.as_array().unwrap() {
        if let Some(files) = case["files"].as_object() {
            for (path, content) in files {
                std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
                std::fs::write(path, content.as_str().unwrap()).unwrap();
            }
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_closure-rs"))
            .args(
                case["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(case["stdin"].as_str().unwrap().as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert_eq!(
            result.status.code().unwrap(),
            case["exit"].as_i64().unwrap() as i32,
            "{}",
            case["name"]
        );
        assert_eq!(
            result.stdout,
            case["stdout"].as_str().unwrap().as_bytes(),
            "{}",
            case["name"]
        );
        assert_eq!(
            result.stderr,
            case["stderr"].as_str().unwrap().as_bytes(),
            "{}",
            case["name"]
        );
    }
}
