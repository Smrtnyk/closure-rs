/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CompilerPass.java.

use crate::abstract_compiler::AbstractCompiler;
use closure_rhino::node::NodeId;
pub trait CompilerPass {
    // port: CompilerPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId);
    /// Rust-only: the concrete pass, for test harnesses that set a pass's fields as Java's
    /// tests do through its setters (`None` unless the pass opts in).
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}
impl<F> CompilerPass for F
where
    F: FnMut(&mut AbstractCompiler, NodeId, NodeId),
{
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self(compiler, externs, root);
    }
}
