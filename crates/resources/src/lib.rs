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

//! The externs and runtime JS libraries of Closure Compiler as data, and the port of the Java code
//! that lists and loads them (as far as it does not need the `Compiler` or the AST).
//!
//! The data is the reference `closure-compiler.jar`'s resources, verbatim (Apache-2.0), embedded
//! with `include_bytes!` and keyed by the exact paths Java uses (see [`jar_contents`]). Module
//! paths mirror the Java packages below `com.google.javascript.jscomp`; closure-jscomp re-exports
//! these items instead of porting them again.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

pub mod abstract_command_line_runner;
pub mod compiler_options;
pub mod default_externs;
pub mod guava;
pub mod jar;
pub mod jar_contents;
pub mod js;
pub mod resources;
