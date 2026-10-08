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

use closure_rhino::dtoa::d_to_a::DToAError;

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Value(Vec<u16>),
    Exception { class: String, message: String },
    Infeasible(i32),
}

pub fn decode(encoded: &str) -> Result<Outcome, Box<dyn std::error::Error>> {
    if let Some(value) = encoded.strip_prefix('=') {
        let mut units = Vec::new();
        let mut bytes = value.bytes();
        while let Some(c) = bytes.next() {
            if c == b'\\' {
                match bytes.next().ok_or("truncated escape")? {
                    b'\\' => units.push(b'\\' as u16),
                    b'u' => {
                        let mut unit = 0u16;
                        for _ in 0..4 {
                            let c = bytes.next().ok_or("truncated UTF-16 escape")?;
                            let digit = match c {
                                b'0'..=b'9' => c - b'0',
                                b'a'..=b'f' => c - b'a' + 10,
                                _ => return Err("invalid lowercase UTF-16 escape".into()),
                            };
                            unit = unit * 16 + digit as u16;
                        }
                        units.push(unit);
                    }
                    _ => return Err("invalid escape".into()),
                }
            } else if (0x20..=0x7e).contains(&c) {
                units.push(c as u16);
            } else {
                return Err("unencoded UTF-16 unit".into());
            }
        }
        Ok(Outcome::Value(units))
    } else if let Some(exception) = encoded.strip_prefix('!') {
        let (class, message) = exception
            .split_once(": ")
            .ok_or("missing exception message")?;
        Ok(Outcome::Exception {
            class: class.into(),
            message: message.into(),
        })
    } else if let Some(k) = encoded.strip_prefix("?infeasible k=") {
        let k: i32 = k.parse()?;
        if !(20000 < (k as i64).abs() && (k as i64).abs() < 715827894) {
            return Err("invalid infeasibility estimate".into());
        }
        Ok(Outcome::Infeasible(k))
    } else {
        Err("missing DToA outcome marker".into())
    }
}

pub fn outcome(actual: Result<Vec<u16>, DToAError>) -> Outcome {
    match actual {
        Ok(units) => Outcome::Value(units),
        Err(error) => Outcome::Exception {
            class: error.java_class().into(),
            message: error.to_string(),
        },
    }
}
