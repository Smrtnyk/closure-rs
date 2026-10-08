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

//! Rust-only checks of the generated TypedAST protos (D-003): the Java-written
//! `runtime_libs.typedast` (a `TypedAst.List`) parses and re-serializes to the same bytes, so
//! the Rust messages are wire-compatible with protobuf-java, field order and defaults included.
use closure_jscomp::serialization::protobuf::{
    CodedInputStream, CodedOutputStream, InvalidProtocolBufferException, Message,
};
use closure_jscomp::serialization::typed_ast_proto::{AstNode, NodeKind, TypedAstList};

fn runtime_libs() -> &'static [u8] {
    closure_resources::jar::get_resource_as_stream(
        "com.google.javascript.jscomp.Compiler",
        "/runtime_libs.typedast",
    )
    .unwrap()
    .read_all_bytes()
}

#[test]
fn runtime_libs_round_trip_byte_identical() {
    let bytes = runtime_libs();
    let list = TypedAstList::parse_from(bytes).unwrap();
    assert!(list.get_typed_asts_count() > 0);
    let typed_ast = list.get_typed_asts(0);
    assert!(typed_ast.get_code_ast_count() > 0);
    assert!(typed_ast.get_string_pool().get_strings_count() > 0);
    // Every lazy script parses as an AstNode and re-serializes identically.
    for lazy in typed_ast.get_code_ast_list() {
        let mut input = CodedInputStream::new_instance(lazy.get_script());
        input.set_recursion_limit(i32::MAX);
        let script = AstNode::parse_from_stream(&mut input).unwrap();
        assert_eq!(script.get_kind(), NodeKind::SOURCE_FILE);
        assert_eq!(script.to_byte_array(), lazy.get_script());
        assert_eq!(script.get_serialized_size(), lazy.get_script().len());
    }
    assert_eq!(list.get_serialized_size(), bytes.len());
    assert_eq!(list.to_byte_array(), bytes);
}

#[test]
fn varint_and_zigzag_encoding() {
    let mut out = CodedOutputStream::new_instance();
    out.write_int32_no_tag(-1);
    out.write_sint32_no_tag(-1);
    out.write_uint32_no_tag(300);
    let bytes = out.into_bytes();
    assert_eq!(
        bytes,
        vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x01, 0xAC, 0x02
        ]
    );
    let mut input = CodedInputStream::new_instance(&bytes);
    assert_eq!(input.read_int32().unwrap(), -1);
    assert_eq!(input.read_sint32().unwrap(), -1);
    assert_eq!(input.read_uint32().unwrap(), 300);
    assert!(input.is_at_end());
    assert_eq!(CodedOutputStream::compute_int32_size_no_tag(-1), 10);
    assert_eq!(CodedOutputStream::compute_uint32_size_no_tag(300), 2);
    assert_eq!(CodedOutputStream::compute_uint32_size_no_tag(0), 1);
}

#[test]
fn truncated_input_fails() {
    let bytes = runtime_libs();
    let err = TypedAstList::parse_from(&bytes[..bytes.len() - 1]).unwrap_err();
    assert_eq!(err, InvalidProtocolBufferException::truncated_message());
}

#[test]
fn runtime_libs_string_pool_round_trip() {
    use closure_jscomp::serialization::string_pool::StringPool;
    let list = TypedAstList::parse_from(runtime_libs()).unwrap();
    for typed_ast in list.get_typed_asts_list() {
        let proto = typed_ast.get_string_pool();
        let pool = StringPool::from_proto(proto);
        assert_eq!(&pool.to_proto(), proto);
    }
}

#[test]
fn axiomatic_color_count_matches_list() {
    use closure_jscomp::serialization::type_pointers::{
        AXIOMATIC_COLOR_COUNT, OFFSET_TO_AXIOMATIC_COLOR,
    };
    assert_eq!(
        OFFSET_TO_AXIOMATIC_COLOR.len() as i32,
        AXIOMATIC_COLOR_COUNT
    );
}

#[test]
fn runtime_libs_color_pool_builds() {
    use closure_jscomp::serialization::color_pool::ColorPool;
    use closure_jscomp::serialization::string_pool::StringPool;
    use std::sync::Arc;
    let list = TypedAstList::parse_from(runtime_libs()).unwrap();
    let mut builder = ColorPool::builder();
    let mut shards = Vec::new();
    for typed_ast in list.get_typed_asts_list() {
        let string_pool = Arc::new(StringPool::from_proto(typed_ast.get_string_pool()));
        shards.push(builder.add_shard(Arc::new(typed_ast.get_type_pool().clone()), string_pool));
    }
    let pool = builder.build();
    assert!(
        pool.get_registry()
            .get(closure_rhino::jscomp_colors::standard_colors::ARRAY_ID)
            .get_id()
            == closure_rhino::jscomp_colors::standard_colors::ARRAY_ID
    );
}

#[test]
fn runtime_libs_scripts_deserialize() {
    use closure_jscomp::serialization::script_node_deserializer::ScriptNodeDeserializer;
    use closure_jscomp::serialization::string_pool::StringPool;
    use closure_jscomp::source_file::SourceFile;
    use closure_rhino::node::Ast;
    use closure_rhino::token::Token;
    use std::sync::Arc;
    let list = TypedAstList::parse_from(runtime_libs()).unwrap();
    let mut ast = Ast::new();
    let mut count = 0;
    for typed_ast in list.get_typed_asts_list() {
        let string_pool = Arc::new(StringPool::from_proto(typed_ast.get_string_pool()));
        let file_pool: Arc<Vec<Arc<SourceFile>>> = Arc::new(
            typed_ast
                .get_source_file_pool()
                .get_source_file_list()
                .iter()
                .map(|p| Arc::new(SourceFile::from_proto(p)))
                .collect(),
        );
        for lazy in typed_ast.get_code_ast_list() {
            let deserializer =
                ScriptNodeDeserializer::new(lazy, string_pool.clone(), None, file_pool.clone());
            let script = deserializer.deserialize_new(&mut ast);
            assert_eq!(script.get_token(&ast), Token::SCRIPT);
            count += 1;
        }
    }
    assert!(count > 100, "{count}");
}
