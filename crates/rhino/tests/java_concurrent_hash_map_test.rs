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

//! Java `ConcurrentHashMap` iteration order checked against orders printed by the pinned JDK
//! (`new ConcurrentHashMap<>()`, String keys; tests/data/java_concurrent_hash_map_order.tsv,
//! written by tests/java/ConcurrentHashMapOrder.java; columns: case, operations (`A:k1,k2` =
//! putAll of an insertion-ordered map, `P:k` = put, separated by `;`), Java keySet() order).
use closure_rhino::java_lang::concurrent_hash_map::ConcurrentHashMap;

#[test]
fn iteration_order_matches_java_concurrent_hash_map() {
    let data = include_str!("data/java_concurrent_hash_map_order.tsv");
    let mut cases = 0;
    for line in data.lines() {
        let columns: Vec<&str> = line.split('\t').collect();
        let mut map = ConcurrentHashMap::<String, usize>::new();
        for (step, op) in columns[1].split(';').enumerate() {
            match op.split_once(':').unwrap() {
                ("A", keys) => map.put_all(
                    keys.split(',')
                        .map(|key| (key.to_string(), step))
                        .collect::<Vec<_>>(),
                ),
                ("P", key) => {
                    let existed = map.contains_key(key);
                    assert_eq!(map.put(key.to_string(), step).is_some(), existed);
                    assert_eq!(map.get(key), Some(&step));
                }
                (other, _) => panic!("unknown operation {other}"),
            }
        }
        let actual: Vec<&str> = map.keys().map(String::as_str).collect();
        let expected: Vec<&str> = columns[2].split(',').collect();
        assert_eq!(map.len(), expected.len(), "{}", columns[0]);
        assert_eq!(actual, expected, "{}", columns[0]);
        cases += 1;
    }
    assert_eq!(cases, 66);
}

#[test]
fn put_keeps_the_position_and_replaces_the_value() {
    let mut map = ConcurrentHashMap::<String, i32>::new();
    map.put_all(vec![("b.js".to_string(), 1), ("a.js".to_string(), 2)]);
    let before: Vec<String> = map.keys().cloned().collect();
    assert_eq!(map.put("b.js".to_string(), 3), Some(1));
    assert_eq!(map.keys().cloned().collect::<Vec<_>>(), before);
    assert_eq!(map.get("b.js"), Some(&3));
}
