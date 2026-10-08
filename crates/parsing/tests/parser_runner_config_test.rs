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

//! Checks the ParserRunner configuration helpers against the names Java computes.
//!
//! `fixtures/parser_runner_config.txt` was produced with the reference jar:
//! `ParserRunner.createConfig(ECMASCRIPT3, null, SLOPPY)`'s `annotations().keySet()` and
//! `suppressionNames()`, and `ParserRunner.getReservedVars()`, each joined with ','.

use closure_parsing::config::{LanguageMode, StrictMode};
use closure_parsing::parser_runner::ParserRunner;
use closure_rhino::js_string::JsString;

fn expected(prefix: &str) -> String {
    let text = include_str!("fixtures/parser_runner_config.txt");
    text.lines()
        .find_map(|l| l.strip_prefix(prefix))
        .unwrap()
        .to_string()
}

fn join<'a>(names: impl Iterator<Item = &'a JsString>) -> String {
    names.map(|n| n.to_string()).collect::<Vec<_>>().join(",")
}

#[test]
fn create_config_matches_java() {
    let config = ParserRunner::create_config(LanguageMode::ECMASCRIPT3, None, StrictMode::SLOPPY);
    assert_eq!(join(config.annotations().keys()), expected("annotations="));
    assert_eq!(
        join(config.suppression_names().iter()),
        expected("suppressions=")
    );
    assert_eq!(
        join(ParserRunner::get_reserved_vars().iter()),
        expected("reserved=")
    );
}
