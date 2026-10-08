/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SourceMapInput.java.

use crate::{
    diagnostic_type::DiagnosticType, error_manager::ErrorManager, js_error::JSError,
    source_file::SourceFile,
};
use closure_rhino::{js_string::JsString, static_source_file::StaticSourceFile};
use closure_sourcemap::source_map_consumer_v3::SourceMapConsumerV3;
use std::sync::{Arc, Mutex};
pub static SOURCEMAP_RESOLVE_FAILED: DiagnosticType = DiagnosticType::warning(
    "SOURCEMAP_RESOLVE_FAILED",
    "Failed to resolve sourcemap at {0}: {1}",
);
pub static SOURCEMAP_PARSE_FAILED: DiagnosticType = DiagnosticType::warning(
    "SOURCEMAP_PARSE_FAILED",
    "Failed to parse malformed sourcemap in {0}: {1}",
);
pub struct SourceMapInput {
    source_file: Arc<SourceFile>,
    state: Mutex<State>,
}
struct State {
    parsed_source_map: Option<Arc<SourceMapConsumerV3>>,
    cached: bool,
}
impl SourceMapInput {
    // port: SourceMapInput#SourceMapInput
    pub fn new(source_file: Arc<SourceFile>) -> Self {
        Self {
            source_file,
            state: Mutex::new(State {
                parsed_source_map: None,
                cached: false,
            }),
        }
    }
    // port: SourceMapInput#getSourceMap
    pub fn get_source_map(
        &self,
        error_manager: &mut dyn ErrorManager,
    ) -> Option<Arc<SourceMapConsumerV3>> {
        let mut state = self.state.lock().unwrap();
        if !state.cached {
            state.cached = true;
            let source_map_path = self.source_file.get_name();
            match self.source_file.get_code() {
                Ok(source_map_contents) => {
                    let mut consumer = SourceMapConsumerV3::new();
                    match consumer.parse(source_map_contents) {
                        Ok(()) => {
                            let file = consumer.get_file();
                            let default_map = file.as_ref().is_some_and(|file| !file.is_empty())
                                && consumer.get_original_sources().len() == 1
                                && consumer.get_original_sources().contains(&file)
                                && consumer
                                    .get_original_sources_content()
                                    .unwrap_or_else(|| panic!("java.lang.NullPointerException"))
                                    .len()
                                    == 1
                                && consumer.get_original_names().is_empty()
                                && consumer.get_line_count() == -1;
                            state.parsed_source_map = if default_map {
                                None
                            } else {
                                Some(Arc::new(consumer))
                            };
                        }
                        Err(e) => {
                            let message = e.to_string();
                            let error = JSError::make_without_location(
                                &SOURCEMAP_PARSE_FAILED,
                                &[source_map_path, &message],
                            );
                            error_manager.report(error.default_level(), error);
                        }
                    }
                }
                Err(e) => {
                    let message = e.to_string();
                    let error = JSError::make_without_location(
                        &SOURCEMAP_RESOLVE_FAILED,
                        &[source_map_path, &message],
                    );
                    error_manager.report(error.default_level(), error);
                }
            }
        }
        state.parsed_source_map.clone()
    }
    // port: SourceMapInput#getOriginalPath
    pub fn get_original_path(&self) -> &str {
        self.source_file.get_name()
    }
    // port: SourceMapInput#getRawSourceMapContents
    pub fn get_raw_source_map_contents(&self) -> Option<JsString> {
        self.source_file.get_code().ok()
    }
}

// Typed views for java.lang.reflect.Field replay (unit-harness); the instance fields remain private.
pub struct SourceMapInputReplayFields {
    pub source_file: Arc<SourceFile>,
    pub parsed_source_map: Option<Arc<SourceMapConsumerV3>>,
    pub cached: bool,
}
impl SourceMapInput {
    // port: java.lang.reflect.Field#get (native replay access)
    pub fn replay_fields(&self) -> SourceMapInputReplayFields {
        let state = self.state.lock().unwrap();
        SourceMapInputReplayFields {
            source_file: self.source_file.clone(),
            parsed_source_map: state.parsed_source_map.clone(),
            cached: state.cached,
        }
    }
    // port: java.lang.reflect.Field#set (native replay access)
    pub fn from_replay_fields(fields: SourceMapInputReplayFields) -> Self {
        Self {
            source_file: fields.source_file,
            state: Mutex::new(State {
                parsed_source_map: fields.parsed_source_map,
                cached: fields.cached,
            }),
        }
    }
}
