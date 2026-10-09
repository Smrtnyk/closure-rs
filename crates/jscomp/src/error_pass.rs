/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ErrorPass.java.

//! Port of `ErrorPass.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use closure_rhino::node::NodeId;

/// A pass that reports one error when it runs.
#[derive(Debug, Clone)]
pub struct ErrorPass {
    error: JSError,
}

impl ErrorPass {
    // port: ErrorPass#ErrorPass(AbstractCompiler,DiagnosticType)
    pub fn new(compiler: &AbstractCompiler, error: &'static DiagnosticType) -> Self {
        Self::new_with_error(compiler, JSError::make_without_location(error, &[]))
    }

    // port: ErrorPass#ErrorPass(AbstractCompiler,JSError)
    pub fn new_with_error(_compiler: &AbstractCompiler, error: JSError) -> Self {
        Self { error }
    }
}

impl CompilerPass for ErrorPass {
    // port: ErrorPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        compiler.report(self.error.clone());
    }
}
