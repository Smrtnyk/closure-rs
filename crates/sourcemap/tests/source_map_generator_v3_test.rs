/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   test/com/google/debugging/sourcemap/SourceMapGeneratorV3Test.java.

mod common;
use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    file_position::FilePosition,
    gson::{JsonArray, JsonElement, JsonObject},
    source_map_generator_v3::{ExtensionValue, SourceMapGeneratorV3},
    source_map_section::SourceMapSection,
};
use common::*;
// port: SourceMapGeneratorV3Test#testWriteMetaMap
#[test]
fn test_write_meta_map() {
    let mut out = String::new();
    let name = "./app.js";
    let app_sections = [
        SourceMapSection::for_url("src1", 0, 0),
        SourceMapSection::for_url("src2", 100, 10),
        SourceMapSection::for_url("src3", 150, 5),
    ];
    let generator = SourceMapGeneratorV3::new();
    generator
        .append_index_map_to(&mut out, name, &app_sections)
        .unwrap();
    assert_eq!(
        out,
        r#"{
"version":3,
"file":"./app.js",
"sections":[
{
"offset":{
"line":0,
"column":0
},
"url":"src1"
},
{
"offset":{
"line":100,
"column":10
},
"url":"src2"
},
{
"offset":{
"line":150,
"column":5
},
"url":"src3"
}
]
}
"#
    );
}
// port: SourceMapGeneratorV3Test#getEmptyMapFor
fn get_empty_map_for(name: &str) -> String {
    let mut out = String::new();
    let mut generator = SourceMapGeneratorV3::new();
    generator.append_to(&mut out, Some(name.into())).unwrap();
    out
}
// port: SourceMapGeneratorV3Test#testWriteMetaMap2
#[test]
fn test_write_meta_map2() {
    let mut out = String::new();
    let name = "./app.js";
    let app_sections = [
        SourceMapSection::for_map(get_empty_map_for("./part.js"), 0, 0),
        SourceMapSection::for_url("src2", 100, 10),
    ];
    let generator = SourceMapGeneratorV3::new();
    generator
        .append_index_map_to(&mut out, name, &app_sections)
        .unwrap();
    assert_eq!(
        out,
        r#"{
"version":3,
"file":"./app.js",
"sections":[
{
"offset":{
"line":0,
"column":0
},
"map":{
"version":3,
"file":"./part.js",
"lineCount":1,
"mappings":";",
"sources":[],
"names":[]
}

},
{
"offset":{
"line":100,
"column":10
},
"url":"src2"
}
]
}
"#
    );
}
// port: SourceMapGeneratorV3Test#testSourceMerging1
#[test]
fn test_source_merging1() {
    let mut source_map_generator_v3 = SourceMapGeneratorV3::new();
    source_map_generator_v3.add_mapping(
        Some("sourceName1".into()),
        Some("symbolName1".into()),
        FilePosition::new(1, 1),
        FilePosition::new(2, 2),
        FilePosition::new(3, 3),
    );
    let mut w = String::new();
    source_map_generator_v3
        .append_to(&mut w, Some("foo.js.sourcemap".into()))
        .unwrap();
    let source_map1 = w.clone();
    source_map_generator_v3 = SourceMapGeneratorV3::new();
    source_map_generator_v3.add_mapping(
        Some("sourceName2".into()),
        Some("symbolName2".into()),
        FilePosition::new(1, 4),
        FilePosition::new(2, 5),
        FilePosition::new(3, 6),
    );
    w = String::new();
    source_map_generator_v3
        .append_to(&mut w, Some("bar.js.sourcemap".into()))
        .unwrap();
    let source_map2 = w.clone();
    source_map_generator_v3 = SourceMapGeneratorV3::new();
    source_map_generator_v3
        .merge_map_section(0, 0, source_map1.as_str())
        .unwrap();
    source_map_generator_v3
        .merge_map_section(4, 0, source_map2.as_str())
        .unwrap();
    source_map_generator_v3
        .append_to(&mut w, Some("foobar.js.sourcemap".into()))
        .unwrap();
    let combined_map = w.clone();
    source_map_generator_v3 = SourceMapGeneratorV3::new();
    source_map_generator_v3.add_mapping(
        Some("sourceName1".into()),
        Some("symbolName1".into()),
        FilePosition::new(1, 1),
        FilePosition::new(2, 2),
        FilePosition::new(3, 3),
    );
    source_map_generator_v3.add_mapping(
        Some("sourceName2".into()),
        Some("symbolName2".into()),
        FilePosition::new(1, 4),
        FilePosition::new(6, 5),
        FilePosition::new(7, 6),
    );
    source_map_generator_v3
        .append_to(&mut w, Some("foobar.js.sourcemap".into()))
        .unwrap();
    let manually_combined = w.clone();
    // TODO(b/62591567): combinedMap is missing the last mapping.
    assert_ne!(combined_map, manually_combined);
}
// port: SourceMapGeneratorV3Test#testSourceMapExtensions
#[test]
fn test_source_map_extensions() {
    let mut mapper = SourceMapGeneratorV3::new();
    mapper
        .add_extension(
            "x_google_foo",
            ExtensionValue::JsonElement(JsonElement::Object(JsonObject::default())),
        )
        .unwrap();
    mapper
        .add_extension(
            "x_google_test",
            ExtensionValue::JsonElement(JsonElement::Object(parse_json_object(
                r#"{"number" : 1}"#,
            ))),
        )
        .unwrap();
    mapper
        .add_extension(
            "x_google_array",
            ExtensionValue::JsonElement(JsonElement::Array(JsonArray::default())),
        )
        .unwrap();
    mapper
        .add_extension("x_google_int", ExtensionValue::Integer(2))
        .unwrap();
    mapper
        .add_extension("x_google_str", ExtensionValue::String("Some text".into()))
        .unwrap();
    mapper.remove_extension("x_google_foo");
    let mut out = String::new();
    mapper.append_to(&mut out, Some("out.js".into())).unwrap();
    assert!(mapper.has_extension("x_google_test"));
    let source_map = parse_json_object(&out);
    assert!(!source_map.has("x_google_foo"));
    assert!(!source_map.has("google_test"));
    assert_eq!(
        source_map
            .get("x_google_test")
            .unwrap()
            .get_as_json_object()
            .get("number")
            .unwrap()
            .get_as_int(),
        1
    );
    assert!(
        source_map
            .get("x_google_array")
            .unwrap()
            .get_as_json_array()
            .elements
            .is_empty()
    );
    assert_eq!(source_map.get("x_google_int").unwrap().get_as_int(), 2);
    assert_eq!(
        source_map.get("x_google_str").unwrap().get_as_string(),
        "Some text"
    );
}
// port: SourceMapGeneratorV3Test#testSourceMapMergeExtensions
#[test]
fn test_source_map_merge_extensions() {
    let mut mapper = SourceMapGeneratorV3::new();
    mapper
        .merge_map_section(
            0,
            0,
            r#"{
"version":3,
"file":"testcode",
"lineCount":1,
"mappings":"AAAAA,a,QAASA,UAAS,EAAG;",
"sources":["testcode"],
"names":["__BASIC__"],
"x_company_foo":2
}
"#,
        )
        .unwrap();
    assert!(!mapper.has_extension("x_company_foo"));
    mapper
        .add_extension("x_company_baz", ExtensionValue::Integer(2))
        .unwrap();
    mapper
        .merge_map_section_with_merge_action(
            0,
            0,
            r#"{
"version":3,
"file":"testcode2",
"lineCount":0,
"mappings":"",
"sources":["testcode2"],
"names":[],
"x_company_baz":3,
"x_company_bar":false
}
"#,
            &mut |_extension_key: &JsString,
                  current_value: &ExtensionValue,
                  new_value: &ExtensionValue| {
                let ExtensionValue::Integer(current_value) = current_value else {
                    panic!()
                };
                let ExtensionValue::JsonElement(new_value) = new_value else {
                    panic!()
                };
                ExtensionValue::Integer(
                    current_value + new_value.get_as_json_primitive().get_as_int(),
                )
            },
        )
        .unwrap();
    assert_eq!(
        mapper.get_extension("x_company_baz"),
        Some(&ExtensionValue::Integer(5))
    );
    let Some(ExtensionValue::JsonElement(value)) = mapper.get_extension("x_company_bar") else {
        panic!()
    };
    assert!(!value.get_as_json_primitive().get_as_boolean());
}
// port: SourceMapGeneratorV3Test#testSourceRoot
#[test]
fn test_source_root() {
    let mut mapper = SourceMapGeneratorV3::new();
    let mut out = String::new();
    mapper.append_to(&mut out, Some("out.js".into())).unwrap();
    let mut mapping = parse_json_object(&out);
    assert_eq!(mapping.get("version").unwrap().get_as_int(), 3);
    assert!(!mapping.has("sourceRoot"));
    out = String::new();
    mapper.set_source_root("");
    mapper.append_to(&mut out, Some("out2.js".into())).unwrap();
    mapping = parse_json_object(&out);
    assert!(!mapping.has("sourceRoot"));
    out = String::new();
    mapper.set_source_root("http://url/path");
    mapper.append_to(&mut out, Some("out3.js".into())).unwrap();
    mapping = parse_json_object(&out);
    assert_eq!(
        mapping.get("sourceRoot").unwrap().get_as_string(),
        "http://url/path"
    );
}
