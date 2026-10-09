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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/debugging/sourcemap/SourceMapConsumerV3Test.java.

use closure_rhino::js_string::JsString;
mod common;
use closure_sourcemap::{
    gson::{Gson, JsonArray},
    proto::mapping::Precision,
    source_map_consumer_v3::SourceMapConsumerV3,
    source_map_generator_v3::ExtensionValue,
};
use common::*;
static GSON: Gson = Gson::new();
// port: SourceMapConsumerV3Test#testSources
#[test]
fn test_sources() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .build(),
            ),
        )
        .unwrap();
    assert_eq!(consumer.get_original_sources(), [Some("testcode".into())]);
    assert_eq!(consumer.get_source_root(), None);
}
// port: SourceMapConsumerV3Test#testSectionsFormat_parsesSuccessfully
#[test]
fn test_sections_format_parses_successfully() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .add_section(
                        1,
                        2,
                        TestJsonBuilder::create()
                            .set_version(3)
                            .set_file("part_a")
                            .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                            .set_sources(&["testcode.js"])
                            .set_names(&["foo"]),
                    )
                    .build(),
            ),
        )
        .unwrap();
}
// port: SourceMapConsumerV3Test#testSectionsFormat_fileNameIsOptional_inSectionAndTopLevel
#[test]
fn test_sections_format_file_name_is_optional_in_section_and_top_level() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .add_section(
                        1,
                        2,
                        TestJsonBuilder::create()
                            .set_version(3)
                            .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                            .set_sources(&["testcode.js"])
                            .set_names(&["foo"]),
                    )
                    .build(),
            ),
        )
        .unwrap();
}
// port: SourceMapConsumerV3Test#testSectionsFormat_ignoresEmptySources
#[test]
fn test_sections_format_ignores_empty_sources() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_sources(&[])
                    .add_section(
                        1,
                        2,
                        TestJsonBuilder::create()
                            .set_version(3)
                            .set_file("part_a")
                            .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                            .set_sources(&["testcode.js"])
                            .set_names(&["foo"]),
                    )
                    .build(),
            ),
        )
        .unwrap();
}
// port: SourceMapConsumerV3Test#testSourcesWithRoot
#[test]
fn test_sources_with_root() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_source_root("http://server/path/")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .build(),
            ),
        )
        .unwrap();
    assert_eq!(consumer.get_original_sources(), [Some("testcode".into())]);
    assert_eq!(
        consumer.get_source_root(),
        Some(closure_rhino::js_string::JsString::from(
            "http://server/path/"
        ))
    );
}
// port: SourceMapConsumerV3Test#testExtensions
#[test]
fn test_extensions() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .set_custom_property("x_org_int", number(2))
                    .set_custom_property(
                        "x_org_array",
                        closure_sourcemap::gson::JsonElement::Array(JsonArray::default()),
                    )
                    .build(),
            ),
        )
        .unwrap();
    let exts = consumer.get_extensions();
    assert_eq!(exts.len(), 2);
    assert!(!exts.contains_key(&closure_rhino::js_string::JsString::from("org_int")));
    let ExtensionValue::JsonElement(v) =
        &exts[&closure_rhino::js_string::JsString::from("x_org_int")]
    else {
        panic!()
    };
    assert_eq!(v.get_as_int(), 2);
    let ExtensionValue::JsonElement(v) =
        &exts[&closure_rhino::js_string::JsString::from("x_org_array")]
    else {
        panic!()
    };
    assert!(v.get_as_json_array().elements.is_empty());
}
// port: SourceMapConsumerV3Test#testSourceMappingExactMatch
#[test]
fn test_source_mapping_exact_match() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .build(),
            ),
        )
        .unwrap();
    let mapping = consumer.get_mapping_for_line(1, 1);
    assert!(mapping.is_some());
    let mapping = mapping.unwrap();
    assert_eq!(mapping.get_line_number(), 1);
    assert_eq!(mapping.get_precision(), Precision::EXACT);
}
// port: SourceMapConsumerV3Test#testSourceMappingApproximatedLine
#[test]
fn test_source_mapping_approximated_line() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(GSON.to_json(
        &TestJsonBuilder::create()
            .set_version(3)
            .set_mappings(";;;;;;;;;;;;;;;;;;IAAMA,K,GACL,eAAaC,EAAb,EAAiB;AAAA;;AAAA;;AAChB,OAAKA,EAAL,GAAUA,EAAV;AACA,C;;IAEIC,S;;;;;;;AACL,qBAAYD,EAAZ,EAAgB;AAAA;;AAAA,6BACTA,EADS;AAEf;;;EAHsBD,K;;AAKxB,IAAIG,CAAC,GAAG,IAAID,SAAJ,CAAc,UAAd,CAAR")
            .set_sources_content(&[r#"class Shape {
	constructor (id) {
		this.id = id;
	}
}
class Rectangle extends Shape {
	constructor(id) {
		super(id);
	}
}
var s = new Rectangle("Shape ID");
"#])
            .set_sources(&["testcode"])
            .set_names(&["Shape", "id", "Rectangle", "s"])
            .build()
    )).unwrap();
    let mapping = consumer.get_mapping_for_line(40, 10);
    assert!(mapping.is_some());
    let mapping = mapping.unwrap();
    assert_eq!(mapping.get_line_number(), 9);
    assert_eq!(mapping.get_precision(), Precision::APPROXIMATE_LINE);
}
// port: SourceMapConsumerV3Test#testLargeMappings
#[test]
fn test_large_mappings() {
    let mut mappings = String::new();
    for i in 0..2000 {
        mappings.push_str("AAAA");
        if i % 2 == 0 {
            mappings.push('A');
        }
        mappings.push(',');
    }
    mappings.push(';');
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings(&mappings)
                    .set_sources(&["testcode"])
                    .set_names(&["foo"])
                    .build(),
            ),
        )
        .unwrap();
    assert_eq!(consumer.get_line_count(), 1);
    let mapping = consumer.get_mapping_for_line(1, 1);
    assert!(mapping.is_some());
}
// port: SourceMapConsumerV3Test#testMixedEntryTypes
#[test]
fn test_mixed_entry_types() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("A,CAAA,EAAAA;")
                    .set_sources(&["testcode"])
                    .set_names(&["name1"])
                    .build(),
            ),
        )
        .unwrap();
    let m1 = consumer.get_mapping_for_line(1, 1);
    let m2 = consumer.get_mapping_for_line(1, 2);
    let m4 = consumer.get_mapping_for_line(1, 4);
    assert!(m1.is_none());
    assert!(m2.is_some());
    assert!(m4.is_some());
    let m2 = m2.unwrap();
    let m4 = m4.unwrap();
    assert_eq!(m2.get_original_file(), "testcode");
    assert_eq!(m2.get_identifier(), "");
    assert_eq!(m4.get_original_file(), "testcode");
    assert_eq!(m4.get_identifier(), "name1");
}
// port: SourceMapConsumerV3Test#testReverseMapping
#[test]
fn test_reverse_mapping() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .build(),
            ),
        )
        .unwrap();
    let reverse = consumer.get_reverse_mapping(Some("testcode".into()), 0, 0);
    assert!(!reverse.is_empty());
}
// port: SourceMapConsumerV3Test#testBinarySearchEdgeCases
#[test]
fn test_binary_search_edge_cases() {
    let mut consumer = SourceMapConsumerV3::new();
    for num_entries in 1..20 {
        let mut mappings = String::new();
        for i in 0..num_entries {
            if i == 0 {
                mappings.push_str("AAAA");
            } else {
                mappings.push_str("CAAA");
            }
            mappings.push(',');
        }
        mappings.push(';');
        consumer
            .parse(
                GSON.to_json(
                    &TestJsonBuilder::create()
                        .set_version(3)
                        .set_file("testcode")
                        .set_line_count(1)
                        .set_mappings(&mappings)
                        .set_sources(&["testcode"])
                        .set_names(&["foo"])
                        .build(),
                ),
            )
            .unwrap();
        for col in 0..num_entries {
            let mapping = consumer.get_mapping_for_line(1, col + 1);
            assert!(mapping.is_some());
            let mapping = mapping.unwrap();
            assert_eq!(mapping.get_line_number(), 1);
            assert_eq!(mapping.get_column_position(), 1);
        }
    }
}
// port: SourceMapConsumerV3Test#testVisitMappings
#[test]
fn test_visit_mappings() {
    let mut consumer = SourceMapConsumerV3::new();
    consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings("AAAAA,QAASA,UAAS,EAAG;")
                    .set_sources(&["testcode"])
                    .set_names(&["__BASIC__"])
                    .build(),
            ),
        )
        .unwrap();
    let mut symbols = Vec::new();
    consumer.visit_mappings(&mut |_source_name: Option<&JsString>,
                                  symbol_name: Option<&JsString>,
                                  _source_pos,
                                  _start_pos,
                                  _end_pos| {
        if let Some(symbol_name) = symbol_name {
            symbols.push(symbol_name.to_owned());
        }
    });
    assert_eq!(symbols, ["__BASIC__", "__BASIC__"]);
}
// port: SourceMapConsumerV3Test#testInvalidEntryValuesThrows
#[test]
fn test_invalid_entry_values_throws() {
    let mut consumer = SourceMapConsumerV3::new();
    let invalid_mappings = "AA;";
    let expected = consumer
        .parse(
            GSON.to_json(
                &TestJsonBuilder::create()
                    .set_version(3)
                    .set_file("testcode")
                    .set_line_count(1)
                    .set_mappings(invalid_mappings)
                    .set_sources(&["testcode"])
                    .set_names(&["foo"])
                    .build(),
            ),
        )
        .unwrap_err();
    assert!(
        expected
            .to_string()
            .contains("Unexpected number of values for entry:2")
    );
}
