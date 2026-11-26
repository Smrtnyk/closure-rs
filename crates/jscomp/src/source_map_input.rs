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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/SourceMapInput.java.

use crate::{
    diagnostic_type::DiagnosticType, error_manager::ErrorManager, js_error::JSError,
    source_file::SourceFile,
};
use closure_rhino::{js_string::JsString, static_source_file::StaticSourceFile};
use closure_sourcemap::source_map_consumer_v3::SourceMapConsumerV3;
use std::sync::{Arc, Condvar, Mutex};
pub static SOURCEMAP_RESOLVE_FAILED: DiagnosticType = DiagnosticType::warning(
    "SOURCEMAP_RESOLVE_FAILED",
    "Failed to resolve sourcemap at {0}: {1}",
);
pub static SOURCEMAP_PARSE_FAILED: DiagnosticType = DiagnosticType::warning(
    "SOURCEMAP_PARSE_FAILED",
    "Failed to parse malformed sourcemap in {0}: {1}",
);
enum DecodeError {
    Resolve(String),
    Parse(String),
}
pub struct SourceMapInput {
    source_file: Arc<SourceFile>,
    state: Mutex<State>,
    ready: Condvar,
}
struct State {
    parsed_source_map: Option<Arc<SourceMapConsumerV3>>,
    decode_error: Option<DecodeError>,
    in_progress: bool,
    decoded: bool,
    cached: bool,
    /// Rust-only: the decode succeeded (`decode_error` is taken when it is reported).
    decoded_ok: bool,
}
impl SourceMapInput {
    // port: SourceMapInput#SourceMapInput
    pub fn new(source_file: Arc<SourceFile>) -> Self {
        Self {
            source_file,
            state: Mutex::new(State {
                parsed_source_map: None,
                decode_error: None,
                in_progress: false,
                decoded: false,
                cached: false,
                decoded_ok: false,
            }),
            ready: Condvar::new(),
        }
    }
    fn decode_raw(&self) -> Result<Option<Arc<SourceMapConsumerV3>>, DecodeError> {
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
                        Ok(if default_map {
                            None
                        } else {
                            Some(Arc::new(consumer))
                        })
                    }
                    Err(e) => Err(DecodeError::Parse(e.get_message().to_string())),
                }
            }
            Err(e) => Err(DecodeError::Resolve(e.to_string())),
        }
    }
    /// Rust-only (performance divergence): decodes this input source map without reporting errors
    /// so a worker thread can parse it while the compiler runs. Any decode error is replayed when
    /// `get_source_map` is first called on the compiler thread.
    pub(crate) fn predecode_into_cache(&self) {
        {
            let mut state = self.state.lock().unwrap();
            if state.decoded || state.in_progress {
                return;
            }
            state.in_progress = true;
        }
        let decoded = self.decode_raw();
        let mut state = self.state.lock().unwrap();
        match decoded {
            Ok(map) => {
                state.parsed_source_map = map;
                state.decode_error = None;
                state.cached = true;
                state.decoded_ok = true;
            }
            Err(err) => {
                state.parsed_source_map = None;
                state.decode_error = Some(err);
            }
        }
        state.decoded = true;
        state.in_progress = false;
        self.ready.notify_all();
    }
    // port: SourceMapInput#getSourceMap
    pub fn get_source_map(
        &self,
        error_manager: &mut dyn ErrorManager,
    ) -> Option<Arc<SourceMapConsumerV3>> {
        let mut state = self.state.lock().unwrap();
        while state.in_progress {
            state = self.ready.wait(state).unwrap();
        }
        if !state.cached {
            if !state.decoded {
                let decoded = self.decode_raw();
                state.decoded = true;
                match decoded {
                    Ok(map) => {
                        state.parsed_source_map = map;
                        state.decode_error = None;
                        state.decoded_ok = true;
                    }
                    Err(err) => {
                        state.parsed_source_map = None;
                        state.decode_error = Some(err);
                    }
                }
            }
            state.cached = true;
            if let Some(err) = state.decode_error.take() {
                let source_map_path = self.source_file.get_name();
                let error = match err {
                    DecodeError::Parse(message) => JSError::make_without_location(
                        &SOURCEMAP_PARSE_FAILED,
                        &[source_map_path, &message],
                    ),
                    DecodeError::Resolve(message) => JSError::make_without_location(
                        &SOURCEMAP_RESOLVE_FAILED,
                        &[source_map_path, &message],
                    ),
                };
                error_manager.report(error.default_level(), error);
            }
        }
        state.parsed_source_map.clone()
    }
    /// Rust-only (performance divergence): takes the decoded map of `earlier`, an input source map
    /// of the same input that this one replaces, when `earlier` decoded without an error and its
    /// text is the text of this one (typically `--source_map_input` and the file named by the
    /// input's `//# sourceMappingURL` comment, which Java reads and decodes twice). Decoding
    /// depends on the text alone, so this map is then in the state a successful decode leaves
    /// (with nothing to report), sharing the immutable consumer. Waits for a decode of `earlier`
    /// in progress on a worker. With `decode_earlier` (the compiler decodes both maps either way)
    /// it first decodes `earlier` as the worker would, reporting nothing, when no decode of it
    /// has started. Does nothing when this map could not be read: its own decode reports that.
    /// `CLOSURE_RS_SHARED_INPUT_MAPS=off` disables it.
    pub(crate) fn adopt_decoded_from(&self, earlier: &SourceMapInput, decode_earlier: bool) {
        static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
            std::env::var("CLOSURE_RS_SHARED_INPUT_MAPS").as_deref() != Ok("off")
        });
        if !*ENABLED || std::ptr::eq(self, earlier) {
            return;
        }
        if decode_earlier {
            earlier.predecode_into_cache();
        }
        let map = {
            let mut state = earlier.state.lock().unwrap();
            while state.in_progress {
                state = earlier.ready.wait(state).unwrap();
            }
            if !state.decoded_ok {
                return;
            }
            state.parsed_source_map.clone()
        };
        let (Ok(text), Ok(earlier_text)) =
            (self.source_file.get_code(), earlier.source_file.get_code())
        else {
            return;
        };
        if text != earlier_text {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if state.decoded || state.in_progress {
            return;
        }
        state.parsed_source_map = map;
        state.decode_error = None;
        state.decoded = true;
        state.cached = true;
        state.decoded_ok = true;
    }
    /// Rust-only: `getSourceMap()` once it has been parsed (it then reports nothing), else
    /// `None`.
    pub fn get_cached_source_map(&self) -> Option<Option<Arc<SourceMapConsumerV3>>> {
        let state = self.state.lock().unwrap();
        state.cached.then(|| state.parsed_source_map.clone())
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
                decode_error: None,
                in_progress: false,
                decoded: fields.cached,
                cached: fields.cached,
                decoded_ok: false,
            }),
            ready: Condvar::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorting_error_manager::SortingErrorManager;

    const MAP: &str = r#"{"version":3,"file":"a.js","sources":["s.js"],"sourcesContent":["x"],"names":["n"],"mappings":"AAAAA"}"#;

    fn input(name: &str, text: &str) -> SourceMapInput {
        SourceMapInput::new(Arc::new(SourceFile::from_code(name, text)))
    }

    #[test]
    fn adopts_a_successful_decode_of_the_same_text_only() {
        let mut errors = SortingErrorManager::new(Vec::new());
        let earlier = input("a.js.map", MAP);
        let decoded = earlier.get_source_map(&mut errors).unwrap();
        // The same text in another file: shares the consumer, reports nothing.
        let same = input("./a.js.map", MAP);
        same.adopt_decoded_from(&earlier, false);
        let shared = same.get_cached_source_map().unwrap().unwrap();
        assert!(Arc::ptr_eq(&decoded, &shared));
        assert!(Arc::ptr_eq(
            &same.get_source_map(&mut errors).unwrap(),
            &decoded
        ));
        // Another text decodes itself.
        let other = input("a.js.map", &MAP.replace("AAAAA", "AACAA"));
        other.adopt_decoded_from(&earlier, false);
        assert!(other.get_cached_source_map().is_none());
        assert!(other.get_source_map(&mut errors).is_some());
        // A failed decode is not shared: each map reports its own error (the sorting manager
        // drops a repeated error, so the files differ).
        let broken = input("b.js.map", "{");
        assert!(broken.get_source_map(&mut errors).is_none());
        let again = input("c.js.map", "{");
        again.adopt_decoded_from(&broken, true);
        assert!(again.get_cached_source_map().is_none());
        assert!(again.get_source_map(&mut errors).is_none());
        assert_eq!(errors.get_warning_count(), 2);
        assert_eq!(errors.get_error_count(), 0);
    }
}
