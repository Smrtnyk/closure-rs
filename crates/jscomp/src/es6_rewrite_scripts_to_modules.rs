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
//   src/com/google/javascript/jscomp/Es6RewriteScriptsToModules.java.

use crate::{
    Compiler,
    compiler_input::ModuleType,
    compiler_pass::CompilerPass,
    es6_rewrite_modules::Es6RewriteModules,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{check_argument, node::NodeId, token::Token};
pub struct Es6RewriteScriptsToModules;
impl Es6RewriteScriptsToModules {
    // port: Es6RewriteScriptsToModules#Es6RewriteScriptsToModules
    pub fn new() -> Self {
        Self
    }
    // port: Es6RewriteScriptsToModules#forceToEs6Module
    pub fn force_to_es6_module(&mut self, compiler: &mut Compiler, root: NodeId) {
        if Es6RewriteModules::is_es6_module_root(compiler, root) {
            return;
        }
        let module_node = compiler.new_node(Token::MODULE_BODY).srcref(compiler, root);
        let children = root.remove_children(compiler);
        module_node.add_children_to_back(compiler, children);
        root.add_child_to_back(compiler, module_node);
        compiler.report_change_to_change_scope(root);
    }
}
impl CompilerPass for Es6RewriteScriptsToModules {
    // port: Es6RewriteScriptsToModules#process
    fn process(&mut self, compiler: &mut Compiler, _externs: NodeId, root: NodeId) {
        let mut file = root.get_first_child(compiler);
        while let Some(n) = file {
            check_argument!(n.is_script(compiler));
            NodeTraversal::traverse(compiler, n, self);
            file = n.get_next(compiler);
        }
    }
}
impl Callback for Es6RewriteScriptsToModules {
    // port: Es6RewriteScriptsToModules#shouldTraverse
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let compiler = node_traversal.get_compiler();
        if n.is_script(compiler) {
            let id = n.get_input_id(compiler).unwrap();
            let module_type = compiler.get_input(&id).unwrap().get_js_module_type();
            if module_type == ModuleType::IMPORTED_SCRIPT {
                self.force_to_es6_module(compiler, n);
            }
        }
        false
    }
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}
impl Default for Es6RewriteScriptsToModules {
    fn default() -> Self {
        Self::new()
    }
}
