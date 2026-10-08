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
//   src/com/google/javascript/jscomp/RewriteCatchWithNoBinding.java.

//! Port of `RewriteCatchWithNoBinding.java`.
use crate::{
    AbstractCompiler, abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    ast_factory::AstFactory, node_util::NodeUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::node::NodeId;

/// Transpiles catch statements with no bindings by adding an unused binding.
///
/// ```text
/// try {
///   stuff();
/// } catch {
///   onError();
/// }
/// ```
///
/// Becomes
///
/// ```text
/// try {
///   stuff();
/// } catch ($jscomp$unused$catch) {
///   onError();
/// }
/// ```
pub struct RewriteCatchWithNoBinding {
    ast_factory: AstFactory,
}

// port: RewriteCatchWithNoBinding#BINDING_NAME
const BINDING_NAME: &str = "$jscomp$unused$catch$";

impl RewriteCatchWithNoBinding {
    // port: RewriteCatchWithNoBinding#RewriteCatchWithNoBinding
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        // Java also caches compiler.getUniqueIdSupplier(); the compiler owns it (DESIGN §6), so
        // transpileSubtree reads it from the compiler at each use.
        Self {
            ast_factory: compiler.create_ast_factory(),
        }
    }
}

impl AbstractPeepholeTranspilation for RewriteCatchWithNoBinding {
    // port: RewriteCatchWithNoBinding#getTranspiledAwayFeatures
    fn get_transpiled_away_features(&self) -> FeatureSet {
        FeatureSet::BARE_MINIMUM.with(Feature::OPTIONAL_CATCH_BINDING)
    }

    // port: RewriteCatchWithNoBinding#transpileSubtree
    fn transpile_subtree(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> Option<NodeId> {
        if !n.is_catch(compiler) || !n.get_first_child(compiler).unwrap().is_empty(compiler) {
            return Some(n);
        }

        let input = compiler
            .get_input(
                &NodeUtil::get_input_id(compiler, n).expect("java.lang.NullPointerException"),
            )
            .cloned()
            .expect("java.lang.NullPointerException");
        let unique_id = compiler.get_unique_id_supplier().get_unique_id(&input);
        let name = self
            .ast_factory
            .create_name_with_unknown_type(compiler, format!("{BINDING_NAME}{unique_id}"));
        let first = n.get_first_child(compiler).unwrap();
        let name = name.srcref_tree(compiler, first);
        first.replace_with(compiler, name);
        compiler.report_change_to_enclosing_scope(name);
        Some(n)
    }

    fn get_simple_name(&self) -> &'static str {
        "RewriteCatchWithNoBinding"
    }
}
