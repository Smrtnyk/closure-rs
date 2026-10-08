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

#[path = "../tests/support/d2_replay.rs"]
mod d2_replay;
use indexmap::IndexMap;
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};
fn main() {
    let file = std::env::args().nth(1).expect("JSONL path");
    let mut types = IndexMap::new();
    let mut rows = 0;
    let mut identical = 0;
    let mut excluded = IndexMap::<&str, usize>::new();
    let mut mismatches = 0;
    for line in BufReader::new(File::open(file).unwrap()).lines() {
        let row: Value = serde_json::from_str(&line.unwrap()).unwrap();
        rows += 1;
        let reason = d2_replay::exclusion(&row);
        if let Some(reason) = reason
            && row["generated_report"] == false
        {
            *excluded.entry(reason).or_default() += 1;
            eprintln!("EXCLUDED {} / {}: {reason}", row["case_id"], row["profile"]);
            continue;
        }
        let actual = d2_replay::render(&row, &mut types);
        let expected = row["report_stderr"].as_str().unwrap();
        let golden = row["golden_stderr"].as_str().unwrap();
        let captured = row["stderr"].as_str().unwrap();
        if actual != expected || reason.is_none() && (actual != golden || actual != captured) {
            mismatches += 1;
            if mismatches <= 5 {
                eprintln!(
                    "{} / {}\n{}",
                    row["case_id"],
                    row["profile"],
                    d2_replay::unified_diff(
                        if actual != expected { expected } else { golden },
                        &actual
                    )
                );
            }
        } else if let Some(reason) = reason {
            *excluded.entry(reason).or_default() += 1;
            eprintln!("EXCLUDED {} / {}: {reason}", row["case_id"], row["profile"]);
        } else {
            identical += 1;
        }
    }
    println!(
        "rows={rows} identical={identical} excluded={} mismatches={mismatches}",
        excluded.values().sum::<usize>()
    );
    for (reason, count) in excluded {
        println!("excluded {count}: {reason}");
    }
    if mismatches != 0 {
        std::process::exit(1);
    }
}
