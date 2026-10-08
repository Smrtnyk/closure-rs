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
    java_lang::{digit, format_message, is_digit, is_letter, parse_int, trim},
    js_string::JsString,
};

// Captured by corpus-cache/parser/JavaLangRound2Samples.java with the pinned JDK.
const JDK_SAMPLES: &str = include_str!("java_lang_parser_samples.txt");

fn units(text: &str) -> JsString {
    JsString::from_units(
        text.split(',')
            .filter(|value| !value.is_empty())
            .map(|value| u16::from_str_radix(value, 16).unwrap())
            .collect::<Vec<_>>(),
    )
}

// port: java.lang.Character#isLetter(char), #isDigit(char), #digit(char, int)
#[test]
fn jdk21_character_samples() {
    for line in JDK_SAMPLES.lines() {
        if let Some(sample) = line.strip_prefix("char ") {
            let fields: Vec<_> = sample.split_whitespace().collect();
            let ch = u16::from_str_radix(fields[0], 16).unwrap();
            assert_eq!(is_letter(ch), fields[1] == "true", "{line}");
            assert_eq!(is_digit(ch), fields[2] == "true", "{line}");
            for (column, radix) in [(3, 2), (4, 16), (5, 36)] {
                assert_eq!(digit(ch, radix), fields[column].parse().unwrap(), "{line}");
            }
        } else if let Some(sample) = line.strip_prefix("radix ") {
            let (radix, expected) = sample.split_once(' ').unwrap();
            assert_eq!(
                digit(u16::from(b'1'), radix.parse().unwrap()),
                expected.parse().unwrap(),
                "{line}"
            );
        }
    }
}

// port: java.lang.String#trim
#[test]
fn jdk21_trim_samples() {
    for line in JDK_SAMPLES.lines() {
        if let Some(sample) = line.strip_prefix("trim ") {
            let (input, expected) = sample.split_once(" -> ").unwrap();
            assert_eq!(trim(&units(input)), units(expected), "{line}");
        }
    }
}

// port: java.lang.String#format
#[test]
fn jdk21_format_sample() {
    let expected = JDK_SAMPLES
        .lines()
        .find_map(|line| line.strip_prefix("format "))
        .unwrap();
    assert_eq!(
        format_message(
            &JsString::from("%s|%c|%d|%04X|%%|%n"),
            &[
                units("D800,0078,DC00"),
                units("D800"),
                JsString::from(i32::MIN.to_string()),
                JsString::from(0xd800u32.to_string()),
            ],
        ),
        units(expected)
    );
}

// port: java.lang.Integer#parseInt(String, int)
#[test]
fn jdk21_parse_int_radix16_samples() {
    for line in JDK_SAMPLES.lines() {
        if let Some(sample) = line.strip_prefix("parseInt ") {
            let (input, expected) = sample.split_once(' ').unwrap();
            let expected = if expected == "exception" {
                None
            } else {
                Some(expected.parse().unwrap())
            };
            assert_eq!(
                parse_int(units(input).as_units(), 0x10).ok(),
                expected,
                "{line}"
            );
        }
    }
}
