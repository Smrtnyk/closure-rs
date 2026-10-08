/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2023 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/RemovePropertyRenamingCalls.java.

//! Port of RemovePropertyRenamingCalls.java.
//!
//! Replaces all calls to property-renaming functions like JSCompiler_renameProperty with the
//! string literal property name.
//!
//! Intended for use when `RenameProperties` isn't running, since this functionality is also baked
//! into `RenameProperties`.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::node::NodeId;

pub struct RemovePropertyRenamingCalls;

impl RemovePropertyRenamingCalls {
    // port: RemovePropertyRenamingCalls#RemovePropertyRenamingCalls
    pub fn new() -> Self {
        Self
    }

    // port: RemovePropertyRenamingCalls#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, &mut RemoveCompilerPropertyRenamingCallback);
    }
}

impl Default for RemovePropertyRenamingCalls {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for RemovePropertyRenamingCalls {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        RemovePropertyRenamingCalls::process(self, compiler, externs, root);
    }
}

struct RemoveCompilerPropertyRenamingCallback;

impl RemoveCompilerPropertyRenamingCallback {
    /// Replaces JSCompiler_renameProperty("bar", obj) with "bar"
    // port: RemovePropertyRenamingCalls.RemoveCompilerPropertyRenamingCallback#replacePropertyRenamingCall
    fn replace_property_renaming_call(compiler: &mut AbstractCompiler, call_node: NodeId) {
        let prop_name = NodeUtil::get_argument_for_call_or_new(compiler, call_node, 0);
        if let Some(prop_name) = prop_name {
            let detached = prop_name.detach(compiler);
            call_node.replace_with(compiler, detached);
            compiler.report_change_to_enclosing_scope(prop_name);
        }
    }
}

impl Callback for RemoveCompilerPropertyRenamingCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RemovePropertyRenamingCalls.RemoveCompilerPropertyRenamingCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_call(t) {
            let fn_ = n.get_first_child(t).unwrap();
            let compiler = t.get_compiler();
            if compiler
                .get_coding_convention()
                .is_property_rename_function(compiler, fn_)
            {
                Self::replace_property_renaming_call(compiler, n);
            }
        }
    }
}
