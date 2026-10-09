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

#[path = "support/printer_dump.rs"]
mod printer_dump;
use closure_jscomp::{
    code_consumer::CodeConsumer,
    code_generator::{CodeGeneration, CodeGenerator, Context},
    code_printer::{
        Builder, CodeGeneratorFactory, CompactCodePrinter, Format as PrinterFormat, LicenseTracker,
    },
    compiler_options::CompilerOptions,
    source_file::SourceFile,
    source_map::{DetailLevel, Format, PrefixLocationMapping},
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{
    java_lang::charset::Charset,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

// Mirrors the Java probe's anonymous CodeGenerator subclass. Its override must
// receive nested nodes through the inherited generator bodies, including functions.
struct RenameFactory;
struct RenameGenerator<'a> {
    base: CodeGenerator<'a>,
}
impl CodeGeneratorFactory for RenameFactory {
    fn get_code_generator<'a>(
        &'a self,
        _format: PrinterFormat,
        consumer: &'a mut dyn CodeConsumer,
        options: &CompilerOptions,
        _registry: Option<&'a mut closure_jstype::JSTypeRegistry>,
    ) -> Box<dyn CodeGeneration<'a> + 'a> {
        Box::new(RenameGenerator {
            base: CodeGenerator::new(consumer, options),
        })
    }
}
impl<'a> CodeGeneration<'a> for RenameGenerator<'a> {
    fn code_generator(&self) -> &CodeGenerator<'a> {
        &self.base
    }
    fn code_generator_mut(&mut self) -> &mut CodeGenerator<'a> {
        &mut self.base
    }
    fn add_node_with_context(&mut self, ast: &Ast, node: NodeId, context: Context) {
        if node.is_name(ast) && !node.has_children(ast) && node.get_string(ast) == "special" {
            self.add(&JsString::from("renamed"));
        } else {
            // super.add(n, context) calls the private add(n, context, true) body.
            self.add_node_with_comments(ast, node, context, true);
        }
    }
}

#[derive(Default)]
struct ProbeLicenseTracker {
    licenses: closure_rhino::fast_hash::IndexSet<JsString>,
}
impl LicenseTracker for ProbeLicenseTracker {
    fn track_licenses_for_node(&mut self, ast: &Ast, node: NodeId) {
        if let Some(info) = node.get_jsdoc_info(ast)
            && let Some(license) = info.get_license()
        {
            self.licenses.insert(license);
        }
    }
    fn emit_licenses(&self) -> closure_rhino::fast_hash::IndexSet<JsString> {
        self.licenses.clone()
    }
}
fn options(config: &str) -> CompilerOptions {
    let mut options = CompilerOptions::default();
    match config {
        "output_types" | "licenses" | "compact" | "strict" | "type_summary" | "builder_pretty"
        | "builder_line_break" => {}
        "types" => options.set_preserve_type_annotations(true),
        "pretty_types" => {
            options.set_pretty_print(true);
            options.set_preserve_type_annotations(true);
        }
        "pretty" => options.set_pretty_print(true),
        "pretty_map" | "pretty_map_all" => {
            options.set_pretty_print(true);
            options.set_always_gather_source_map_info(true);
        }
        "pretty_cut_40" => {
            options.set_pretty_print(true);
            options.set_line_length_threshold(40);
            options.set_always_gather_source_map_info(true);
        }
        "pretty_line_break" => {
            options.set_pretty_print(true);
            options.set_line_break(true);
        }
        "pretty_single_quotes_latin1" => {
            options.set_pretty_print(true);
            options.set_prefer_single_quotes(true);
            options.set_output_charset(Charset::ISO_8859_1);
        }
        "quote_keywords" => options.set_quote_keyword_properties(true),
        "comments" => options.set_preserve_non_jsdoc_comments(true),
        "pretty_comments" => {
            options.set_pretty_print(true);
            options.set_preserve_non_jsdoc_comments(true);
        }
        "gents" => {
            options.set_gents_mode(true);
            options.set_preserve_type_annotations(true);
            options.set_preserve_non_jsdoc_comments(true);
        }
        "uncut_map" => {
            options.set_line_length_threshold(0);
            options.set_always_gather_source_map_info(true);
        }
        "map_path" => options.set_source_map_output_path("out.map".to_owned()),
        "ijs_keywords" => {
            options.set_quote_keyword_properties(true);
            options.set_incremental_checks(
                closure_jscomp::compiler_options::IncrementalCheckMode::GENERATE_IJS,
            );
            options.set_preserve_type_annotations(false);
        }
        "es3" => options.set_output_feature_set(FeatureSet::ES3),
        "utf16" => options.set_output_charset(Charset::UTF_16),
        "utf16be" => options.set_output_charset(Charset::UTF_16BE),
        "utf16le" => options.set_output_charset(Charset::UTF_16LE),
        "line_break" => options.set_line_break(true),
        "cut_40" => {
            options.set_line_length_threshold(40);
            options.set_always_gather_source_map_info(true);
        }
        "map_all" => {
            options.set_always_gather_source_map_info(true);
            options.set_source_map_detail_level(DetailLevel::ALL);
        }
        "map_symbols" => {
            options.set_always_gather_source_map_info(true);
            options.set_source_map_detail_level(DetailLevel::SYMBOLS);
        }
        "single_quotes" => options.set_prefer_single_quotes(true),
        "ascii" => options.set_output_charset(Charset::US_ASCII),
        "utf8" => options.set_output_charset(Charset::UTF_8),
        "latin1" => options.set_output_charset(Charset::ISO_8859_1),
        "trusted" => options.set_trusted_strings(true),
        "untrusted" => options.set_trusted_strings(false),
        "original_names" => options.set_use_original_names_in_output(true),
        "es5" => options.set_output_feature_set(FeatureSet::ES5),
        "es2015" => options.set_output_feature_set(FeatureSet::ES2015),
        "es2019" => options.set_output_feature_set(FeatureSet::ES2019),
        _ => panic!("Unported fixture configuration {config}"),
    }
    options
}
fn check_file(
    path: &Path,
    configurations: &mut closure_rhino::fast_hash::IndexMap<String, usize>,
    source_maps: &mut closure_rhino::fast_hash::IndexMap<String, usize>,
) -> (usize, usize) {
    let mut json = String::new();
    GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_to_string(&mut json)
        .unwrap();
    let mut deserializer = serde_json::Deserializer::from_str(&json);
    deserializer.disable_recursion_limit();
    let fixture = Value::deserialize(&mut deserializer).unwrap();
    let mut ast = Ast::default();
    let source = Arc::new(if fixture["source_stub"].as_bool().unwrap_or(false) {
        SourceFile::builder()
            .with_path(fixture["name"].as_str().unwrap())
            .set_is_stub_source_file_for_already_provided_input()
            .build()
    } else {
        SourceFile::from_code(
            fixture["name"].as_str().unwrap(),
            printer_dump::js_string(&fixture["code"]),
        )
    });
    if let Some(expected) = fixture.get("consumer_edges") {
        let mut consumer = CompactCodePrinter::new(false, 2, false, DetailLevel::ALL, None);
        consumer.add(&"a=".into());
        consumer.note_preferred_line_break();
        consumer.add(&"b".into());
        consumer.maybe_line_break();
        let code = consumer.mapped.get_code();
        assert_eq!(code, printer_dump::js_string(&expected["source"]));
        assert!(expected["mappings"].is_null());
        assert!(consumer.mapped.get_source_mappings(&code).is_none());
        let lone = JsString::from("file_").concat(&JsString::from_units(vec![0xd800]));
        let mut map = Format::V3.get_instance();
        map.set_prefix_mappings(vec![Box::new(PrefixLocationMapping::new(
            "dummy.js",
            lone.clone(),
        ))]);
        map.set_wrapper_prefix(
            &JsString::from("prefix_").concat(&JsString::from_units(vec![0xdfff, b'\n' as u16])),
        );
        map.add_source_file(&"dummy.js".into(), &lone);
        let mut mapping_ast = Ast::default();
        let number = mapping_ast.new_number(1.0);
        number.set_static_source_file(
            &mut mapping_ast,
            Some(Arc::new(SourceFile::from_code("dummy.js", "1"))),
        );
        number.set_lineno_charno(&mut mapping_ast, 1, 0);
        map.add_mapping_for_node(
            &mapping_ast,
            number,
            closure_sourcemap::file_position::FilePosition::new(0, 0),
            Some(closure_sourcemap::file_position::FilePosition::new(0, 1)),
        );
        let mut json = String::new();
        map.append_to(&mut json, lone).unwrap();
        assert_eq!(json, expected["source_map_utf16"].as_str().unwrap());
    }
    let mut number_expectations = Vec::new();
    let root = printer_dump::load_node(
        &mut ast,
        &fixture["dump"]["ast"],
        &source,
        &mut number_expectations,
    );
    for (node, expected) in &number_expectations {
        if let Some(expected) = expected {
            assert_eq!(
                Builder::new(*node).build(&ast),
                *expected,
                "number in {}",
                path.display()
            );
        } else {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || Builder::new(*node).build(&ast)
                ))
                .is_err(),
                "Java rejects number in {}",
                path.display()
            );
        }
    }
    let mut registry = closure_jstype::JSTypeRegistry::new(
        &mut ast,
        Box::new(closure_rhino::error_reporter::NullErrorReporter),
        Vec::new(),
    );
    let outputs = fixture["outputs"].as_object().unwrap();
    for (config, expected) in outputs {
        let mut tracker = ProbeLicenseTracker::default();
        let compiler_options = options(config);
        let builder = Builder::new(root)
            .set_compiler_options(&compiler_options)
            .set_tag_as_strict(config == "strict")
            .set_tag_as_type_summary(config == "type_summary");
        let builder = if config == "builder_pretty" {
            builder.set_pretty_print(true)
        } else {
            builder
        };
        let builder = if config == "builder_line_break" {
            builder.set_line_break(true)
        } else {
            builder
        };
        let builder = if fixture["custom_factory"].as_bool().unwrap_or(false) {
            builder.set_code_generator_factory(RenameFactory)
        } else {
            builder
        };
        let builder = if config == "output_types" {
            builder
                .set_output_types(true)
                .set_type_registry(&mut registry)
        } else {
            builder
        };
        let builder = if config == "licenses" {
            builder.set_license_tracker(Some(&mut tracker))
        } else {
            builder
        };
        if let Some(exception) = expected.get("threw") {
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                builder.build_with_source_mappings(&ast)
            }))
            .err()
            .expect("Java throws for this AST");
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap();
            assert_eq!(exception.as_str(), Some("java.lang.IllegalStateException"));
            assert_eq!(message, expected["message"].as_str().unwrap());
            *configurations.entry(config.clone()).or_default() += 1;
            continue;
        }
        let result = builder.build_with_source_mappings(&ast);
        if let Some(licenses) = expected.get("licenses") {
            assert_eq!(
                tracker.emit_licenses().into_iter().collect::<Vec<_>>(),
                licenses
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(printer_dump::js_string)
                    .collect::<Vec<_>>()
            );
        }
        let expected_source = printer_dump::js_string(&expected["source"]);
        if result.source != expected_source {
            let first = result
                .source
                .as_units()
                .iter()
                .zip(expected_source.as_units())
                .position(|(a, b)| a != b)
                .unwrap_or(result.source.length().min(expected_source.length()));
            panic!(
                "{config} in {}: first mismatch at UTF-16 column {first}; actual {:?}, expected {:?}",
                path.display(),
                result
                    .source
                    .substring(
                        first.saturating_sub(60),
                        (first + 120).min(result.source.length())
                    )
                    .to_string_lossy(),
                expected_source
                    .substring(
                        first.saturating_sub(60),
                        (first + 120).min(expected_source.length())
                    )
                    .to_string_lossy()
            );
        }
        if let Some(expected_map) = expected.get("map") {
            let mut map = Format::V3.get_instance();
            for mapping in result.mappings.unwrap() {
                map.add_mapping(&ast, &mapping);
            }
            let mut json = String::new();
            map.append_to(&mut json, "out.js").unwrap();
            assert_eq!(
                json,
                expected_map.as_str().unwrap(),
                "{config} source map in {}",
                path.display()
            );
            *source_maps.entry(config.clone()).or_default() += 1;
        } else {
            assert!(result.mappings.is_none());
        }
        *configurations.entry(config.clone()).or_default() += 1;
    }
    (outputs.len(), number_expectations.len())
}
#[test]
fn code_printer_matches_java_parse_dumps() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run_differential)
        .unwrap()
        .join()
        .unwrap();
}

// Java differential probe: CodeGenerator#add rejects subclasses before checking
// FUNCTION children or printing SCRIPT/MODULE_BODY/BLOCK/ROOT contents.
#[test]
fn unexpected_node_subclasses_match_java() {
    let cases: Value =
        serde_json::from_str(include_str!("data/printer_subclass_guards.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let mut ast = Ast::default();
        let node = match case["class"].as_str().unwrap() {
            "NumberNode" => ast.new_number(1.0),
            "StringNode" => ast.new_string("name"),
            "BigIntNode" => ast.new_big_int(num_bigint::BigInt::from(1)),
            "TemplateLiteralSubstringNode" => {
                ast.new_template_lit_string(Some("cooked".into()), "raw")
            }
            class => panic!("Unknown Java subclass {class}"),
        };
        node.set_token(
            &mut ast,
            match case["token"].as_str().unwrap() {
                "FUNCTION" => Token::FUNCTION,
                "SCRIPT" => Token::SCRIPT,
                "MODULE_BODY" => Token::MODULE_BODY,
                "BLOCK" => Token::BLOCK,
                "ROOT" => Token::ROOT,
                token => panic!("Unknown Java token {token}"),
            },
        );
        assert_eq!(case["threw"], "java.lang.Error");
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Builder::new(node).build(&ast)
        }))
        .expect_err("Java throws for this subclass");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap();
        assert_eq!(message, case["message"].as_str().unwrap(), "{case}");
    }
}
fn run_differential() {
    let directory = std::env::var_os("CLOSURE_RS_PRINTER_DIFF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/printer"));
    let mut paths: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "gz"))
        .collect();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "No printer fixtures in {}",
        directory.display()
    );
    let mut pairs = 0;
    let mut numbers = 0;
    let mut configurations = closure_rhino::fast_hash::IndexMap::<_, _>::default();
    let mut source_maps = closure_rhino::fast_hash::IndexMap::<_, _>::default();
    for path in &paths {
        let (p, n) = check_file(path, &mut configurations, &mut source_maps);
        pairs += p;
        numbers += n;
    }
    println!(
        "printer differential: {} files, {pairs} configuration pairs, {numbers} number nodes",
        paths.len()
    );
    for (config, count) in configurations {
        println!(
            "{config}: {count}/{} exact, {} exact source maps",
            paths.len(),
            source_maps.get(&config).copied().unwrap_or(0)
        );
    }
}
