/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteLogicalAssignmentOperatorsPass.java.

//! Port of `RewriteLogicalAssignmentOperatorsPass.java`.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    rewrite_logical_assignment_operators_helper::RewriteLogicalAssignmentOperatorsHelper,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{node::NodeId, token::Token};
use std::rc::Rc;

/// Replaces the ES2020 `||=`, `&&=`, and `??=` operators.
pub struct RewriteLogicalAssignmentOperatorsPass {
    rewrite_logical_assignment_operators_helper: RewriteLogicalAssignmentOperatorsHelper,
}

impl RewriteLogicalAssignmentOperatorsPass {
    // port: RewriteLogicalAssignmentOperatorsPass#RewriteLogicalAssignmentOperatorsPass
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        // Java passes compiler.getUniqueIdSupplier(); the helper reaches it through the compiler.
        Self {
            rewrite_logical_assignment_operators_helper:
                RewriteLogicalAssignmentOperatorsHelper::new(Rc::new(compiler.create_ast_factory())),
        }
    }
}

impl CompilerPass for RewriteLogicalAssignmentOperatorsPass {
    // port: RewriteLogicalAssignmentOperatorsPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        TranspilationPasses::maybe_mark_feature_as_transpiled_away(
            compiler,
            root,
            Feature::LOGICAL_ASSIGNMENT,
        );
    }
}

impl Callback for RewriteLogicalAssignmentOperatorsPass {
    // port: RewriteLogicalAssignmentOperatorsPass#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            let script_features = NodeUtil::get_feature_set_of_script(t, n);
            // runs only if logical assignment present
            return script_features.is_none()
                || script_features
                    .unwrap()
                    .contains(Feature::LOGICAL_ASSIGNMENT);
        }
        true
    }

    // port: RewriteLogicalAssignmentOperatorsPass#visit
    fn visit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        logical_assignment: NodeId,
        _parent: Option<NodeId>,
    ) {
        match logical_assignment.get_token(t) {
            Token::ASSIGN_OR | Token::ASSIGN_AND | Token::ASSIGN_COALESCE => self
                .rewrite_logical_assignment_operators_helper
                .visit_logical_assignment_operator(t, logical_assignment),
            _ => {}
        }
    }
}
