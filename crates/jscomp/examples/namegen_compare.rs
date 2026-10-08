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

#[path = "../tests/support/namegen_compare.rs"]
mod compare;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: namegen_compare <corpus-cache/namegen>");
    let dir = Path::new(&dir);
    std::panic::set_hook(Box::new(|_| {}));
    let mut mismatches = 0;
    let mut name_configs = 0;
    let mut name_count = 0;
    let digests = fs::read_to_string(dir.join("names.sha256")).unwrap();
    for line in fs::read_to_string(dir.join("names.tsv")).unwrap().lines() {
        let id = line.split('\t').next().unwrap();
        let mut expected = Vec::new();
        GzDecoder::new(fs::File::open(dir.join("names").join(format!("{id}.gz"))).unwrap())
            .read_to_end(&mut expected)
            .unwrap();
        let actual = compare::generate(line);
        if expected != actual {
            for (i, (e, a)) in expected
                .split(|c| *c == b'\n')
                .zip(actual.split(|c| *c == b'\n'))
                .enumerate()
            {
                if e != a {
                    eprintln!(
                        "{id} line {i}: expected {}, actual {}",
                        String::from_utf8_lossy(e),
                        String::from_utf8_lossy(a)
                    );
                    mismatches += 1;
                }
            }
            if expected.split(|c| *c == b'\n').count() != actual.split(|c| *c == b'\n').count() {
                eprintln!("{id}: output line counts differ");
                mismatches += 1;
            }
        }
        let digest = format!("{id}\t{:x}", Sha256::digest(&actual));
        if !digests.lines().any(|s| s == digest) {
            eprintln!("{id}: SHA-256 differs");
            mismatches += 1;
        }
        if !actual.starts_with(b"ERR|") {
            name_count += actual.iter().filter(|&&b| b == b'\n').count();
        }
        name_configs += 1;
    }
    let mut variable_cases = 0;
    for line in fs::read_to_string(dir.join("variables.expected.tsv"))
        .unwrap()
        .lines()
    {
        let (input, expected) = line.rsplit_once('\t').unwrap();
        let actual = compare::variable(input);
        if actual != expected {
            eprintln!(
                "{}: expected {expected}, actual {actual}",
                input.split('\t').next().unwrap()
            );
            mismatches += 1;
        }
        variable_cases += 1;
    }
    println!(
        "{name_configs} name configurations, {name_count} names, {variable_cases} VariableMap cases, {mismatches} mismatches"
    );
    assert_eq!(mismatches, 0);
}
