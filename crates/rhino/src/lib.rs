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

//! Port of com.google.javascript.rhino (Node, Token, IR, JSDocInfo, strings, dtoa).
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub mod closure_primitive;
pub mod common_hash;
pub mod dtoa;
pub mod error_reporter;
pub mod hamt_pmap;
pub mod input_id;
pub mod ir;
pub mod java_lang;
pub mod java_util;
pub mod js_identifier;
pub mod js_string;
pub mod js_type_expression;
pub mod jscomp_base;
pub mod jscomp_colors;
pub mod jscomp_parsing_parser;
pub mod jscomp_serialization;
pub mod jsdoc_info;
pub mod jstype;
pub mod msg;
pub mod node;
pub mod non_jsdoc_comment;
pub mod outcome;
pub mod pmap;
pub mod prop_translator;
pub mod qualified_name;
pub mod rhino_string_pool;
pub mod simple_source_file;
pub mod source_position;
pub mod static_ref;
pub mod static_scope;
pub mod static_slot;
pub mod static_source_file;
pub mod static_symbol_table;
pub mod testing;
pub mod token;
pub mod token_stream;
pub mod token_util;
pub mod type_declarations_ir;
