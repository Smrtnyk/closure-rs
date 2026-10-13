/*
 * Copyright 2016 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 1b38218:
//   src/com/google/javascript/jscomp/SourceFileMapping.java.

use closure_rhino::js_string::JsString;
use closure_sourcemap::{
    proto::mapping::OriginalMapping, source_map_consumer_v3::SourceMapConsumerV3,
};
use std::sync::Arc;
/// Rust-only, not in Java (performance divergence): what
/// `SourceFileMapping::get_source_mapping_cached` keeps from one call to the next, for a caller
/// that asks about one node after another of the same file.
#[derive(Default)]
pub struct SourceMappingCache {
    /// The file last asked about and its parsed input map (`None`: it has none that maps).
    pub file: Option<(JsString, Option<Arc<SourceMapConsumerV3>>)>,
    /// The original file that map last named, and the path resolved for it.
    pub resolved: Option<(JsString, JsString)>,
}
/// A SourceFileMapping maps a source file, line, and column into an {@link OriginalMapping}.
///
/// @see com.google.debugging.sourcemap.SourceMapping
pub trait SourceFileMapping {
    /// Returns the original mapping for the file name, line number and column position found in the
    /// source map. Returns {@code null} if the source file does not have a input map, and returns
    /// {@code OriginalMapping.getDefaultInstance()} if the input map does not map the location.
    ///
    /// @param lineNo The line number, 1-based.
    /// @param columnNo The column index, 1-based.
    // port: SourceFileMapping#getSourceMapping
    fn get_source_mapping(
        &self,
        file_name: &JsString,
        line_no: i32,
        column_no: i32,
    ) -> Option<OriginalMapping>;
    /// Rust-only, not in Java (performance divergence): `get_source_mapping` for a caller that asks
    /// about one node after another (the output source map), keeping in `cache` what the last call
    /// found, so that the file's input map and the resolved path are not looked up for every
    /// node. The result is the one `get_source_mapping` gives.
    fn get_source_mapping_cached(
        &self,
        cache: &mut SourceMappingCache,
        file_name: &JsString,
        line_no: i32,
        column_no: i32,
    ) -> Option<OriginalMapping> {
        let _ = cache;
        self.get_source_mapping(file_name, line_no, column_no)
    }
}
