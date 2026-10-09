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
//   test/com/google/javascript/jscomp/VariableMapTest.java.

use closure_jscomp::variable_map::VariableMap;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{java_lang::utf_8, js_string::JsString};

// port: VariableMapTest#cycleTest (ImmutableMap test input)
fn map(entries: &[(&str, &str)]) -> IndexMap<JsString, JsString> {
    entries
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect()
}
// port: VariableMapTest#cycleTest
fn cycle_test(map: IndexMap<JsString, JsString>) {
    let input = VariableMap::new(&map);
    let serialized = utf_8::decode(&input.to_bytes());
    let output = VariableMap::from_bytes(&utf_8::encode(&serialized)).unwrap();
    assert_maps_equals(&input.to_map(), &output.to_map());
}
// port: VariableMapTest#assertMapsEquals
fn assert_maps_equals(
    expected: &IndexMap<JsString, JsString>,
    result: &IndexMap<JsString, JsString>,
) {
    assert_eq!(result.len(), expected.len());
    for (key, value) in expected {
        assert_eq!(result.get(key), Some(value));
    }
}
// port: VariableMapTest#assertEqual
fn assert_equal(bytes1: &[u8], bytes2: &[u8]) {
    if bytes1.as_ptr() != bytes2.as_ptr() {
        assert_eq!(bytes2.len(), bytes1.len(), "length differs.");
        for i in 0..bytes1.len() {
            assert_eq!(bytes2[i], bytes1[i], "byte {i}differs.");
        }
    }
}
// port: VariableMapTest#testCycle1
#[test]
fn test_cycle1() {
    cycle_test(map(&[("AAA", "a"), ("BBB", "b")]));
    cycle_test(map(&[("AA:AA", "a"), ("BB:BB", "b")]));
    cycle_test(map(&[("AAA", "a:a"), ("BBB", "b:b")]));
}
// port: VariableMapTest#testToBytes
#[test]
fn test_to_bytes() {
    let vm = VariableMap::new(&map(&[("AAA", "a"), ("BBB", "b")]));
    let serialized = utf_8::decode(&vm.to_bytes()).to_string_lossy();
    assert!(serialized.ends_with('\n'));
    let lines: Vec<_> = serialized.trim_end_matches('\n').split('\n').collect();
    assert_eq!(lines.len(), 2);
    assert!(lines.contains(&"AAA:a"));
    assert!(lines.contains(&"BBB:b"));
}
// port: VariableMapTest#testFromBytes
#[test]
fn test_from_bytes() {
    let vm = VariableMap::from_bytes(b"AAA:a\nBBB:b\n").unwrap();
    assert_eq!(vm.get_original_name_to_new_name_map().len(), 2);
    assert_eq!(vm.lookup_new_name(&"AAA".into()), Some("a".into()));
    assert_eq!(vm.lookup_new_name(&"BBB".into()), Some("b".into()));
    assert_eq!(vm.lookup_source_name(&"a".into()), Some("AAA".into()));
    assert_eq!(vm.lookup_source_name(&"b".into()), Some("BBB".into()));
}
// port: VariableMapTest#testFromBytesWithEmptyValue
#[test]
fn test_from_bytes_with_empty_value() {
    let vm = VariableMap::from_bytes(b"AAA:").unwrap();
    assert!(vm.lookup_new_name(&"AAA".into()).unwrap().is_empty());
}
// port: VariableMapTest#testFromStream
#[test]
fn test_from_stream() {
    let vm = VariableMap::from_stream(&b"AAA:a\nBBB:b\n"[..]).unwrap();
    assert_eq!(vm.get_original_name_to_new_name_map().len(), 2);
    assert_eq!(vm.lookup_new_name(&"AAA".into()), Some("a".into()));
    assert_eq!(vm.lookup_new_name(&"BBB".into()), Some("b".into()));
}
// port: VariableMapTest#testFileFormat1
#[test]
fn test_file_format1() {
    for (key, expected) in [
        ("x\ny", "x\\ny:a\n"),
        ("x:y", "x\\:y:a\n"),
        ("x\ny", "x\\ny:a\n"),
        ("x\\y", "x\\\\y:a\n"),
        ("\n", "\\n:a\n"),
        (":", "\\::a\n"),
        ("\n", "\\n:a\n"),
        ("\\", "\\\\:a\n"),
    ] {
        assert_equal(
            &VariableMap::new(&map(&[(key, "a")])).to_bytes(),
            expected.as_bytes(),
        );
    }
}
// port: VariableMapTest#testFromBytesComplex1
#[test]
fn test_from_bytes_complex1() {
    cycle_test(map(&[("AAA[':f']", "a")]));
    let input = VariableMap::new(&map(&[("AAA[':f']", "a")]));
    assert_equal(&input.to_bytes(), b"AAA['\\:f']:a\n");
}
// port: VariableMapTest#testFromBytesComplex2
#[test]
fn test_from_bytes_complex2() {
    let vm = VariableMap::from_bytes(b"AAA['\\:f']:a\n").unwrap();
    assert_eq!(vm.get_original_name_to_new_name_map().len(), 1);
    assert_eq!(vm.lookup_new_name(&"AAA[':f']".into()), Some("a".into()));
    assert_eq!(vm.get_new_name_to_original_name_map().len(), 1);
    assert_eq!(vm.lookup_source_name(&"a".into()), Some("AAA[':f']".into()));
}
// port: VariableMapTest#testReverseThrowsErrorOnDuplicate
#[test]
fn test_reverse_throws_error_on_duplicate() {
    assert!(
        std::panic::catch_unwind(|| VariableMap::new(&map(&[("AA", "b"), ("BB", "b")]))).is_err()
    );
}
