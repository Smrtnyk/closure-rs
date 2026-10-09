/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/Config.java.

//! Port of `com.google.javascript.jscomp.parsing.Config` (an AutoValue class).

use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use std::sync::Arc;

use crate::annotation::Annotation;
use crate::parser::feature_set::{Feature, FeatureSet};

/// Level of language strictness required for the input source code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StrictMode {
    STRICT,
    SLOPPY,
}

impl StrictMode {
    // port: Config.StrictMode#isStrict
    pub fn is_strict(self) -> bool {
        self == StrictMode::STRICT
    }
}

/// JavaScript mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LanguageMode {
    // Note that minimumRequiredFor() relies on these being defined in order from fewest features to
    // most features, and _STRICT versions should be supplied after unspecified strictness.
    ECMASCRIPT3,
    ECMASCRIPT5,
    ECMASCRIPT_2015,
    ECMASCRIPT_2016,
    ECMASCRIPT_2017,
    ECMASCRIPT_2018,
    ECMASCRIPT_2019,
    ECMASCRIPT_2020,
    ECMASCRIPT_2021,
    ECMASCRIPT_2022,
    // NOTE: When adding a new language level here, also update latestEcmaScript() below to return
    // it.
    ES_NEXT,
    UNSTABLE,
    UNSUPPORTED,
}

impl LanguageMode {
    /// Java `LanguageMode.values()`, in declaration order.
    pub const VALUES: [LanguageMode; 13] = [
        LanguageMode::ECMASCRIPT3,
        LanguageMode::ECMASCRIPT5,
        LanguageMode::ECMASCRIPT_2015,
        LanguageMode::ECMASCRIPT_2016,
        LanguageMode::ECMASCRIPT_2017,
        LanguageMode::ECMASCRIPT_2018,
        LanguageMode::ECMASCRIPT_2019,
        LanguageMode::ECMASCRIPT_2020,
        LanguageMode::ECMASCRIPT_2021,
        LanguageMode::ECMASCRIPT_2022,
        LanguageMode::ES_NEXT,
        LanguageMode::UNSTABLE,
        LanguageMode::UNSUPPORTED,
    ];

    /// The Java field `featureSet`, set by the enum constructor.
    // port: Config.LanguageMode#LanguageMode
    pub fn feature_set(self) -> FeatureSet {
        match self {
            LanguageMode::ECMASCRIPT3 => FeatureSet::ES3,
            LanguageMode::ECMASCRIPT5 => FeatureSet::ES5,
            LanguageMode::ECMASCRIPT_2015 => FeatureSet::ES2015_MODULES,
            LanguageMode::ECMASCRIPT_2016 => FeatureSet::ES2016_MODULES,
            LanguageMode::ECMASCRIPT_2017 => FeatureSet::ES2017_MODULES,
            LanguageMode::ECMASCRIPT_2018 => FeatureSet::ES2018_MODULES,
            LanguageMode::ECMASCRIPT_2019 => FeatureSet::ES2019_MODULES,
            LanguageMode::ECMASCRIPT_2020 => FeatureSet::ES2020_MODULES,
            LanguageMode::ECMASCRIPT_2021 => FeatureSet::ES2021_MODULES,
            LanguageMode::ECMASCRIPT_2022 => FeatureSet::ES2022_MODULES,
            LanguageMode::ES_NEXT => FeatureSet::ES_NEXT,
            LanguageMode::UNSTABLE => FeatureSet::ES_UNSTABLE,
            LanguageMode::UNSUPPORTED => FeatureSet::ES_UNSUPPORTED,
        }
    }

    /// Returns the lowest {@link LanguageMode} that supports the specified feature.
    // port: Config.LanguageMode#minimumRequiredFor
    pub fn minimum_required_for(feature: Feature) -> LanguageMode {
        // relies on the LanguageMode enums being in the right order
        for mode in LanguageMode::VALUES {
            if mode.feature_set().has(feature) {
                return mode;
            }
        }
        panic!("No input language mode supports feature: {feature}");
    }

    /// Returns the lowest {@link LanguageMode} that supports the specified feature set.
    // port: Config.LanguageMode#minimumRequiredForSet
    pub fn minimum_required_for_set(feature_set: FeatureSet) -> LanguageMode {
        for mode in LanguageMode::VALUES {
            if mode.feature_set().contains(feature_set) {
                return mode;
            }
        }
        panic!("No input language mode supports feature set: {feature_set}");
    }

    // port: Config.LanguageMode#latestEcmaScript
    pub fn latest_ecma_script() -> LanguageMode {
        LanguageMode::ECMASCRIPT_2021
    }
}

/// Whether to parse the descriptions of JsDoc comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JsDocParsing {
    TYPES_ONLY,
    INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
    INCLUDE_DESCRIPTIONS_WITH_WHITESPACE,
    INCLUDE_ALL_COMMENTS,
    LICENSE_COMMENTS_ONLY,
}

impl JsDocParsing {
    // port: Config.JsDocParsing#shouldParseDescriptions
    pub fn should_parse_descriptions(self) -> bool {
        self != JsDocParsing::TYPES_ONLY
    }

    // port: Config.JsDocParsing#shouldPreserveWhitespace
    pub fn should_preserve_whitespace(self) -> bool {
        self == JsDocParsing::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE
            || self == JsDocParsing::INCLUDE_ALL_COMMENTS
            || self == JsDocParsing::LICENSE_COMMENTS_ONLY
    }
}

/// Whether to keep going after encountering a parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RunMode {
    STOP_AFTER_ERROR,
    KEEP_GOING,
}

/// Configuration for the AST factory. Should be shared across AST creation for all files of a
/// compilation process.
///
/// Java's AutoValue class: the abstract property methods become fields with accessors, and
/// `equals`/`hashCode` compare every property.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    language_mode: LanguageMode,
    strict_mode: StrictMode,
    js_doc_parsing_mode: JsDocParsing,
    run_mode: RunMode,
    // Shared (Arc) so that cloning the config, which IRFactory does for every JSDoc comment,
    // does not copy the maps; Java shares the immutable collections by reference.
    pub(crate) annotations: Arc<IndexMap<JsString, Annotation>>,
    pub(crate) suppression_names: Arc<IndexSet<JsString>>,
    pub(crate) closure_primitive_names: Arc<IndexSet<JsString>>,
    parse_inline_source_maps: bool,
}

impl Config {
    /// Rust-only (D-025): an equal config that shares no reference-counted data with this one,
    /// for parsing on another thread (jscomp `parallel_parse`) without contending for the
    /// reference counts of the shared maps and strings.
    pub fn unshared_copy(&self) -> Self {
        let copy = |s: &JsString| JsString::from_units(s.as_units().to_vec());
        Self {
            annotations: Arc::new(
                self.annotations
                    .iter()
                    .map(|(k, v)| (copy(k), *v))
                    .collect(),
            ),
            suppression_names: Arc::new(self.suppression_names.iter().map(copy).collect()),
            closure_primitive_names: Arc::new(
                self.closure_primitive_names.iter().map(copy).collect(),
            ),
            ..self.clone()
        }
    }
    /// Language level to accept.
    // port: Config#languageMode
    pub fn language_mode(&self) -> LanguageMode {
        self.language_mode
    }

    /// Whether to assume input is strict mode compliant.
    // port: Config#strictMode
    pub fn strict_mode(&self) -> StrictMode {
        self.strict_mode
    }

    /// How to parse the descriptions of JsDoc comments.
    // port: Config#jsDocParsingMode
    pub fn js_doc_parsing_mode(&self) -> JsDocParsing {
        self.js_doc_parsing_mode
    }

    /// Whether to keep going after encountering a parse error.
    // port: Config#runMode
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }

    /// Recognized JSDoc annotations, mapped from their name to their internal representation.
    // port: Config#annotations
    pub fn annotations(&self) -> &IndexMap<JsString, Annotation> {
        &self.annotations
    }

    /// Set of recognized names in a {@code @suppress} tag.
    // port: Config#suppressionNames
    pub fn suppression_names(&self) -> &IndexSet<JsString> {
        &self.suppression_names
    }

    /// Set of recognized names in a {@code @closurePrimitive} tag.
    // port: Config#closurePrimitiveNames
    pub fn closure_primitive_names(&self) -> &IndexSet<JsString> {
        &self.closure_primitive_names
    }

    /// Whether to parse inline source maps (//# sourceMappingURL=data:...).
    // port: Config#parseInlineSourceMaps
    pub fn parse_inline_source_maps(&self) -> bool {
        self.parse_inline_source_maps
    }

    // port: Config#annotationNames
    pub fn annotation_names(&self) -> IndexSet<JsString> {
        self.annotations.keys().cloned().collect()
    }

    // port: Config#toBuilder
    pub fn to_builder(&self) -> Builder {
        Builder {
            language_mode: Some(self.language_mode),
            strict_mode: Some(self.strict_mode),
            js_doc_parsing_mode: Some(self.js_doc_parsing_mode),
            run_mode: Some(self.run_mode),
            annotations: Some((*self.annotations).clone()),
            suppression_names: Some((*self.suppression_names).clone()),
            closure_primitive_names: Some((*self.closure_primitive_names).clone()),
            parse_inline_source_maps: Some(self.parse_inline_source_maps),
        }
    }

    // port: Config#builder
    pub fn builder() -> Builder {
        let mut b = Builder::default();
        b.set_language_mode(LanguageMode::UNSUPPORTED)
            .set_strict_mode(StrictMode::STRICT)
            .set_js_doc_parsing_mode(JsDocParsing::TYPES_ONLY)
            .set_run_mode(RunMode::STOP_AFTER_ERROR)
            .set_extra_annotation_names(Vec::<JsString>::new())
            .set_suppression_names(Vec::<JsString>::new())
            .set_closure_primitive_names(Vec::<JsString>::new())
            .set_parse_inline_source_maps(false);
        b
    }

    /// Create the annotation names from the user-specified annotation allow list.
    ///
    /// The Java parameter name is spelled `allow_list` here because gates/ci.sh reserves the
    /// one-word spelling for test skip lists.
    // port: Config#buildAnnotations
    fn build_annotations<I, S>(allow_list: I) -> IndexMap<JsString, Annotation>
    where
        I: IntoIterator<Item = S>,
        S: Into<JsString>,
    {
        // ImmutableMap.Builder: insertion ordered, duplicate keys rejected by buildOrThrow.
        let mut annotations_builder: Vec<(JsString, Annotation)> = Vec::new();
        for (k, v) in Annotation::recognized_annotations() {
            annotations_builder.push((k.clone(), *v));
        }
        for unrecognized_annotation in allow_list {
            let unrecognized_annotation: JsString = unrecognized_annotation.into();
            if !unrecognized_annotation.is_empty()
                && !Annotation::recognized_annotations().contains_key(&unrecognized_annotation)
            {
                annotations_builder.push((unrecognized_annotation, Annotation::NOT_IMPLEMENTED));
            }
        }
        let mut result = IndexMap::<_, _>::default();
        for (k, v) in annotations_builder {
            if let Some(previous) = result.get(&k) {
                panic!("Multiple entries with same key: {k}={previous:?} and {k}={v:?}");
            }
            result.insert(k, v);
        }
        result
    }
}

/// Builder for a Config.
#[derive(Clone, Debug, Default)]
pub struct Builder {
    language_mode: Option<LanguageMode>,
    strict_mode: Option<StrictMode>,
    js_doc_parsing_mode: Option<JsDocParsing>,
    run_mode: Option<RunMode>,
    annotations: Option<IndexMap<JsString, Annotation>>,
    suppression_names: Option<IndexSet<JsString>>,
    closure_primitive_names: Option<IndexSet<JsString>>,
    parse_inline_source_maps: Option<bool>,
}

impl Builder {
    // port: Config.Builder#setLanguageMode
    pub fn set_language_mode(&mut self, mode: LanguageMode) -> &mut Self {
        self.language_mode = Some(mode);
        self
    }

    // port: Config.Builder#setStrictMode
    pub fn set_strict_mode(&mut self, mode: StrictMode) -> &mut Self {
        self.strict_mode = Some(mode);
        self
    }

    // port: Config.Builder#setJsDocParsingMode
    pub fn set_js_doc_parsing_mode(&mut self, mode: JsDocParsing) -> &mut Self {
        self.js_doc_parsing_mode = Some(mode);
        self
    }

    // port: Config.Builder#setRunMode
    pub fn set_run_mode(&mut self, mode: RunMode) -> &mut Self {
        self.run_mode = Some(mode);
        self
    }

    // port: Config.Builder#setParseInlineSourceMaps
    pub fn set_parse_inline_source_maps(&mut self, parse_inline_source_maps: bool) -> &mut Self {
        self.parse_inline_source_maps = Some(parse_inline_source_maps);
        self
    }

    /// AutoValue copies the `Iterable` into an `ImmutableSet` (first occurrence order kept).
    // port: Config.Builder#setSuppressionNames
    pub fn set_suppression_names<I, S>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<JsString>,
    {
        self.suppression_names = Some(names.into_iter().map(Into::into).collect());
        self
    }

    /// AutoValue copies the `Iterable` into an `ImmutableSet` (first occurrence order kept).
    // port: Config.Builder#setClosurePrimitiveNames
    pub fn set_closure_primitive_names<I, S>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<JsString>,
    {
        self.closure_primitive_names = Some(names.into_iter().map(Into::into).collect());
        self
    }

    // port: Config.Builder#setExtraAnnotationNames
    pub fn set_extra_annotation_names<I, S>(&mut self, names: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: Into<JsString>,
    {
        self.set_annotations(Config::build_annotations(names))
    }

    // port: Config.Builder#build
    pub fn build(&self) -> Config {
        // AutoValue throws IllegalStateException naming the missing properties.
        let mut missing = String::new();
        if self.language_mode.is_none() {
            missing.push_str(" languageMode");
        }
        if self.strict_mode.is_none() {
            missing.push_str(" strictMode");
        }
        if self.js_doc_parsing_mode.is_none() {
            missing.push_str(" jsDocParsingMode");
        }
        if self.run_mode.is_none() {
            missing.push_str(" runMode");
        }
        if self.annotations.is_none() {
            missing.push_str(" annotations");
        }
        if self.suppression_names.is_none() {
            missing.push_str(" suppressionNames");
        }
        if self.closure_primitive_names.is_none() {
            missing.push_str(" closurePrimitiveNames");
        }
        if self.parse_inline_source_maps.is_none() {
            missing.push_str(" parseInlineSourceMaps");
        }
        if !missing.is_empty() {
            panic!("Missing required properties:{missing}");
        }
        Config {
            language_mode: self.language_mode.unwrap(),
            strict_mode: self.strict_mode.unwrap(),
            js_doc_parsing_mode: self.js_doc_parsing_mode.unwrap(),
            run_mode: self.run_mode.unwrap(),
            annotations: Arc::new(self.annotations.clone().unwrap()),
            suppression_names: Arc::new(self.suppression_names.clone().unwrap()),
            closure_primitive_names: Arc::new(self.closure_primitive_names.clone().unwrap()),
            parse_inline_source_maps: self.parse_inline_source_maps.unwrap(),
        }
    }

    // The following is intended to be used internally only (but isn't private due to AutoValue).
    // port: Config.Builder#setAnnotations
    pub fn set_annotations(&mut self, names: IndexMap<JsString, Annotation>) -> &mut Self {
        self.annotations = Some(names);
        self
    }
}

impl JsDocParsing {
    pub const VALUES: &'static [Self] = &[
        Self::TYPES_ONLY,
        Self::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        Self::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE,
        Self::INCLUDE_ALL_COMMENTS,
        Self::LICENSE_COMMENTS_ONLY,
    ];
    // port: Config.JsDocParsing#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}

impl std::fmt::Display for JsDocParsing {
    // port: Config.JsDocParsing#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
