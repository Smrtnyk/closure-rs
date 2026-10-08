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

//! Port of com.google.javascript.jscomp.parsing.parser.

pub mod class_or_object_element_info;
pub mod feature_set;
pub mod identifier_token;
pub mod identifiers;
pub mod keywords;
pub mod line_number_scanner;
pub mod literal_token;
pub mod predefined_name;
pub use closure_rhino::jscomp_parsing_parser::source_file;
pub mod string_literal_token;
pub mod template_literal_token;
pub mod token;
pub mod token_type;
pub mod trees;
pub mod util;

pub use closure_rhino::js_string::JsString;

#[cfg(test)]
pub mod testing;

pub mod scanner;

#[allow(clippy::module_inception)] // Java package and Parser.java both map to parser.
pub mod parser;
