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

//! Port of com.google.javascript.jscomp.parsing (the parser, IRFactory, JSDoc parser).
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

pub mod config;

pub mod annotation;
pub mod feature_collector;
pub mod ir_factory;
pub mod js_doc_info_parser;
pub mod js_doc_token;
pub mod js_doc_token_stream;
pub mod parser;
pub mod parser_configuration;
pub mod parser_runner;
pub mod parsing_util;
pub mod type_transformation_parser;
