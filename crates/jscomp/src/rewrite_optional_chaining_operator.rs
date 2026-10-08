/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteOptionalChainingOperator.java.

//! Port of `RewriteOptionalChainingOperator.java`.
use crate::{
    AbstractCompiler,
    compiler_input::CompilerInput,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    optional_chain_rewriter::{self, OptionalChainRewriter, TmpVarNameCreator},
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::node::NodeId;
use std::sync::Arc;

/// `Function<CompilerInput, TmpVarNameCreator>`. Java hands `t.getInput()`, which may be null.
pub type TmpVarNameCreatorForInput =
    Arc<dyn Fn(Option<CompilerInput>) -> Arc<dyn TmpVarNameCreator + Send + Sync> + Send + Sync>;

/// Replaces the ES2020 `?.` operator with conditional (? :).
pub struct RewriteOptionalChainingOperator {
    /// Produces the object responsible for creating temporary variable names for a given
    /// CompilerInput.
    get_tmp_var_name_creator_for_input: TmpVarNameCreatorForInput,
}

impl RewriteOptionalChainingOperator {
    /// Constructor to be used for actual transpilation
    // port: RewriteOptionalChainingOperator#RewriteOptionalChainingOperator(AbstractCompiler)
    pub fn new(_compiler: &mut AbstractCompiler) -> Self {
        // Temporary variable names are generated based on the compiler input they will go into and
        // use a recognizable prefix to aid debugging. (Java captures
        // compiler.getUniqueIdSupplier(); the compiler owns it and is passed to the creator.)
        Self {
            get_tmp_var_name_creator_for_input: Arc::new(|input: Option<CompilerInput>| {
                Arc::new(move |compiler: &mut AbstractCompiler| {
                    let input = input.as_ref().expect("NullPointerException: input");
                    format!(
                        "$jscomp$optchain$tmp{}",
                        compiler.get_unique_id_supplier().get_unique_id(input)
                    )
                })
            }),
        }
    }

    /// Constructor for testing.
    // port: RewriteOptionalChainingOperator#RewriteOptionalChainingOperator(AbstractCompiler, TmpVarNameCreator)
    pub fn new_with_tmp_var_name_creator(
        _compiler: &mut AbstractCompiler,
        tmp_var_name_creator: Arc<dyn TmpVarNameCreator + Send + Sync>,
    ) -> Self {
        // The test provides a simple variable name creator that makes for more readable test
        // cases.
        Self {
            get_tmp_var_name_creator_for_input: Arc::new(move |_input: Option<CompilerInput>| {
                tmp_var_name_creator.clone()
            }),
        }
    }
}

impl CompilerPass for RewriteOptionalChainingOperator {
    // port: RewriteOptionalChainingOperator#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut callback = TranspilationCallback {
            get_tmp_var_name_creator_for_input: self.get_tmp_var_name_creator_for_input.clone(),
            rewriter_builder: OptionalChainRewriter::builder(compiler),
            opt_chains: Vec::new(),
        };
        NodeTraversal::traverse_roots(compiler, &mut callback, externs, root);
        TranspilationPasses::maybe_mark_feature_as_transpiled_away(
            compiler,
            root,
            Feature::OPTIONAL_CHAINING,
        );
    }
}

/// Locates and transpiles all optional chains.
struct TranspilationCallback {
    get_tmp_var_name_creator_for_input: TmpVarNameCreatorForInput,
    rewriter_builder: optional_chain_rewriter::Builder,
    opt_chains: Vec<OptionalChainRewriter>,
}

impl Callback for TranspilationCallback {
    // port: RewriteOptionalChainingOperator.TranspilationCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            // Set the TmpVarNameCreator to be used when rewriting optional chains in this script.
            let input = t.get_input().cloned();
            self.rewriter_builder
                .set_tmp_var_name_creator((self.get_tmp_var_name_creator_for_input)(input));
            let script_features = NodeUtil::get_feature_set_of_script(t, n);
            // ensures that the pass early exits if script does not contain the feature
            return script_features.is_none()
                || script_features
                    .unwrap()
                    .contains(Feature::OPTIONAL_CHAINING);
        }
        true
    }

    // port: RewriteOptionalChainingOperator.TranspilationCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if NodeUtil::is_end_of_full_opt_chain(t, n) {
            let rewriter = self.rewriter_builder.build(t.get_compiler(), n);
            self.opt_chains.push(rewriter);
        } else if n.is_script(t) {
            // We transpile all of the optional chains in a single script as a batch because,
            // rewriting changes the AST in ways that could interfere with traversal
            // if we change the chains as we visit them.
            if !self.opt_chains.is_empty() {
                for opt_chain in self.opt_chains.iter_mut() {
                    opt_chain.rewrite(t.get_compiler());
                }
                self.opt_chains.clear();
            }
        }
    }
}
