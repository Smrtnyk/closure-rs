/*
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java.

//! `AbstractCommandLineRunner#getBuiltinExterns` against Java's result for each environment.

mod common;

use closure_resources::abstract_command_line_runner::AbstractCommandLineRunner;
use closure_resources::compiler_options::Environment;
use common::{fixture_rows, sha256_hex};

fn check(env: Environment, fixture: &str) {
    let rows = fixture_rows(fixture);
    let externs = AbstractCommandLineRunner::get_builtin_externs(env);
    assert_eq!(externs.len(), rows.len(), "{env}: extern count");
    for (index, (row, file)) in rows.iter().zip(&externs).enumerate() {
        let code = file.get_code();
        assert_eq!(row[0], index.to_string());
        assert_eq!(row[1], file.get_name(), "{env}: name at {index}");
        assert_eq!(
            row[2],
            code.len().to_string(),
            "{env}: length of {}",
            row[1]
        );
        assert_eq!(
            row[3],
            sha256_hex(code.as_bytes()),
            "{env}: sha256 of {}",
            row[1]
        );
    }
}

// port: AbstractCommandLineRunner#getBuiltinExterns(BROWSER) (fixture builtin_externs_BROWSER.tsv)
#[test]
fn get_builtin_externs_browser() {
    check(Environment::BROWSER, "builtin_externs_BROWSER.tsv");
}

// port: AbstractCommandLineRunner#getBuiltinExterns(CUSTOM) (fixture builtin_externs_CUSTOM.tsv)
#[test]
fn get_builtin_externs_custom() {
    check(Environment::CUSTOM, "builtin_externs_CUSTOM.tsv");
}

// port: AbstractCommandLineRunner#getBuiltinExterns (the browser/ directory entry becomes "")
#[test]
fn get_builtin_externs_keeps_directory_entry() {
    let externs = AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER);
    let empty: Vec<_> = externs
        .iter()
        .enumerate()
        .filter(|(_, f)| f.get_name() == "externs.zip//")
        .collect();
    assert_eq!(empty.len(), 1);
    assert_eq!(empty[0].0, 29);
    assert_eq!(empty[0].1.get_code(), "");
}

// port: AbstractCommandLineRunner#getBuiltinExterns (returns a fresh list on every call)
#[test]
fn get_builtin_externs_is_repeatable() {
    let a = AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER);
    let b = AbstractCommandLineRunner::get_builtin_externs(Environment::BROWSER);
    assert_eq!(a, b);
}
