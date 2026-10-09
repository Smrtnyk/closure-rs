/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Compiler.java.

use crate::{
    black_hole_error_manager::BlackHoleErrorManager, check_level::CheckLevel, compiler::Compiler,
    error_manager::ErrorManager, region::Region, source_excerpt_provider::SourceExcerptProvider,
    source_file::SourceFile, source_map_input::SourceMapInput,
};
use closure_rhino::fast_hash::IndexMap;
use closure_sourcemap::{
    proto::mapping::OriginalMapping, source_map_consumer_v3::SourceMapConsumerV3,
};
use std::sync::{Arc, Mutex};
#[derive(Default)]
pub(crate) struct CompilerSourceExcerptProvider {
    pub files: Mutex<IndexMap<String, Arc<SourceFile>>>,
    pub original_sources: Arc<Mutex<IndexMap<String, Arc<SourceFile>>>>,
    pub input_source_maps: Arc<Mutex<crate::compiler::InputSourceMaps>>,
    pub pending_errors: crate::sorting_error_manager::DeferredReports,
    resolved_source_map: Mutex<ResolvedSourceMap>,
}
#[derive(Default)]
struct ResolvedSourceMap {
    original_path: String,
    source_map_path: String,
    relative_path: String,
}
impl CompilerSourceExcerptProvider {
    fn get_file(&self, name: Option<&str>) -> Option<Arc<SourceFile>> {
        let name = name?;
        self.files
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .or_else(|| self.original_sources.lock().unwrap().get(name).cloned())
    }
    pub fn get_consumer(&self, map: &SourceMapInput) -> Option<Arc<SourceMapConsumerV3>> {
        // Formatters can run with ErrorManager already borrowed. Collect and replay only the
        // SourceMapInput reports, which Java sends directly to ErrorManager (without guards).
        let mut manager = BlackHoleErrorManager::new();
        let consumer = map.get_source_map(&mut manager);
        self.pending_errors.lock().unwrap().extend(
            manager
                .get_errors()
                .into_iter()
                .map(|error| (CheckLevel::ERROR, error))
                .chain(
                    manager
                        .get_warnings()
                        .into_iter()
                        .map(|error| (CheckLevel::WARNING, error)),
                ),
        );
        consumer
    }
}
impl SourceExcerptProvider for CompilerSourceExcerptProvider {
    // port: Compiler#getSourceLine
    fn get_source_line(&self, name: Option<&str>, line: i32) -> Option<String> {
        if line < 1 {
            return None;
        }
        let file = self.get_file(name)?;
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_line(line)
    }
    // port: Compiler#getSourceLines
    fn get_source_lines(
        &self,
        name: Option<&str>,
        line: i32,
        length: i32,
    ) -> Option<Box<dyn Region>> {
        if line < 1 {
            return None;
        }
        let file = self.get_file(name)?;
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_lines(line, length)
            .map(|region| Box::new(region) as Box<dyn Region>)
    }
    // port: Compiler#getSourceRegion
    fn get_source_region(&self, name: Option<&str>, line: i32) -> Option<Box<dyn Region>> {
        if line < 1 {
            return None;
        }
        let file = self.get_file(name)?;
        if file.is_stub_source_file_for_already_provided_input() {
            return None;
        }
        file.get_region(line)
            .map(|region| Box::new(region) as Box<dyn Region>)
    }
    // port: Compiler#getSourceMapping
    fn get_source_mapping(
        &self,
        name: Option<&str>,
        line: i32,
        column: i32,
    ) -> Option<OriginalMapping> {
        let map = self.input_source_maps.lock().unwrap().get(name?).cloned()?;
        let consumer = self.get_consumer(&map)?;
        let Some(result) = consumer.get_mapping_for_line(line, column.wrapping_add(1)) else {
            return Some(OriginalMapping::get_default_instance().clone());
        };
        let source_map_original_path = map.get_original_path();
        let result_original_path = result.get_original_file().to_string_lossy();
        let mut cache = self.resolved_source_map.lock().unwrap();
        let relative_path = if source_map_original_path == cache.original_path
            && result_original_path == cache.source_map_path
        {
            cache.relative_path.clone()
        } else {
            let relative =
                Compiler::resolve_sibling(source_map_original_path, &result_original_path);
            if self.get_file(Some(&relative)).is_none() && !result_original_path.is_empty() {
                let source = crate::source_map_resolver::SourceMapResolver::get_relative_path(
                    map.get_original_path(),
                    &result_original_path,
                );
                self.original_sources
                    .lock()
                    .unwrap()
                    .entry(relative.clone())
                    .or_insert_with(|| Arc::new(source));
            }
            cache.original_path = source_map_original_path.into();
            cache.source_map_path = result_original_path;
            cache.relative_path = relative.clone();
            relative
        };
        Some(
            result
                .to_builder()
                .set_original_file(relative_path)
                .set_column_position(result.get_column_position().wrapping_sub(1))
                .build(),
        )
    }
}
impl crate::source_file_mapping::SourceFileMapping for Arc<CompilerSourceExcerptProvider> {
    fn get_source_mapping(
        &self,
        name: &closure_rhino::js_string::JsString,
        line: i32,
        column: i32,
    ) -> Option<OriginalMapping> {
        SourceExcerptProvider::get_source_mapping(
            self.as_ref(),
            Some(&name.to_string_lossy()),
            line,
            column,
        )
    }
}
