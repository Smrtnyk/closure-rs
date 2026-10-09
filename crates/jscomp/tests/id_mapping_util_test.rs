/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/IdMappingUtilTest.java.

//! Port of `IdMappingUtilTest.java`.

use closure_jscomp::id_mapping_util::{IdMappingUtil, NEW_LINE};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::js_string::JsString;

// port: IdMappingUtilTest#testParseIdMapping
#[test]
fn test_parse_id_mapping() {
    let mut mapping = String::new();
    mapping.push_str("[gen1]");
    mapping.push(NEW_LINE);
    mapping.push_str("id1:data1");
    mapping.push(NEW_LINE);
    mapping.push_str("id2:data2:data22");
    mapping.push(NEW_LINE);
    mapping.push(NEW_LINE);
    mapping.push_str("[gen2]");

    let result = IdMappingUtil::parse_serialized_id_mappings(Some(&mapping));

    assert_eq!(result.len(), 2);

    let gen1 = result.get(&JsString::from("gen1")).expect("gen1");
    assert_eq!(gen1.size(), 2);
    assert_eq!(
        gen1.get(&JsString::from("id1")),
        Some(&JsString::from("data1"))
    );
    assert_eq!(
        gen1.get(&JsString::from("id2")),
        Some(&JsString::from("data2:data22"))
    );

    let gen2 = result.get(&JsString::from("gen2")).expect("gen2");
    assert!(gen2.is_empty());
}

// port: IdMappingUtilTest#testParseSectionAsStream
#[test]
fn test_parse_section_as_stream() {
    let mut mapping = String::new();
    mapping.push_str("[gen1]");
    mapping.push(NEW_LINE);
    mapping.push_str("id1:data1");
    mapping.push(NEW_LINE);
    mapping.push_str("[gen2]");
    mapping.push(NEW_LINE);
    mapping.push_str("id2:data2");

    let result = IdMappingUtil::parse_section_as_stream(&mut mapping.as_bytes(), "gen1").unwrap();

    let expected: IndexMap<JsString, JsString> = [(JsString::from("id1"), JsString::from("data1"))]
        .into_iter()
        .collect();
    assert_eq!(result, expected);
}
