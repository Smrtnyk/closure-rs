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

//! zcat standard.tsv.gz | cargo run --release -p closure-rhino --example dtoa_compare -- standard
//! Other kinds: modes (JS_dtostr), raw (all internal JS_dtoa modes), and jdk.
use closure_rhino::dtoa::d_to_a;
use closure_rhino::java_lang::double_to_string;
use std::io::{self, BufRead};
#[path = "../tests/support/dtoa_records.rs"]
mod dtoa_records;
use dtoa_records::{Outcome, decode, outcome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kind = std::env::args()
        .nth(1)
        .ok_or("expected standard, modes, raw, or jdk")?;
    if !["standard", "modes", "raw", "jdk"].contains(&kind.as_str()) {
        return Err("expected standard, modes, raw, or jdk".into());
    }
    let (mut cases, mut doubles, mut infeasible, mut infeasible_doubles) = (0u64, 0u64, 0u64, 0u64);
    let mut exceptions = 0u64;
    let mut mismatches = [0u64; 2];
    let mut printed = 0;
    for line in io::stdin().lock().lines() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        let bits = u64::from_str_radix(fields[0], 16)?;
        let v = f64::from_bits(bits);
        cases += 1;
        let mut failed = false;
        let mut actual_description = String::new();
        if kind == "jdk" {
            if fields.len() != 2 {
                return Err("expected two jdk columns".into());
            }
            doubles += 1;
            let actual = double_to_string(v);
            failed = actual != fields[1];
            actual_description = actual;
        } else {
            let (encoded, first) = match kind.as_str() {
                "standard" => {
                    if fields.len() != 3 {
                        return Err("expected three standard columns".into());
                    }
                    let jdk = double_to_string(v);
                    if jdk != fields[2] {
                        mismatches[1] += 1;
                        if printed < 20 {
                            eprintln!(
                                "case={cases} bits={bits:016x} double_to_string: expected={:?} actual={jdk:?}",
                                fields[2]
                            );
                            printed += 1;
                        }
                    }
                    (fields[1], true)
                }
                "modes" => {
                    if fields.len() != 4 {
                        return Err("expected four modes columns".into());
                    }
                    (
                        fields[3],
                        fields[1].parse::<i32>()? == d_to_a::DTOSTR_STANDARD_EXPONENTIAL,
                    )
                }
                _ => {
                    if fields.len() != 7 && fields.len() != 5 {
                        return Err(
                            "expected seven raw columns (five for exception/infeasible)".into()
                        );
                    }
                    (
                        fields[fields.len() - 1],
                        fields[1] == "-1" && fields[2] == "false",
                    )
                }
            };
            if first {
                doubles += 1;
            }
            let expected = decode(encoded)?;
            if matches!(expected, Outcome::Infeasible(_)) {
                infeasible += 1;
                if first {
                    infeasible_doubles += 1;
                }
            } else {
                if matches!(expected, Outcome::Exception { .. }) {
                    exceptions += 1;
                }
                let mut buffer = Vec::new();
                let result = match kind.as_str() {
                    "standard" => d_to_a::number_to_string(v),
                    "modes" => {
                        d_to_a::js_dtostr(&mut buffer, fields[1].parse()?, fields[2].parse()?, v)
                            .map(|()| buffer)
                    }
                    _ => {
                        let mut sign = false;
                        let result = d_to_a::js_dtoa(
                            v,
                            fields[1].parse()?,
                            fields[2].parse()?,
                            fields[3].parse()?,
                            &mut sign,
                            &mut buffer,
                        );
                        if let Ok(decpt) = result
                            && (fields.len() != 7
                                || decpt != fields[4].parse::<i32>()?
                                || sign != fields[5].parse::<bool>()?)
                        {
                            failed = true;
                        }
                        result.map(|_| buffer)
                    }
                };
                let actual = outcome(result);
                failed |= actual != expected;
                if failed {
                    actual_description = format!("{actual:?}");
                }
            }
        }
        if failed {
            mismatches[0] += 1;
            if printed < 20 {
                eprintln!(
                    "case={cases} bits={bits:016x}: input={line:?} actual={actual_description}"
                );
                printed += 1;
            }
        }
    }
    if kind == "standard" {
        println!(
            "standard: doubles={doubles} cases={cases} number_to_string_mismatches={} double_to_string_mismatches={} exceptions={exceptions} infeasible={infeasible} infeasible_doubles={infeasible_doubles}",
            mismatches[0], mismatches[1]
        );
    } else {
        println!(
            "{kind}: doubles={doubles} cases={cases} mismatches={} exceptions={exceptions} infeasible={infeasible} infeasible_doubles={infeasible_doubles}",
            mismatches[0]
        );
    }
    if mismatches.iter().any(|&n| n != 0) {
        std::process::exit(1);
    }
    Ok(())
}
