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

#![forbid(unsafe_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

pub mod base64;
pub mod base64_vlq;
pub mod file_position;
pub mod gson;
mod java_math;
pub mod java_string;
pub mod proto;
pub mod source_map_consumer;
pub mod source_map_consumer_factory;
pub mod source_map_consumer_v3;
pub mod source_map_format;
pub mod source_map_generator;
pub mod source_map_generator_factory;
pub mod source_map_generator_v3;
pub mod source_map_json_lexer;
pub mod source_map_object;
pub mod source_map_object_parser;
pub mod source_map_parse_exception;
pub mod source_map_section;
pub mod source_map_supplier;
pub mod source_mapping;
pub mod source_mapping_reversable;
pub mod util;

use closure_rhino::js_string::JsString;

#[derive(Debug)]
pub struct JavaException(pub JsString);
impl JavaException {
    pub(crate) fn throw(prefix: &str, value: &JsString, suffix: &str) -> ! {
        std::panic::panic_any(Self(
            JsString::from(prefix).concat(value).concat(&suffix.into()),
        ))
    }
}

pub fn panic_message(p: Box<dyn std::any::Any + Send>) -> JsString {
    if let Some(s) = p.downcast_ref::<JavaException>() {
        s.0.clone()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone().into()
    } else if let Some(s) = p.downcast_ref::<&str>() {
        (*s).into()
    } else {
        "java.lang.RuntimeException".into()
    }
}
