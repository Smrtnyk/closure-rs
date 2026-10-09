/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TranspilationUtil.java.

//! Port of `TranspilationUtil.java`: static utilities for transpilation passes.

use crate::{AbstractCompiler, diagnostic_type::DiagnosticType, js_error::JSError};
use closure_rhino::node::NodeId;

// port: TranspilationUtil
pub struct TranspilationUtil;

// port: TranspilationUtil#CANNOT_CONVERT
pub static CANNOT_CONVERT: DiagnosticType =
    DiagnosticType::error("JSC_CANNOT_CONVERT", "This code cannot be transpiled. {0}");

// TODO(tbreisacher): Remove this once we have implemented transpilation for all the features
// we intend to support.
// port: TranspilationUtil#CANNOT_CONVERT_YET
pub static CANNOT_CONVERT_YET: DiagnosticType = DiagnosticType::error(
    "JSC_CANNOT_CONVERT_YET",
    "Transpilation of ''{0}'' is not yet implemented.",
);

impl TranspilationUtil {
    // port: TranspilationUtil#cannotConvert
    pub fn cannot_convert(compiler: &mut AbstractCompiler, n: NodeId, message: &str) {
        compiler.report(JSError::make(compiler, n, &CANNOT_CONVERT, &[message]));
    }

    /// Warns the user that the given feature cannot be transpiled because the transpilation is
    /// not yet implemented. A call to this method is essentially a "TODO(tbreisacher): Implement
    /// `feature`" comment.
    // port: TranspilationUtil#cannotConvertYet
    pub fn cannot_convert_yet(compiler: &mut AbstractCompiler, n: NodeId, feature: &str) {
        compiler.report(JSError::make(compiler, n, &CANNOT_CONVERT_YET, &[feature]));
    }

    // port: TranspilationUtil#preloadTranspilationRuntimeFunction
    pub fn preload_transpilation_runtime_function(compiler: &mut AbstractCompiler, function: &str) {
        let resource_name = ["es6/util/", &function.to_lowercase()].join("");
        compiler
            .get_runtime_js_lib_manager()
            .lock()
            .unwrap()
            .ensure_library_injected(compiler, &resource_name, false);
    }
}
