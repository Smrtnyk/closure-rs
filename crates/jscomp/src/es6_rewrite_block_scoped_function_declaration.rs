/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Es6RewriteBlockScopedFunctionDeclaration.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `Es6RewriteBlockScopedFunctionDeclaration.java`.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{check_not_null, ir::IR, node::NodeId, token::Token};

// port: Es6RewriteBlockScopedFunctionDeclaration#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with(Feature::BLOCK_SCOPED_FUNCTION_DECLARATION)
}

/// Rewrite block-scoped function declarations as "let"s. This pass must happen before
/// Es6RewriteBlockScopedDeclaration, which rewrites "let" to "var".
pub struct Es6RewriteBlockScopedFunctionDeclaration;

impl Es6RewriteBlockScopedFunctionDeclaration {
    // port: Es6RewriteBlockScopedFunctionDeclaration#Es6RewriteBlockScopedFunctionDeclaration
    pub fn new(_compiler: &mut AbstractCompiler) -> Self {
        Self
    }

    /// Rewrite the function declaration from:
    ///
    /// ```text
    ///   function f() {}
    ///   FUNCTION
    ///     NAME x
    ///     PARAM_LIST
    ///     BLOCK
    /// ```
    ///
    /// to
    ///
    /// ```text
    ///   let f = function() {};
    ///   LET
    ///     NAME f
    ///       FUNCTION
    ///         NAME (w/ empty string)
    ///         PARAM_LIST
    ///         BLOCK
    /// ```
    ///
    /// This is similar to `Normalize.NormalizeStatements#rewriteFunctionDeclaration` but rewrites
    /// to "let" instead of "var".
    // port: Es6RewriteBlockScopedFunctionDeclaration#visitBlockScopedFunctionDeclaration
    fn visit_block_scoped_function_declaration(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: NodeId,
    ) {
        // Prepare a spot for the function.
        let old_name_node = n.get_first_child(t).unwrap();
        let fn_name_node = old_name_node.clone_node(t);
        let r#let = IR::declaration(t, fn_name_node, Token::LET).srcref(t, n);
        let current_script = check_not_null!(t.get_current_script());
        NodeUtil::add_feature_to_script(
            t.get_compiler(),
            current_script,
            Feature::LET_DECLARATIONS,
        );

        // Prepare the function.
        old_name_node.set_string(t, "");
        t.get_compiler()
            .report_change_to_enclosing_scope(old_name_node);

        // Move the function to the front of the parent.
        n.detach(t);
        parent.add_child_to_front(t, r#let);
        t.get_compiler().report_change_to_enclosing_scope(r#let);
        fn_name_node.add_child_to_front(t, n);
    }
}

impl CompilerPass for Es6RewriteBlockScopedFunctionDeclaration {
    // port: Es6RewriteBlockScopedFunctionDeclaration#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for Es6RewriteBlockScopedFunctionDeclaration {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteBlockScopedFunctionDeclaration#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_function(t)
            && let Some(parent) = parent
            && parent.is_block(t)
            && !parent.get_parent(t).unwrap().is_function(t)
        {
            // Only consider declarations (all expressions have non-block parents) that are not
            // directly within a function or top-level.
            self.visit_block_scoped_function_declaration(t, n, parent);
        }
    }
}
