/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteNewDotTarget.java.

//! Port of `RewriteNewDotTarget.java`.
use crate::{
    AbstractCompiler, abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    ast_factory::AstFactory, node_util::NodeUtil, transpilation_util::TranspilationUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{node::NodeId, token::Token};

/// Transpiles away `new.target`.
pub struct RewriteNewDotTarget {
    ast_factory: AstFactory,
}

impl RewriteNewDotTarget {
    // port: RewriteNewDotTarget#RewriteNewDotTarget
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
        }
    }
}

impl AbstractPeepholeTranspilation for RewriteNewDotTarget {
    // port: RewriteNewDotTarget#getTranspiledAwayFeatures
    fn get_transpiled_away_features(&self) -> FeatureSet {
        FeatureSet::BARE_MINIMUM.with(Feature::NEW_TARGET)
    }

    // port: RewriteNewDotTarget#transpileSubtree
    fn transpile_subtree(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        if n.get_token(compiler) != Token::NEW_TARGET {
            // there's nothing to rewrite
            return Some(n);
        }

        let enclosing_non_arrow_function = NodeUtil::get_enclosing_non_arrow_function(compiler, n);
        if let Some(enclosing_non_arrow_function) = enclosing_non_arrow_function
            && NodeUtil::is_es6_constructor(compiler, enclosing_non_arrow_function)
        {
            // Within an ES6 class constructor that we're about to transpile.
            // `new.target` -> `this.constructor`
            let enclosing_class = enclosing_non_arrow_function
                .get_parent(compiler)
                .unwrap()
                .get_grandparent(compiler)
                .unwrap();
            let this_node = self
                .ast_factory
                .create_this_for_es6_class(compiler, enclosing_class);
            let replacement = self
                .ast_factory
                .create_get_prop(compiler, this_node, "constructor", AstFactory::type_node(n))
                .srcref_tree(compiler, n);
            n.replace_with(compiler, replacement);
            compiler.report_change_to_enclosing_scope(replacement);
            return Some(replacement);
        } else {
            // Getting new.target correct in functions other than transpiled ES6 class
            // constructors requires determining whether the function was called with `new` or
            // not, which is more hassle than its worth. There's no good reason to use
            // `new.target` in such places anyway.
            TranspilationUtil::cannot_convert_yet(compiler, n, "new.target");
        }
        Some(n)
    }

    fn get_simple_name(&self) -> &'static str {
        "RewriteNewDotTarget"
    }
}
