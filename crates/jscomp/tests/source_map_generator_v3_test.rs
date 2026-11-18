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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit ddfede7:
//   test/com/google/debugging/sourcemap/SourceMapGeneratorV3Test.java.

#[path = "support/source_map_test_case.rs"]
mod source_map_test_case;
use closure_jscomp::source_map::{DetailLevel, Format};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    file_position::FilePosition, source_map_consumer_v3::SourceMapConsumerV3,
    source_map_generator_v3::SourceMapGeneratorV3, source_map_section::SourceMapSection,
};
use source_map_test_case::common::TestJsonBuilder;
use source_map_test_case::{SourceMapTest as _, SourceMapTestCase, get_source_map};
#[derive(Default)]
struct SourceMapGeneratorV3Test {
    base: SourceMapTestCase,
}
impl source_map_test_case::SourceMapTest for SourceMapGeneratorV3Test {
    fn test_case(&self) -> &SourceMapTestCase {
        &self.base
    }
    // port: SourceMapGeneratorV3Test#getSourceMapFormat
    fn get_source_map_format(&self) -> Format {
        Format::V3
    }
    // port: SourceMapGeneratorV3Test#getSourceMapConsumer
    fn get_source_map_consumer(&self) -> SourceMapConsumerV3 {
        SourceMapConsumerV3::new()
    }
}
// port: SourceMapGeneratorV3Test#getEncodedFileName
fn get_encoded_file_name() -> &'static str {
    if std::path::MAIN_SEPARATOR == '\\' {
        "c:/myfile.js"
    } else {
        "c:\\myfile.js"
    }
}

// port: SourceMapGeneratorV3Test#testBasicMapping1
#[test]
fn test_basic_mapping1() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check("function __BASIC__() { }");
}

// port: SourceMapGeneratorV3Test#testBasicMappingGoldenOutput
#[test]
fn test_basic_mapping_golden_output() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map(
        "function __BASIC__() { }",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA,QAASA,UAAS,EAAG;")
            .set_sources(&["testcode"])
            .set_names(&["__BASIC__"])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testBasicMapping2
#[test]
fn test_basic_mapping2() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check("function __BASIC__(__PARAM1__) {}");
}

// port: SourceMapGeneratorV3Test#testLiteralMappings
#[test]
fn test_literal_mappings() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check(
        "function __BASIC__(__PARAM1__, __PARAM2__) { var __VAR__ = '__STR__'; }",
    );
}

// port: SourceMapGeneratorV3Test#testLiteralMappingsGoldenOutput
#[test]
fn test_literal_mappings_golden_output() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map(
        "function __BASIC__(__PARAM1__, __PARAM2__) { var __VAR__ = '__STR__'; }",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA,QAASA,UAAS,CAACC,UAAD,CAAaC,UAAb,CAAyB,CAAE,IAAIC,QAAU,SAAhB;")
            .set_sources(&["testcode"])
            .set_names(&["__BASIC__", "__PARAM1__", "__PARAM2__", "__VAR__"])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testMultilineMapping
#[test]
fn test_multiline_mapping() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check("function __BASIC__(__PARAM1__, __PARAM2__) {\nvar __VAR__ = '__STR__';\nvar __ANO__ = \"__STR2__\";\n}\n");
}

// port: SourceMapGeneratorV3Test#testMultilineMapping2
#[test]
fn test_multiline_mapping2() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check(
        "function __BASIC__(__PARAM1__, __PARAM2__) {\nvar __VAR__ = 1;\nvar __ANO__ = 2;\n}\n",
    );
}

// port: SourceMapGeneratorV3Test#testMultiFunctionMapping
#[test]
fn test_multi_function_mapping() {
    let test = SourceMapGeneratorV3Test::default();
    test.compile_and_check("function __BASIC__(__PARAM1__, __PARAM2__) {\nvar __VAR__ = '__STR__';\nvar __ANO__ = \"__STR2__\";\n}\nfunction __BASIC2__(__PARAM3__, __PARAM4__) {\nvar __VAR2__ = '__STR2__';\nvar __ANO2__ = \"__STR3__\";\n}\n");
}

// port: SourceMapGeneratorV3Test#testGoldenOutput0
#[test]
fn test_golden_output0() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map(
        "",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A;")
            .set_sources(&[])
            .set_names(&[])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testGoldenOutput0a
#[test]
fn test_golden_output0a() {
    let mut test = SourceMapGeneratorV3Test::default();
    test.check_source_map(
        "a;",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA;")
            .set_sources(&["testcode"])
            .set_names(&["a"])
            .build(),
    );
    test.base.source_map_include_sources_content = true;
    test.check_source_map(
        "a;",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA;")
            .set_sources(&["testcode"])
            .set_sources_content(&["a;"])
            .set_names(&["a"])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testGoldenOutput1
#[test]
fn test_golden_output1() {
    let mut test = SourceMapGeneratorV3Test::default();
    test.base.detail_level = DetailLevel::ALL;
    test.check_source_map("function f(foo, bar) { foo = foo + bar + 2; return foo; }", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(1).set_mappings("A,aAAAA,QAASA,EAAC,CAACC,GAAD,CAAMC,GAAN,CAAW,CAAED,GAAA,CAAMA,GAAN,CAAYC,GAAZ,CAAkB,CAAG,OAAOD,IAA9B;").set_sources(&["testcode"]).set_names(&["f", "foo", "bar"]).build());
    test.base.detail_level = DetailLevel::SYMBOLS;
    test.check_source_map("function f(foo, bar) { foo = foo + bar + 2; return foo; }", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(1).set_mappings("A,aAAAA,QAASA,EAATA,CAAWC,GAAXD,CAAgBE,GAAhBF,EAAuBC,GAAvBD,CAA6BC,GAA7BD,CAAmCE,GAAnCF,SAAmDC,IAAnDD;").set_sources(&["testcode"]).set_names(&["f", "foo", "bar"]).build());
    test.base.source_map_include_sources_content = true;
    test.check_source_map("function f(foo, bar) { foo = foo + bar + 2; return foo; }", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(1).set_mappings("A,aAAAA,QAASA,EAATA,CAAWC,GAAXD,CAAgBE,GAAhBF,EAAuBC,GAAvBD,CAA6BC,GAA7BD,CAAmCE,GAAnCF,SAAmDC,IAAnDD;").set_sources(&["testcode"]).set_sources_content(&["function f(foo, bar) { foo = foo + bar + 2; return foo; }"]).set_names(&["f", "foo", "bar"]).build());
}

// port: SourceMapGeneratorV3Test#testGoldenOutput2
#[test]
fn test_golden_output2() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map("function f(foo, bar) {\r\n\n\n\nfoo = foo + bar + foo;\nreturn foo;\n}", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(1).set_mappings("A,aAAAA,QAASA,EAAC,CAACC,GAAD,CAAMC,GAAN,CAAW,CAIrBD,GAAA,CAAMA,GAAN,CAAYC,GAAZ,CAAkBD,GAClB,OAAOA,IALc;").set_sources(&["testcode"]).set_names(&["f", "foo", "bar"]).build());
}

// port: SourceMapGeneratorV3Test#testGoldenOutput3
#[test]
fn test_golden_output3() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map_for_file(
        "c:\\myfile.js",
        "foo;",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA;")
            .set_sources(&[get_encoded_file_name()])
            .set_names(&["foo"])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testGoldenOutput4
#[test]
fn test_golden_output4() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map_for_file(
        "c:\\myfile.js",
        "foo;   boo;   goo;",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(1)
            .set_mappings("A,aAAAA,GAAOC,IAAOC;")
            .set_sources(&[get_encoded_file_name()])
            .set_names(&["foo", "boo", "goo"])
            .build(),
    );
}

// port: SourceMapGeneratorV3Test#testGoldenOutput5
#[test]
fn test_golden_output5() {
    let mut test = SourceMapGeneratorV3Test::default();
    test.base.detail_level = DetailLevel::ALL;
    test.check_source_map_for_file("c:\\myfile.js", "/** @preserve\n * this is a test.\n */\nconsole.log(a + 'this is a really long line that will force the mapping to span multiple lines 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789' + c + d + e);", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(6).set_mappings("A;;;;aAGAA,OAAQC,CAAAA,GAAR,CAAYC,CAAZ,CAAgB,mxCAAhB;AAAsyCC,CAAtyC,CAA0yCC,CAA1yC,CAA8yCC,CAA9yC;").set_sources(&[get_encoded_file_name()]).set_names(&["console", "log", "a", "c", "d", "e"]).build());
    test.base.detail_level = DetailLevel::SYMBOLS;
    test.check_source_map_for_file("c:\\myfile.js", "/** @preserve\n * this is a test.\n */\nconsole.log(a + 'this is a really long line that will force the mapping to span multiple lines 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789 123456789' + c + d + e);", TestJsonBuilder::create().set_version(3).set_file("testcode").set_line_count(6).set_mappings("A;;;;aAGAA,OAAQC,CAAAA,GAAR,CAAYC,CAAZ;AAAsyCC,CAAtyC,CAA0yCC,CAA1yC,CAA8yCC,CAA9yC;").set_sources(&[get_encoded_file_name()]).set_names(&["console", "log", "a", "c", "d", "e"]).build());
}

// port: SourceMapGeneratorV3Test#testGoldenOutput_semanticLineBreaks
#[test]
fn test_golden_output_semantic_line_breaks() {
    let test = SourceMapGeneratorV3Test::default();
    test.check_source_map_for_file(
        "c:\\myfile.js",
        "var myMultilineTemplate = `Item ${a}\nItem ${b}\nItem ${c}`;\n",
        TestJsonBuilder::create()
            .set_version(3)
            .set_file("testcode")
            .set_line_count(3)
            .set_mappings("A,aAAA,IAAIA,oBAAuB,QAAOC,CAAP;OACpBC,CADoB;OAEpBC,CAFoB;")
            .set_sources(&[get_encoded_file_name()])
            .set_names(&["myMultilineTemplate", "a", "b", "c"])
            .build(),
    );
}
// port: SourceMapGeneratorV3Test#testBasicDeterminism
#[test]
fn test_basic_determinism() {
    let test = SourceMapGeneratorV3Test::default();
    let result1 = test.compile_two("file1", "foo;", Some("file2".into()), Some("bar;"));
    let result2 = test.compile_two("file2", "foo;", Some("file1".into()), Some("bar;"));
    let map1 = get_source_map(&result1);
    let map2 = get_source_map(&result2);
    let files1 = map1.split('\n').nth(4).unwrap();
    let files2 = map2.split('\n').nth(4).unwrap();
    assert_eq!(files2, files1);
}
// port: SourceMapGeneratorV3Test#testParseSourceMetaMap
#[test]
fn test_parse_source_meta_map() {
    let test = SourceMapGeneratorV3Test::default();
    const INPUT1: &str = "file1";
    const INPUT2: &str = "file2";
    let mut inputs: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
    inputs.insert(INPUT1.into(), "var __FOO__ = 1;".into());
    inputs.insert(INPUT2.into(), "var __BAR__ = 2;".into());
    let result1 = test.compile(inputs[&JsString::from(INPUT1)].clone(), INPUT1);
    let result2 = test.compile(inputs[&JsString::from(INPUT2)].clone(), INPUT2);
    const MAP1: &str = "map1";
    const MAP2: &str = "map2";
    let mut maps: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
    maps.insert(MAP1.into(), result1.source_map_file_content.into());
    maps.insert(MAP2.into(), result2.source_map_file_content.into());
    let mut sections = Vec::new();
    let mut output = String::new();
    let offset = append_and_count(&mut output, &result1.generated_source);
    sections.push(SourceMapSection::for_url(MAP1, 0, 0));
    output.push_str(&result2.generated_source.to_string_lossy());
    sections.push(SourceMapSection::for_url(
        MAP2,
        offset.get_line(),
        offset.get_column(),
    ));
    let generator = SourceMapGeneratorV3::new();
    let mut map_contents = String::new();
    generator
        .append_index_map_to(&mut map_contents, "out.js", &sections)
        .unwrap();
    // port: SourceMapGeneratorV3Test#testParseSourceMetaMap.getSourceMap
    let supplier = |url: &JsString| Ok(maps.get(url).cloned());
    test.base.checks.check_with_supplier(
        &inputs,
        &output.into(),
        &map_contents.into(),
        Some(&supplier),
    );
}
// port: SourceMapGeneratorV3Test#testSourceMapMerging
#[test]
fn test_source_map_merging() {
    let test = SourceMapGeneratorV3Test::default();
    const INPUT1: &str = "file1";
    const INPUT2: &str = "file2";
    let mut inputs: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();
    inputs.insert(INPUT1.into(), "var __FOO__ = 1;".into());
    inputs.insert(INPUT2.into(), "var __BAR__ = 2;".into());
    let result1 = test.compile(inputs[&JsString::from(INPUT1)].clone(), INPUT1);
    let result2 = test.compile(inputs[&JsString::from(INPUT2)].clone(), INPUT2);
    let mut output = String::new();
    let offset = append_and_count(&mut output, &result1.generated_source);
    output.push_str(&result2.generated_source.to_string_lossy());
    let mut generator = SourceMapGeneratorV3::new();
    generator
        .merge_map_section(0, 0, result1.source_map_file_content)
        .unwrap();
    generator
        .merge_map_section(
            offset.get_line(),
            offset.get_column(),
            result2.source_map_file_content,
        )
        .unwrap();
    let mut map_contents = String::new();
    generator
        .append_to(&mut map_contents, Some("out.js".into()))
        .unwrap();
    test.base
        .checks
        .check_inputs(&inputs, &output.into(), &map_contents.into());
}
// port: SourceMapGeneratorV3Test#buildMergeSampleMappings
fn build_merge_sample_mappings(merge_identical_segments: bool) -> String {
    let mut g = SourceMapGeneratorV3::new();
    g.set_merge_identical_segments(merge_identical_segments);
    add_mapping(&mut g, "a.js", Some("x"), 1, 1, 0, 0, 0, 20);
    add_mapping(&mut g, "a.js", Some("x"), 1, 1, 0, 2, 0, 5);
    add_mapping(&mut g, "a.js", Some("x"), 1, 1, 0, 5, 0, 8);
    add_mapping(&mut g, "a.js", None, 2, 3, 0, 10, 0, 12);
    add_mapping(&mut g, "a.js", Some("y"), 1, 1, 0, 14, 0, 16);
    add_mapping(&mut g, "b.js", Some("z"), 5, 0, 0, 22, 1, 3);
    add_mapping(&mut g, "b.js", Some("z"), 5, 0, 1, 3, 1, 6);
    add_mapping(&mut g, "b.js", Some("z"), 5, 0, 1, 6, 1, 9);
    add_mapping(&mut g, "b.js", Some("z"), 5, 0, 1, 11, 1, 13);
    add_mapping(&mut g, "c.js", Some("z"), 5, 0, 1, 13, 1, 15);
    let mut out = String::new();
    g.append_to(&mut out, Some("out.js".into())).unwrap();
    out
}

// port: SourceMapGeneratorV3Test#addMapping
#[allow(clippy::too_many_arguments)] // Java's signature.
fn add_mapping(
    generator: &mut SourceMapGeneratorV3,
    source: &str,
    name: Option<&str>,
    src_line: i32,
    src_col: i32,
    start_line: i32,
    start_col: i32,
    end_line: i32,
    end_col: i32,
) {
    generator.add_mapping(
        Some(source.into()),
        name.map(JsString::from),
        FilePosition::new(src_line, src_col),
        FilePosition::new(start_line, start_col),
        FilePosition::new(end_line, end_col),
    );
}

// port: SourceMapGeneratorV3Test#getMappings
fn get_mappings(source_map: &str) -> String {
    source_map_test_case::common::parse_json_object(source_map)
        .get("mappings")
        .unwrap()
        .get_as_string()
        .to_string()
}

// port: SourceMapGeneratorV3Test#countSegments
fn count_segments(source_map: &str) -> usize {
    let mut count = 0;
    for line in get_mappings(source_map).split(';') {
        if !line.is_empty() {
            count += line.split(',').count();
        }
    }
    count
}

// port: SourceMapGeneratorV3Test#assertSameLookups
fn assert_same_lookups(expected_map: &str, actual_map: &str, max_column: i32) {
    let mut expected = SourceMapConsumerV3::new();
    expected.parse(expected_map).unwrap();
    let mut actual = SourceMapConsumerV3::new();
    actual.parse(actual_map).unwrap();
    assert_eq!(actual.get_line_count(), expected.get_line_count());
    for line in 1..=expected.get_line_count() + 1 {
        for column in 1..=max_column {
            assert_eq!(
                actual.get_mapping_for_line(line, column),
                expected.get_mapping_for_line(line, column),
                "line {line}, column {column}"
            );
        }
    }
}

// port: SourceMapGeneratorV3Test#testMergeIdenticalSegments_disabledByDefaultKeepsEverySegment
#[test]
fn test_merge_identical_segments_disabled_by_default_keeps_every_segment() {
    let expected_mappings =
        "AACCA,EAAAA,GAAAA,GAAAA,EACE,EADFA,EAAAC,EAAAD,I,ECIDE;GAAAA,GAAAA,G,EAAAA,ECAAA;";
    assert_eq!(
        get_mappings(&build_merge_sample_mappings(false)),
        expected_mappings
    );

    let mut generator = SourceMapGeneratorV3::new();
    add_mapping(&mut generator, "a.js", Some("x"), 1, 1, 0, 0, 0, 20);
    add_mapping(&mut generator, "a.js", Some("x"), 1, 1, 0, 2, 0, 5);
    let mut out = String::new();
    generator
        .append_to(&mut out, Some("out.js".into()))
        .unwrap();
    assert_eq!(count_segments(&out), 3);
}

// port: SourceMapGeneratorV3Test#testMergeIdenticalSegments
#[test]
fn test_merge_identical_segments() {
    let unmerged = build_merge_sample_mappings(false);
    let merged = build_merge_sample_mappings(true);
    assert_eq!(
        get_mappings(&merged),
        "AACCA,UACE,EADFA,EAAAC,EAAAD,I,ECIDE;GAAAA,M,EAAAA,ECAAA;"
    );
    assert_same_lookups(&unmerged, &merged, 30);

    // Distinct original line/col and line starts are never merged.
    let mut g = SourceMapGeneratorV3::new();
    g.set_merge_identical_segments(true);
    add_mapping(&mut g, "a.js", Some("x"), 1, 1, 0, 0, 0, 2);
    add_mapping(&mut g, "a.js", Some("x"), 2, 1, 0, 2, 0, 4);
    add_mapping(&mut g, "a.js", Some("x"), 2, 2, 0, 4, 0, 6);
    add_mapping(&mut g, "a.js", Some("x"), 2, 2, 1, 0, 1, 4);
    let mut out = String::new();
    g.append_to(&mut out, Some("out.js".into())).unwrap();
    // 3 distinct segments on line 0 + 1 unmapped segment from col 6 to line end + 1 on line 1.
    assert_eq!(count_segments(&out), 5);

    let js = "goog.asserts.assert(__A__.__B__.__C__);
var __X__ = `line ${__D__.__E__}
line ${__F__.__G__.__H__}`;
__I__.__J__.__K__(1, __L__.__M__);
";
    let mut test = SourceMapGeneratorV3Test::default();
    test.base.source_map_merge_identical_segments = false;
    let unmerged_run = test.compile(js, "testcode");
    test.base.source_map_merge_identical_segments = true;
    let merged_run = test.compile(js, "testcode");
    assert!(
        count_segments(&merged_run.source_map_file_content)
            < count_segments(&unmerged_run.source_map_file_content)
    );
    assert_same_lookups(
        &unmerged_run.source_map_file_content,
        &merged_run.source_map_file_content,
        80,
    );
    test.base.checks.check(
        &"testcode".into(),
        &js.into(),
        &merged_run.generated_source,
        &merged_run.source_map_file_content.clone().into(),
    );
}

// port: SourceMapGeneratorV3Test#count
fn count(js: &JsString) -> FilePosition {
    let mut line = 0;
    let mut column = 0;
    for i in 0..js.length() {
        if js.char_at(i) == u16::from(b'\n') {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    FilePosition::new(line, column)
}
// port: SourceMapGeneratorV3Test#appendAndCount
fn append_and_count(out: &mut String, js: &JsString) -> FilePosition {
    out.push_str(&js.to_string_lossy());
    count(js)
}
