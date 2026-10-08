/*
 * Copyright 2025 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/js/RuntimeJsLibManager.java.

//! `RuntimeJsLibManager.FieldsTable` against Java's table and failure messages.

mod common;

use closure_resources::js::runtime_js_lib_manager::{FieldsTable, RUNTIME_LIB_DIR};
use common::{fixture_rows, panic_message};

// port: RuntimeJsLibManager.FieldsTable#INSTANCE (fixture fields_table.tsv)
#[test]
fn fields_table_matches_java() {
    let rows = fixture_rows("fields_table.tsv");
    let table = FieldsTable::instance();
    let actual: Vec<(String, String)> = table.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let expected: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(
        table.get("inherits").map(String::as_str),
        Some("es6/util/inherits")
    );
}

fn error_row(case: &str) -> Vec<String> {
    fixture_rows("fields_table_errors.tsv")
        .into_iter()
        .find(|row| row[0] == case)
        .unwrap()
}

// port: RuntimeJsLibManager.FieldsTable#loadFields (checkState(tokens.size() == 2, tokens))
#[test]
fn load_fields_malformed_line() {
    let row = error_row("malformed");
    assert_eq!(row[1], "java.lang.IllegalStateException");
    let message = panic_message(|| FieldsTable::load_fields_from("a,x/y\nfoo\n"));
    assert_eq!(message, row[2]);
}

// port: RuntimeJsLibManager.FieldsTable#loadFields (ImmutableMap.Builder#buildOrThrow)
#[test]
fn load_fields_duplicate_field() {
    let row = error_row("duplicate");
    assert_eq!(row[1], "java.lang.IllegalArgumentException");
    let message = panic_message(|| FieldsTable::load_fields_from("a,x/y\nb,z\na,w\n"));
    assert_eq!(message, row[2]);
}

// port: RuntimeJsLibManager.FieldsTable#loadFields (omitEmptyStrings, limit(2))
#[test]
fn load_fields_empty_lines_and_extra_commas() {
    let row = error_row("extracomma");
    assert_eq!(row[1], "ok");
    let table = FieldsTable::load_fields_from("\n\na,x,y\n\nb,\n");
    let shown = format!(
        "{{{}}}",
        table
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert_eq!(shown, row[2]);
}

// port: RuntimeJsLibManager#RUNTIME_LIB_DIR
#[test]
fn runtime_lib_dir() {
    assert_eq!(RUNTIME_LIB_DIR, "src/com/google/javascript/jscomp/js/");
}
