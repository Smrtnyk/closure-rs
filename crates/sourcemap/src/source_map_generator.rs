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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
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
    /// Write the map to its scalar destination without retaining a second UTF-16 result.
    fn write_to(
        &mut self,
        out: &mut dyn std::fmt::Write,
        name: Option<JsString>,
    ) -> std::fmt::Result {
        self.append_to(out, name).map(|_| ())
    }
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
    /// Sets whether to omit a mapped segment whose original position and name are identical to
    /// those of the previous segment on the same generated line. Generators that do not support
    /// this ignore it.
    // port: SourceMapGenerator#setMergeIdenticalSegments
    fn set_merge_identical_segments(&mut self, _merge_identical_segments: bool) {}
    /// Rust-only, not in Java (performance divergence): a copy of this generator in its current
    /// state, so that the maps of several chunks can be built on worker threads from one
    /// prepared generator (`Compiler::prepare_chunk_outputs`). `None`: it cannot be copied.
    fn try_clone(&self) -> Option<Box<dyn SourceMapGenerator + Send>> {
        None
    }
    /// Rust-only, not in Java (performance divergence): room for `additional` more mappings,
    /// for a caller that knows how many it will add (it changes no output). Generators that keep
    /// no list ignore it.
    fn reserve_mappings(&mut self, _additional: usize) {}
}
