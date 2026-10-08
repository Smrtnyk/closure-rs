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

//! Port of com.google.javascript.jscomp.deps.
// port: com.google.javascript.jscomp.deps#package
pub mod browser_module_resolver;
pub mod browser_with_transformed_prefixes_module_resolver;
pub mod closure_bundler;
pub mod dependency_info;
pub mod deps_file_regex_parser;
pub mod js_file_line_parser;
pub mod js_file_regex_parser;
pub mod module_loader;
pub mod module_names;
pub mod module_resolver;
pub mod node_module_resolver;
pub mod simple_dependency_info;
pub mod sorted_dependencies;
pub mod source_code_escapers;
pub mod webpack_module_resolver;
