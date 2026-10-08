/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/PeepholeTranspilationsPass.java.

//! Port of `PeepholeTranspilationsPass.java`.
use crate::{
    AbstractCompiler,
    abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::{check_state, node::NodeId, token::Token};

/// A compiler pass to run various peephole transpilations (e.g. rewriteCatchBindings,
/// rewriteNewDotTarget, reportUntranspilableFeatures, es6NormalizeShorthandProperties, etc).
pub struct PeepholeTranspilationsPass {
    // NOTE: Use a native array rather than a List to avoid creating iterators for every node in
    // the AST.
    peephole_transpilations: Vec<Box<dyn AbstractPeepholeTranspilation>>,

    // The feature set to mark as transpiled away after all peephole transpilations are done.
    feature_set_to_mark_as_transpiled_away: FeatureSet,

    // The feature set that triggers this pass. That is, this pass will only run on scripts that
    // contain at least one of these features.
    feature_set_to_run_on: FeatureSet,
}

impl PeepholeTranspilationsPass {
    // port: PeepholeTranspilationsPass#PeepholeTranspilationsPass
    fn new(
        transpilations: Vec<Box<dyn AbstractPeepholeTranspilation>>,
        feature_set_to_run_on: FeatureSet,
        feature_set_to_mark_as_transpiled_away: FeatureSet,
    ) -> Self {
        Self {
            peephole_transpilations: transpilations,
            feature_set_to_run_on,
            feature_set_to_mark_as_transpiled_away,
        }
    }

    /// Creates a peephole optimization pass that runs the given optimizations.
    // port: PeepholeTranspilationsPass#create
    pub fn create(
        _compiler: &AbstractCompiler,
        transpilations: Vec<Box<dyn AbstractPeepholeTranspilation>>,
    ) -> PeepholeTranspilationsPass {
        let mut feature_set_to_run_on = FeatureSet::BARE_MINIMUM;
        let mut feature_set_to_mark_as_transpiled_away = FeatureSet::BARE_MINIMUM;
        // initialize the feature sets to the union of all the features that the peephole
        // transpilations need to run on and the features that they transpile away.
        for transpilation in &transpilations {
            check_state!(
                !transpilation
                    .get_additional_features_to_run_on()
                    .contains_at_least_one_of(transpilation.get_transpiled_away_features()),
                "Transpilation pass %s has getAdditionalFeaturesToRunOn() that contains features that it transpiles away.",
                transpilation.get_simple_name()
            );
            feature_set_to_run_on = feature_set_to_run_on
                .with_feature_set(transpilation.get_transpiled_away_features());
            // some passes need to run on additional features beyond those they transpile away.
            feature_set_to_run_on = feature_set_to_run_on
                .with_feature_set(transpilation.get_additional_features_to_run_on());
            feature_set_to_mark_as_transpiled_away = feature_set_to_mark_as_transpiled_away
                .with_feature_set(transpilation.get_transpiled_away_features());
        }
        PeepholeTranspilationsPass::new(
            transpilations,
            feature_set_to_run_on,
            feature_set_to_mark_as_transpiled_away,
        )
    }
}

impl CompilerPass for PeepholeTranspilationsPass {
    // port: PeepholeTranspilationsPass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut PeepCallback {
                peephole_transpilations: &mut self.peephole_transpilations,
                feature_set_to_run_on: self.feature_set_to_run_on,
            },
        );
        // Update the featureSets with all features that got transpiled.
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            self.feature_set_to_mark_as_transpiled_away,
        );
    }
}

// The Java inner class reads the enclosing pass's fields; here it borrows them.
struct PeepCallback<'a> {
    peephole_transpilations: &'a mut Vec<Box<dyn AbstractPeepholeTranspilation>>,
    feature_set_to_run_on: FeatureSet,
}

impl Callback for PeepCallback<'_> {
    // port: PeepholeTranspilationsPass.PeepCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let mut current_node = n;
        for transpilation in self.peephole_transpilations.iter_mut() {
            match transpilation.transpile_subtree(t.get_compiler(), current_node) {
                Some(next) => current_node = next,
                None => {
                    // this subtree was removed by the current transpilation, so we don't need to
                    // run the rest of the peephole transpilations.
                    return;
                }
            }
        }
    }

    // port: PeepholeTranspilationsPass.PeepCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::SCRIPT => {
                // check if the script contains any of the features that we are transpiling.
                let script_features = NodeUtil::get_feature_set_of_script(t, n)
                    .expect("java.lang.NullPointerException");
                script_features.contains_at_least_one_of(self.feature_set_to_run_on)
            }
            _ => true,
        }
    }
}
