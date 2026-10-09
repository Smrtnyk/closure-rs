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
//   src/com/google/javascript/jscomp/InjectRuntimeLibraries.java.

//! Adds runtime libraries to the beginning of the AST. Any libraries explicitly requested via the
//! `CompilerOptions#forceLibraryInjection` field. Port of `InjectRuntimeLibraries.java`.
//!
//! TODO(b/120486392): merge this pass with `InjectTranspilationRuntimeLibraries`.
use crate::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    js::runtime_js_lib_manager::RuntimeJsLibManager,
};
use closure_rhino::fast_hash::IndexSet;
use closure_rhino::node::NodeId;
use std::sync::{Arc, Mutex};

// port: InjectRuntimeLibraries
pub struct InjectRuntimeLibraries {
    runtime_libs: Arc<Mutex<RuntimeJsLibManager>>,
    force_injected_libraries: IndexSet<String>,
}

impl InjectRuntimeLibraries {
    // port: InjectRuntimeLibraries#InjectRuntimeLibraries
    pub fn new(
        compiler: &mut AbstractCompiler,
        force_injected_libraries: IndexSet<String>,
    ) -> Self {
        Self {
            runtime_libs: compiler.get_runtime_js_lib_manager(),
            force_injected_libraries,
        }
    }

    // port: InjectRuntimeLibraries#injectLibraries
    fn inject_libraries(&mut self, compiler: &mut AbstractCompiler) {
        for forced in &self.force_injected_libraries {
            self.runtime_libs
                .lock()
                .unwrap()
                .ensure_library_injected(compiler, forced, true);
        }
    }
}

impl CompilerPass for InjectRuntimeLibraries {
    // port: InjectRuntimeLibraries#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        self.inject_libraries(compiler);
    }
}
