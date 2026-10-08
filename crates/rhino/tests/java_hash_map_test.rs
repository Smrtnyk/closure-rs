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

//! Java `HashMap` iteration order checked against orders printed by the pinned JDK
//! (`new HashMap<>(16)`, String keys inserted in the listed order; tests/data/java_hash_map_order.tsv
//! columns: size, insertion order, Java keySet() order).
use closure_rhino::java_lang::hash_map::{iteration_order, table_size_for};
use closure_rhino::js_string::JsString;

#[test]
fn iteration_order_matches_java_hash_map() {
    let data = include_str!("data/java_hash_map_order.tsv");
    let mut cases = 0;
    for line in data.lines() {
        let columns: Vec<&str> = line.split('\t').collect();
        let inserted: Vec<&str> = columns[1].split(',').collect();
        let expected: Vec<&str> = columns[2].split(',').collect();
        assert_eq!(columns[0].parse::<usize>().unwrap(), inserted.len());
        let actual = iteration_order(inserted, |k| JsString::from(*k).hash_code(), 16);
        assert_eq!(actual, expected, "size {}", columns[0]);
        cases += 1;
    }
    assert_eq!(cases, 6);
}

#[test]
fn table_size_for_matches_java() {
    assert_eq!(table_size_for(0), 1);
    assert_eq!(table_size_for(1), 1);
    assert_eq!(table_size_for(2), 2);
    assert_eq!(table_size_for(16), 16);
    assert_eq!(table_size_for(17), 32);
}
