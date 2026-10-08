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

use closure_rhino::java_lang::double::parse_double;
use closure_rhino::js_string::JsString;
use flate2::read::GzDecoder;
use serde_json::Value;
use std::io::Read;

// JDK 21 oracle values include hexadecimal ties, subnormal boundaries, long
// significands, exponent overflow, decimal rounding, suffixes and rejected grammar.
#[test]
fn parse_double_matches_jdk_21() {
    let mut json = String::new();
    GzDecoder::new(&include_bytes!("data/java_double.json.gz")[..])
        .read_to_string(&mut json)
        .unwrap();
    let cases: Value = serde_json::from_str(&json).unwrap();
    for case in cases.as_array().unwrap() {
        let input = if let Some(text) = case["input"].as_str() {
            JsString::from(text)
        } else {
            JsString::from_units(
                case["input"]["utf16"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|unit| unit.as_u64().unwrap() as u16)
                    .collect::<Vec<_>>(),
            )
        };
        let actual = parse_double(&input);
        if let Some(bits) = case["bits"].as_str() {
            assert_eq!(
                actual.unwrap().to_bits(),
                u64::from_str_radix(bits, 16).unwrap(),
                "{input:?}"
            );
        } else {
            assert!(actual.is_err(), "JDK rejects {input:?}");
        }
    }
}
