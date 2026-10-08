// Copyright 2009 Google Inc. All rights reserved.
//
// Protocol Buffer definitions of the various source map structures.
//
// Author: jschorr@google.com (Joseph Schorr)
/*
 * Copyright The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/debugging/sourcemap/proto/mapping.proto.

use closure_rhino::js_string::JsString;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Precision {
    #[default]
    UNKNOWN_PRECISION = 0,
    EXACT = 1,
    APPROXIMATE_LINE = 2,
}
impl Precision {
    // port: OriginalMapping.Precision#getNumber
    pub fn get_number(self) -> i32 {
        self as i32
    }
    // port: OriginalMapping.Precision#forNumber
    pub fn for_number(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::UNKNOWN_PRECISION),
            1 => Some(Self::EXACT),
            2 => Some(Self::APPROXIMATE_LINE),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct OriginalMapping {
    original_file: Option<JsString>,
    line_number: Option<i32>,
    column_position: Option<i32>,
    identifier: Option<JsString>,
    precision: Option<Precision>,
}
#[derive(Clone, Debug, Default)]
pub struct Builder {
    mapping: OriginalMapping,
}
impl OriginalMapping {
    // port: OriginalMapping#getOriginalFile
    pub fn get_original_file(&self) -> JsString {
        self.original_file.clone().unwrap_or_default()
    }
    // port: OriginalMapping#hasOriginalFile
    pub fn has_original_file(&self) -> bool {
        self.original_file.is_some()
    }
    // port: OriginalMapping#getLineNumber
    pub fn get_line_number(&self) -> i32 {
        self.line_number.unwrap_or(0)
    }
    // port: OriginalMapping#hasLineNumber
    pub fn has_line_number(&self) -> bool {
        self.line_number.is_some()
    }
    // port: OriginalMapping#getColumnPosition
    pub fn get_column_position(&self) -> i32 {
        self.column_position.unwrap_or(0)
    }
    // port: OriginalMapping#hasColumnPosition
    pub fn has_column_position(&self) -> bool {
        self.column_position.is_some()
    }
    // port: OriginalMapping#getIdentifier
    pub fn get_identifier(&self) -> JsString {
        self.identifier.clone().unwrap_or_default()
    }
    // port: OriginalMapping#hasIdentifier
    pub fn has_identifier(&self) -> bool {
        self.identifier.is_some()
    }
    // port: OriginalMapping#getPrecision
    pub fn get_precision(&self) -> Precision {
        self.precision.unwrap_or(Precision::UNKNOWN_PRECISION)
    }
    // port: OriginalMapping#hasPrecision
    pub fn has_precision(&self) -> bool {
        self.precision.is_some()
    }
    // port: OriginalMapping#newBuilder
    pub fn new_builder() -> Builder {
        Builder::default()
    }
    // port: OriginalMapping#toBuilder
    pub fn to_builder(&self) -> Builder {
        Builder {
            mapping: self.clone(),
        }
    }
    // port: OriginalMapping#getDefaultInstance
    pub fn get_default_instance() -> &'static Self {
        static INSTANCE: std::sync::LazyLock<OriginalMapping> =
            std::sync::LazyLock::new(OriginalMapping::default);
        &INSTANCE
    }
}
impl Builder {
    // port: OriginalMapping.Builder#setOriginalFile
    pub fn set_original_file(&mut self, value: impl Into<JsString>) -> &mut Self {
        let value = value.into();
        self.mapping.original_file = Some(value);
        self
    }
    // port: OriginalMapping.Builder#setLineNumber
    pub fn set_line_number(&mut self, value: i32) -> &mut Self {
        self.mapping.line_number = Some(value);
        self
    }
    // port: OriginalMapping.Builder#setColumnPosition
    pub fn set_column_position(&mut self, value: i32) -> &mut Self {
        self.mapping.column_position = Some(value);
        self
    }
    // port: OriginalMapping.Builder#setIdentifier
    pub fn set_identifier(&mut self, value: impl Into<JsString>) -> &mut Self {
        let value = value.into();
        self.mapping.identifier = Some(value);
        self
    }
    // port: OriginalMapping.Builder#setPrecision
    pub fn set_precision(&mut self, value: Precision) -> &mut Self {
        self.mapping.precision = Some(value);
        self
    }
    // port: OriginalMapping.Builder#build
    pub fn build(&self) -> OriginalMapping {
        self.mapping.clone()
    }
}
