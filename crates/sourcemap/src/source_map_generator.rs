/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/debugging/sourcemap/SourceMapGenerator.java.

use crate::{file_position::FilePosition, source_map_section::SourceMapSection};
use closure_rhino::js_string::JsString;
pub trait SourceMapGenerator {
    // port: SourceMapGenerator#appendTo
    fn append_to(
        &mut self,
        out: &mut dyn std::fmt::Write,
        name: Option<JsString>,
    ) -> Result<JsString, std::fmt::Error>;
    // port: SourceMapGenerator#appendIndexMapTo
    fn append_index_map_to(
        &self,
        out: &mut dyn std::fmt::Write,
        name: JsString,
        sections: &[SourceMapSection],
    ) -> Result<JsString, std::fmt::Error>;
    // port: SourceMapGenerator#reset
    fn reset(&mut self);
    // port: SourceMapGenerator#addMapping
    fn add_mapping(
        &mut self,
        source_name: Option<JsString>,
        symbol_name: Option<JsString>,
        source_start_position: FilePosition,
        start_position: FilePosition,
        end_position: FilePosition,
    );
    // port: SourceMapGenerator#addSourcesContent
    fn add_sources_content(&mut self, source: JsString, content: Option<JsString>);
    // port: SourceMapGenerator#validate
    fn validate(&mut self, validate: bool);
    // port: SourceMapGenerator#setWrapperPrefix
    fn set_wrapper_prefix(&mut self, prefix: JsString);
    // port: SourceMapGenerator#setStartingPosition
    fn set_starting_position(&mut self, offset_line: i32, offset_index: i32);
}
