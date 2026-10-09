/*
 * Copyright 2008 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/DiagnosticType.java.

use closure_jscomp::platform::Platform;
use closure_rhino::java_lang::formatter::format_one_decimal;
use serde_json::Value;
// port: DiagnosticType#format (JVM differential table)
#[test]
fn message_format_parity() {
    let table: Value =
        serde_json::from_str(include_str!("data/message_format_cases.json")).unwrap();
    assert_eq!(table["reflection_failures"], 0);
    let mut rows = 0;
    for case in table["cases"].as_array().unwrap() {
        let pattern = case["pattern"].as_str().unwrap();
        for (args, expected) in table["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["results"].as_array().unwrap())
        {
            let arguments: Vec<_> = args
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap())
                .collect();
            let actual = std::panic::catch_unwind(|| Platform::format_message(pattern, &arguments));
            if let Some(output) = expected.get("output") {
                assert_eq!(
                    actual.unwrap(),
                    output.as_str().unwrap(),
                    "pattern={pattern:?} args={arguments:?}"
                );
            } else {
                let exception = actual.expect_err(&format!("pattern={pattern:?}"));
                let message = exception
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| exception.downcast_ref::<&str>().copied())
                    .unwrap();
                assert_eq!(
                    message,
                    expected["exception"].as_str().unwrap(),
                    "pattern={pattern:?} args={arguments:?}"
                );
            }
            rows += 1;
        }
    }
    assert_eq!(rows, 5341);
}
// port: Formatter.FormatSpecifier#print(double,Locale) (JVM differential table)
#[test]
fn formatter_one_decimal_parity() {
    let table: Value =
        serde_json::from_str(include_str!("data/message_format_cases.json")).unwrap();
    for row in table["floats"].as_array().unwrap() {
        let value = match row[0].as_str().unwrap() {
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            s => s.parse().unwrap(),
        };
        assert_eq!(
            format_one_decimal(value),
            row[1].as_str().unwrap(),
            "value={value}"
        );
    }
}
