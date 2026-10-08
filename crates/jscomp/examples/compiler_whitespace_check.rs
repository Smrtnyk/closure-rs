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

//! Scratch full-candidates comparison using the same Compiler API as the committed regression.
#[path = "../tests/support/whitespace_compiler.rs"]
mod whitespace_compiler;
use std::io::{BufRead, Write};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "requests.jsonl results.jsonl");
    let input = std::io::BufReader::new(std::fs::File::open(&args[1]).unwrap());
    let mut output = std::io::BufWriter::new(std::fs::File::create(&args[2]).unwrap());
    for (index, line) in input.lines().enumerate() {
        let case: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        let mut result = whitespace_compiler::compile(&case);
        result["id"] = case["id"].clone();
        serde_json::to_writer(&mut output, &result).unwrap();
        writeln!(output).unwrap();
        output.flush().unwrap();
        if (index + 1) % 50 == 0 {
            eprintln!("Compared {}", index + 1);
        }
    }
}
