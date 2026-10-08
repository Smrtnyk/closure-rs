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

use closure_rhino::{
    java_lang::{big_integer::parse_big_integer, double::parse_double},
    js_string::JsString,
};

fn units(encoded: &str) -> Vec<u16> {
    assert_eq!(encoded.len() % 4, 0, "invalid fixture UTF-16 encoding");
    encoded
        .as_bytes()
        .chunks_exact(4)
        .map(|unit| u16::from_str_radix(std::str::from_utf8(unit).unwrap(), 16).unwrap())
        .collect()
}

// Rust-only differential regression against the pinned JDK 21. The fixture
// includes each record; failures compare NumberFormatException's exact WTF-16
// message, and successes compare raw IEEE 754 bits (including signed zero).
#[test]
fn jdk21_numeric_parse_differential() {
    let mut doubles = 0;
    let mut big_integers = 0;
    for (line_number, line) in include_str!("data/jdk_numeric_parse.tsv")
        .lines()
        .enumerate()
    {
        if line.starts_with('#') {
            continue;
        }
        let row: Vec<&str> = line.split('\t').collect();
        assert_eq!(row.len(), 6, "fixture line {}", line_number + 1);
        let input = JsString::from_units(units(row[3]));
        let context = format!(
            "fixture line {}, {} {}, radix {}, input units {}",
            line_number + 1,
            row[0],
            row[1],
            row[2],
            row[3]
        );
        match row[0] {
            "double" => {
                doubles += 1;
                match parse_double(&input) {
                    Ok(value) => {
                        assert_eq!(row[4], "OK", "{context}: unexpectedly accepted");
                        let expected = u64::from_str_radix(row[5], 16).unwrap();
                        assert_eq!(value.to_bits(), expected, "{context}");
                    }
                    Err(error) => {
                        assert_eq!(row[4], "ERR", "{context}: unexpectedly rejected");
                        assert_eq!(error.message, units(row[5]), "{context}");
                    }
                }
            }
            "bigint" => {
                big_integers += 1;
                let radix = row[2].parse().unwrap();
                match parse_big_integer(&input, radix) {
                    Ok(value) => {
                        assert_eq!(row[4], "OK", "{context}: unexpectedly accepted");
                        assert_eq!(value.to_string(), row[5], "{context}");
                    }
                    Err(error) => {
                        assert_eq!(row[4], "ERR", "{context}: unexpectedly rejected");
                        assert_eq!(error.message().as_units(), units(row[5]), "{context}");
                    }
                }
            }
            kind => panic!("unknown fixture kind {kind}: {context}"),
        }
    }
    // Produced by generate_numeric_fixture.py, checked here to ensure all rows
    // remain exercised when the fixture or the record decoder is edited.
    assert_eq!(doubles, 4330);
    assert_eq!(big_integers, 2350);
}
