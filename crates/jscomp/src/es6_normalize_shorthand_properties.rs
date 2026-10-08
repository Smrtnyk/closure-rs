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
//   src/com/google/javascript/jscomp/Es6NormalizeShorthandProperties.java.

//! Port of `Es6NormalizeShorthandProperties.java`.
use crate::{AbstractCompiler, abstract_peephole_transpilation::AbstractPeepholeTranspilation};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::node::NodeId;

/// Normalizes shorthand object properties. This should be one of the first things done when
/// transpiling from ES6 down to ES5, as it allows all the following checks and transpilations to
/// not care about shorthand and destructured assignments.
pub struct Es6NormalizeShorthandProperties;

impl Es6NormalizeShorthandProperties {
    // port: Es6NormalizeShorthandProperties#Es6NormalizeShorthandProperties
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }
}

impl AbstractPeepholeTranspilation for Es6NormalizeShorthandProperties {
    // port: Es6NormalizeShorthandProperties#transpileSubtree
    fn transpile_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        if subtree.is_string_key(compiler) {
            subtree.set_shorthand_property(compiler, false);
        }
        Some(subtree)
    }

    // port: Es6NormalizeShorthandProperties#getTranspiledAwayFeatures
    fn get_transpiled_away_features(&self) -> FeatureSet {
        FeatureSet::BARE_MINIMUM.with(Feature::SHORTHAND_OBJECT_PROPERTIES)
    }

    fn get_simple_name(&self) -> &'static str {
        "Es6NormalizeShorthandProperties"
    }
}
