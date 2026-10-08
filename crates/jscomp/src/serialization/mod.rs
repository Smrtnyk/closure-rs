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

pub mod color_pool;
pub mod color_serializer;
pub mod convert_types_to_colors;
pub mod fast_gzip_output_stream;
pub mod js_type_color_id_hasher;
pub mod js_type_reconserializer;
pub mod jsdoc_serializer;
pub mod malformed_typed_ast_exception;
pub mod optimization_jsdoc_proto;
pub mod protobuf;
pub mod script_node_deserializer;
pub mod serialization_options;
pub mod serialize_typed_ast_pass;
pub mod serialize_types_to_pointers;
pub mod source_file_proto;
pub mod string_pool;
pub mod type_pointers;
pub mod typed_ast_deserializer;
pub mod typed_ast_proto;
pub mod typed_ast_serializer;
pub mod types_proto;
pub mod wtf8;
