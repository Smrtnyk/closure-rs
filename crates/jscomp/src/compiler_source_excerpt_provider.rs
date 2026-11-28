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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/Compiler.java.

use crate::{
    black_hole_error_manager::BlackHoleErrorManager, check_level::CheckLevel, compiler::Compiler,
    error_manager::ErrorManager, region::Region, source_excerpt_provider::SourceExcerptProvider,
    source_file::SourceFile, source_map_input::SourceMapInput,
};
use closure_rhino::{fast_hash::IndexMap, js_string::JsString};
use closure_sourcemap::{
    proto::mapping::OriginalMapping, source_map_consumer_v3::SourceMapConsumerV3,
};
use std::sync::{Arc, Mutex};
/// Rust-only (performance divergence): `CLOSURE_RS_SOURCE_PATH_CACHE` for the paths the provider
/// keeps (`relative_paths`): `off` resolves every path anew, as Java does; `verify` resolves each
/// kept path anew as well and panics if it differs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SourcePathCache {
    Off,
    On,
    Verify,
}
static SOURCE_PATH_CACHE: std::sync::LazyLock<SourcePathCache> =
    std::sync::LazyLock::new(
        || match std::env::var("CLOSURE_RS_SOURCE_PATH_CACHE").as_deref() {
            Ok("off") => SourcePathCache::Off,
            Ok("verify") => SourcePathCache::Verify,
            _ => SourcePathCache::On,
        },
    );
/// A path the provider keeps for `base` and `relative` (`None` with
/// `CLOSURE_RS_SOURCE_PATH_CACHE=off`), checked against `resolve` with `=verify`.
fn kept_path(
    kept: &Mutex<IndexMap<String, IndexMap<JsString, String>>>,
    base: &str,
    relative: &JsString,
    resolve: impl FnOnce() -> String,
) -> Option<String> {
    if *SOURCE_PATH_CACHE == SourcePathCache::Off {
        return None;
    }
    let known = kept
        .lock()
        .unwrap()
        .get(base)
        .and_then(|paths| paths.get(relative))
        .cloned()?;
    if *SOURCE_PATH_CACHE == SourcePathCache::Verify {
        let resolved = resolve();
        assert!(
            known == resolved,
            "CLOSURE_RS_SOURCE_PATH_CACHE=verify: base {base:?}, relative {:?}: kept {known:?}, \
             resolved {resolved:?}",
            relative.to_string_lossy()
        );
    }
    Some(known)
}
#[derive(Default)]
pub(crate) struct CompilerSourceExcerptProvider {
    pub files: Mutex<IndexMap<String, Arc<SourceFile>>>,
    pub original_sources: Arc<Mutex<IndexMap<String, Arc<SourceFile>>>>,
    pub input_source_maps: Arc<Mutex<crate::compiler::InputSourceMaps>>,
    pub pending_errors: crate::sorting_error_manager::DeferredReports,
    resolved_source_map: Mutex<ResolvedSourceMap>,
    /// Rust-only (performance divergence): every path `SourceMapResolver#getRelativePath` gave,
    /// by its base path (an input map's) and the relative path (a source the map names): the
    /// names of an input map's sources and the original sources the mappings register.
    relative_paths: Mutex<IndexMap<String, IndexMap<JsString, String>>>,
    /// Rust-only: the file name of the last `SourceFileMapping::get_source_mapping` call and its
    /// conversion to a Rust string.
    last_mapping_name: Mutex<Option<(closure_rhino::js_string::JsString, String)>>,
}
/// Java's `resolvedSourceMap` (the map path, the original file and the path resolved for them,
/// of the last lookup), kept as JS strings so that a mapping in the file last resolved needs no
/// string conversion (Rust-only, performance divergence).
#[derive(Default)]
struct ResolvedSourceMap {
    original_path: String,
    source_map_path_js: closure_rhino::js_string::JsString,
    relative_path_js: closure_rhino::js_string::JsString,
}
impl CompilerSourceExcerptProvider {
    /// `SourceMapResolver.getRelativePath(base_file_path, relative_path)`.
    ///
    /// Rust-only (performance divergence): the path is resolved once for each pair (see
    /// `relative_paths`); the file is built from it as `getRelativePath` builds it.
    pub(crate) fn get_relative_path(
        &self,
        base_file_path: &str,
        relative_path: &JsString,
    ) -> SourceFile {
        let resolve = || {
            crate::source_map_resolver::SourceMapResolver::relative_path_string(
                base_file_path,
                &relative_path.to_string_lossy(),
            )
        };
        let known = kept_path(&self.relative_paths, base_file_path, relative_path, resolve);
        let path = known.unwrap_or_else(|| {
            let path = resolve();
            self.relative_paths
                .lock()
                .unwrap()
                .entry(base_file_path.to_owned())
                .or_default()
                .insert(relative_path.clone(), path.clone());
            path
        });
        crate::source_map_resolver::SourceMapResolver::source_file_at(path)
    }
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
        let result_original_file = result.get_original_file();
        let relative_path = if source_map_original_path == cache.original_path
            && result_original_file == cache.source_map_path_js
        {
            cache.relative_path_js.clone()
        } else {
            let relative = Compiler::resolve_sibling(
                source_map_original_path,
                &result_original_file.to_string_lossy(),
            );
            // Rust-only (performance divergence): a pool string, which every mapping into this
            // file copies without a reference count (see `SourceMapConsumerV3`'s names).
            let relative_js =
                closure_rhino::rhino_string_pool::RhinoStringPool::add_or_get(relative.as_str());
            if self.get_file(Some(&relative)).is_none() && !result_original_file.is_empty() {
                let source = self.get_relative_path(map.get_original_path(), &result_original_file);
                self.original_sources
                    .lock()
                    .unwrap()
                    .entry(relative)
                    .or_insert_with(|| Arc::new(source));
            }
            cache.original_path = source_map_original_path.into();
            cache.source_map_path_js = result_original_file;
            cache.relative_path_js = relative_js;
            cache.relative_path_js.clone()
        };
        Some(with_resolved_file(result, relative_path))
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
        // The lookups below call `SourceExcerptProvider::get_source_mapping` with the name this
        // cache converted, which is what `get_source_mapping` above does with the name it
        // converted (and keeps in `last_mapping_name`, which all threads share).
        let file = match cache.current {
            Some(index) if cache.files.get_index(index).unwrap().0 == name => index,
            _ => match cache.files.get_index_of(name) {
                Some(index) => {
                    cache.current = Some(index);
                    index
                }
                None => {
                    // The first node of a file: the lookup, which parses the input map and
                    // reports its errors, then what it parsed for the next nodes.
                    let rust_name = name.to_string_lossy();
                    let mapping = SourceExcerptProvider::get_source_mapping(
                        self.as_ref(),
                        Some(&rust_name),
                        line,
                        column,
                    );
                    let consumer = self
                        .input_source_maps
                        .lock()
                        .unwrap()
                        .get(&rust_name)
                        .and_then(|map| map.get_cached_source_map().flatten());
                    let (index, _) = cache.files.insert_full(
                        name.clone(),
                        crate::source_file_mapping::FileMappingCache {
                            name: rust_name,
                            consumer,
                            resolved_paths: Vec::new(),
                        },
                    );
                    cache.current = Some(index);
                    return mapping;
                }
            },
        };
        let file = &mut cache.files[file];
        let consumer = file.consumer.as_ref()?;
        // Rust-only (performance divergence): the fields of the entry found, read in place; the
        // mapping is built once, with the resolved path (`with_resolved_file` of the consumer's
        // mapping).
        let Some(entry) = consumer.get_entry_for_line(line, column.wrapping_add(1)) else {
            return Some(OriginalMapping::get_default_instance().clone());
        };
        let Some(Some(relative_path)) = file.resolved_paths.get(entry.source_index) else {
            // The first node mapped to this source: the lookup resolves the path (and registers
            // the original source), then what it resolved. The lookup finds the same entry, with
            // the path resolved for its file (read from the mapping, not from
            // `resolved_source_map`, which a lookup on another thread may have replaced in the
            // meantime).
            let source_index = entry.source_index;
            let mapping = SourceExcerptProvider::get_source_mapping(
                self.as_ref(),
                Some(&file.name),
                line,
                column,
            );
            if let Some(mapping) = &mapping {
                if file.resolved_paths.len() <= source_index {
                    file.resolved_paths.resize(source_index + 1, None);
                }
                file.resolved_paths[source_index] = Some(mapping.get_original_file());
            }
            return mapping;
        };
        let mut builder = OriginalMapping::new_builder();
        builder
            .set_original_file(relative_path.clone())
            .set_line_number(entry.line_number)
            .set_column_position(entry.column_position.wrapping_sub(1))
            .set_precision(entry.precision);
        if let Some(identifier) = entry.identifier {
            builder.set_identifier(identifier.clone());
        }
        Some(builder.finish())
    }
    fn try_clone(&self) -> Option<Box<dyn SourceFileMapping + Send>> {
        // Every lookup locks the state it shares (input maps, resolved paths, original sources)
        // and gives the same result on any thread.
        Some(Box::new(self.clone()))
    }
}

// Rust-only (performance divergence): resolving the file only changes its name and column.
// Transfer the decoded mapping, including the optional identifier and precision, into the builder.
fn with_resolved_file(
    result: OriginalMapping,
    relative_path: closure_rhino::js_string::JsString,
) -> OriginalMapping {
    let column = result.get_column_position().wrapping_sub(1);
    let mut builder = result.into_builder();
    builder
        .set_original_file(relative_path)
        .set_column_position(column);
    builder.finish()
}
