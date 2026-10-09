/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/J2clSourceFileChecker.java.

//! Port of `J2clSourceFileChecker.java`.
use crate::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass,
    j2cl_source_utils::J2clSourceUtils,
};
use closure_rhino::{
    check_state,
    node::{Ast, NodeId},
};

/// Checks whether the inputs contain J2CL sources and records it on the compiler.
#[derive(Debug, Default)]
pub struct J2clSourceFileChecker;

impl J2clSourceFileChecker {
    // port: J2clSourceFileChecker#J2clSourceFileChecker
    pub fn new() -> Self {
        Self
    }

    // port: J2clSourceFileChecker#hasJ2cl
    fn has_j2cl(ast: &Ast, root: NodeId) -> bool {
        let mut script = root.get_first_child(ast);
        while let Some(s) = script {
            check_state!(s.is_script(ast));
            if J2clSourceUtils::is_j2cl_source_node(ast, s) {
                return true;
            }
            script = s.get_next(ast);
        }
        false
    }

    // port: J2clSourceFileChecker#markToRunJ2clPasses
    pub fn mark_to_run_j2cl_passes(compiler: &mut AbstractCompiler) {
        compiler.set_run_j2cl_passes(true);
    }

    /// Indicates whether it should run future J2CL passes with information from the compiler. For
    /// example, if the compiler's HAS_J2CL annotation is false, it should.
    // port: J2clSourceFileChecker#shouldRunJ2clPasses
    pub fn should_run_j2cl_passes(compiler: &AbstractCompiler) -> bool {
        compiler.run_j2cl_passes()
    }
}

impl CompilerPass for J2clSourceFileChecker {
    // port: J2clSourceFileChecker#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        if Self::has_j2cl(compiler, root) {
            Self::mark_to_run_j2cl_passes(compiler);
        }
    }
}
