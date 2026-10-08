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
//   src/com/google/javascript/jscomp/AbstractPeepholeTranspilation.java.

//! Port of `AbstractPeepholeTranspilation.java`.
use crate::AbstractCompiler;
use closure_parsing::parser::feature_set::FeatureSet;
use closure_rhino::node::NodeId;

/// A peephole transpilation run by `PeepholeTranspilationsPass` on every node of the scripts that
/// contain one of its features. Java's abstract class becomes a trait; its default method stays a
/// default method.
pub trait AbstractPeepholeTranspilation {
    /// Represents the set of features that get transpiled away by this pass.
    // port: AbstractPeepholeTranspilation#getTranspiledAwayFeatures
    fn get_transpiled_away_features(&self) -> FeatureSet;

    /// The set of additional features, if any, beyond those transpiled away which if present in a
    /// SCRIPT should trigger this pass.
    ///
    /// Most passes shouldn't override this, but it is available for the rare pass that needs to
    /// run on files containing features that it doesn't transpile away.
    ///
    /// For example, we trigger the peephole pass `ReportUntranspilableFeatures` pass for a SCRIPT
    /// which uses the ES3 feature Feature.REGEXP_SYNTAX, but we only transpile away certain
    /// unsupported RegExp flags (e.g. REGEXP_LOOKBEHIND) in that pass.
    ///
    /// Another example, the `Es6NormalizeClasses`, although not a peephole transpilation, must
    /// trigger on Feature.CLASS, but it does not transpile it away.
    ///
    /// Similarly, the pass `Es6ConvertSuper`, although not a peephole transpilation, runs on
    /// Feature.SUPER, but does not transpile it away as calls to `super()` are not transpiled by
    /// it.
    // port: AbstractPeepholeTranspilation#getAdditionalFeaturesToRunOn
    fn get_additional_features_to_run_on(&self) -> FeatureSet {
        FeatureSet::BARE_MINIMUM
    }

    /// Transpile the given node. Subclasses should override to do their own peephole rewriting.
    ///
    /// `subtree`: The subtree that will be transpiled. Returns the new version of the subtree (or
    /// `None` if the subtree was removed from the AST). If the subtree has not changed, this method
    /// must return `subtree`.
    // port: AbstractPeepholeTranspilation#transpileSubtree
    fn transpile_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId>;

    /// Java `getClass().getSimpleName()`, used in `PeepholeTranspilationsPass#create`'s message.
    fn get_simple_name(&self) -> &'static str;
}
