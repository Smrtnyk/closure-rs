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

#[path = "support/namegen_compare.rs"]
mod compare;
use sha2::{Digest, Sha256};
use std::io::Read;

#[test]
fn java_name_generation_golden() {
    let configs = include_str!("data/names.tsv");
    let digests = include_str!("data/names.sha256");
    for (config, expected) in configs.lines().zip(digests.lines()) {
        let id = config.split('\t').next().unwrap();
        assert_eq!(
            format!("{id}\t{:x}", Sha256::digest(compare::generate(config))),
            expected,
            "{id}"
        );
    }
    assert_eq!(configs.lines().count(), digests.lines().count());
}
#[test]
fn java_variable_map_golden() {
    let mut cases = String::new();
    flate2::read::GzDecoder::new(&include_bytes!("data/variables.expected.tsv.gz")[..])
        .read_to_string(&mut cases)
        .unwrap();
    for line in cases.lines() {
        let (input, expected) = line.rsplit_once('\t').unwrap();
        assert_eq!(
            compare::variable(input),
            expected,
            "{}",
            input.split('\t').next().unwrap()
        );
        if input.split('\t').nth(1) == Some("S") {
            for chunk_size in [1, 2, 3, 7] {
                assert_eq!(
                    compare::variable_with_stream_chunk(input, chunk_size),
                    expected,
                    "{input}, chunk size {chunk_size}"
                );
            }
        }
    }
}
