/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/serialization/JSDocSerializerTest.java.

//! Port of serialization/JSDocSerializerTest.java.
use closure_jscomp::serialization::jsdoc_serializer::JSDocSerializer;
use closure_rhino::jsdoc_info::{JSDocInfo, PerFileClosureUnawareMode};
use closure_rhino::node::Ast;

// port: JSDocSerializerTest#preserveIsTypeSummary
#[test]
fn preserve_is_type_summary() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_type_summary();
    let original_js_doc_info = builder.build();
    let converted_js_doc_info = JSDocSerializer::convert_jsdoc_info_for_optimizations(
        &mut ast,
        original_js_doc_info.as_deref(),
    )
    .unwrap();
    assert!(converted_js_doc_info.is_type_summary());
}

// port: JSDocSerializerTest#preserveIsSassGeneratedCssTs
#[test]
fn preserve_is_sass_generated_css_ts() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_sass_generated_css_ts();
    let original_js_doc_info = builder.build();
    let converted_js_doc_info = JSDocSerializer::convert_jsdoc_info_for_optimizations(
        &mut ast,
        original_js_doc_info.as_deref(),
    )
    .unwrap();
    assert!(converted_js_doc_info.is_sass_generated_css_ts());
}

// port: JSDocSerializerTest#preserveSupportsClosureUnaware_simple
#[test]
fn preserve_supports_closure_unaware_simple() {
    let mut ast = Ast::new();
    let mut builder = JSDocInfo::builder();
    builder.record_type_summary();
    assert!(builder.record_closure_unaware_code_with_mode(PerFileClosureUnawareMode::SIMPLE));
    let original_js_doc_info = builder.build();

    let converted_js_doc_info = JSDocSerializer::convert_jsdoc_info_for_optimizations(
        &mut ast,
        original_js_doc_info.as_deref(),
    )
    .unwrap();
    assert_eq!(
        converted_js_doc_info.get_per_file_closure_unaware_mode(),
        Some(PerFileClosureUnawareMode::SIMPLE)
    );
}
