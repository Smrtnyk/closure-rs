/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReportUntranspilableFeatures.java.

//! Port of `ReportUntranspilableFeatures.java`.
use crate::{
    AbstractCompiler, abstract_peephole_transpilation::AbstractPeepholeTranspilation,
    check_reg_exp::MALFORMED_REGEXP, compiler_options::BrowserFeaturesetYear,
    diagnostic_type::DiagnosticType, js_error::JSError, node_util::NodeUtil,
};
use closure_parsing::{
    config::LanguageMode,
    parser::feature_set::{Feature, FeatureSet},
};
use closure_regex::reg_exp_tree::RegExpTree;
use closure_rhino::{
    check_argument,
    js_string::JsString,
    node::{NodeId, Prop},
    token::Token,
};

/// Looks for presence of features that are not supported for transpilation (mostly new RegExp
/// features and bigint literal).
///
/// Reports errors for any features are present in the root and not present in the targeted
/// output language.
///
/// Note: the errors reported by this pass are suppressible. In most cases the right thing to do
/// if suppressing them is instead to bump the output language level; for example, someone who
/// only needs to support ES2020 but is setting --language_out=ECMASCRIPT_2015 will see errors on
/// ES2018 regex literal syntax. The best solution is to set --language_out=ECMASCRIPT_2020. A
/// second solution, which we support but do not recommend, is to suppress these errors & let the
/// compiler silently output untranspiled ES2018 regex literals.
pub struct ReportUntranspilableFeatures {
    browser_feature_set_year: Option<BrowserFeaturesetYear>,
    untranspilable_features_to_remove: FeatureSet,
}

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_FEATURE_PRESENT
pub static UNTRANSPILABLE_FEATURE_PRESENT: DiagnosticType = DiagnosticType::error(
    "JSC_UNTRANSPILABLE",
    "Cannot convert feature \"{0}\" to targeted output language. Feature requires at minimum {1}.{2}",
);

// port: ReportUntranspilableFeatures#UNSUPPORTED_FEATURE_PRESENT
pub static UNSUPPORTED_FEATURE_PRESENT: DiagnosticType = DiagnosticType::error(
    "JSC_UNSUPPORTED",
    "The feature \"{0}\" is currently unsupported for transpilation.",
);

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_ES5_FEATURES
const UNTRANSPILABLE_ES5_FEATURES: FeatureSet =
    FeatureSet::BARE_MINIMUM.with_feature_set(FeatureSet::from_features(&[
        Feature::GETTER,
        Feature::SETTER,
    ]));

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_2018_FEATURES
const UNTRANSPILABLE_2018_FEATURES: FeatureSet =
    FeatureSet::BARE_MINIMUM.with_feature_set(FeatureSet::from_features(&[
        Feature::REGEXP_FLAG_S,
        Feature::REGEXP_LOOKBEHIND,
        Feature::REGEXP_NAMED_GROUPS,
        Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
    ]));

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_2019_FEATURES
const UNTRANSPILABLE_2019_FEATURES: FeatureSet =
    FeatureSet::BARE_MINIMUM.with_feature_set(FeatureSet::from_features(&[
        // We could transpile this, but there's no point. We always escape these in the output,
        // no need to have a separate pass to escape them. So we'll piggy back off this pass to
        // mark it as transpiled. Note that we never complain that this feature won't be
        // transpiled below.
        Feature::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP,
    ]));

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_2020_FEATURES
const UNTRANSPILABLE_2020_FEATURES: FeatureSet =
    FeatureSet::BARE_MINIMUM.with_feature_set(FeatureSet::from_features(&[Feature::BIGINT]));

// port: ReportUntranspilableFeatures#UNTRANSPILABLE_2022_FEATURES
const UNTRANSPILABLE_2022_FEATURES: FeatureSet =
    FeatureSet::BARE_MINIMUM.with_feature_set(FeatureSet::from_features(&[
        Feature::REGEXP_FLAG_D,
        Feature::TOP_LEVEL_AWAIT,
    ]));

// port: ReportUntranspilableFeatures#ALL_UNTRANSPILABLE_FEATURES
const ALL_UNTRANSPILABLE_FEATURES: FeatureSet = FeatureSet::BARE_MINIMUM
    .union(UNTRANSPILABLE_ES5_FEATURES)
    .union(UNTRANSPILABLE_2018_FEATURES)
    .union(UNTRANSPILABLE_2019_FEATURES)
    .union(UNTRANSPILABLE_2022_FEATURES)
    .union(UNTRANSPILABLE_2020_FEATURES);

impl ReportUntranspilableFeatures {
    /// `compiler`: the compiler instance. `browser_feature_set_year`: the given
    /// BrowserFeaturesetYear, or `None` if it was not specified. `output_features`: the set of
    /// features supported in the given output language (Java's `checkNotNull` holds by type).
    // port: ReportUntranspilableFeatures#ReportUntranspilableFeatures
    pub fn new(
        _compiler: &AbstractCompiler,
        browser_feature_set_year: Option<BrowserFeaturesetYear>,
        output_features: FeatureSet,
    ) -> Self {
        Self {
            browser_feature_set_year,
            untranspilable_features_to_remove: ALL_UNTRANSPILABLE_FEATURES // Features that we can't transpile...
                .without(output_features), // and do not exist in the output language features
        }
    }

    // port: ReportUntranspilableFeatures#checkForUntranspilable
    #[allow(clippy::collapsible_match)] // Java's switch arms keep their nested ifs.
    fn check_for_untranspilable(&self, compiler: &mut AbstractCompiler, root: NodeId) {
        // Non-flag RegExp features are not attached to nodes, so we must force traversal.
        match root.get_token(compiler) {
            Token::REGEXP => {
                let pattern = root.get_first_child(compiler).unwrap().get_string(compiler);
                let flags = if root.has_two_children(compiler) {
                    root.get_last_child(compiler).unwrap().get_string(compiler)
                } else {
                    JsString::from("")
                };
                let reg = match RegExpTree::parse_reg_exp(&pattern, &flags) {
                    Ok(reg) => reg,
                    Err(ex) => {
                        // Java passes ex.getMessage(); MessageFormat prints a null argument as
                        // "null".
                        let message = ex
                            .get_message()
                            .map_or_else(|| "null".to_string(), JsString::to_string_lossy);
                        compiler.report(JSError::make(
                            compiler,
                            root,
                            &MALFORMED_REGEXP,
                            &[&message],
                        ));
                        return;
                    }
                };
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::REGEXP_FLAG_S)
                {
                    self.check_for_reg_exp_s_flag(compiler, root);
                }
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::REGEXP_LOOKBEHIND)
                {
                    self.check_for_lookbehind(compiler, Some(root), &reg);
                }
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::REGEXP_NAMED_GROUPS)
                {
                    self.check_for_named_groups(compiler, Some(root), &reg);
                }
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::REGEXP_UNICODE_PROPERTY_ESCAPE)
                {
                    self.check_for_unicode_property_escape(compiler, Some(root), &reg);
                }
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::REGEXP_FLAG_D)
                {
                    self.check_for_reg_exp_d_flag(compiler, root);
                }
            }
            Token::BIGINT => {
                // Transpilation of BigInt is not supported
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::BIGINT)
                {
                    self.report_untranspilable(compiler, Feature::BIGINT, root);
                }
            }
            Token::GETTER_DEF => {
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::GETTER)
                {
                    self.report_untranspilable(compiler, Feature::GETTER, root);
                }
            }
            Token::SETTER_DEF => {
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::SETTER)
                {
                    self.report_untranspilable(compiler, Feature::SETTER, root);
                }
            }
            Token::AWAIT | Token::FOR_AWAIT_OF => {
                if self
                    .untranspilable_features_to_remove
                    .contains(Feature::TOP_LEVEL_AWAIT)
                {
                    self.check_for_top_level_await(compiler, root);
                }
            }
            _ => {}
        }
    }

    // port: ReportUntranspilableFeatures#reportUntranspilable
    fn report_untranspilable(
        &self,
        compiler: &mut AbstractCompiler,
        feature: Feature,
        node: NodeId,
    ) {
        let minimum_language_mode = LanguageMode::minimum_required_for(feature);
        if minimum_language_mode == LanguageMode::UNSUPPORTED {
            compiler.report(JSError::make(
                compiler,
                node,
                &UNSUPPORTED_FEATURE_PRESENT,
                &[&feature.to_string()],
            ));
            return;
        }

        // The compiler always has an output featureset configured. Sometimes this is configured
        // directly or via a LanguageMode, and in this case browserFeatureSetYear is null.
        // Otherwise if browserFeatureSetYear is not null, the user configured a
        // browserFeatureSetYear specifically. Report an error message based on the actual API the
        // user invoked: sometimes the year in a browser featureset year (e.g. 2020) does not
        // correspond to e.g. ECMASCRIPT_2020.
        let minimum;
        let mut suggestion = String::from(" Consider targeting a more modern output.");
        if let Some(browser_feature_set_year) = self.browser_feature_set_year {
            let minimum_year = BrowserFeaturesetYear::minimum_required_for(feature);
            match minimum_year {
                None => {
                    // Java's LanguageMode.toString() is the enum constant name (its Debug).
                    minimum = format!(
                        "{minimum_language_mode:?}, which is not yet supported by any browser featureset year"
                    );
                    suggestion = String::new();
                }
                Some(minimum_year) => {
                    minimum = format!("browser featureset year {}", minimum_year.get_year());
                    suggestion = format!(
                        "{suggestion}\nCurrent browser featureset year: {}",
                        browser_feature_set_year.get_year()
                    );
                }
            }
        } else {
            minimum = format!("{minimum_language_mode:?}");
        }
        compiler.report(JSError::make(
            compiler,
            node,
            &UNTRANSPILABLE_FEATURE_PRESENT,
            &[&feature.to_string(), &minimum, &suggestion],
        ));
    }

    // port: ReportUntranspilableFeatures#checkForRegExpSFlag
    fn check_for_reg_exp_s_flag(&self, compiler: &mut AbstractCompiler, regexp_node: NodeId) {
        check_argument!(regexp_node.is_reg_exp(compiler));
        let flags = if regexp_node.has_two_children(compiler) {
            regexp_node
                .get_last_child(compiler)
                .unwrap()
                .get_string(compiler)
        } else {
            JsString::from("")
        };
        if flags.index_of(&JsString::from("s")) >= 0 {
            self.report_untranspilable(compiler, Feature::REGEXP_FLAG_S, regexp_node);
        }
    }

    // port: ReportUntranspilableFeatures#checkForLookbehind
    fn check_for_lookbehind(
        &self,
        compiler: &mut AbstractCompiler,
        regexp_node: Option<NodeId>,
        tree: &RegExpTree,
    ) {
        check_argument!(regexp_node.is_some());
        if Self::any_subtree_meets_predicate(tree, &|t| {
            matches!(t, RegExpTree::LookbehindAssertion(_))
        }) {
            self.report_untranspilable(compiler, Feature::REGEXP_LOOKBEHIND, regexp_node.unwrap());
        }
    }

    // port: ReportUntranspilableFeatures#checkForNamedGroups
    fn check_for_named_groups(
        &self,
        compiler: &mut AbstractCompiler,
        regexp_node: Option<NodeId>,
        tree: &RegExpTree,
    ) {
        check_argument!(regexp_node.is_some());
        if Self::any_subtree_meets_predicate(tree, &|t| {
            matches!(t, RegExpTree::NamedCaptureGroup(_))
        }) {
            self.report_untranspilable(
                compiler,
                Feature::REGEXP_NAMED_GROUPS,
                regexp_node.unwrap(),
            );
        }
    }

    // port: ReportUntranspilableFeatures#checkForUnicodePropertyEscape
    fn check_for_unicode_property_escape(
        &self,
        compiler: &mut AbstractCompiler,
        regexp_node: Option<NodeId>,
        tree: &RegExpTree,
    ) {
        check_argument!(regexp_node.is_some());
        if Self::any_subtree_meets_predicate(tree, &|t| {
            matches!(t, RegExpTree::UnicodePropertyEscape(_))
        }) {
            self.report_untranspilable(
                compiler,
                Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
                regexp_node.unwrap(),
            );
        }
    }

    // port: ReportUntranspilableFeatures#checkForRegExpDFlag
    fn check_for_reg_exp_d_flag(&self, compiler: &mut AbstractCompiler, regexp_node: NodeId) {
        check_argument!(regexp_node.is_reg_exp(compiler));
        let flags = if regexp_node.has_two_children(compiler) {
            regexp_node
                .get_last_child(compiler)
                .unwrap()
                .get_string(compiler)
        } else {
            JsString::from("")
        };
        if flags.index_of(&JsString::from("d")) >= 0 {
            self.report_untranspilable(compiler, Feature::REGEXP_FLAG_D, regexp_node);
        }
    }

    // port: ReportUntranspilableFeatures#checkForTopLevelAwait
    fn check_for_top_level_await(&self, compiler: &mut AbstractCompiler, await_node: NodeId) {
        check_argument!(await_node.is_await(compiler) || await_node.is_for_await_of(compiler));

        if NodeUtil::get_enclosing_function(compiler, await_node).is_none() {
            check_argument!(
                NodeUtil::get_enclosing_script(compiler, await_node)
                    .unwrap()
                    .get_boolean_prop(compiler, Prop::ES6_MODULE),
                "Top-level await is only allowed in ES module sources"
            );
            self.report_untranspilable(compiler, Feature::TOP_LEVEL_AWAIT, await_node);
        }
    }

    // port: ReportUntranspilableFeatures#anySubtreeMeetsPredicate
    fn any_subtree_meets_predicate(tree: &RegExpTree, p: &dyn Fn(&RegExpTree) -> bool) -> bool {
        if p(tree) {
            return true;
        }
        for sub_tree in tree.children() {
            if Self::any_subtree_meets_predicate(sub_tree, p) {
                return true;
            }
        }
        false
    }
}

impl AbstractPeepholeTranspilation for ReportUntranspilableFeatures {
    // port: ReportUntranspilableFeatures#getTranspiledAwayFeatures
    fn get_transpiled_away_features(&self) -> FeatureSet {
        self.untranspilable_features_to_remove
    }

    // port: ReportUntranspilableFeatures#getAdditionalFeaturesToRunOn
    fn get_additional_features_to_run_on(&self) -> FeatureSet {
        // This pass needs to run on all ES3 REGEXP_SYNTAX because it checks for the presence of
        // non-flag RegExp features that are not parsed and not attached to nodes.
        FeatureSet::BARE_MINIMUM.with(Feature::REGEXP_SYNTAX)
    }

    // port: ReportUntranspilableFeatures#transpileSubtree
    fn transpile_subtree(
        &mut self,
        compiler: &mut AbstractCompiler,
        subtree: NodeId,
    ) -> Option<NodeId> {
        self.check_for_untranspilable(compiler, subtree);
        Some(subtree)
    }

    fn get_simple_name(&self) -> &'static str {
        "ReportUntranspilableFeatures"
    }
}
