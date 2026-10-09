/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/WhitespaceWrapGoogModules.java.

use crate::{Compiler, node_util::NodeUtil};
use closure_rhino::{check_state, ir::IR, node::NodeId, token::Token};
pub struct WhitespaceWrapGoogModules;
impl WhitespaceWrapGoogModules {
    // port: WhitespaceWrapGoogModules#WhitespaceWrapGoogModules
    pub fn new() -> Self {
        Self
    }
    // port: WhitespaceWrapGoogModules#process
    pub fn process(&mut self, compiler: &mut Compiler, _externs: NodeId, root: NodeId) {
        let mut c = root.get_first_child(compiler);
        while let Some(script) = c {
            check_state!(script.is_script(compiler));
            self.visit_script(compiler, script);
            c = script.get_next(compiler);
        }
    }
    // port: WhitespaceWrapGoogModules#visitScript
    fn visit_script(&mut self, compiler: &mut Compiler, script_root: NodeId) {
        if !NodeUtil::is_goog_module_file(compiler, script_root) {
            return;
        }
        let module_body = script_root.get_first_child(compiler).unwrap();
        module_body.set_token(compiler, Token::BLOCK);
        NodeUtil::try_merge_block(compiler, module_body, true);
        compiler.report_change_to_enclosing_scope(script_root);
        let block = IR::block(compiler);
        let strict = IR::string(compiler, "use strict");
        let strict = IR::expr_result(compiler, strict);
        block.add_child_to_back(compiler, strict);
        let name = IR::name(compiler, "");
        let exports = IR::name(compiler, "exports");
        let params = IR::param_list(compiler, &[exports]);
        let function = IR::function(compiler, name, params, block);
        compiler.report_change_to_change_scope(function);
        let goog = IR::name(compiler, "goog");
        let target = IR::getprop(compiler, goog, "loadModule");
        let call = IR::call(compiler, target, &[function]);
        let load_mod = IR::expr_result(compiler, call).srcref_tree(compiler, script_root);
        if script_root.has_children(compiler) {
            let children = script_root.remove_children(compiler);
            block.add_children_to_back(compiler, children);
        }
        let exports = IR::name(compiler, "exports");
        let ret =
            IR::return_node_with_expression(compiler, exports).srcref_tree(compiler, script_root);
        block.add_child_to_back(compiler, ret);
        script_root.add_child_to_back(compiler, load_mod);
        compiler.report_change_to_change_scope(script_root);
    }
}
impl Default for WhitespaceWrapGoogModules {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::compiler_pass::CompilerPass for WhitespaceWrapGoogModules {
    fn process(&mut self, compiler: &mut Compiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}
