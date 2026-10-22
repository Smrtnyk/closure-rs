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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit e751d6f:
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
    /// Rust-only: the file name of the last `SourceFileMapping::get_source_mapping` call and its
    /// conversion to a Rust string.
    last_mapping_name: Mutex<Option<(closure_rhino::js_string::JsString, String)>>,
}
#[derive(Default)]
struct ResolvedSourceMap {
    original_path: String,
    source_map_path: String,
    relative_path: String,
    /// Rust-only: `source_map_path` and `relative_path` as JS strings, so that a mapping in the
    /// file last resolved needs no string conversion (performance divergence).
    source_map_path_js: closure_rhino::js_string::JsString,
    relative_path_js: closure_rhino::js_string::JsString,
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
        // Rust-only fast path: a map parsed before reports nothing more (performance divergence).
        if let Some(consumer) = map.get_cached_source_map() {
            return consumer;
        }
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
        let mut cache = self.resolved_source_map.lock().unwrap();
        // Rust-only: compares the result's file as a JS string, so the common case (the file
        // last resolved) converts no string (performance divergence).
        let relative_path = if source_map_original_path == cache.original_path
            && result.get_original_file() == cache.source_map_path_js
        {
            cache.relative_path_js.clone()
        } else {
            let result_original_path = result.get_original_file().to_string_lossy();
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
            cache.source_map_path_js = result.get_original_file().clone();
            cache.relative_path_js = relative.as_str().into();
            cache.relative_path = relative;
            cache.relative_path_js.clone()
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
use crate::source_file_mapping::SourceFileMapping;
impl SourceFileMapping for Arc<CompilerSourceExcerptProvider> {
    fn get_source_mapping(
        &self,
        name: &closure_rhino::js_string::JsString,
        line: i32,
        column: i32,
    ) -> Option<OriginalMapping> {
        // Rust-only: the name converted for the previous call is reused (performance divergence).
        let mut last = self.last_mapping_name.lock().unwrap();
        if last.as_ref().is_none_or(|(js, _)| js != name) {
            *last = Some((name.clone(), name.to_string_lossy()));
        }
        let name = &last.as_ref().unwrap().1;
        SourceExcerptProvider::get_source_mapping(self.as_ref(), Some(name), line, column)
    }
    fn get_source_mapping_cached(
        &self,
        cache: &mut crate::source_file_mapping::SourceMappingCache,
        name: &closure_rhino::js_string::JsString,
        line: i32,
        column: i32,
    ) -> Option<OriginalMapping> {
        let consumer = match &cache.file {
            Some((cached_name, consumer)) if cached_name == name => consumer.as_ref(),
            _ => {
                // The first node of a file: the lookup above, which parses the input map and
                // reports its errors, then what it parsed for the next nodes.
                let mapping = SourceFileMapping::get_source_mapping(self, name, line, column);
                let consumer = self
                    .input_source_maps
                    .lock()
                    .unwrap()
                    .get(&name.to_string_lossy())
                    .and_then(|map| map.get_cached_source_map().flatten());
                cache.file = Some((name.clone(), consumer));
                cache.resolved = None;
                return mapping;
            }
        };
        let consumer = consumer?;
        let Some(result) = consumer.get_mapping_for_line(line, column.wrapping_add(1)) else {
            return Some(OriginalMapping::get_default_instance().clone());
        };
        let original_file = result.get_original_file();
        let relative_path = match &cache.resolved {
            Some((resolved_file, relative_path)) if *resolved_file == original_file => {
                relative_path.clone()
            }
            _ => {
                // The first node mapped to this original file: the lookup above resolves the
                // path (and registers the original source), then what it resolved.
                let mapping = SourceFileMapping::get_source_mapping(self, name, line, column);
                let resolved = self.resolved_source_map.lock().unwrap();
                cache.resolved = Some((
                    resolved.source_map_path_js.clone(),
                    resolved.relative_path_js.clone(),
                ));
                return mapping;
            }
        };
        // The mapping `get_source_mapping` builds from `result`, made once
        // (performance divergence).
        let mut mapping = OriginalMapping::new_builder();
        mapping
            .set_original_file(relative_path)
            .set_column_position(result.get_column_position().wrapping_sub(1));
        if result.has_line_number() {
            mapping.set_line_number(result.get_line_number());
        }
        if result.has_identifier() {
            mapping.set_identifier(result.get_identifier());
        }
        if result.has_precision() {
            mapping.set_precision(result.get_precision());
        }
        Some(mapping.finish())
    }
}
