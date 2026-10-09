/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTest.java.

use closure_jscomp::{
    Compiler,
    black_hole_error_manager::BlackHoleErrorManager,
    check_level::CheckLevel,
    compilation_level::CompilationLevel,
    compiler::CodeBuilder,
    compiler_input::CompilerInput,
    compiler_options::{CompilerOptions, LanguageMode, SegmentOfCompilationToRun},
    dependency_options::DependencyOptions,
    diagnostic_groups,
    index_provider::IndexProvider,
    js_chunk::{JSChunk, WEAK_CHUNK_NAME},
    source_file::SourceFile,
    source_map_input::SourceMapInput,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{input_id::InputId, static_source_file::SourceKind};
use closure_sourcemap::{
    file_position::FilePosition,
    proto::mapping::{OriginalMapping, Precision},
    source_map_consumer_v3::SourceMapConsumerV3,
    source_map_generator_v3::SourceMapGeneratorV3,
};
use std::{any::TypeId, sync::Arc};
fn file(name: &str, code: &str) -> Arc<SourceFile> {
    Arc::new(SourceFile::from_code(name, code))
}
fn compiler() -> Compiler {
    Compiler::new_with_error_manager(Box::new(BlackHoleErrorManager::new()))
}
fn advanced() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    options.set_emit_use_strict(false);
    options
}
fn compile_input(code: &str, options: CompilerOptions) -> Compiler {
    let mut compiler = compiler();
    compiler.compile_single(file("extern.js", ""), file("test.js", code), options);
    compiler
}
fn panic_message(f: impl FnOnce()) -> String {
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_err();
    error
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_else(|| error.downcast_ref::<&str>().unwrap().to_string())
}
// port: CompilerTest#testCodeBuilderColumnAfterReset
#[test]
fn test_code_builder_column_after_reset() {
    let mut cb = CodeBuilder::default();
    cb.append("foo();\ngoo();");
    assert_eq!(cb.to_js_string(), "foo();\ngoo();");
    assert_eq!(cb.get_line_index(), 1);
    assert_eq!(cb.get_column_index(), 6);
    cb.reset();
    assert_eq!(cb.to_js_string(), "");
    assert_eq!(cb.get_line_index(), 1);
    assert_eq!(cb.get_column_index(), 6);
}
// port: CompilerTest#testCodeBuilderAppend
#[test]
fn test_code_builder_append() {
    let mut cb = CodeBuilder::default();
    cb.append("foo();");
    assert_eq!(cb.get_line_index(), 0);
    assert_eq!(cb.get_column_index(), 6);
    cb.append("goo();");
    assert_eq!(cb.get_line_index(), 0);
    assert_eq!(cb.get_column_index(), 12);
    cb.append("blah();\ngoo();");
    assert_eq!(cb.get_line_index(), 1);
    assert_eq!(cb.get_column_index(), 6);
}
// port: CompilerTest#testCyclicalDependencyInInputs
#[test]
fn test_cyclical_dependency_in_inputs() {
    let inputs = [
        file(
            "gin",
            "goog.provide('gin'); goog.require('tonic'); var gin = {};",
        ),
        file(
            "tonic",
            "goog.provide('tonic'); goog.require('gin'); var tonic = {};",
        ),
        file("mix", "goog.require('gin'); goog.require('tonic');"),
    ];
    let mut options = CompilerOptions::new();
    options.set_checks_only(true);
    options.set_continue_after_errors(true);
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(Vec::new()));
    let mut c = compiler();
    c.init(&[], &inputs, options);
    c.parse_inputs();
    assert_eq!(c.get_js_root().unwrap().get_parent(&c), c.get_root());
    assert_eq!(c.get_externs_root().unwrap().get_parent(&c), c.get_root());
    assert!(c.get_root().is_some());
    assert_eq!(c.get_js_root().unwrap().get_child_count(&c), 3);
}
// port: CompilerTest#testPrintExterns
#[test]
fn test_print_externs() {
    let mut options = CompilerOptions::new();
    options.set_preserve_type_annotations(true);
    options.set_language_in(LanguageMode::ECMASCRIPT3);
    options.set_print_externs(true);
    let mut c = compiler();
    c.init(
        &[file("extern", "/** @externs */ function alert(x) {}")],
        &[],
        options,
    );
    c.parse_inputs();
    assert_eq!(c.to_source(), "/** @externs */ function alert(x){};");
}
// port: CompilerTest#testClosureUnawareCodeMarks
#[test]
fn test_closure_unaware_code_marks() {
    let mut options = CompilerOptions::new();
    options.set_preserve_type_annotations(true);
    options.set_language_in(LanguageMode::ECMASCRIPT3);
    let mut c = compiler();
    c.init(
        &[],
        &[file(
            "closure_unaware_code.js",
            "/** @fileoverview @closureUnaware */ function alert(x) {}",
        )],
        options,
    );
    c.parse_inputs();
    assert_eq!(c.to_source(), "/** @closureUnaware */ function alert(x){};");
    let root = c.get_js_root().unwrap();
    assert_eq!(root.get_child_count(&c), 1);
    let source = root
        .get_first_child(&c)
        .unwrap()
        .get_static_source_file(&c)
        .unwrap();
    assert_eq!(source.get_name(), "closure_unaware_code.js");
    assert!(source.is_closure_unaware_code());
}
// port: CompilerTest#testLocalUndefined
#[test]
fn test_local_undefined() {
    let mut options = CompilerOptions::new();
    CompilationLevel::SIMPLE_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let mut c = compiler();
    c.compile_single(
        file("externs.js", ""),
        file(
            "input.js",
            "(function (undefined) { alert(undefined); })();",
        ),
        options,
    );
}
// port: CompilerTest#testNormalInputs
#[test]
fn test_normal_inputs() {
    let mut c = compiler();
    c.compile(
        &[file("externs", "")],
        &[file("in1", ""), file("in2", "")],
        CompilerOptions::new(),
    );
    assert!(c.get_input(&InputId::new("externs")).unwrap().is_extern());
    assert!(!c.get_input(&InputId::new("in1")).unwrap().is_extern());
    assert!(!c.get_input(&InputId::new("in2")).unwrap().is_extern());
}
// port: CompilerTest#testRebuildInputsFromChunk
#[test]
fn test_rebuild_inputs_from_chunk() {
    let m1 = JSChunk::new("m1");
    let m2 = JSChunk::new("m2");
    m1.add_source_file(file("in1", ""));
    m2.add_source_file(file("in2", ""));
    let mut c = compiler();
    c.init_chunks(&[], vec![m1, m2.clone()], CompilerOptions::new());
    m2.add_source_file(file("in3", ""));
    assert!(c.get_input(&InputId::new("in3")).is_none());
    c.rebuild_inputs_from_chunks();
    assert!(c.get_input(&InputId::new("in3")).is_some());
}
// port: CompilerTest#testMalformedFunctionInExterns
#[test]
fn test_malformed_function_in_externs() {
    compiler().compile(
        &[file("externs", "function f {}")],
        &[file("foo", "")],
        CompilerOptions::new(),
    );
}
// port: CompilerTest#testGetSourceInfoInExterns
#[test]
fn test_get_source_info_in_externs() {
    let mut c = compiler();
    c.compile(
        &[file("externs", "function f() {}\n")],
        &[file("foo", "function g() {}\n")],
        CompilerOptions::new(),
    );
    assert_eq!(
        c.get_source_line("externs", 1).as_deref(),
        Some("function f() {}")
    );
    assert_eq!(
        c.get_source_line("foo", 1).as_deref(),
        Some("function g() {}")
    );
    assert_eq!(c.get_source_line("bar", 1), None);
}
// port: CompilerTest#testFileoverviewTwice
#[test]
fn test_fileoverview_twice() {
    assert!(
        compiler()
            .compile(
                &[file("externs", "")],
                &[file(
                    "foo",
                    "/** @fileoverview */ var x; /** @fileoverview */ var y;"
                )],
                CompilerOptions::new()
            )
            .success
    );
}
// port: CompilerTest#testInputDelimiters
#[test]
fn test_input_delimiters() {
    let mut options = advanced();
    options.set_print_input_delimiter(true);
    let mut c = compiler();
    assert!(
        c.compile(
            &[file("externs", "")],
            &[file("i1", ""), file("i2", "/** @fileoverview Foo */")],
            options
        )
        .success
    );
    assert_eq!(c.to_source(), "// Input 0\n// Input 1\n");
}
// port: CompilerTest#testBug2176967Default
#[test]
fn test_bug2176967_default() {
    let c = compile_input("/** @XYZ */\n var x", advanced());
    assert_eq!(c.get_warnings().len(), 1);
    assert!(c.get_errors().is_empty());
}
// port: CompilerTest#testBug2176967Off
#[test]
fn test_bug2176967_off() {
    let mut options = advanced();
    options.set_warning_level(
        diagnostic_groups::NON_STANDARD_JSDOC.clone(),
        CheckLevel::OFF,
    );
    let c = compile_input("/** @XYZ */\n var x", options);
    assert!(c.get_warnings().is_empty());
    assert!(c.get_errors().is_empty());
}
// port: CompilerTest#testBug2176967Error
#[test]
fn test_bug2176967_error() {
    let mut options = advanced();
    options.set_warning_level(
        diagnostic_groups::NON_STANDARD_JSDOC.clone(),
        CheckLevel::ERROR,
    );
    let c = compile_input("/** @XYZ */\n var x", options);
    assert!(c.get_warnings().is_empty());
    assert_eq!(c.get_errors().len(), 1);
}
// port: CompilerTest#testGetEmptyResult
#[test]
fn test_get_empty_result() {
    assert!(Compiler::new().get_result().errors.is_empty());
}
// port: CompilerTest#testAnnotation
#[test]
fn test_annotation() {
    let mut c = Compiler::new();
    assert!(!c.run_j2cl_passes());
    c.set_run_j2cl_passes(true);
    assert!(c.run_j2cl_passes());
}
struct StringProvider;
impl IndexProvider<String> for StringProvider {
    fn get(&mut self) -> String {
        "String".into()
    }
    fn get_type(&self) -> TypeId {
        TypeId::of::<String>()
    }
}
struct DoubleProvider;
impl IndexProvider<f64> for DoubleProvider {
    fn get(&mut self) -> f64 {
        f64::MAX
    }
    fn get_type(&self) -> TypeId {
        TypeId::of::<f64>()
    }
}
// port: CompilerTest#testAddIndexProvider_ThenGetIndex
#[test]
fn test_add_index_provider_then_get_index() {
    let mut c = Compiler::new();
    c.add_index_provider(StringProvider);
    c.add_index_provider(DoubleProvider);
    assert_eq!(c.get_index::<String>().as_deref(), Some("String"));
    assert_eq!(c.get_index::<f64>(), Some(f64::MAX));
    assert_eq!(c.get_index::<()>(), None);
}
// port: CompilerTest#testAddIndexProviderTwice_isException
#[test]
fn test_add_index_provider_twice_is_exception() {
    let mut c = Compiler::new();
    c.add_index_provider(StringProvider);
    panic_message(|| c.add_index_provider(StringProvider));
}
// port: CompilerTest#testCreateSyntheticExternsInput_setsCorrectInputId
#[test]
fn test_create_synthetic_externs_input_sets_correct_input_id() {
    let mut c = compiler();
    c.init(&[], &[], CompilerOptions::new());
    let input = c.get_synthesized_externs_input().clone();
    assert_eq!(c.get_inputs_by_id().get(input.get_input_id()), Some(&input));
}
// port: CompilerTest#testAssumePropertiesAreStaticallyAnalyzable_compatibleWithSimpleOptimizationsMode
#[test]
fn test_assume_properties_are_statically_analyzable_compatible_with_simple_optimizations_mode() {
    let mut options = CompilerOptions::new();
    CompilationLevel::SIMPLE_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    options.set_assume_properties_are_statically_analyzable(false);
    let c = compile_input("", options);
    assert!(c.get_errors().is_empty());
}
// port: CompilerTest#testAssumePropertiesAreStaticallyAnalyzable_forbidsPropertyRenaming
#[test]
fn test_assume_properties_are_statically_analyzable_forbids_property_renaming() {
    let mut options = CompilerOptions::new();
    options.set_property_renaming(
        closure_jscomp::property_renaming_policy::PropertyRenamingPolicy::ALL_UNQUOTED,
    );
    options.set_assume_properties_are_statically_analyzable(false);
    let mut compiler = compiler();
    let externs = file("externs.js", "");
    let input = file("input.js", ";");

    let ex = panic_message(|| {
        let _ = compiler.compile_single(externs, input, options);
    });

    assert_eq!(
        ex,
        "Precondition for pass renameProperties failed: \
         requires assumePropertiesAreStaticallyAnalyzable to be enabled"
    );
}

const SOURCE_MAP_TEST_CODE: &str = "var X = (function () {\n    function X(input) {\n        this.y = input;\n    }\n    return X;\n}());\nconsole.log(new X(1));\n";

#[allow(dead_code)] // Keep the shared Java fixture for the pending source-information test.
const SOURCE_MAP: &str = "{\"version\":3,\"file\":\"foo.js\",\"sourceRoot\":\"\",\"sources\":[\"foo.ts\"],\"names\":[],\"mappings\":\"AAAA;IAGE,WAAY,KAAa;QACvB,IAAI,CAAC,CAAC,GAAG,KAAK,CAAC;IACjB,CAAC;IACH,QAAC;AAAD,CAAC,AAND,IAMC;AAED,OAAO,CAAC,GAAG,CAAC,IAAI,CAAC,CAAC,CAAC,CAAC,CAAC,CAAC\"}";

const BASE64_ENCODED_SOURCE_MAP: &str = "data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiZm9vLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZm9vLnRzIl0sIm5hbWVzIjpbXSwibWFwcGluZ3MiOiJBQUFBO0lBR0UsV0FBWSxLQUFhO1FBQ3ZCLElBQUksQ0FBQyxDQUFDLEdBQUcsS0FBSyxDQUFDO0lBQ2pCLENBQUM7SUFDSCxRQUFDO0FBQUQsQ0FBQyxBQU5ELElBTUM7QUFFRCxPQUFPLENBQUMsR0FBRyxDQUFDLElBQUksQ0FBQyxDQUFDLENBQUMsQ0FBQyxDQUFDLENBQUMifQ==";

const SOURCE_MAP_TEST_CONTENT: &str = "var A = (function () {\n    function A(input) {\n        this.a = input;\n    }\n    return A;\n}());\nconsole.log(new A(1));";

const BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT: &str = "data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiZm9vLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiLi4vdGVzdC9mb28udHMiXSwic291cmNlc0NvbnRlbnQiOlsidmFyIEEgPSAoZnVuY3Rpb24gKCkge1xuICAgIGZ1bmN0aW9uIEEoaW5wdXQpIHtcbiAgICAgICAgdGhpcy5hID0gaW5wdXQ7XG4gICAgfVxuICAgIHJldHVybiBBO1xufSgpKTtcbmNvbnNvbGUubG9nKG5ldyBBKDEpKTsiXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUE7SUFHRSxXQUFZLEtBQWE7UUFDdkIsSUFBSSxDQUFDLENBQUMsR0FBRyxLQUFLLENBQUM7SUFDakIsQ0FBQztJQUNILFFBQUM7QUFBRCxDQUFDLEFBTkQsSUFNQztBQUVELE9BQU8sQ0FBQyxHQUFHLENBQUMsSUFBSSxDQUFDLENBQUMsQ0FBQyxDQUFDLENBQUMsQ0FBQyJ9";
// port: CompilerTest#sourcemap
fn sourcemap(path: &str, original: &str, position: FilePosition) -> Arc<SourceMapInput> {
    let mut map = SourceMapGeneratorV3::new();
    map.add_mapping(
        Some(original.into()),
        Some("testSymbolName".into()),
        position,
        FilePosition::new(1, 1),
        FilePosition::new(100, 1),
    );
    let mut buffer = String::new();
    let output = map
        .append_to(&mut buffer, Some("unused.js".into()))
        .unwrap();
    Arc::new(SourceMapInput::new(Arc::new(SourceFile::from_code(
        path, output,
    ))))
}
// port: CompilerTest#testInputSourceMaps
#[test]
fn test_input_source_maps() {
    let mut options = CompilerOptions::new();
    options.set_input_source_maps(IndexMap::<_, _>::from_iter([(
        "generated_js/example.js".into(),
        sourcemap(
            "generated_js/example.srcmap",
            "../original/source.html",
            FilePosition::new(17, 25),
        ),
    )]));
    let mut c = compiler();
    c.init(
        &[],
        &[file("original/source.html", "<div ng-show='foo()'>")],
        options,
    );
    assert_eq!(
        c.get_source_mapping(Some("generated_js/example.js"), 3, 3),
        Some(
            OriginalMapping::new_builder()
                .set_original_file("original/source.html")
                .set_line_number(18)
                .set_column_position(25)
                .set_identifier("testSymbolName")
                .set_precision(Precision::APPROXIMATE_LINE)
                .build()
        )
    );
    assert_eq!(
        c.get_source_line("original/source.html", 1).as_deref(),
        Some("<div ng-show='foo()'>")
    );
}
// port: CompilerTest#testInputSourceMapInline
#[test]
fn test_input_source_map_inline() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let input = CompilerInput::new(file(
        "tmp",
        &format!("{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP}"),
    ));
    input.get_ast_root(&mut c);
    let maps = c.get_input_source_maps();
    let consumer = maps
        .get("tmp")
        .unwrap()
        .get_source_map(&mut BlackHoleErrorManager::new())
        .unwrap();
    assert_eq!(consumer.get_original_sources(), &[Some("foo.ts".into())]);
    assert_eq!(consumer.get_original_sources_content(), None);
    assert!(consumer.get_original_names().is_empty());
}
// port: CompilerTest#testInputSourceMapInlineContent
#[test]
fn test_input_source_map_inline_content() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let code = format!(
        "{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT}"
    );
    let input = CompilerInput::new(file("tmp", &code));
    input.get_ast_root(&mut c);
    let maps = c.get_input_source_maps();
    let consumer = maps
        .get("tmp")
        .unwrap()
        .get_source_map(&mut BlackHoleErrorManager::new())
        .unwrap();
    assert_eq!(
        consumer.get_original_sources(),
        &[Some("../test/foo.ts".into())]
    );
    assert_eq!(
        consumer.get_original_sources_content(),
        Some(&[Some(SOURCE_MAP_TEST_CONTENT.into())][..])
    );
    assert!(consumer.get_original_names().is_empty());
}
// port: CompilerTest#testApplyInputSourceMaps
#[test]
fn test_apply_input_source_maps() {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT3);
    options.set_source_map_output_path("fake/source_map_path.js.map".into());
    options.set_input_source_maps(IndexMap::<_, _>::from_iter([(
        "input.js".into(),
        sourcemap("input.js.map", "input.ts", FilePosition::new(17, 25)),
    )]));
    options.set_apply_input_source_maps(true);
    let mut c = compiler();
    c.compile_single(
        file("externs", ""),
        file("input.js", "// Unmapped line\nvar x = 1;\nalert(x);"),
        options,
    );
    assert_eq!(c.to_source(), "var x=1;alert(x);");
    let mut output = String::new();
    c.get_source_map()
        .unwrap()
        .lock()
        .unwrap()
        .append_to(&mut output, "source.js.map")
        .unwrap();
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(output.as_str()).unwrap();
    let mapping = consumer.get_mapping_for_line(1, 5).unwrap();
    assert_eq!(mapping.get_original_file(), "input.ts");
    assert_eq!(mapping.get_line_number(), 18);
    assert_eq!(mapping.get_column_position(), 26);
    assert_eq!(mapping.get_identifier(), "testSymbolName");
    assert_eq!(consumer.get_original_sources(), &[Some("input.ts".into())]);
    assert_eq!(consumer.get_original_sources_content(), None);
    assert_eq!(
        consumer.get_original_names(),
        &[Some("testSymbolName".into())]
    );
}
// port: CompilerTest#testKeepInputSourceMapsSourcesContent
#[test]
fn test_keep_input_source_maps_sources_content() {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT3);
    options.set_source_map_output_path("fake/source_map_path.js.map".into());
    options.set_apply_input_source_maps(true);
    options.set_source_map_include_sources_content(true);
    let code = format!(
        "{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT}"
    );
    let mut c = compiler();
    c.compile_single(
        file("externs", ""),
        file("temp/path/input.js", &code),
        options,
    );
    assert_eq!(
        c.to_source(),
        "var X=function(){function X(input){this.y=input}return X}();console.log(new X(1));"
    );
    let mut output = String::new();
    c.get_source_map()
        .unwrap()
        .lock()
        .unwrap()
        .append_to(&mut output, "source.js.map")
        .unwrap();
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(output.as_str()).unwrap();
    assert_eq!(
        consumer.get_original_sources(),
        &[Some("temp/test/foo.ts".into())]
    );
    assert_eq!(
        consumer.get_original_sources_content(),
        Some(&[Some(SOURCE_MAP_TEST_CONTENT.into())][..])
    );
    assert_eq!(
        consumer.get_original_names(),
        &[
            Some("X".into()),
            Some("input".into()),
            Some("y".into()),
            Some("console".into()),
            Some("log".into())
        ]
    );
}
const RESULT_SOURCE_MAP_WITH_CONTENT: &str = r#"{
"version":3,
"file":"output.js",
"lineCount":1,
"mappings":"AAQAA,OAAQC,CAAAA,GAAR,CAAY,IALVC,QAAA,EAAyB,EAKf,CAAM,CAAN,CAAZ;",
"sources":["../test/foo.ts"],
"sourcesContent":["var A = (function () {\n    function A(input) {\n        this.a = input;\n    }\n    return A;\n}());\nconsole.log(new A(1));"],
"names":["console","log","X"]
}
"#;

// port: CompilerTest#testSingleStageCompileSourceMaps
#[test]
fn test_single_stage_compile_source_maps() {
    let mut compiler = test_error_manager_compiler();

    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
    options.set_language_out(LanguageMode::ECMASCRIPT_NEXT);
    options.set_check_types(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    options.set_preserve_detailed_source_info(true);
    // The name passed here doesn't matter, because the compiler itself only stores it and enables
    // tracking of source map information when it is non-null.
    // In real usage AbstractCommandLineRunner is responsible for actually writing the file whose
    // path is stored in this field.
    options.set_source_map_output_path("dummy".into());
    options.set_apply_input_source_maps(true);
    options.set_source_map_include_sources_content(true);
    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs = vec![file(
        "externs.js",
        "var console = {};\n console.log = function() {};\n",
    )];
    let srcs = vec![file(
        "input.js",
        &format!(
            "{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT}"
        ),
    )];
    compiler.init(&externs, &srcs, options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    compiler.perform_finalizations();

    let result = compiler.get_result();

    let source = compiler.to_source();
    assert_eq!(source, "console.log(new function(){}(1));");

    let source_map = result.source_map.clone();
    assert!(source_map.is_some());

    // Check sourcemap output
    let mut source_map_string_builder = String::new();
    let output_js_file = "output.js";
    source_map
        .unwrap()
        .lock()
        .unwrap()
        .append_to(&mut source_map_string_builder, output_js_file)
        .unwrap();
    assert_eq!(source_map_string_builder, RESULT_SOURCE_MAP_WITH_CONTENT);
}

// port: CompilerTest#testNoSourceMapIsGeneratedWithoutPath
#[test]
fn test_no_source_map_is_generated_without_path() {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT3);
    options.set_apply_input_source_maps(true);
    options.set_source_map_include_sources_content(true);
    let code = format!(
        "{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT}"
    );
    let c = compile_input(&code, options);
    assert!(c.get_source_map().is_none());
}
fn strictness(input: LanguageMode, output: Option<LanguageMode>, run_passes: bool, expected: &str) {
    let mut c = compiler();
    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_language_in(input);
    if let Some(output) = output {
        options.set_language_out(output);
    }
    c.init(&[], &[file("input.js", "console.log(0);")], options);
    c.parse();
    if run_passes {
        c.check();
        c.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    }
    assert_eq!(c.to_source(), expected);
}
// port: CompilerTest#testStrictnessWithNonStrictOutputLanguage
#[test]
fn test_strictness_with_non_strict_output_language() {
    strictness(
        LanguageMode::ECMASCRIPT_2017,
        Some(LanguageMode::ECMASCRIPT5),
        false,
        "console.log(0);",
    );
}
// port: CompilerTest#testStrictnessWithStrictOutputLanguage
#[test]
fn test_strictness_with_strict_output_language() {
    strictness(
        LanguageMode::ECMASCRIPT_2017,
        Some(LanguageMode::ECMASCRIPT5_STRICT),
        false,
        "'use strict';console.log(0);",
    );
}
// port: CompilerTest#testStrictnessWithNonStrictInputLanguage
#[test]
fn test_strictness_with_non_strict_input_language() {
    strictness(LanguageMode::ECMASCRIPT5, None, false, "console.log(0);");
}
// port: CompilerTest#testStrictnessWithStrictInputLanguage
#[test]
fn test_strictness_with_strict_input_language() {
    strictness(
        LanguageMode::ECMASCRIPT5_STRICT,
        None,
        false,
        "'use strict';console.log(0);",
    );
}
// port: CompilerTest#testStrictnessWithNonStrictInputLanguageAndNoTranspileOutput
#[test]
fn test_strictness_with_non_strict_input_language_and_no_transpile_output() {
    strictness(
        LanguageMode::ECMASCRIPT5,
        Some(LanguageMode::NO_TRANSPILE),
        true,
        "console.log(0);",
    );
}
// port: CompilerTest#testStrictnessWithStrictInputLanguageAndNoTranspileOutput
#[test]
fn test_strictness_with_strict_input_language_and_no_transpile_output() {
    strictness(
        LanguageMode::ECMASCRIPT5_STRICT,
        Some(LanguageMode::NO_TRANSPILE),
        true,
        "'use strict';console.log(0);",
    );
}
fn weak_input(name: &str, code: &str, kind: SourceKind) -> Arc<SourceFile> {
    Arc::new(SourceFile::from_code_with_kind(name, code, kind))
}
// port: CompilerTest#testPreexistingWeakChunkWithAdditionalStrongSources
#[test]
fn test_preexisting_weak_chunk_with_additional_strong_sources() {
    let strong = JSChunk::new("m");
    strong.add_source_file(weak_input(
        "strong.js",
        "goog.provide('a');",
        SourceKind::STRONG,
    ));
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    weak.add_source_file(weak_input(
        "weak.js",
        "goog.provide('b');",
        SourceKind::WEAK,
    ));
    weak.add_source_file(weak_input(
        "weak_but_actually_strong.js",
        "goog.provide('c');",
        SourceKind::STRONG,
    ));
    weak.add_dependency(&strong);
    let message =
        panic_message(|| compiler().init_chunks(&[], vec![strong, weak], CompilerOptions::new()));
    assert!(
        message.contains(
            "Found these strong sources in the weak chunk:\n  weak_but_actually_strong.js"
        )
    );
}
// port: CompilerTest#testPreexistingWeakChunkWithMissingWeakSources
#[test]
fn test_preexisting_weak_chunk_with_missing_weak_sources() {
    let strong = JSChunk::new("m");
    strong.add_source_file(weak_input(
        "strong.js",
        "goog.provide('a');",
        SourceKind::STRONG,
    ));
    strong.add_source_file(weak_input(
        "strong_but_actually_weak.js",
        "goog.provide('b');",
        SourceKind::WEAK,
    ));
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    weak.add_source_file(weak_input(
        "weak.js",
        "goog.provide('c');",
        SourceKind::WEAK,
    ));
    weak.add_dependency(&strong);
    let message =
        panic_message(|| compiler().init_chunks(&[], vec![strong, weak], CompilerOptions::new()));
    assert!(message.contains(
        "Found these weak sources in other chunks:\n  strong_but_actually_weak.js (in chunk m)"
    ));
}
// port: CompilerTest#testPreexistingWeakChunkWithIncorrectDependencies
#[test]
fn test_preexisting_weak_chunk_with_incorrect_dependencies() {
    let m1 = JSChunk::new("m1");
    let m2 = JSChunk::new("m2");
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    weak.add_dependency(&m1);
    let message =
        panic_message(|| compiler().init_chunks(&[], vec![m1, m2, weak], CompilerOptions::new()));
    assert_eq!(
        message,
        "A weak chunk already exists but it does not depend on every other chunk."
    );
}

use closure_jscomp::{
    compiler_pass::CompilerPass, custom_pass_execution_time::CustomPassExecutionTime,
    default_pass_config::DefaultPassConfig, error_manager::ErrorManager,
    module_identifier::ModuleIdentifier,
};
use closure_rhino::{
    jscomp_base::Tri,
    node::{Ast, NodeId},
};
use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const CONSOLE_EXTERNS: &str = include_str!("data/compiler_console_externs.js");

// port: CompilerTest#test
fn test_comment_output(js: &str, expected: &str) {
    let mut c = compiler();
    let result = c.compile(
        &[file("externs", "")],
        &[file("testcode", js), file("stdexterns", CONSOLE_EXTERNS)],
        advanced(),
    );
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(c.to_source(), expected.trim());
}

// port: CompilerTest#testImportantCommentOutput
#[test]
fn test_important_comment_output() {
    test_comment_output(
        "/*! Your favorite license goes here */ console.log(0);",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testOverviewAndImportantCommentOutput
#[test]
fn test_overview_and_important_comment_output() {
    test_comment_output(
        "/** @fileoverview This is my favorite file! */\n/*! Your favorite license goes here */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testImportantCommentOverviewImportantComment
#[test]
fn test_important_comment_overview_important_comment() {
    test_comment_output(
        "/*! Another license */\n/** @fileoverview This is my favorite file! */\n/*! Your favorite license goes here */\nconsole.log(0);\n",
        "/*\n Another license  Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testCombinedImportantCommentOverviewDirectiveOutput
#[test]
fn test_combined_important_comment_overview_directive_output() {
    test_comment_output(
        "/*! Your favorite license goes here\n * @fileoverview This is my favorite file! */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here\n @fileoverview This is my favorite file! */\nconsole.log(0);\n",
    );
}

// port: CompilerTest#testCombinedImportantCommentAuthorDirectiveOutput
#[test]
fn test_combined_important_comment_author_directive_output() {
    test_comment_output(
        "/*! Your favorite license goes here\n * @author Robert */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here\n @author Robert */\nconsole.log(0);",
    );
}

// port: CompilerTest#testMultipleImportantCommentDirectiveOutput
#[test]
fn test_multiple_important_comment_directive_output() {
    test_comment_output(
        "/*! Your favorite license goes here */\n/*! Another license */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here  Another license */\nconsole.log(0);",
    );
}

// port: CompilerTest#testImportantCommentLicenseDirectiveOutput
#[test]
fn test_important_comment_license_directive_output() {
    test_comment_output(
        "/*! Your favorite license goes here */\n/** @license Another license */\nconsole.log(0);\n",
        "/*\n Another license  Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testLicenseImportantCommentDirectiveOutput
#[test]
fn test_license_important_comment_directive_output() {
    test_comment_output(
        "/** @license Your favorite license goes here */\n/*! Another license */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here  Another license */\nconsole.log(0);",
    );
}

// port: CompilerTest#testLicenseDirectiveOutput
#[test]
fn test_license_directive_output() {
    test_comment_output(
        "/** @license Your favorite license goes here */ console.log(0);",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testOverviewAndLicenseDirectiveOutput
#[test]
fn test_overview_and_license_directive_output() {
    test_comment_output(
        "/** @fileoverview This is my favorite file! */\n/** @license Your favorite license goes here */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testLicenseOverviewLicense
#[test]
fn test_license_overview_license() {
    test_comment_output(
        "/** @license Another license */\n/** @fileoverview This is my favorite file! */\n/** @license Your favorite license goes here */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here  Another license */\nconsole.log(0);",
    );
}

// port: CompilerTest#testCombinedLicenseOverviewDirectiveOutput
#[test]
fn test_combined_license_overview_directive_output() {
    test_comment_output(
        "/** @license Your favorite license goes here\n * @fileoverview This is my favorite file! */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here\n @fileoverview This is my favorite file! */\nconsole.log(0);\n",
    );
}

// port: CompilerTest#testCombinedLicenseAuthorDirectiveOutput
#[test]
fn test_combined_license_author_directive_output() {
    test_comment_output(
        "/** @license Your favorite license goes here\n * @author Robert */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here\n @author Robert */\nconsole.log(0);",
    );
}

// port: CompilerTest#testMultipleLicenseDirectiveOutput
#[test]
fn test_multiple_license_directive_output() {
    test_comment_output(
        "/** @license Your favorite license goes here */\n/** @license Another license */\nconsole.log(0);\n",
        "/*\n Another license  Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testTwoLicenseInSameComment
#[test]
fn test_two_license_in_same_comment() {
    test_comment_output(
        "/** @license Your favorite license goes here\n  * @license Another license */\nconsole.log(0);\n",
        "/*\n Your favorite license goes here\n @license Another license */\nconsole.log(0);\n",
    );
}
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);
struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/compiler_io")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn parse_file(c: &mut Compiler, directory: &TempDirectory, name: &str, code: &str) {
    let path = directory.0.join(name);
    std::fs::write(&path, code).unwrap();
    CompilerInput::new(SourceFile::from_file(path.to_str().unwrap())).get_ast_root(c);
}
fn assert_original_sources(c: &Compiler) {
    let maps = c.get_input_source_maps();
    assert_eq!(maps.len(), 1);
    for input in maps.values() {
        let map = input
            .get_source_map(&mut BlackHoleErrorManager::new())
            .unwrap();
        assert_eq!(map.get_original_sources(), &[Some("foo.ts".into())]);
        assert_eq!(map.get_original_sources_content(), None);
        assert!(map.get_original_names().is_empty());
    }
}
// port: CompilerTest#testResolveRelativeSourceMap
#[test]
fn test_resolve_relative_source_map() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let directory = TempDirectory::new();
    std::fs::write(directory.0.join("foo.js.map"), SOURCE_MAP).unwrap();
    parse_file(
        &mut c,
        &directory,
        "foo.js",
        &format!("{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL=foo.js.map"),
    );
    assert_original_sources(&c);
}
// port: CompilerTest#testResolveRelativeDirSourceMap
#[test]
fn test_resolve_relative_dir_source_map() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let directory = TempDirectory::new();
    std::fs::create_dir(directory.0.join("relativedir")).unwrap();
    std::fs::write(directory.0.join("relativedir/foo.js.map"), SOURCE_MAP).unwrap();
    parse_file(
        &mut c,
        &directory,
        "foo.js",
        &format!("{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL=relativedir/foo.js.map"),
    );
    assert_original_sources(&c);
}
// port: CompilerTest#testMissingSourceMapFile
#[test]
fn test_missing_source_map_file() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let directory = TempDirectory::new();
    parse_file(
        &mut c,
        &directory,
        "foo2.js",
        &format!("{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL=foo-does-not-exist.js.map"),
    );
    let maps = c.get_input_source_maps();
    assert_eq!(maps.len(), 1);
    let mut manager = BlackHoleErrorManager::new();
    for input in maps.values() {
        assert!(input.get_source_map(&mut manager).is_none());
    }
    assert_eq!(manager.get_warning_count(), 1);
}
// port: CompilerTest#testNoWarningMissingAbsoluteSourceMap
#[test]
fn test_no_warning_missing_absolute_source_map() {
    let mut c = compiler();
    c.init_compiler_options_if_testing();
    let directory = TempDirectory::new();
    parse_file(
        &mut c,
        &directory,
        "foo.js",
        &format!("{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL=/some/missing/path/foo.js.map"),
    );
    assert!(c.get_input_source_maps().is_empty());
    assert_eq!(c.get_warning_count(), 0);
}
// port: CompilerTest#testArtificialFunctionValidation_defaultsToOnIfEnablesDiambiguateProperties
#[test]
fn test_artificial_function_validation_defaults_to_on_if_enables_diambiguate_properties() {
    let mut c = compiler();
    let mut options = advanced();
    options.set_disambiguate_properties(true);
    c.init_options(options);
    assert_eq!(
        c.get_options()
            .get_warnings_guard()
            .must_run_checks(&diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION),
        Tri::TRUE
    );
}
// port: CompilerTest#testArtificialFunctionValidation_defaultBehaviorWhenDisambiguateDisabled
#[test]
fn test_artificial_function_validation_default_behavior_when_disambiguate_disabled() {
    let mut c = compiler();
    let mut options = advanced();
    options.set_disambiguate_properties(false);
    c.init_options(options);
    assert_eq!(
        c.get_options()
            .get_warnings_guard()
            .must_run_checks(&diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION),
        Tri::UNKNOWN
    );
}
// port: CompilerTest#testArtificialFunctionValidation_canOverrideDefault
#[test]
fn test_artificial_function_validation_can_override_default() {
    let mut c = compiler();
    let mut options = advanced();
    options.set_disambiguate_properties(true);
    options.set_warning_level(
        diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION.clone(),
        CheckLevel::OFF,
    );
    c.init_options(options);
    assert_eq!(
        c.get_options()
            .get_warnings_guard()
            .must_run_checks(&diagnostic_groups::ARTIFICIAL_FUNCTION_PURITY_VALIDATION),
        Tri::FALSE
    );
}
// port: CompilerTest#testConsecutiveSemicolons
#[test]
fn test_consecutive_semicolons() {
    let mut c = compiler();
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    c.init_options(options);
    let js = "if(a);";
    let n = c.parse_test_code(js);
    let mut cb = CodeBuilder::default();
    let mut tracker =
        closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker::new(&c);
    c.to_source_with_builder(&mut cb, &mut tracker, 0, n);
    assert_eq!(cb.to_js_string(), js);
}
// port: CompilerTest#hasOutput
fn has_output(show_warnings_only_for: Option<&str>, path: &str, level: CheckLevel) -> bool {
    let mut options = advanced();
    if let Some(path) = show_warnings_only_for {
        options.add_warnings_guard(Arc::new(
            closure_jscomp::show_by_path_warnings_guard::ShowByPathWarningsGuard::new(path),
        ));
    }
    let mut c = compiler();
    c.init(&[], &[], options);
    c.report(
        closure_jscomp::js_error::JSError::builder(&TEST_ERROR, &[])
            .set_source_location(path, 1, 1)
            .set_level(level)
            .build(),
    );
    c.get_error_count() + c.get_warning_count() > 0
}
// port: CompilerTest#testWarningsFiltering
#[test]
fn test_warnings_filtering() {
    for (filter, path, level, expected) in [
        (None, "foo/bar.js", CheckLevel::WARNING, true),
        (None, "foo/bar.js", CheckLevel::ERROR, true),
        (Some("baz"), "foo/bar.js", CheckLevel::WARNING, false),
        (Some("foo"), "foo/bar.js", CheckLevel::WARNING, true),
        (Some("baz"), "foo/bar.js", CheckLevel::ERROR, true),
        (Some("foo"), "foo/bar.js", CheckLevel::ERROR, true),
    ] {
        assert_eq!(has_output(filter, path, level), expected);
    }
}
struct RecordPass(Arc<AtomicBool>);
impl CompilerPass for RecordPass {
    fn process(&mut self, _compiler: &mut Compiler, _externs: NodeId, _root: NodeId) {
        self.0.store(true, Ordering::Relaxed);
    }
}
// port: CompilerTest#testChecksOnlyModeSkipsOptimizations
#[test]
fn test_checks_only_mode_skips_optimizations() {
    let before = Arc::new(AtomicBool::new(false));
    let after = Arc::new(AtomicBool::new(false));
    let mut options = advanced();
    options.set_checks_only(true);
    options.add_custom_pass(
        CustomPassExecutionTime::BEFORE_OPTIMIZATIONS,
        Arc::new(Mutex::new(RecordPass(before.clone()))),
    );
    options.add_custom_pass(
        CustomPassExecutionTime::BEFORE_OPTIMIZATION_LOOP,
        Arc::new(Mutex::new(RecordPass(after.clone()))),
    );
    compiler().compile(
        &[file("externs", "")],
        &[file("testcode", "var x = 1;")],
        options,
    );
    assert!(before.load(Ordering::Relaxed));
    assert!(!after.load(Ordering::Relaxed));
}
// port: CompilerTest#testAdditionalReplacementsForClosure
#[test]
fn test_additional_replacements_for_closure() {
    let mut options = advanced();
    options.set_locale("it_IT".into());
    options.set_closure_pass(true);
    let mut ast = Ast::new();
    let replacements = DefaultPassConfig::get_additional_replacements(&mut ast, &options);
    assert_eq!(replacements.len(), 2);
    assert_eq!(replacements["goog.LOCALE"].get_string(&ast), "it_IT");
}
// port: CompilerTest#testExternsDependencySorting
#[test]
fn test_externs_dependency_sorting() {
    let inputs = [
        file(
            "leaf",
            "/** @fileoverview @typeSummary */ goog.require('beer');",
        ),
        file(
            "beer",
            "/** @fileoverview @typeSummary */ goog.provide('beer');\ngoog.require('hops');",
        ),
        file(
            "hops",
            "/** @fileoverview @typeSummary */ goog.provide('hops');",
        ),
    ];

    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::sort_only());

    let externs: [Arc<SourceFile>; 0] = [];
    let mut c = compiler();
    c.compile(&externs, &inputs, options);

    let externs_root = c.get_externs_root().unwrap();
    assert_eq!(externs_root.get_child_count(&c), 4);
    assert_extern_index(&mut c, 0, " [synthetic:externs] "); // added by VarCheck
    assert_extern_index(&mut c, 1, "hops");
    assert_extern_index(&mut c, 2, "beer");
    assert_extern_index(&mut c, 3, "leaf");
}
// port: CompilerTest#testImportantCommentAndOverviewDirectiveWarning
#[test]
fn test_important_comment_and_overview_directive_warning() {
    assert!(compiler().compile(&[file("externs","")],&[file("foo","/*! Your favorite license goes here */\n/**\n  * @fileoverview This is my favorite file! */\nvar x;\n")],CompilerOptions::new()).success);
}
// port: CompilerTest#testLicenseAndOverviewDirectiveWarning
#[test]
fn test_license_and_overview_directive_warning() {
    assert!(compiler().compile(&[file("externs",CONSOLE_EXTERNS)],&[file("foo","/** @license Your favorite license goes here */\n/** \n  * @fileoverview This is my favorite file! */\nvar x;\n")],CompilerOptions::new()).success);
}
// port: CompilerTest#testWeakExternsFileAsEntryPointNoError
#[test]
fn test_weak_externs_file_as_entry_point_no_error() {
    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        ModuleIdentifier::for_file("/externs.js"),
    ]));
    let mut c = compiler();
    let result = c.compile(
        &[],
        &[weak_input(
            "/externs.js",
            "/** @fileoverview @externs */ /** @const {number} */ var bar = 1;",
            SourceKind::WEAK,
        )],
        options,
    );
    assert!(result.errors.is_empty());
    assert!(c.to_source().is_empty());
}

// port: CompilerTest#testImportantCommentInTree
#[test]
fn test_important_comment_in_tree() {
    test_comment_output(
        "var a = function() {
 +
/*! Your favorite license goes here */
 1;};
console.log(0);
",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testMultipleUniqueImportantComments
#[test]
fn test_multiple_unique_important_comments() {
    let mut c = compiler();
    let result = c.compile(
        &[file("externs", CONSOLE_EXTERNS)],
        &[
            file("testcode1", "/*! One license here */\nconsole.log(0);\n"),
            file(
                "testcode2",
                "/*! Another license here */\nconsole.log(1);\n",
            ),
        ],
        advanced(),
    );
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(
        c.to_source(),
        "/*\n One license here */\nconsole.log(0);/*\n Another license here */\nconsole.log(1);"
    );
}

// port: CompilerTest#testMultipleIdenticalImportantComments
#[test]
fn test_multiple_identical_important_comments() {
    let mut c = compiler();
    let result = c.compile(
        &[file("externs", CONSOLE_EXTERNS)],
        &[
            file(
                "testcode1",
                "/*! Identical license here */\nconsole.log(0);\n",
            ),
            file(
                "testcode2",
                "/*! Identical license here */\nconsole.log(1);\n",
            ),
        ],
        advanced(),
    );
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(
        c.to_source(),
        "/*\n Identical license here */\nconsole.log(0);console.log(1);"
    );
}

// port: CompilerTest#testLicenseInTree
#[test]
fn test_license_in_tree() {
    // Do we correctly handle the license if it's not at the top level, but
    // inside another declaration?
    test_comment_output(
        "var a = function() {
/** @license Your favorite license goes here */
 console.log(0);};a();
",
        "/*\n Your favorite license goes here */\nconsole.log(0);",
    );
}

// port: CompilerTest#testMultipleUniqueLicenses
#[test]
fn test_multiple_unique_licenses() {
    let mut c = compiler();
    let result = c.compile(
        &[file("externs", CONSOLE_EXTERNS)],
        &[
            file(
                "testcode1",
                "/** @license One license here */\nconsole.log(0);\n",
            ),
            file(
                "testcode2",
                "/** @license Another license here */\nconsole.log(1);\n",
            ),
        ],
        advanced(),
    );
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(
        c.to_source(),
        "/*\n One license here */\nconsole.log(0);/*\n Another license here */\nconsole.log(1);"
    );
}

// port: CompilerTest#testMultipleIdenticalLicenses
#[test]
fn test_multiple_identical_licenses() {
    let mut c = compiler();
    let result=c.compile(&[file("externs",CONSOLE_EXTERNS)],&[file("testcode1","/** @license Identical license here */\nconsole.log(0);\n"),file("testcode2","/** @license Identical license here */\nconsole.log(1);\n"),file("bundled","/** @license Identical license here */\nconsole.log(2);\n/** @license Identical license here */\n")],advanced());
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(
        c.to_source(),
        "/*\n Identical license here */\nconsole.log(0);console.log(1);console.log(2);"
    );
}

// port: CompilerTest#testIdenticalLicenseAndImportantComment
#[test]
fn test_identical_license_and_important_comment() {
    let mut c = compiler();
    let result = c.compile(
        &[file("externs", CONSOLE_EXTERNS)],
        &[
            file(
                "testcode1",
                "/** @license Identical license here */\nconsole.log(0);\n",
            ),
            file(
                "testcode2",
                "/*! Identical license here */\nconsole.log(1);\n",
            ),
        ],
        advanced(),
    );
    assert!(result.success, "{:?}", result.errors);
    assert_eq!(
        c.to_source(),
        "/*\n Identical license here */\nconsole.log(0);console.log(1);"
    );
}

use closure_jscomp::warnings_guard::WarningsGuard;

const TEST_ERROR: closure_jscomp::diagnostic_type::DiagnosticType =
    closure_jscomp::diagnostic_type::DiagnosticType::error("TEST_ERROR", "Test error");

fn extern_entry_point(source: &str, entries: &[&str], expected: &str) {
    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(
        entries.iter().map(|name| ModuleIdentifier::for_file(name)),
    ));
    let mut c = compiler();
    let result = c.compile(
        &[file("default_externs.js", CONSOLE_EXTERNS)],
        &[
            file(
                "/externs.js",
                "/** @fileoverview @externs */ /** @const {number} */ var bar = 1;",
            ),
            file("/foo.js", source),
        ],
        options,
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(c.to_source(), expected);
}
// port: CompilerTest#testExternsFileAsEntryPoint
#[test]
fn test_externs_file_as_entry_point() {
    extern_entry_point("console.log(0);", &["/externs.js"], "");
}
// port: CompilerTest#testExternsFileAsEntryPoint2
#[test]
fn test_externs_file_as_entry_point2() {
    extern_entry_point("console.log(nonexistentExtern);", &["/externs.js"], "");
}
// https://github.com/google/closure-compiler/issues/2692
// port: CompilerTest#testExternsFileAsEntryPoint3
#[test]
fn test_externs_file_as_entry_point3() {
    // Test code reference to an extern that doesn't exist,
    // but the extern and source files are both entry points
    let inputs = [
        file(
            "/externs.js",
            "/** @fileoverview @externs */ /** @const {number} */ var bar = 1;",
        ),
        file("/foo.js", "console.log(nonexistentExtern);"),
    ];

    let entry_points = [
        ModuleIdentifier::for_file("/externs.js"),
        ModuleIdentifier::for_file("/foo.js"),
    ];

    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(entry_points));

    let externs = [file("default_externs.js", CONSOLE_EXTERNS)];

    let mut c = compiler();
    c.compile(&externs, &inputs, options);

    let result = c.get_result();
    assert_eq!(result.errors.len(), 1);
    assert!(std::ptr::eq(
        result.errors[0].get_type(),
        &closure_jscomp::var_check::UNDEFINED_VAR_ERROR
    ));
}
// port: CompilerTest#testExternsFileAsEntryPoint4
#[test]
fn test_externs_file_as_entry_point4() {
    extern_entry_point(
        "console.log(bar);",
        &["/externs.js", "/foo.js"],
        "console.log(bar);",
    );
}
// port: CompilerTest#testExternsFileAsEntryPoint5
#[test]
fn test_externs_file_as_entry_point5() {
    extern_entry_point("console.log(bar);", &["/foo.js"], "console.log(bar);");
}

// port: CompilerTest#testExplicitWeakEntryPointIsError
#[test]
fn test_explicit_weak_entry_point_is_error() {
    let weak_entry = Arc::new(SourceFile::from_code_with_kind(
        "weakEntry.js",
        "goog.module('weakEntry');\n/** @typedef {number|string} */ exports.T;\nsideEffect();\n",
        SourceKind::WEAK,
    ));
    let mut options = CompilerOptions::new();
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_closure("weakEntry"),
    ]));
    let mut c = compiler();
    c.init(&[file("extern.js", "")], &[weak_entry], options);
    c.parse();
    assert!(c.get_warnings().is_empty());
    assert_eq!(c.get_errors().len(), 1);
    assert_eq!(
        c.get_errors()[0].description,
        "Explicit entry point input must not be weak: weakEntry.js"
    );
}

// port: CompilerTest#testImplicitWeakEntryPointIsWarning
#[test]
fn test_implicit_weak_entry_point_is_warning() {
    let weak_moocher = Arc::new(SourceFile::from_code_with_kind(
        "weakMoocher.js",
        "const {T} = goog.require('weakByAssociation');\n/** @param {!T} x */ function f(x) { alert(x); }\n",
        SourceKind::WEAK,
    ));
    let weak_by_association = file(
        "weakByAssociation.js",
        "goog.module('weakByAssociation');\n/** @typedef {number|string} */ exports.T;\nsideeffect();\n",
    );
    let mut options = CompilerOptions::new();
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(Vec::new()));
    let mut c = compiler();
    c.init(
        &[file("extern.js", "/** @externs */ function alert(x) {}")],
        &[weak_moocher, weak_by_association],
        options,
    );
    c.parse();
    assert!(c.get_errors().is_empty());
    assert_eq!(c.get_warnings().len(), 1);
    assert_eq!(
        c.get_warnings()[0].description,
        "Implicit entry point input should not be weak: weakMoocher.js"
    );
}

// port: CompilerTest#testWeakStronglyReachableIsError
#[test]
fn test_weak_strongly_reachable_is_error() {
    let strong = Arc::new(SourceFile::from_code_with_kind(
        "strong.js",
        "goog.module('strong');\nconst T = goog.require('weak');\n/** @param {!T} x */ function f(x) { alert(x); }\n",
        SourceKind::STRONG,
    ));
    let weak = Arc::new(SourceFile::from_code_with_kind(
        "weak.js",
        "goog.module('weak');\n/** @typedef {number|string} */ exports.T;\nsideEffect();\n",
        SourceKind::WEAK,
    ));
    let mut options = CompilerOptions::new();
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_closure("strong"),
    ]));
    let mut c = compiler();
    c.init(
        &[file("extern.js", "/** @externs */ function alert(x) {}")],
        &[strong, weak],
        options,
    );
    c.parse();
    assert!(c.get_warnings().is_empty());
    assert_eq!(c.get_errors().len(), 1);
    assert_eq!(
        c.get_errors()[0].description,
        "File strongly reachable from an entry point must not be weak: weak.js"
    );
}

// port: CompilerTest#testExternsDependencyPruning
#[test]
fn test_externs_dependency_pruning() {
    let inputs = [
        file(
            "unused",
            "/** @fileoverview @typeSummary */ goog.provide('unused');",
        ),
        file(
            "moocher",
            "/** @fileoverview @typeSummary */ goog.require('something');",
        ),
        file(
            "something",
            "/** @fileoverview @typeSummary */ goog.provide('something');",
        ),
    ];

    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(Vec::new()));

    let externs: [Arc<SourceFile>; 0] = [];
    let mut c = compiler();
    c.compile(&externs, &inputs, options);

    let externs_root = c.get_externs_root().unwrap();
    assert_eq!(externs_root.get_child_count(&c), 3);
    assert_extern_index(&mut c, 0, " [synthetic:externs] "); // added by VarCheck
    assert_extern_index(&mut c, 1, "something");
    assert_extern_index(&mut c, 2, "moocher");
}

// port: CompilerTest#assertExternIndex
fn assert_extern_index(compiler: &mut Compiler, index: i32, name: &str) {
    let child = compiler
        .get_externs_root()
        .unwrap()
        .get_child_at_index(compiler, index);
    let input = compiler.get_input(&InputId::new(name)).unwrap().clone();
    assert_eq!(child, Some(input.get_ast_root(compiler)));
}

// port: CompilerTest#testEs6ModulePathWithOddCharacters
#[test]
fn test_es6_module_path_with_odd_characters() {
    let mut options = advanced();
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_file("/index[0].js"),
    ]));
    let mut c = compiler();
    c.compile(
        &[file(
            "default_externs.js",
            include_str!("data/compiler_alert_externs.js"),
        )],
        &[
            file("/index[0].js", "import foo from './foo.js'; foo('hello');"),
            file("/foo.js", "export default (foo) => { alert(foo); }"),
        ],
        options,
    );
    assert!(c.get_result().errors.is_empty());
}

// Regression for Compiler#parseSyntheticCode: Java adds the synthetic source to
// the existing map; it does not call initBasedOnOptions and discard that map.
#[test]
fn synthetic_source_preserves_map_and_adds_source_content() {
    let mut c = compiler();
    let mut options = CompilerOptions::new();
    options.set_source_map_output_path("out.map".into());
    options.set_source_map_include_sources_content(true);
    c.init(&[], &[file("input.js", "var input = 1;")], options);
    c.parse_inputs();
    let original_map = c.get_source_map().unwrap();
    let root = c.parse_synthetic_code("runtime", "var runtime = 2;");
    assert_eq!(
        root.get_source_file_name(&c).as_deref(),
        Some(" [synthetic:runtime] ")
    );
    let map = c.get_source_map().unwrap();
    assert!(Arc::ptr_eq(&original_map, &map));
    c.get_js_root().unwrap().add_child_to_back(&mut c, root);
    c.to_source();
    let mut output = String::new();
    map.lock()
        .unwrap()
        .append_to(&mut output, "out.js")
        .unwrap();
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(output.as_str()).unwrap();
    assert_eq!(
        consumer.get_original_sources_content(),
        Some(
            &[
                Some("var input = 1;".into()),
                Some("var runtime = 2;".into())
            ][..]
        )
    );
}

// Compiler's ErrorReporter must accept the parser's WTF-16 diagnostic overload.
#[test]
fn supplementary_identifier_parse_error_reaches_diagnostics() {
    let mut c = compiler();
    c.parse_test_code("var \u{10c00};");
    let errors = c.get_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].get_type().key, "JSC_PARSE_ERROR");
    assert_eq!(
        errors[0].description(),
        "Parse error. Character '?' (U+D803) is not a valid identifier start char"
    );
    assert_eq!(errors[0].get_line_number(), 1);
    assert_eq!(errors[0].get_charno(), 4);
}
// port: CompilerTest#restoreCompilerState
fn restore_compiler_state(compiler: &mut Compiler, state_after_checks: &[u8]) {
    let mut byte_array_input_stream = std::io::Cursor::new(state_after_checks.to_vec());
    if let Err(e) = compiler.restore_state(&mut byte_array_input_stream) {
        panic!("restoring compiler state failed: {e}");
    }
}
// port: CompilerTest#getSavedCompilerState
fn get_saved_compiler_state(compiler: &mut Compiler) -> Vec<u8> {
    let mut output_stream: Vec<u8> = Vec::new();
    compiler.save_state(&mut output_stream).unwrap();
    output_stream
}
/// Simple error manager that tracks whether anything was reported/output.
// port: CompilerTest.TestErrorManager
#[derive(Default)]
struct TestErrorManager {
    output: bool,
    warning_count: i32,
    error_count: i32,
}
impl closure_jscomp::error_handler::ErrorHandler for TestErrorManager {
    // port: CompilerTest.TestErrorManager#report
    fn report(&mut self, level: CheckLevel, _error: closure_jscomp::js_error::JSError) {
        self.output = true;
        if level == CheckLevel::WARNING {
            self.warning_count += 1;
        }
        if level == CheckLevel::ERROR {
            self.error_count += 1;
        }
    }
}
impl closure_jscomp::error_manager::ErrorManager for TestErrorManager {
    // Methods we don't care about
    // port: CompilerTest.TestErrorManager#generateReport
    fn generate_report(&mut self, _ast: &closure_rhino::node::Ast) {}
    // port: CompilerTest.TestErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.error_count
    }
    // port: CompilerTest.TestErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.warning_count
    }
    // port: CompilerTest.TestErrorManager#getErrors
    fn get_errors(&self) -> Vec<closure_jscomp::js_error::JSError> {
        Vec::new()
    }
    // port: CompilerTest.TestErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<closure_jscomp::js_error::JSError> {
        Vec::new()
    }
    // port: CompilerTest.TestErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, _typed_percent: f64) {}
    // port: CompilerTest.TestErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        0.0
    }
}
fn test_error_manager_compiler() -> Compiler {
    Compiler::new_with_error_manager(Box::new(TestErrorManager::default()))
}
// port: CompilerTest#testStage2SplittingResultsInSameOutput
#[test]
fn test_stage2_splitting_results_in_same_output() {
    let mut compiler = test_error_manager_compiler();
    let mut options = CompilerOptions::new();

    options.set_emit_use_strict(false);

    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs = vec![file(
        "externs.js",
        "var console = {};\n console.log = function() {};\n",
    )];
    let srcs = vec![file(
        "input.js",
        "goog.module('foo');\nconst hello = 'hello';\nfunction f() { return hello; }\nconsole.log(f());\n",
    )];
    compiler.init(&externs, &srcs, options.clone());

    // This is what the output should look like after all optimizations.
    let final_output_after_optimizations = "console.log(\"hello\");";

    // Stage 1
    compiler.parse();
    compiler.check();
    let state_after_checks = get_saved_compiler_state(&mut compiler);

    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());
    restore_compiler_state(&mut compiler, &state_after_checks);

    // Stage 2, all passes
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    let mut source = compiler.to_source().to_string();
    assert_eq!(source, final_output_after_optimizations); // test output stage 2 code

    // Now reset the compiler and test splitting stage 2 into two halves. We want to test that the
    // output is the same as when we run all of stage 2 in one go.
    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());

    // Stage 1
    compiler.parse();
    compiler.check();
    let state_after_checks2 = get_saved_compiler_state(&mut compiler);

    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());
    restore_compiler_state(&mut compiler, &state_after_checks2);

    // Stage 2, first half
    compiler.perform_transpilation_and_optimizations(
        SegmentOfCompilationToRun::OPTIMIZATIONS_FIRST_HALF,
    );
    source = compiler.to_source().to_string();
    assert_eq!(source, "console.log(function(){return\"hello\"}());");

    let state_after_first_half_optimizations = get_saved_compiler_state(&mut compiler);
    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());
    restore_compiler_state(&mut compiler, &state_after_first_half_optimizations);

    // Stage 2, second half
    compiler.perform_transpilation_and_optimizations(
        SegmentOfCompilationToRun::OPTIMIZATIONS_SECOND_HALF,
    );
    source = compiler.to_source().to_string();
    assert_eq!(source, final_output_after_optimizations); // output is the same as before
}
// port: CompilerTest#testCheckSaveRestore3Stages
#[test]
fn test_check_save_restore3_stages() {
    let mut compiler = test_error_manager_compiler();

    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
    options.set_language_out(LanguageMode::ECMASCRIPT_NEXT);
    options.set_check_types(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    options.set_preserve_detailed_source_info(true);
    // Late localization happens in stage 3, so this forces stage 3 to actually have some
    // effect on the AST.
    options.set_do_late_localization(true);
    // Supply a message bundle to trigger the replaceStrings pass.
    // We don't need to actually translate anything, though.
    options.set_message_bundle(Arc::new(
        closure_jscomp::empty_message_bundle::EmptyMessageBundle,
    ));
    // Enable the ReplaceStrings pass, so we can confirm that the stringMap it creates survives
    // serialization and deserialization.
    options.set_replace_strings_function_descriptions(vec!["Error(*)".to_string()]);

    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs = vec![file(
        "externs.js",
        "var console = {};\n console.log = function() {};\n",
    )];
    let srcs = vec![file(
        "input.js",
        "/** @desc greeting */\nconst MSG_HELLO = goog.getMsg('hello');\nfunction f() { return MSG_HELLO; }\n// Use `Error()` in order to make sure we generate a non-empty\n// compiler.stringMap, so we can confirm it is saved and restored.\nconsole.log(Error('string to replace'), f());\n",
    )];
    compiler.init(&externs, &srcs, options.clone());

    compiler.parse();
    compiler.check();

    let state_after_checks = get_saved_compiler_state(&mut compiler);

    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());
    restore_compiler_state(&mut compiler, &state_after_checks);

    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    let mut source = compiler.to_source().to_string();
    assert_eq!(
        source,
        [
            "console.log(",
            "Error(\"a\"),", // replaceStrings obfuscated this
            "__jscomp_define_msg__(",
            "{\"key\":\"MSG_HELLO\",\"msg_text\":\"hello\"}",
            "));",
        ]
        .concat()
    );

    let state_after_optimizations = get_saved_compiler_state(&mut compiler);

    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options);
    restore_compiler_state(&mut compiler, &state_after_optimizations);

    compiler.perform_finalizations();

    let result = compiler.get_result();
    // confirm that the string map was built with a mapping from an obfuscated string to
    // the string used in the Error() call above.
    let string_map = result.string_map.as_ref().unwrap().to_map();
    assert_eq!(
        string_map
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        vec![("a".to_string(), "string to replace".to_string())]
    );

    source = compiler.to_source().to_string();
    assert_eq!(source, "console.log(Error(\"a\"),\"hello\");");
}
// port: CompilerTest#testCheckSaveRestore3StagesSourceMaps
#[test]
fn test_check_save_restore3_stages_source_maps() {
    let mut compiler = test_error_manager_compiler();

    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_language_in(LanguageMode::ECMASCRIPT_NEXT);
    options.set_language_out(LanguageMode::ECMASCRIPT_NEXT);
    options.set_check_types(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    options.set_preserve_detailed_source_info(true);
    // 3-stage builds require late localization
    options.set_do_late_localization(true);
    // For stages 1 and 2 we generally expect no source map output path to be set,
    // since it won't actually be generated until compilation is completed in
    // stage 3.
    // (Java `setSourceMapOutputPath(null)`: the Rust setter takes a String, and the path of a fresh
    // CompilerOptions is already null.)
    // The code that is running the compiler is expected to set this option
    // when executing a partial compilation and it expects to request source
    // maps when running the final stage later.
    options.set_always_gather_source_map_info(true);
    options.set_apply_input_source_maps(true);
    options.set_source_map_include_sources_content(true);
    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs = vec![file(
        "externs.js",
        "var console = {};\n console.log = function() {};\n",
    )];
    let srcs = vec![file(
        "input.js",
        &format!(
            "{SOURCE_MAP_TEST_CODE}\n//# sourceMappingURL={BASE64_ENCODED_SOURCE_MAP_WITH_CONTENT}"
        ),
    )];

    // Stage 1
    compiler.init(&externs, &srcs, options.clone());
    compiler.parse();
    compiler.check();
    let state_after_checks = get_saved_compiler_state(&mut compiler);

    // Stage 2
    compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options.clone());
    restore_compiler_state(&mut compiler, &state_after_checks);
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);

    let mut source = compiler.to_source().to_string();
    assert_eq!(source, "console.log(new function(){}(1));");

    let state_after_optimizations = get_saved_compiler_state(&mut compiler);

    // Stage 3
    compiler = test_error_manager_compiler();
    // In general the options passed to the compiler should be the same for all
    // 3 stages. The source map output path is an exception.
    // It only makes sense to specify it for the final stage when the output
    // file will actually be generated.
    // The name passed here doesn't matter, because the compiler itself only stores it and enables
    // tracking of source map information when it is non-null.
    // In real usage AbstractCommandLineRunner is responsible for actually writing the file whose
    // path is stored in this field.
    options.set_source_map_output_path("dummy".into());
    // The code that is running the compiler is expected to set this option
    // to false when executing the final stage, so no time and space will
    // be wasted on generating source maps if the source map output path is null.
    options.set_always_gather_source_map_info(false);
    compiler.init(&externs, &srcs, options);
    restore_compiler_state(&mut compiler, &state_after_optimizations);
    compiler.perform_finalizations();
    let result = compiler.get_result();

    source = compiler.to_source().to_string();
    assert_eq!(source, "console.log(new function(){}(1));");

    let source_map = result.source_map.clone();
    assert!(source_map.is_some());

    // Check sourcemap output
    let mut source_map_string_builder = String::new();
    let output_js_file = "output.js";
    source_map
        .unwrap()
        .lock()
        .unwrap()
        .append_to(&mut source_map_string_builder, output_js_file)
        .unwrap();
    assert_eq!(source_map_string_builder, RESULT_SOURCE_MAP_WITH_CONTENT);
}
// port: CompilerTest#testCheckSaveRestore3StagesNoInputFiles
#[test]
fn test_check_save_restore3_stages_no_input_files() {
    // There's an edge case where a chunk may be empty.
    // The compiler covers this weird case by adding a special "fillFile" into empty chunks.
    // This makes the logic in passes like CrossChunkCodeMotion easier.
    // However, we also need to drop the phony "fillFiles" in several cases.
    // One of those cases is serialization.
    // This can lead to an odd situation where deserialization doesn't see a SourceFile
    // for one of these "fillFiles".
    // This led to a NullPointerException in the past.
    // This test case exists to test the fix for that.
    let mut compiler = test_error_manager_compiler();

    let mut options = CompilerOptions::new();
    // Late localization happens in stage 3, so this forces stage 3 to actually have some
    // effect on the AST.
    options.set_do_late_localization(true);

    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs: Vec<Arc<SourceFile>> = vec![];
    let srcs: Vec<Arc<SourceFile>> = vec![];
    compiler.init(&externs, &srcs, options.clone());

    compiler.parse();
    compiler.check();
    let all_inputs = compiler.get_chunk_graph().unwrap().get_all_inputs();
    assert_eq!(all_inputs.len(), 1);
    let only_input_before_save = &all_inputs[0];
    // this is the special name used for a single fill file when there are no inputs
    assert_eq!(only_input_before_save.get_name(), "$strong$$fillFile");

    let state_after_checks = get_saved_compiler_state(&mut compiler);

    let mut compiler = test_error_manager_compiler();
    compiler.init(&externs, &srcs, options);
    restore_compiler_state(&mut compiler, &state_after_checks);

    // The fillFile is still listed as the only input
    let all_inputs = compiler.get_chunk_graph().unwrap().get_all_inputs();
    assert_eq!(all_inputs.len(), 1);
    let only_input_after_restore = &all_inputs[0];
    assert_eq!(only_input_after_restore.get_name(), "$strong$$fillFile");
}
// port: CompilerTest#testCustomStateCompressionWrapper
#[test]
fn test_custom_state_compression_wrapper() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut compiler = test_error_manager_compiler();
    let mut options = CompilerOptions::new();

    let compression_used = Arc::new(AtomicBool::new(false));
    let decompression_used = Arc::new(AtomicBool::new(false));

    let used = Arc::clone(&compression_used);
    options.set_state_compression_wrapper(Some(Arc::new(move |out| {
        used.store(true, Ordering::SeqCst);
        out
    })));
    let used = Arc::clone(&decompression_used);
    options.set_state_decompression_wrapper(Some(Arc::new(move |input| {
        used.store(true, Ordering::SeqCst);
        input
    })));

    let inputs = vec![file("test.js", "var x = 1;")];
    compiler.init(&[], &inputs, options);

    compiler.parse();

    // Save state
    let mut baos: Vec<u8> = Vec::new();
    compiler.save_state(&mut baos).unwrap();

    assert!(compression_used.load(Ordering::SeqCst));

    // Restore state
    let mut compiler2 = test_error_manager_compiler();
    let mut options2 = CompilerOptions::new();
    let used = Arc::clone(&decompression_used);
    options2.set_state_decompression_wrapper(Some(Arc::new(move |input| {
        used.store(true, Ordering::SeqCst);
        input
    })));
    compiler2.init(&[], &inputs, options2);

    let mut bais = std::io::Cursor::new(baos);
    compiler2.restore_state(&mut bais).unwrap();

    assert!(decompression_used.load(Ordering::SeqCst));
}
// port: CompilerTest#restoreState_doesNotCreateColorRegistryIfTypecheckingSkipped
#[test]
fn restore_state_does_not_create_color_registry_if_typechecking_skipped() {
    let mut options = CompilerOptions::new();
    options.set_check_types(false);
    let mut compiler = Compiler::new();

    let inputs = vec![file("in1", "")];
    compiler.init(&[], &inputs, options.clone());

    compiler.parse();
    compiler.check();

    assert!(!compiler.has_type_checking_run());

    let mut byte_array_output_stream: Vec<u8> = Vec::new();
    compiler.save_state(&mut byte_array_output_stream).unwrap();

    let mut compiler = Compiler::new();
    compiler.init(&[], &inputs, options);
    restore_compiler_state(&mut compiler, &byte_array_output_stream);

    assert!(!compiler.has_type_checking_run());
    assert!(!compiler.has_optimization_colors());
}
// port: CompilerTest#librariesInjectedInStage1_notReinjectedInStage2
#[test]
fn libraries_injected_in_stage1_not_reinjected_in_stage2() {
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);

    let mut compiler = Compiler::new();

    let inputs = vec![file("in1", "")];
    compiler.init(&[], &inputs, options.clone());

    compiler.parse();
    compiler.check();
    compiler
        .get_runtime_js_lib_manager()
        .lock()
        .unwrap()
        .ensure_library_injected(&mut compiler, "base", /* force= */ true);

    let mut byte_array_output_stream: Vec<u8> = Vec::new();
    compiler.save_state(&mut byte_array_output_stream).unwrap();

    let mut compiler = Compiler::new();
    compiler.init(&[], &inputs, options);
    restore_compiler_state(&mut compiler, &byte_array_output_stream);

    let js_root = compiler.get_js_root().unwrap();
    let old_ast = js_root.clone_tree(&mut compiler.ast);

    // should not change the AST as 'base' was already injected.
    compiler
        .get_runtime_js_lib_manager()
        .lock()
        .unwrap()
        .ensure_library_injected(&mut compiler, "base", /* force= */ true);

    let js_root = compiler.get_js_root().unwrap();
    assert!(
        js_root.is_equivalent_to(&compiler.ast, old_ast),
        "expected:\n{}\nactual:\n{}",
        old_ast.to_string_tree(&compiler.ast),
        js_root.to_string_tree(&compiler.ast)
    );
}
// port: CompilerTest#injectLibrariesBeforeAndAfterStage1
#[test]
fn inject_libraries_before_and_after_stage1() {
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);

    let mut compiler = Compiler::new();

    let inputs = vec![file("in1", "")];
    compiler.init(&[], &inputs, options.clone());

    compiler.parse();
    compiler.check();
    compiler
        .get_runtime_js_lib_manager()
        .lock()
        .unwrap()
        .ensure_library_injected(&mut compiler, "base", /* force= */ true);

    let mut byte_array_output_stream: Vec<u8> = Vec::new();
    compiler.save_state(&mut byte_array_output_stream).unwrap();

    let mut compiler = Compiler::new();
    compiler.init(&[], &inputs, options);

    restore_compiler_state(&mut compiler, &byte_array_output_stream);

    let js_root = compiler.get_js_root().unwrap();
    let old_ast = js_root.clone_tree(&mut compiler.ast);

    compiler
        .get_runtime_js_lib_manager()
        .lock()
        .unwrap()
        .ensure_library_injected(&mut compiler, "es6/set", /* force= */ true);

    let js_root = compiler.get_js_root().unwrap();
    assert!(!js_root.is_equivalent_to(&compiler.ast, old_ast));

    let source = compiler.to_source().to_string();
    let jscomp_definition = source.find("var $jscomp").map_or(-1, |i| i as i64);
    let jscomp_polyfill_definition = source.find("$jscomp.polyfill").map_or(-1, |i| i as i64);

    // The definition of $jscomp.polyfill should be injected after the definition of 'var $jscomp',
    // not before.
    assert!(jscomp_definition < jscomp_polyfill_definition);
}
mod typed_ast_filesystem {
    use super::*;
    use closure_jscomp::{
        js_chunk::STRONG_CHUNK_NAME,
        serialization::{
            protobuf::Message,
            source_file_proto::SourceFilePool,
            string_pool::StringPool,
            typed_ast_proto::{AstNode, LazyAst, NodeKind, TypedAst, TypedAstList},
        },
    };
    use closure_rhino::{node::NodeId, token::Token};

    fn script(children: Vec<AstNode>) -> Vec<u8> {
        let mut node = AstNode::new_builder().set_kind(NodeKind::SOURCE_FILE);
        for child in children {
            node = node.add_child(child);
        }
        node.build().to_byte_array()
    }
    fn lazy_ast(source_file: i32, script: Vec<u8>) -> LazyAst {
        LazyAst::new_builder()
            .set_source_file(source_file)
            .set_script(script)
            .build()
    }
    fn number_statement(value: f64) -> AstNode {
        AstNode::new_builder()
            .set_kind(NodeKind::EXPRESSION_STATEMENT)
            .add_child(
                AstNode::new_builder()
                    .set_kind(NodeKind::NUMBER_LITERAL)
                    .set_double_value(value)
                    .build(),
            )
            .build()
    }
    fn same_file(compiler: &Compiler, script: NodeId, file: &Arc<SourceFile>) -> bool {
        let source = script.get_static_source_file(compiler).unwrap();
        std::ptr::addr_eq(Arc::as_ptr(&source), Arc::as_ptr(file))
    }

    // port: CompilerTest#testTypedAstFilesystem_extraInputFilesAvailable
    #[test]
    fn test_typed_ast_filesystem_extra_input_files_available() {
        // Given
        let file1 = file("test1.js", "");
        let file2 = file("test2.js", "");

        let typed_ast_list = TypedAstList::new_builder()
            .add_typed_asts(
                TypedAst::new_builder()
                    .set_string_pool(StringPool::empty().to_proto())
                    .set_source_file_pool(
                        SourceFilePool::new_builder()
                            .add_source_file(file1.get_proto())
                            .add_source_file(file2.get_proto())
                            .build(),
                    )
                    .add_code_ast(lazy_ast(1, script(vec![])))
                    .add_code_ast(lazy_ast(2, script(vec![])))
                    .build(),
            )
            .build();
        let bytes = typed_ast_list.to_byte_array();
        let mut typed_ast_list_stream = bytes.as_slice();

        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());

        // When
        compiler.init_with_typed_ast_filesystem(
            &[],
            std::slice::from_ref(&file1),
            compiler_options,
            &mut typed_ast_list_stream,
        );

        let message = panic_message(|| {
            compiler.get_typed_ast_deserializer(&file2);
        });
        assert!(message.contains("missing requested file"), "{message}");

        let script = compiler
            .get_root()
            .unwrap()
            .get_second_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap();
        assert!(same_file(&compiler, script, &file1));
    }

    // port: CompilerTest#testTypedAstFilesystem_someInputFilesUnavailable_crashes
    #[test]
    fn test_typed_ast_filesystem_some_input_files_unavailable_crashes() {
        // Given
        let file = file("test.js", "");
        let mut typed_ast_list_stream: &[u8] = &[];
        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());

        let e = panic_message(|| {
            compiler.init_with_typed_ast_filesystem(
                &[],
                std::slice::from_ref(&file),
                compiler_options,
                &mut typed_ast_list_stream,
            );
        });
        let pattern = regex_lite_contains_missing(&e, "test.js");
        assert!(pattern, "{e}");
    }
    /// `containsMatch("missing .* test.js")`
    fn regex_lite_contains_missing(message: &str, name: &str) -> bool {
        message.find("missing ").is_some_and(|start| {
            message[start + "missing ".len()..]
                .find(&format!(" {name}"))
                .is_some()
        })
    }

    // port: CompilerTest#testTypedAstFilesystem_someAvailableFilesDuplicated_usesFirstCopy
    #[test]
    fn test_typed_ast_filesystem_some_available_files_duplicated_uses_first_copy() {
        // Given
        let file = file("test.js", "");

        let typed_ast0 = TypedAst::new_builder()
            .set_string_pool(StringPool::empty().to_proto())
            .set_source_file_pool(
                SourceFilePool::new_builder()
                    .add_source_file(file.get_proto())
                    .build(),
            )
            .add_code_ast(lazy_ast(1, script(vec![])))
            .build();
        let typed_ast1 = TypedAst::new_builder()
            .set_string_pool(StringPool::empty().to_proto())
            .set_source_file_pool(
                SourceFilePool::new_builder()
                    .add_source_file(file.get_proto())
                    .build(),
            )
            .add_code_ast(lazy_ast(
                1,
                script(vec![
                    AstNode::new_builder()
                        .set_kind(NodeKind::VAR_DECLARATION)
                        .build(),
                ]),
            ))
            .build();
        let bytes = TypedAstList::new_builder()
            .add_typed_asts(typed_ast0)
            .add_typed_asts(typed_ast1)
            .build()
            .to_byte_array();
        let mut typed_ast_list_stream = bytes.as_slice();

        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());

        // When
        compiler.init_with_typed_ast_filesystem(
            &[],
            std::slice::from_ref(&file),
            compiler_options,
            &mut typed_ast_list_stream,
        );

        // Then
        let script = compiler
            .get_root()
            .unwrap()
            .get_second_child(&compiler)
            .unwrap()
            .get_first_child(&compiler)
            .unwrap();
        assert!(same_file(&compiler, script, &file));
        assert!(!script.has_children(&compiler));
    }

    // port: CompilerTest#testTypedAstFilesystem_syntheticExternsFile_isCattedAcrossTypedAsts
    #[test]
    fn test_typed_ast_filesystem_synthetic_externs_file_is_catted_across_typed_asts() {
        // Given
        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());
        let synthetic_file = compiler.synthetic_externs_file();
        let file_one = file("one.js", "");
        let file_two = file("two.js", "");

        let typed_ast0 = TypedAst::new_builder()
            .set_string_pool(StringPool::empty().to_proto())
            .set_source_file_pool(
                SourceFilePool::new_builder()
                    .add_source_file(synthetic_file.get_proto())
                    .add_source_file(file_one.get_proto())
                    .build(),
            )
            .add_code_ast(lazy_ast(
                1,
                script(vec![
                    AstNode::new_builder()
                        .set_kind(NodeKind::CONST_DECLARATION)
                        .build(),
                ]),
            ))
            .add_code_ast(lazy_ast(2, script(vec![])))
            .build();
        let typed_ast1 = TypedAst::new_builder()
            .set_string_pool(StringPool::empty().to_proto())
            .set_source_file_pool(
                SourceFilePool::new_builder()
                    .add_source_file(synthetic_file.get_proto())
                    .add_source_file(file_two.get_proto())
                    .build(),
            )
            .add_code_ast(lazy_ast(
                1,
                script(vec![
                    AstNode::new_builder()
                        .set_kind(NodeKind::VAR_DECLARATION)
                        .build(),
                ]),
            ))
            .add_code_ast(lazy_ast(2, script(vec![])))
            .build();
        let bytes = TypedAstList::new_builder()
            .add_typed_asts(typed_ast0)
            .add_typed_asts(typed_ast1)
            .build()
            .to_byte_array();
        let mut typed_ast_list_stream = bytes.as_slice();

        // When
        compiler.init_with_typed_ast_filesystem(
            &[],
            &[file_one, file_two],
            compiler_options,
            &mut typed_ast_list_stream,
        );

        // Then
        let inserted_externs = compiler
            .get_externs_root()
            .unwrap()
            .get_only_child(&compiler);
        let synthesized_externs_input = compiler.get_synthesized_externs_input().clone();
        let lazy_externs = synthesized_externs_input.get_ast_root(&mut compiler);

        assert_eq!(inserted_externs, lazy_externs);
        assert!(same_file(
            &compiler,
            lazy_externs,
            &compiler.synthetic_externs_file()
        ));
        assert!(
            lazy_externs
                .get_first_child(&compiler)
                .unwrap()
                .is_const(&compiler)
        );
        assert!(
            lazy_externs
                .get_second_child(&compiler)
                .unwrap()
                .is_var(&compiler)
        );
    }

    fn weak_and_strong_typed_ast_list(
        weak_file: &Arc<SourceFile>,
        strong_file: &Arc<SourceFile>,
    ) -> Vec<u8> {
        let typed_ast = TypedAst::new_builder()
            .set_string_pool(StringPool::empty().to_proto())
            .set_source_file_pool(
                SourceFilePool::new_builder()
                    .add_source_file(weak_file.get_proto())
                    .add_source_file(strong_file.get_proto())
                    .build(),
            )
            .add_code_ast(lazy_ast(1, script(vec![number_statement(0.0)])))
            .add_code_ast(lazy_ast(2, script(vec![number_statement(1.0)])))
            .build();
        TypedAstList::new_builder()
            .add_typed_asts(typed_ast)
            .build()
            .to_byte_array()
    }
    fn assert_weak_and_strong_scripts(
        compiler: &Compiler,
        weak_script: NodeId,
        strong_script: NodeId,
    ) {
        assert!(!weak_script.has_children(compiler));
        assert!(strong_script.has_one_child(compiler));
        assert_eq!(
            strong_script
                .get_first_child(compiler)
                .unwrap()
                .get_token(compiler),
            Token::EXPR_RESULT
        );
        let number = strong_script.get_first_first_child(compiler).unwrap();
        assert!(number.is_number(compiler));
        assert_eq!(number.get_double(compiler), 1.0);
    }

    // port: CompilerTest#testTypedAstFilesystem_doesNotParseWeakFileTypedAstContents
    #[test]
    fn test_typed_ast_filesystem_does_not_parse_weak_file_typed_ast_contents() {
        // Given
        let weak_file = Arc::new(SourceFile::from_code_with_kind(
            "weak.js",
            "0",
            SourceKind::WEAK,
        ));
        let strong_file = file("strong.js", "1");
        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());

        let bytes = weak_and_strong_typed_ast_list(&weak_file, &strong_file);
        let mut typed_ast_list_stream = bytes.as_slice();

        // When
        compiler.init_with_typed_ast_filesystem(
            &[],
            &[weak_file, strong_file],
            compiler_options,
            &mut typed_ast_list_stream,
        );
        let weak_input = compiler
            .get_chunk_graph()
            .unwrap()
            .get_chunk_by_name(WEAK_CHUNK_NAME)
            .unwrap()
            .get_inputs()[0]
            .clone();
        let weak_script = weak_input.get_ast_root(&mut compiler);
        let strong_input = compiler
            .get_chunk_graph()
            .unwrap()
            .get_chunk_by_name(STRONG_CHUNK_NAME)
            .unwrap()
            .get_inputs()[0]
            .clone();
        let strong_script = strong_input.get_ast_root(&mut compiler);

        // Then
        assert_weak_and_strong_scripts(&compiler, weak_script, strong_script);
    }

    // port: CompilerTest#testTypedAstFilesystemWithChunks_doesNotParseWeakFileTypedAstContents
    #[test]
    fn test_typed_ast_filesystem_with_chunks_does_not_parse_weak_file_typed_ast_contents() {
        // Given
        let weak_file = Arc::new(SourceFile::from_code_with_kind(
            "weak.js",
            "0",
            SourceKind::WEAK,
        ));
        let strong_file = file("strong.js", "1");
        let mut compiler = Compiler::new();
        let compiler_options = CompilerOptions::new();
        compiler.init_options(compiler_options.clone());
        let weak_chunk = JSChunk::new(WEAK_CHUNK_NAME);
        let strong_chunk = JSChunk::new("a");
        weak_chunk.add_source_file(weak_file.clone());
        weak_chunk.add_dependency(&strong_chunk);
        strong_chunk.add_source_file(strong_file.clone());

        let bytes = weak_and_strong_typed_ast_list(&weak_file, &strong_file);
        let mut typed_ast_list_stream = bytes.as_slice();

        // When
        compiler.init_chunks_with_typed_ast_filesystem(
            &[],
            vec![strong_chunk.clone(), weak_chunk.clone()],
            compiler_options,
            &mut typed_ast_list_stream,
        );
        compiler.parse();
        let weak_input = weak_chunk.get_inputs()[0].clone();
        let weak_script = weak_input.get_ast_root(&mut compiler);
        let strong_input = strong_chunk.get_inputs()[0].clone();
        let strong_script = strong_input.get_ast_root(&mut compiler);

        // Then
        assert_weak_and_strong_scripts(&compiler, weak_script, strong_script);
    }
}
fn ordered_input_names(compiler: &Compiler) -> Vec<String> {
    let mut ordered_inputs = vec![];
    for input in compiler.get_inputs_in_order() {
        ordered_inputs.push(input.get_name().to_string());
    }
    ordered_inputs
}

// port: CompilerTest#testProperEs6ModuleOrdering
#[test]
fn test_proper_es6_module_ordering() {
    let sources = vec![
        file(
            "/entry.js",
            "import './b/b.js';\nimport './b/a.js';\nimport './important.js';\nimport './a/b.js';\nimport './a/a.js';\n",
        ),
        file("/a/a.js", "window['D'] = true;"),
        file("/a/b.js", "window['C'] = true;"),
        file("/b/a.js", "window['B'] = true;"),
        file(
            "/b/b.js",
            "import foo from './c.js';\nif (foo.settings.inUse) {\n  window['E'] = true;\n}\nwindow['A'] = true;\n",
        ),
        file(
            "/b/c.js",
            "window['BEFOREA'] = true;\n\nexport default {\n  settings: {\n    inUse: Boolean(document.documentElement['attachShadow'])\n  }\n};\n",
        ),
        file("/important.js", "window['E'] = false;"),
    ];

    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT_2015);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_file("/entry.js"),
    ]));
    let externs: Vec<Arc<SourceFile>> = vec![];

    let mut compiler = Compiler::new();
    let result = compiler.compile(&externs, &sources, options);
    assert!(result.success, "{:?}", result.errors);

    assert_eq!(
        ordered_input_names(&compiler),
        [
            "/b/c.js",
            "/b/b.js",
            "/b/a.js",
            "/important.js",
            "/a/b.js",
            "/a/a.js",
            "/entry.js"
        ]
    );
}

// port: CompilerTest#testProperEs6ModuleOrderingWithExport
#[test]
fn test_proper_es6_module_ordering_with_export() {
    let sources = vec![
        file(
            "/entry.js",
            "import {A, B, C1} from './a.js';\nconsole.log(A)\nconsole.log(B)\nconsole.log(C1)\n",
        ),
        file(
            "/a.js",
            "export {B} from './b.js';\nexport {C as C1} from './c.js';\nexport const A = 'a';\n",
        ),
        file("/b.js", "export const B = 'b';"),
        file("/c.js", "export const C = 'c';"),
    ];

    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT_2015);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_file("/entry.js"),
    ]));
    let externs: Vec<Arc<SourceFile>> = vec![];

    let mut compiler = Compiler::new();
    let result = compiler.compile(&externs, &sources, options);
    assert!(result.success, "{:?}", result.errors);

    assert_eq!(
        ordered_input_names(&compiler),
        ["/b.js", "/c.js", "/a.js", "/entry.js"]
    );
}

/// `Collections.shuffle(sources)`: Java shuffles with an unseeded `Random`; any permutation is
/// valid, so this test shuffles with a fixed xorshift sequence (Fisher-Yates, as Collections.shuffle).
fn shuffle<T>(list: &mut [T], state: &mut u64) {
    for i in (1..list.len()).rev() {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        let j = (*state % (i as u64 + 1)) as usize;
        list.swap(i, j);
    }
}

// port: CompilerTest#testProperGoogBaseOrdering
#[test]
fn test_proper_goog_base_ordering() {
    let mut sources = vec![
        file("test.js", "goog.setTestOnly()"),
        file("d.js", "goog.provide('d');"),
        file("c.js", "goog.provide('c');"),
        file("b.js", "goog.provide('b');"),
        file("a.js", "goog.provide('a');"),
        file(
            "base.js",
            "/** @fileoverview @provideGoog */\n/** @const */ var goog = goog || {};\nvar COMPILED = false;\n",
        ),
        file(
            "entry.js",
            "goog.require('a');\ngoog.require('b');\ngoog.require('c');\ngoog.require('d');\n",
        ),
    ];

    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT_2015);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_file("entry.js"),
    ]));
    let externs: Vec<Arc<SourceFile>> = vec![];

    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for _iteration_count in 0..10 {
        shuffle(&mut sources, &mut state);
        let mut compiler = Compiler::new();
        let result = compiler.compile(&externs, &sources, options.clone());
        assert!(result.success, "{:?}", result.errors);

        let ordered_inputs = ordered_input_names(&compiler);

        let mut sorted = ordered_inputs.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            [
                "a.js", "b.js", "base.js", "c.js", "d.js", "entry.js", "test.js"
            ]
        );
        let index_of = |name: &str| ordered_inputs.iter().position(|n| n == name).unwrap();
        assert!(index_of("base.js") < index_of("entry.js"));
        assert!(index_of("base.js") < index_of("test.js"));
    }
}

fn webpack_ordering_compile(
    sources: Vec<Arc<SourceFile>>,
    webpack_modules_by_id: &[(&str, &str)],
) -> Compiler {
    let mut options = CompilerOptions::new();
    options.set_language_in(LanguageMode::ECMASCRIPT_2015);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_dependency_options(DependencyOptions::prune_for_entry_points(vec![
        closure_jscomp::module_identifier::ModuleIdentifier::for_file("/entry.js"),
    ]));
    options.set_process_common_js_modules(true);
    options
        .set_module_resolution_mode(closure_jscomp::deps::module_loader::ResolutionMode::WEBPACK);
    let externs = vec![Arc::new(
        closure_testing::testing::test_externs_builder::TestExternsBuilder::new()
            .add_console()
            .build_externs_file("default_externs.js"),
    )];
    let mut compiler = Compiler::new();
    compiler.init_webpack_map(
        webpack_modules_by_id
            .iter()
            .map(|(id, path)| (id.to_string(), path.to_string()))
            .collect(),
    );
    let result = compiler.compile(&externs, &sources, options);
    assert!(result.success, "{:?}", result.errors);
    compiler
}

// port: CompilerTest#testDynamicImportOrdering
#[test]
fn test_dynamic_import_ordering() {
    let sources = vec![
        file("/entry.js", "__webpack_require__(2);"),
        file(
            "/a.js",
            "console.log(module.id);\n__webpack_require__.e(0).then(function() { return __webpack_require__(3); });\n",
        ),
        file("/b.js", "console.log(module.id); __webpack_require__(4);"),
        file("/c.js", "console.log(module.id);"),
    ];
    let compiler = webpack_ordering_compile(
        sources,
        &[
            ("1", "/entry.js"),
            ("2", "/a.js"),
            ("3", "/b.js"),
            ("4", "/c.js"),
        ],
    );
    assert_eq!(
        ordered_input_names(&compiler),
        ["/a.js", "/entry.js", "/c.js", "/b.js"]
    );
}

// port: CompilerTest#testDynamicImportOrdering2
#[test]
fn test_dynamic_import_ordering2() {
    let sources = vec![
        file("/entry.js", "__webpack_require__(2);"),
        file(
            "/a.js",
            "console.log(module.id);\n__webpack_require__.e(0).then(function() {\n  const foo = __webpack_require__(3);\n  console.log(foo);\n});\n",
        ),
        file("/b.js", "console.log(module.id); module.exports = 'foo';"),
    ];
    let compiler = webpack_ordering_compile(
        sources,
        &[("1", "/entry.js"), ("2", "/a.js"), ("3", "/b.js")],
    );
    assert_eq!(
        ordered_input_names(&compiler),
        ["/a.js", "/entry.js", "/b.js"]
    );
}

// port: CompilerTest#testDynamicImportOrdering3
#[test]
fn test_dynamic_import_ordering3() {
    let sources = vec![
        file("/entry.js", "__webpack_require__(2);"),
        file(
            "/a.js",
            "console.log(module.id);\nPromise.all([__webpack_require__.e(0)]).then(function() {\n  return __webpack_require__(3);\n});\n",
        ),
        file("/b.js", "console.log(module.id); module.exports = 'foo';"),
    ];
    let compiler = webpack_ordering_compile(
        sources,
        &[("1", "/entry.js"), ("2", "/a.js"), ("3", "/b.js")],
    );
    assert_eq!(
        ordered_input_names(&compiler),
        ["/a.js", "/entry.js", "/b.js"]
    );
}

// port: CompilerTest#testCheckSaveRestoreOptimize
#[test]
fn test_check_save_restore_optimize() {
    let mut compiler = test_error_manager_compiler();

    let mut options = CompilerOptions::new();
    options.set_assume_forward_declared_for_missing_types(true);
    options.set_language_in(LanguageMode::ECMASCRIPT_2017);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_check_types(true);
    options.set_strict_mode_input(true);
    options.set_emit_use_strict(false);
    options.set_preserve_detailed_source_info(true);

    CompilationLevel::ADVANCED_OPTIMIZATIONS.set_options_for_compilation_level(&mut options);
    let externs = vec![file(
        "externs.js",
        "var console = {};\n console.log = function() {};\n",
    )];
    let code = vec![file(
        "input.js",
        "function f() { return 2; }\nconsole.log(f());\n",
    )];
    compiler.init(&externs, &code, options.clone());

    compiler.parse();
    compiler.check();

    let state_after_checks = get_saved_compiler_state(&mut compiler);

    let mut compiler = test_error_manager_compiler();
    compiler.init(&externs, &code, options);
    restore_compiler_state(&mut compiler, &state_after_checks);

    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    let source = compiler.to_source();
    assert_eq!(source, "console.log(2);");
}

// port: CompilerTest#testGoogNamespaceEntryPoint
#[test]
fn test_goog_namespace_entry_point() {
    let inputs = vec![
        file(
            "/index.js",
            "var goog = {};\ngoog.provide = function(ns) {}; // stub, compiled out.\ngoog.provide('foobar');\nconst foo = require('./foo.js').default;\nfoo('hello');\n",
        ),
        file("/foo.js", "export default (foo) => { alert(foo); }"),
    ];

    let entry_points =
        vec![closure_jscomp::module_identifier::ModuleIdentifier::for_closure("goog:foobar")];

    let mut options = advanced();
    options.set_language_in(LanguageMode::ECMASCRIPT_2017);
    options.set_language_out(LanguageMode::ECMASCRIPT5);
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points(
        entry_points,
    ));
    options.set_process_common_js_modules(true);
    let externs = vec![file(
        "default_externs.js",
        include_str!("data/compiler_alert_externs.js"),
    )];

    let mut compiler = Compiler::new();
    compiler.compile(&externs, &inputs, options);

    let result = compiler.get_result();
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

// port: CompilerTest#testCodeReferenceToTypeImport
#[test]
fn test_code_reference_to_type_import() {
    let externs = [file("extern.js", "/** @externs */ function alert(x) {}")];
    let sources = [
        file(
            "type.js",
            "goog.module('type');\n\nexports.Type = class {}\n",
        ),
        file(
            "main.js",
            "goog.module('main');\n\nconst {Type} = goog.requireType('type');\n\nalert(new Type());\n",
        ),
    ];

    let mut options = CompilerOptions::new();
    options.set_closure_pass(true);

    let mut compiler = compiler();
    compiler.init(&externs, &sources, options);
    compiler.parse();
    compiler.check();

    assert!(
        compiler.get_warnings().is_empty(),
        "{:?}",
        compiler.get_warnings()
    );
    let errors = compiler.get_errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].get_type().key,
        closure_jscomp::check_type_import_code_references::TYPE_IMPORT_CODE_REFERENCE.key
    );
}

use closure_jscomp::compiler_license_tracker::ScriptNodeLicensesOnlyTracker;

// port: CompilerTest#testWeakSources
#[test]
fn test_weak_sources() {
    let sources = [
        weak_input("weak1.js", "goog.provide('a');", SourceKind::WEAK),
        weak_input("strong1.js", "goog.provide('a.b');", SourceKind::STRONG),
        weak_input("weak2.js", "goog.provide('c');", SourceKind::WEAK),
        weak_input("strong2.js", "goog.provide('d');", SourceKind::STRONG),
    ];

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);

    let mut compiler = Compiler::new();

    compiler.init(&[], &sources, options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);

    let chunk_graph = compiler.get_chunk_graph().unwrap();
    assert_eq!(chunk_graph.get_chunk_count(), 2);
    assert_eq!(
        chunk_graph.get_all_chunks()[0].get_name(),
        closure_jscomp::js_chunk::STRONG_CHUNK_NAME
    );
    assert_eq!(chunk_graph.get_all_chunks()[1].get_name(), WEAK_CHUNK_NAME);

    assert_eq!(
        compiler.to_source().to_string(),
        "var a={};a.b={};var d={};"
    );
}

// port: CompilerTest#weakSourcesChunksHelper
fn weak_sources_chunks_helper(save_and_restore: bool) {
    let mut m1 = JSChunk::new("m1");
    m1.add_source_file(weak_input(
        "weak1.js",
        "goog.provide('a');",
        SourceKind::WEAK,
    ));
    m1.add_source_file(weak_input(
        "strong1.js",
        "goog.provide('a.b');",
        SourceKind::STRONG,
    ));
    let mut m2 = JSChunk::new("m2");
    m2.add_source_file(weak_input(
        "weak2.js",
        "goog.provide('c');",
        SourceKind::WEAK,
    ));
    m2.add_source_file(weak_input(
        "strong2.js",
        "goog.provide('d');",
        SourceKind::STRONG,
    ));

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);

    let mut compiler = Compiler::new();

    compiler.init_chunks(&[], vec![m1.clone(), m2.clone()], options);

    compiler.parse();
    compiler.check();

    if save_and_restore {
        let byte_array_output_stream = get_saved_compiler_state(&mut compiler);

        // NOTE: The AST is not expected to be used after serialization to the save file.
        let mut lt = ScriptNodeLicensesOnlyTracker::new(&compiler);
        assert_eq!(
            compiler
                .to_source_for_chunk_with_tracker(&mut lt, &m1)
                .to_string(),
            "goog.provide(\"a.b\");"
        );

        restore_compiler_state(&mut compiler, &byte_array_output_stream);

        // restoring state creates new JSChunk objects. the old ones are stale.
        m1 = compiler
            .get_chunk_graph()
            .unwrap()
            .get_chunk_by_name("m1")
            .unwrap();
        m2 = compiler
            .get_chunk_graph()
            .unwrap()
            .get_chunk_by_name("m2")
            .unwrap();
    }

    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);

    assert_eq!(compiler.get_chunk_graph().unwrap().get_chunk_count(), 3);

    let weak_chunk = compiler
        .get_chunk_graph()
        .unwrap()
        .get_chunk_by_name("$weak$");
    let mut lt = ScriptNodeLicensesOnlyTracker::new(&compiler);
    let weak_chunk = weak_chunk.expect("weakChunk");

    assert_eq!(
        compiler
            .to_source_for_chunk_with_tracker(&mut lt, &m1)
            .to_string(),
        "var a={};a.b={};"
    );

    assert_eq!(
        compiler
            .to_source_for_chunk_with_tracker(&mut lt, &m2)
            .to_string(),
        "var d={};"
    );
    assert!(
        compiler
            .to_source_for_chunk_with_tracker(&mut lt, &weak_chunk)
            .is_empty()
    );
}

// port: CompilerTest#testWeakSourcesChunks
#[test]
fn test_weak_sources_chunks() {
    weak_sources_chunks_helper(/* save_and_restore= */ false);
}

// port: CompilerTest#testWeakSourcesSaveRestore
#[test]
fn test_weak_sources_save_restore() {
    weak_sources_chunks_helper(/* save_and_restore= */ true);
}

// port: CompilerTest#testWeakSourcesEntryPoint
#[test]
fn test_weak_sources_entry_point() {
    let extern_file = file("extern.js", "/** @externs */ function alert(x) {}");
    let strong = weak_input(
        "strong.js",
        "goog.module('strong');
const T = goog.requireType('weak');
/** @param {!T} x */ function f(x) { alert(x); }
",
        SourceKind::STRONG,
    );
    let weak = weak_input(
        "type.js",
        "goog.module('weak');
/** @typedef {number|string} */ exports.T;
sideeffect();
",
        SourceKind::WEAK,
    );

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);
    options.set_dependency_options(DependencyOptions::prune_for_entry_points([
        ModuleIdentifier::for_closure("strong"),
    ]));

    let mut compiler = Compiler::new();

    compiler.init(&[extern_file], &[strong, weak], options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    compiler.perform_finalizations();

    assert_eq!(
        compiler.to_source().to_string(),
        "var module$exports$strong={};function module$contents$strong_f(x){alert(x)};"
    );
}

// port: CompilerTest#testPreexistingWeakChunk
#[test]
fn test_preexisting_weak_chunk() {
    let strong = JSChunk::new("m");
    strong.add_source_file(weak_input(
        "strong.js",
        "goog.provide('a');",
        SourceKind::STRONG,
    ));
    let weak = JSChunk::new(WEAK_CHUNK_NAME);
    weak.add_source_file(weak_input(
        "weak.js",
        "goog.provide('b');",
        SourceKind::WEAK,
    ));
    weak.add_dependency(&strong);

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);

    let mut compiler = Compiler::new();

    compiler.init_chunks(&[], vec![strong, weak], options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);

    let chunk_graph = compiler.get_chunk_graph().unwrap();
    assert_eq!(chunk_graph.get_chunk_count(), 2);
    assert_eq!(chunk_graph.get_all_chunks()[0].get_name(), "m");
    assert_eq!(chunk_graph.get_all_chunks()[1].get_name(), WEAK_CHUNK_NAME);

    assert_eq!(compiler.to_source().to_string(), "var a={};");
}

/// `ModuleIdentifier.forClosure("strong")` entry point options of the implicit-weak-source tests.
fn strong_entry_point_options() -> CompilerOptions {
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);
    options.set_dependency_options(DependencyOptions::prune_for_entry_points([
        ModuleIdentifier::for_closure("strong"),
    ]));
    options
}

// port: CompilerTest#testImplicitWeakSourcesWithEntryPoint
#[test]
fn test_implicit_weak_sources_with_entry_point() {
    let extern_file = file("extern.js", "/** @externs */ function alert(x) {}");
    let strong = file(
        "strong.js",
        "goog.module('strong');
const T = goog.requireType('weak');
/** @param {!T} x */ function f(x) { alert(x); }
",
    );
    let weak = file(
        "type.js",
        "goog.module('weak');
/** @typedef {number|string} */ exports.T;
sideeffect();
",
    );

    let options = strong_entry_point_options();

    let mut compiler = Compiler::new();

    compiler.init(&[extern_file], &[strong, weak], options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    compiler.perform_finalizations();

    assert_eq!(
        compiler.to_source().to_string(),
        "var module$exports$strong={};function module$contents$strong_f(x){alert(x)};"
    );
}

// port: CompilerTest#testImplicitWeakSourcesWithEntryPointLegacyPrune
#[test]
fn test_implicit_weak_sources_with_entry_point_legacy_prune() {
    let extern_file = file("extern.js", "/** @externs */ function alert(x) {}");
    let strong = file(
        "moocher.js",
        "goog.requireType('weak');
/** @param {!weak.T} x */ function f(x) { alert(x); }
",
    );
    let weak = file(
        "type.js",
        "goog.module('weak');
/** @typedef {number|string} */ exports.T;
sideeffect();
",
    );

    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);
    options.set_dependency_options(DependencyOptions::prune_legacy_for_entry_points([]));

    let mut compiler = Compiler::new();

    compiler.init(&[extern_file], &[strong, weak], options);

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    compiler.perform_finalizations();

    assert_eq!(compiler.to_source().to_string(), "function f(x){alert(x)};");
}

// port: CompilerTest#testTransitiveImplicitWeakSourcesWithEntryPoint
#[test]
fn test_transitive_implicit_weak_sources_with_entry_point() {
    let extern_file = file("extern.js", "/** @externs */ function alert(x) {}");
    let strong = file(
        "strong.js",
        "goog.module('strong');
const T = goog.requireType('weakEntry');
/** @param {!T} x */ function f(x) { alert(x); }
",
    );
    let weak_entry = file(
        "weakEntry.js",
        "goog.module('weakEntry');
const w = goog.require('weakByAssociation');
exports = w;
",
    );
    let weak_by_association = file(
        "weakByAssociation.js",
        "goog.module('weakByAssociation');
/** @typedef {number|string} */ exports.T;
sideEffect();
",
    );

    let options = strong_entry_point_options();

    let mut compiler = Compiler::new();

    compiler.init(
        &[extern_file],
        &[strong, weak_entry, weak_by_association],
        options,
    );

    compiler.parse();
    compiler.check();
    compiler.perform_transpilation_and_optimizations(SegmentOfCompilationToRun::OPTIMIZATIONS);
    compiler.perform_finalizations();

    assert_eq!(compiler.get_warnings(), vec![]);
    assert_eq!(compiler.get_errors(), vec![]);

    assert_eq!(
        compiler.to_source().to_string(),
        "var module$exports$strong={};function module$contents$strong_f(x){alert(x)};"
    );
}

// port: CompilerTest#testTypesAreRemoved
#[test]
fn test_types_are_removed() {
    let mut options = CompilerOptions::new();
    options.set_check_types(true);
    let input = file(
        "input.js",
        "/** @license @type {!Foo} */
class Foo {}
class Bar {}
/** @typedef {number} */ let Num;
const n = /** @type {!Num} */ (5);
var /** !Foo */ f = new Bar;
",
    );
    let mut compiler = Compiler::new();
    // `new WeakReference<>(compiler.getTypeRegistry())`: the registry is created here and the
    // reference is cleared once the compiler drops it.
    compiler.get_type_registry();
    compiler.compile_single(file("externs", ""), input, options);

    // Just making sure that typechecking ran and didn't crash.  It would be reasonable
    // for there also to be other type errors in this code before the final null assignment.
    let warnings = compiler.get_warnings();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].get_type().key,
        closure_jscomp::type_validator::TYPE_MISMATCH_WARNING.key
    );

    // System.gc(); System.runFinalization(); registryWeakReference.get() == null
    assert!(compiler.get_type_registry_field_and_ast().0.is_none());
}

// port: CompilerTest#testExportSymbolReservesNamesForRenameVars
#[test]
fn test_export_symbol_reserves_names_for_rename_vars() {
    let mut compiler = Compiler::new();
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);
    options.set_variable_renaming(
        closure_jscomp::variable_renaming_policy::VariableRenamingPolicy::ALL,
    );

    let js = "var goog, x; goog.exportSymbol('a', x);";
    let inputs = [file("testcode", js)];
    let result = compiler.compile(&[file("externs", "")], &inputs, options);

    assert!(result.success);
    assert_eq!(
        compiler.to_source().to_string(),
        "var b;var c;b.exportSymbol(\"a\",c);"
    );
}

// port: CompilerTest#testSourceMapNamesCollapsedAnonymousFunctionAfterRenaming
#[test]
fn test_source_map_names_collapsed_anonymous_function_after_renaming() {
    // `f` starts out as the name of a variable holding an anonymous function expression.
    // CollapseAnonymousFunctions rewrites `var f = function() {...}` into `function f() {...}`
    // before variable renaming replaces `f` with a minified name. The source map should still
    // attach the original name "f" to the minified identifier, not to the `function` keyword.
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_source_map_output_path("dummy".to_string());
    options.set_collapse_anonymous_functions(true);
    options.set_variable_renaming(
        closure_jscomp::variable_renaming_policy::VariableRenamingPolicy::ALL,
    );

    let externs = [file("externs", "function alert(x) {}")];
    let js = "var f = function() { return 1; };\nalert(f());\n";
    let inputs = [file("testcode.js", js)];
    let mut compiler = Compiler::new();
    let result = compiler.compile(&externs, &inputs, options);

    assert!(result.success);
    // `f` was collapsed into a function declaration and then renamed to `a`.
    let source = compiler.to_source();
    assert_eq!(source.to_string(), "function a(){return 1}alert(a());");

    let source_map = result.source_map.unwrap();
    let mut out = String::new();
    source_map
        .lock()
        .unwrap()
        .append_to(&mut out, "testcode-compiled.js")
        .unwrap();
    let mut consumer = SourceMapConsumerV3::new();
    consumer.parse(out.as_str()).unwrap();

    // Column 9 (0-based) is where the minified function name `a` appears in
    // "function a(){return 1}alert(a());".
    let decl_mapping = consumer.get_mapping_for_line(1, 10).unwrap();
    assert_eq!(decl_mapping.get_identifier(), "f");

    // Column 28 (0-based) is where the call site `a` appears in `alert(a());`.
    let call_mapping = consumer.get_mapping_for_line(1, 29).unwrap();
    assert_eq!(call_mapping.get_identifier(), "f");
}
// port: CompilerTest#testGenerateExportsReservesNames
#[test]
fn test_generate_exports_reserves_names() {
    let mut compiler = Compiler::new();
    let mut options = CompilerOptions::new();
    options.set_emit_use_strict(false);
    options.set_closure_pass(true);
    options.set_variable_renaming(
        closure_jscomp::variable_renaming_policy::VariableRenamingPolicy::ALL,
    );
    options.set_generate_exports(true);

    let js = "var goog; /** @export */ var a={};";
    let inputs = [file("testcode", js)];
    let result = compiler.compile(&[file("externs", "")], &inputs, options);

    assert!(result.success);
    assert_eq!(
        compiler.to_source().to_string(),
        "var b;var c={};b.exportSymbol(\"a\",c);"
    );
}

// Rust-only (D2 advanced_strict): TypeInference#traverseDynamicImport resolves the specifier
// through the module loader, whose ErrorHandler (Java: the Compiler) reports LOAD_WARNING, an
// error, at once; the error halts compilation before TypeCheck. The Rust module loader buffers
// its reports, so every resolving caller must replay them right after the resolution.
#[test]
fn dynamic_import_load_warning_halts_before_type_check() {
    let mut options = advanced();
    options.set_language_in(LanguageMode::UNSTABLE);
    options.set_allow_dynamic_import(true);
    options.set_warning_level(diagnostic_groups::CHECK_TYPES.clone(), CheckLevel::WARNING);
    let c = compile_input(
        "function f() {}\nnew f();\nimport('./missing.js');\n",
        options,
    );
    let errors = c.get_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].get_type().key, "JSC_JS_MODULE_LOAD_WARNING");
    assert_eq!(
        (errors[0].get_line_number(), errors[0].get_charno()),
        (3, 7)
    );
    assert!(c.get_warnings().is_empty());
}
// Rust-only (D2 advanced_strict): ConvertToDottedProperties#visit reads `leftElem.getNext()`,
// null for a computed field without an initializer, and dereferences it only when the key is a
// valid property name string.
#[test]
fn convert_to_dotted_properties_computed_field_without_initializer() {
    let mut options = advanced();
    options.set_language_in(LanguageMode::UNSTABLE);
    options.set_language_out(LanguageMode::NO_TRANSPILE);
    let mut c = compiler();
    c.compile_single(
        file(
            "externs.js",
            "function alert(x) {}\nvar Symbol = {};\nSymbol.iterator;\n",
        ),
        file(
            "test.js",
            "class C {\n  [Symbol.iterator];\n  ['a'] = 1;\n}\nalert(new C().a);\n",
        ),
        options,
    );
    assert!(c.get_errors().is_empty());
    assert_eq!(
        c.to_source(),
        "class a{[Symbol.iterator];a=1;}alert((new a).a);"
    );
}
