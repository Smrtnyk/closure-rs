/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DefaultExterns.java.

//! `DefaultExterns#prepareExterns` ordering, filtering and failure messages.

mod common;

use closure_resources::compiler_options::Environment;
use closure_resources::default_externs::{
    BROWSER_EXTERN_DEP_ORDER, BUILTIN_LANG_EXTERNS, DefaultExterns,
};
use common::panic_message;
use indexmap::IndexMap;

/// A map holding every required extern, in an order unlike the output order, plus extra files.
fn full_map() -> IndexMap<String, String> {
    let mut map = IndexMap::new();
    map.insert("zzz_extra.js".to_string(), "zzz_extra.js".to_string());
    for key in BROWSER_EXTERN_DEP_ORDER.iter().rev() {
        map.insert(key.to_string(), key.to_string());
    }
    map.insert("aaa_extra.js".to_string(), "aaa_extra.js".to_string());
    for key in BUILTIN_LANG_EXTERNS.iter().rev() {
        map.insert(key.to_string(), key.to_string());
    }
    map.insert("mmm_extra.js".to_string(), "mmm_extra.js".to_string());
    map
}

// port: DefaultExterns#prepareExterns (BROWSER: language externs, browser order, then the rest)
#[test]
fn prepare_externs_browser_order() {
    let mut map = full_map();
    let out = DefaultExterns::prepare_externs(Environment::BROWSER, &mut map);
    let mut expected: Vec<String> = Vec::new();
    expected.extend(BUILTIN_LANG_EXTERNS.iter().map(|s| s.to_string()));
    expected.extend(BROWSER_EXTERN_DEP_ORDER.iter().map(|s| s.to_string()));
    // The remaining entries keep their insertion order (LinkedHashMap#remove).
    expected.extend(
        ["zzz_extra.js", "aaa_extra.js", "mmm_extra.js"]
            .iter()
            .map(|s| s.to_string()),
    );
    assert_eq!(out, expected);
    // The extra entries stay in the map, as in Java (`out.addAll(externs.values())`).
    let rest: Vec<&String> = map.keys().collect();
    assert_eq!(rest, ["zzz_extra.js", "aaa_extra.js", "mmm_extra.js"]);
}

// port: DefaultExterns#prepareExterns (CUSTOM: only the language externs)
#[test]
fn prepare_externs_custom_only_language_externs() {
    let mut map = full_map();
    let out = DefaultExterns::prepare_externs(Environment::CUSTOM, &mut map);
    assert_eq!(out, BUILTIN_LANG_EXTERNS.map(String::from).to_vec());
    assert_eq!(map.len(), BROWSER_EXTERN_DEP_ORDER.len() + 3);
}

// port: DefaultExterns#prepareExterns (CUSTOM does not need the browser externs)
#[test]
fn prepare_externs_custom_without_browser_externs() {
    let mut map: IndexMap<String, i32> = IndexMap::new();
    for (i, key) in BUILTIN_LANG_EXTERNS.iter().enumerate() {
        map.insert(key.to_string(), i as i32);
    }
    let out = DefaultExterns::prepare_externs(Environment::CUSTOM, &mut map);
    assert_eq!(out, vec![0, 1, 2, 3, 4]);
    assert!(map.is_empty());
}

// port: DefaultExterns#prepareExterns (Preconditions.checkState on a missing language extern)
#[test]
fn prepare_externs_missing_builtin() {
    let mut map = full_map();
    map.shift_remove("es6.js");
    let message = panic_message(move || {
        DefaultExterns::prepare_externs(Environment::CUSTOM, &mut map);
    });
    assert_eq!(message, "Externs must contain builtin: es6.js");
}

// port: DefaultExterns#prepareExterns (Preconditions.checkState on a missing browser extern)
#[test]
fn prepare_externs_missing_browser_extern() {
    let mut map = full_map();
    map.shift_remove("w3c_dom2.js");
    let message = panic_message(move || {
        DefaultExterns::prepare_externs(Environment::BROWSER, &mut map);
    });
    assert_eq!(
        message,
        "Externs must contain builtin for env BROWSER: w3c_dom2.js"
    );
}
