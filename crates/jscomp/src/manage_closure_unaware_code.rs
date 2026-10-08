/*
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ManageClosureUnawareCode.java.

//! Port of `ManageClosureUnawareCode.java`.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{
    check_state,
    node::{Ast, NodeId},
};

/// The Java object keeps its AbstractCompiler; Rust passes receive it in `process`.
pub struct ManageClosureUnawareCode;
impl ManageClosureUnawareCode {
    // port: ManageClosureUnawareCode#ManageClosureUnawareCode
    fn new() -> Self {
        Self
    }

    // port: ManageClosureUnawareCode#unwrap
    pub fn unwrap() -> Self {
        Self::new()
    }
}
impl CompilerPass for ManageClosureUnawareCode {
    // port: ManageClosureUnawareCode#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, &mut UnwrapConcealedClosureUnawareCode);
    }
}

// TODO jameswr: Given that the CodePrinter supports printing out the shadow instead of the shadow
// host node, do we even need to revert the AST back to the original form at the end of
// compilation?
struct UnwrapConcealedClosureUnawareCode;
impl Callback for UnwrapConcealedClosureUnawareCode {
    // port: ManageClosureUnawareCode.UnwrapConcealedClosureUnawareCode#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        _n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        let Some(parent) = parent.filter(|parent| parent.is_script(t)) else {
            return true; // keep going
        };

        if parent.is_script(t) && !parent.is_closure_unaware_code(t) {
            return false;
        }

        // Once inside a closureUnaware script, we want to traverse the entire thing to make sure
        // we find all the nodes marked as closure-unaware shadows.
        true
    }

    // port: ManageClosureUnawareCode.UnwrapConcealedClosureUnawareCode#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        Self::try_unwrap_closure_unaware_shadowed_code(t, n);
    }
}
impl UnwrapConcealedClosureUnawareCode {
    // port: ManageClosureUnawareCode.UnwrapConcealedClosureUnawareCode#tryUnwrapClosureUnawareShadowedCode
    fn try_unwrap_closure_unaware_shadowed_code(ast: &mut Ast, n: NodeId) {
        let Some(shadow_ast_root) = n.get_closure_unaware_shadow(ast) else {
            return;
        };

        // ROOT -> SCRIPT -> EXPR_RESULT -> CALL -> FUNCTION
        let shadow_script = shadow_ast_root.get_only_child(ast);
        check_state!(
            shadow_script.is_script(ast),
            "%s",
            shadow_script.to_string(ast)
        );
        check_state!(
            shadow_script.has_one_child(ast),
            "%s",
            shadow_script.to_string(ast)
        );
        let expr_result = shadow_script.get_only_child(ast);
        check_state!(
            expr_result.is_expr_result(ast),
            "%s",
            expr_result.to_string(ast)
        );
        let call_node = expr_result.get_only_child(ast);
        check_state!(call_node.is_call(ast), "%s", call_node.to_string(ast));
        let original_code_function = call_node.get_last_child(ast).unwrap();
        check_state!(
            original_code_function.is_function(ast),
            "%s",
            original_code_function.to_string(ast)
        );
        original_code_function.detach(ast);
        n.replace_with(ast, original_code_function);
    }
}
