/*
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/debugging/sourcemap/SourceMappingReversable.java.

use crate::{proto::mapping::OriginalMapping, source_mapping::SourceMapping};
use closure_rhino::js_string::JsString;
pub trait SourceMappingReversable: SourceMapping {
    // port: SourceMappingReversable#getOriginalSources
    fn get_original_sources(&self) -> &[Option<JsString>];
    // port: SourceMappingReversable#getReverseMapping
    fn get_reverse_mapping(
        &mut self,
        original_file: Option<&JsString>,
        line: i32,
        column: i32,
    ) -> &[OriginalMapping];
}
