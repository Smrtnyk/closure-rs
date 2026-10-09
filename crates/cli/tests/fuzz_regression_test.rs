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

//! Regression cases for differential-fuzzing fixes (fuzz/findings/): each directory under
//! tests/data/fuzz_regressions/ holds the inputs, the argv and the Java reference bytes captured
//! by tools/generate_fuzz_regressions.py. The binary runs in a scratch copy of the case with
//! the golden environment, and its exit code, stdout, stderr and every file it creates must be
//! identical to Java's.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn files_under(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "closure-rs-fuzz-regression-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn fuzz_regressions_match_java_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fuzz_regressions");
    let mut names: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("case.json").is_file())
        .collect();
    names.sort();
    assert!(!names.is_empty());
    let mut failures = vec![];
    for case in &names {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(case.join("case.json")).unwrap()).unwrap();
        let args: Vec<&str> = meta["argv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let work = scratch(&name);
        let inputs: BTreeMap<String, Vec<u8>> = files_under(case)
            .into_iter()
            .filter(|(k, _)| k != "case.json" && !k.starts_with("expected/"))
            .collect();
        for (k, v) in &inputs {
            let p = work.join(k);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, v).unwrap();
        }
        let out = Command::new(env!("CARGO_BIN_EXE_closure-rs"))
            .args(&args)
            .current_dir(&work)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C.UTF-8")
            .env("LC_ALL", "C.UTF-8")
            .env("TZ", "UTC")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let made: BTreeMap<String, Vec<u8>> = files_under(&work)
            .into_iter()
            .filter(|(k, _)| !inputs.contains_key(k))
            .collect();
        let expected_files = files_under(&case.join("expected/files"));
        let mut why = vec![];
        if out.status.code() != meta["exit_code"].as_i64().map(|c| c as i32) {
            why.push(format!(
                "exit {:?} != {}",
                out.status.code(),
                meta["exit_code"]
            ));
        }
        if out.stdout != std::fs::read(case.join("expected/stdout")).unwrap() {
            why.push(format!(
                "stdout differs:\n{}",
                String::from_utf8_lossy(&out.stdout)
            ));
        }
        if out.stderr != std::fs::read(case.join("expected/stderr")).unwrap() {
            why.push(format!(
                "stderr differs:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        if made.keys().ne(expected_files.keys()) {
            why.push(format!(
                "output files {:?} != {:?}",
                made.keys().collect::<Vec<_>>(),
                expected_files.keys().collect::<Vec<_>>()
            ));
        } else {
            for (k, v) in &made {
                if expected_files[k] != *v {
                    why.push(format!(
                        "output {k} differs:\n{}",
                        String::from_utf8_lossy(v)
                    ));
                }
            }
        }
        if !why.is_empty() {
            failures.push(format!("{name} ({args:?}): {}", why.join("; ")));
        }
        let _ = std::fs::remove_dir_all(&work);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
