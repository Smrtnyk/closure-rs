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

//! Faithful port of Closure Compiler's Java command line entry points.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub mod abstract_command_line_runner;
pub mod args4j;
pub mod command_line_runner;
pub mod gson;
pub mod java_io;
pub mod java_util_logging_level;
pub mod jdk_globs;
pub mod option_setup;
pub mod out_of_scope;
pub mod stand_in;
pub mod verifying_error_manager;
