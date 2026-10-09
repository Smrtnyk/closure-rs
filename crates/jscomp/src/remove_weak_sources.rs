/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RemoveWeakSources.java.

//! Port of `RemoveWeakSources.java`.
use crate::{
    abstract_compiler::AbstractCompiler, compiler_pass::CompilerPass, node_util::NodeUtil,
};
use closure_rhino::{check_state, node::NodeId};

/// Removes the contents of weak sources, keeping the (empty) scripts.
#[derive(Debug, Default)]
pub struct RemoveWeakSources;

impl RemoveWeakSources {
    // port: RemoveWeakSources#RemoveWeakSources
    pub fn new() -> Self {
        Self
    }
}

impl CompilerPass for RemoveWeakSources {
    // port: RemoveWeakSources#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut script = root.get_first_child(compiler);
        while let Some(s) = script {
            check_state!(s.is_script(compiler));
            if s.get_static_source_file(compiler).unwrap().is_weak() {
                // Keep the file but remove the contents, since some users expect the number of
                // input and output files to be the same.
                NodeUtil::delete_children(compiler, s);
            }
            script = s.get_next(compiler);
        }
    }
}
