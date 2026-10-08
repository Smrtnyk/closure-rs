/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/SerializationOptions.java.

//! Port of serialization/SerializationOptions.java.

/// port: SerializationOptions
///
/// Configuration options for serialization time.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SerializationOptions {
    include_debug_info: bool,
    run_validation: bool,
    runtime_libraries: Vec<String>,
}

impl SerializationOptions {
    // port: SerializationOptions#<init>
    pub fn new(
        include_debug_info: bool,
        run_validation: bool,
        runtime_libraries: Vec<String>,
    ) -> Self {
        Self {
            include_debug_info,
            run_validation,
            runtime_libraries,
        }
    }
    // port: SerializationOptions#includeDebugInfo
    pub fn include_debug_info(&self) -> bool {
        self.include_debug_info
    }
    // port: SerializationOptions#runValidation
    pub fn run_validation(&self) -> bool {
        self.run_validation
    }
    // port: SerializationOptions#runtimeLibraries
    pub fn runtime_libraries(&self) -> &[String] {
        &self.runtime_libraries
    }
    // port: SerializationOptions#builder
    pub fn builder() -> SerializationOptionsBuilder {
        SerializationOptionsBuilder::default()
            .set_run_validation(false)
            .set_include_debug_info(false)
            .set_runtime_libraries(Vec::new())
    }
}

/// port: SerializationOptions.Builder
#[derive(Clone, Debug, Default)]
pub struct SerializationOptionsBuilder {
    include_debug_info: Option<bool>,
    run_validation: Option<bool>,
    runtime_libraries: Option<Vec<String>>,
}

impl SerializationOptionsBuilder {
    // port: SerializationOptions.Builder#setIncludeDebugInfo
    pub fn set_include_debug_info(mut self, include_debug_info: bool) -> Self {
        self.include_debug_info = Some(include_debug_info);
        self
    }
    // port: SerializationOptions.Builder#setRunValidation
    pub fn set_run_validation(mut self, run_validation: bool) -> Self {
        self.run_validation = Some(run_validation);
        self
    }
    // port: SerializationOptions.Builder#setRuntimeLibraries
    pub fn set_runtime_libraries(mut self, runtime_libraries: Vec<String>) -> Self {
        self.runtime_libraries = Some(runtime_libraries);
        self
    }
    // port: SerializationOptions.Builder#build
    pub fn build(self) -> SerializationOptions {
        SerializationOptions::new(
            self.include_debug_info.unwrap(),
            self.run_validation.unwrap(),
            self.runtime_libraries.expect("runtimeLibraries"),
        )
    }
}
