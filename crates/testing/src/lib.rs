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

//! closure-testing: test-support reader for the unit-test corpus in `corpus/unit`
//! (records, descriptors, options defaults, derived expected pipeline). Not linked into the
//! compiler. See README.md.
#![forbid(unsafe_code)]

pub mod compiler_test_case;
pub mod compiler_test_case_utils;
pub mod compiler_type_test_case;
pub mod corpus;
pub mod derived;
pub mod descriptor;
pub mod dsl;
pub mod harness_passes;
pub mod integration;
pub mod jscomp_api;
pub mod json;
pub mod modules_test_utils;
pub mod node_printing;
pub mod post_call;
pub mod proto_neutral;
pub mod reader;
pub mod record;
pub mod replay;
mod stand_in;
pub mod testing;
pub mod throwable;
pub mod type_check_test_case;
pub mod unit_recorder;
pub mod unit_test_utils;
pub mod value;
