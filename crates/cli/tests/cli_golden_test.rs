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

use std::{
    path::Path,
    process::{Command, Stdio},
};
#[test]
fn golden_cli_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/cli_golden");
    let rows: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(root.join("cases.json")).unwrap()).unwrap();
    assert!(rows.len() >= 150);
    for row in &rows {
        let args: Vec<_> = row["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let output = Command::new(env!("CARGO_BIN_EXE_closure-rs"))
            .args(&args)
            .current_dir(&root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(row["exit_code"].as_i64().unwrap() as i32),
            "argv={args:?}"
        );
        assert_eq!(
            output.stdout,
            std::fs::read(root.join(row["stdout"].as_str().unwrap())).unwrap(),
            "stdout argv={args:?}"
        );
        assert_eq!(
            output.stderr,
            std::fs::read(root.join(row["stderr"].as_str().unwrap())).unwrap(),
            "stderr argv={args:?}"
        );
    }
}
