/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/FeatureSet.java.

#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LangVersion {
    ES3,
    ES5,
    ES2015,
    ES2016,
    ES2017,
    ES2018,
    ES2019,
    ES2020,
    ES2021,
    ES2022,
    ES_NEXT,
    ES_UNSTABLE,
    ES_UNSUPPORTED,
    TYPESCRIPT,
}
impl LangVersion {
    // port: FeatureSet.LangVersion#features
    pub const fn features(self) -> FeatureSet {
        let mut set = FeatureSet::empty_enum_set();
        let mut index = 0;
        while index < Feature::ALL.len() {
            let feature = Feature::ALL[index];
            if feature.version() as u32 == self as u32 {
                set = FeatureSet::add(set, feature);
            }
            index += 1;
        }
        set
    }
}

/// Specific features that can be included in a FeatureSet.
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    // ES3 features
    REGEXP_SYNTAX,
    // ES5 features
    ES3_KEYWORDS_AS_IDENTIFIERS,
    GETTER,
    KEYWORDS_AS_PROPERTIES,
    SETTER,
    STRING_CONTINUATION,
    TRAILING_COMMA,
    // ES2015 features (besides modules): all stable browsers are now fully compliant
    // go/keep-sorted start
    ARRAY_DESTRUCTURING,
    ARRAY_PATTERN_REST,
    ARROW_FUNCTIONS,
    BINARY_LITERALS,
    BLOCK_SCOPED_FUNCTION_DECLARATION,
    CLASSES,
    CLASS_GETTER_SETTER,
    COMPUTED_PROPERTIES,
    CONST_DECLARATIONS,
    DEFAULT_PARAMETERS,
    FOR_OF,
    GENERATORS,
    LET_DECLARATIONS,
    MEMBER_DECLARATIONS,
    NEW_TARGET,
    OBJECT_DESTRUCTURING,
    OCTAL_LITERALS,
    REGEXP_FLAG_U,
    REGEXP_FLAG_Y,
    REST_PARAMETERS,
    SHORTHAND_OBJECT_PROPERTIES,
    SPREAD_EXPRESSIONS,
    SUPER,
    TEMPLATE_LITERALS,
    // go/keep-sorted end
    // ES modules
    MODULES,
    // ES 2016 only added one new feature:
    EXPONENT_OP,
    // ES 2017 features:
    ASYNC_FUNCTIONS,
    // ES 2018 adds https://github.com/tc39/proposal-object-rest-spread
    OBJECT_LITERALS_WITH_SPREAD,
    OBJECT_PATTERN_REST,
    // https://github.com/tc39/proposal-async-iteration
    ASYNC_GENERATORS,
    FOR_AWAIT_OF,
    // ES 2018 adds Regex Features:
    // https://github.com/tc39/proposal-regexp-dotall-flag
    // Note: Untranspilable.
    REGEXP_FLAG_S,
    // https://github.com/tc39/proposal-regexp-named-groups
    // Note: Untranspilable.
    REGEXP_NAMED_GROUPS,
    // https://github.com/tc39/proposal-regexp-unicode-property-escapes
    // Note: Untranspilable.
    REGEXP_UNICODE_PROPERTY_ESCAPE,
    // https://github.com/tc39/proposal-regexp-lookbehind
    // Note: Untranspilable.
    REGEXP_LOOKBEHIND,
    // ES 2019 adds https://github.com/tc39/proposal-json-superset
    // Note: Untranspilable.
    UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP,
    // ES 2019 adds optional catch bindings:
    // https://github.com/tc39/proposal-optional-catch-binding
    OPTIONAL_CATCH_BINDING,
    // ES 2020 Stage 4
    DYNAMIC_IMPORT,
    // Note: Untranspilable.
    BIGINT,
    IMPORT_META,
    NULL_COALESCE_OP,
    OPTIONAL_CHAINING,
    // ES 2021 Stage 4
    NUMERIC_SEPARATOR,
    LOGICAL_ASSIGNMENT,
    // ES 2022 adds https://github.com/tc39/proposal-class-fields
    PUBLIC_CLASS_FIELDS,
    // ES 2022 adds https://github.com/tc39/proposal-class-static-block
    CLASS_STATIC_BLOCK,
    // ES 2022 adds https://github.com/tc39/proposal-regexp-match-indices
    // Note: Untranspilable.
    REGEXP_FLAG_D,
    // ES 2022 adds https://github.com/tc39/proposal-top-level-await
    // Note: Untranspilable. Needs to stay UNSUPPORTED because pass-through of top-level await does
    // not make sense as we don't emit ES6 modules so top-level await cannot be used in the output.
    TOP_LEVEL_AWAIT,
    // ES_NEXT: Features that are fully supported, but part of a language version that is not yet
    // fully supported
    // Polyfill implementations can target "es_next" as their fromLang so we need to ensure that
    // the "es_next" version name is distinct from the latest dated version so we don't incorrectly
    // prune those polyfills.
    ES_NEXT_RUNTIME,
    // ES_UNSTABLE: Features fully supported in checks, but not fully supported everywhere else
    // Polyfill implementations can target "es_unstable" as their fromLang so we need to ensure that
    // the "es_unstable" version name is distinct from the latest dated version so we don't
    // incorrectly prune those polyfills.
    ES_UNSTABLE_RUNTIME,
    // ES_UNSUPPORTED: Features that we can parse, but not yet supported in all checks
    // ES 2022 adds https://github.com/tc39/proposal-class-fields
    PRIVATE_ELEMENTS,
    // TypeScript type syntax that will never be implemented in browsers. Only used as an indicator
    // to the CodeGenerator that it should handle type syntax.
    TYPE_ANNOTATION,
    // End of list.
}
// Java enum singleton fields, constructed in declaration order.
struct FeatureData {
    name: &'static str,
    version: LangVersion,
}
impl FeatureData {
    // port: FeatureSet.Feature#<init>
    const fn new(name: &'static str, version: LangVersion) -> Self {
        Self { name, version }
    }
}
impl Feature {
    pub const ALL: &'static [Self] = &[
        Self::REGEXP_SYNTAX,
        Self::ES3_KEYWORDS_AS_IDENTIFIERS,
        Self::GETTER,
        Self::KEYWORDS_AS_PROPERTIES,
        Self::SETTER,
        Self::STRING_CONTINUATION,
        Self::TRAILING_COMMA,
        Self::ARRAY_DESTRUCTURING,
        Self::ARRAY_PATTERN_REST,
        Self::ARROW_FUNCTIONS,
        Self::BINARY_LITERALS,
        Self::BLOCK_SCOPED_FUNCTION_DECLARATION,
        Self::CLASSES,
        Self::CLASS_GETTER_SETTER,
        Self::COMPUTED_PROPERTIES,
        Self::CONST_DECLARATIONS,
        Self::DEFAULT_PARAMETERS,
        Self::FOR_OF,
        Self::GENERATORS,
        Self::LET_DECLARATIONS,
        Self::MEMBER_DECLARATIONS,
        Self::NEW_TARGET,
        Self::OBJECT_DESTRUCTURING,
        Self::OCTAL_LITERALS,
        Self::REGEXP_FLAG_U,
        Self::REGEXP_FLAG_Y,
        Self::REST_PARAMETERS,
        Self::SHORTHAND_OBJECT_PROPERTIES,
        Self::SPREAD_EXPRESSIONS,
        Self::SUPER,
        Self::TEMPLATE_LITERALS,
        Self::MODULES,
        Self::EXPONENT_OP,
        Self::ASYNC_FUNCTIONS,
        Self::OBJECT_LITERALS_WITH_SPREAD,
        Self::OBJECT_PATTERN_REST,
        Self::ASYNC_GENERATORS,
        Self::FOR_AWAIT_OF,
        Self::REGEXP_FLAG_S,
        Self::REGEXP_NAMED_GROUPS,
        Self::REGEXP_UNICODE_PROPERTY_ESCAPE,
        Self::REGEXP_LOOKBEHIND,
        Self::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP,
        Self::OPTIONAL_CATCH_BINDING,
        Self::DYNAMIC_IMPORT,
        Self::BIGINT,
        Self::IMPORT_META,
        Self::NULL_COALESCE_OP,
        Self::OPTIONAL_CHAINING,
        Self::NUMERIC_SEPARATOR,
        Self::LOGICAL_ASSIGNMENT,
        Self::PUBLIC_CLASS_FIELDS,
        Self::CLASS_STATIC_BLOCK,
        Self::REGEXP_FLAG_D,
        Self::TOP_LEVEL_AWAIT,
        Self::ES_NEXT_RUNTIME,
        Self::ES_UNSTABLE_RUNTIME,
        Self::PRIVATE_ELEMENTS,
        Self::TYPE_ANNOTATION,
    ];

    const fn data(self) -> FeatureData {
        let name: &'static str = match self {
            Self::REGEXP_SYNTAX => "RegExp syntax",
            Self::ES3_KEYWORDS_AS_IDENTIFIERS => "ES3 keywords as identifiers",
            Self::GETTER => "getters",
            Self::KEYWORDS_AS_PROPERTIES => "reserved words as properties",
            Self::SETTER => "setters",
            Self::STRING_CONTINUATION => "string continuation",
            Self::TRAILING_COMMA => "trailing comma",
            Self::ARRAY_DESTRUCTURING => "array destructuring",
            Self::ARRAY_PATTERN_REST => "array pattern rest",
            Self::ARROW_FUNCTIONS => "arrow function",
            Self::BINARY_LITERALS => "binary literal",
            Self::BLOCK_SCOPED_FUNCTION_DECLARATION => "block-scoped function declaration",
            Self::CLASSES => "class",
            Self::CLASS_GETTER_SETTER => "class getters/setters",
            Self::COMPUTED_PROPERTIES => "computed property",
            Self::CONST_DECLARATIONS => "const declaration",
            Self::DEFAULT_PARAMETERS => "default parameter",
            Self::FOR_OF => "for-of loop",
            Self::GENERATORS => "generator",
            Self::LET_DECLARATIONS => "let declaration",
            Self::MEMBER_DECLARATIONS => "member declaration",
            Self::NEW_TARGET => "new.target",
            Self::OBJECT_DESTRUCTURING => "object destructuring",
            Self::OCTAL_LITERALS => "octal literal",
            Self::REGEXP_FLAG_U => "RegExp flag 'u'",
            Self::REGEXP_FLAG_Y => "RegExp flag 'y'",
            Self::REST_PARAMETERS => "rest parameter",
            Self::SHORTHAND_OBJECT_PROPERTIES => "shorthand object property",
            Self::SPREAD_EXPRESSIONS => "spread expression",
            Self::SUPER => "super",
            Self::TEMPLATE_LITERALS => "template literal",
            Self::MODULES => "modules",
            Self::EXPONENT_OP => "exponent operator (**)",
            Self::ASYNC_FUNCTIONS => "async function",
            Self::OBJECT_LITERALS_WITH_SPREAD => "object literals with spread",
            Self::OBJECT_PATTERN_REST => "object pattern rest",
            Self::ASYNC_GENERATORS => "async generator functions",
            Self::FOR_AWAIT_OF => "for-await-of loop",
            Self::REGEXP_FLAG_S => "RegExp flag 's'",
            Self::REGEXP_NAMED_GROUPS => "RegExp named groups",
            Self::REGEXP_UNICODE_PROPERTY_ESCAPE => "RegExp unicode property escape",
            Self::REGEXP_LOOKBEHIND => "RegExp Lookbehind",
            Self::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP => {
                "Unescaped unicode line or paragraph separator"
            }
            Self::OPTIONAL_CATCH_BINDING => "Optional catch binding",
            Self::DYNAMIC_IMPORT => "Dynamic module import",
            Self::BIGINT => "bigint",
            Self::IMPORT_META => "import.meta",
            Self::NULL_COALESCE_OP => "Nullish coalescing",
            Self::OPTIONAL_CHAINING => "Optional chaining",
            Self::NUMERIC_SEPARATOR => "numeric separator",
            Self::LOGICAL_ASSIGNMENT => "Logical assignments",
            Self::PUBLIC_CLASS_FIELDS => "Public class fields",
            Self::CLASS_STATIC_BLOCK => "Class static block",
            Self::REGEXP_FLAG_D => "RegExp flag 'd'",
            Self::TOP_LEVEL_AWAIT => "Top-level await",
            Self::ES_NEXT_RUNTIME => "es_next runtime",
            Self::ES_UNSTABLE_RUNTIME => "es_unstable runtime",
            Self::PRIVATE_ELEMENTS => "Private elements",
            Self::TYPE_ANNOTATION => "type annotation",
        };
        let version = match self {
            Self::REGEXP_SYNTAX => LangVersion::ES3,
            Self::ES3_KEYWORDS_AS_IDENTIFIERS => LangVersion::ES5,
            Self::GETTER => LangVersion::ES5,
            Self::KEYWORDS_AS_PROPERTIES => LangVersion::ES5,
            Self::SETTER => LangVersion::ES5,
            Self::STRING_CONTINUATION => LangVersion::ES5,
            Self::TRAILING_COMMA => LangVersion::ES5,
            Self::ARRAY_DESTRUCTURING => LangVersion::ES2015,
            Self::ARRAY_PATTERN_REST => LangVersion::ES2015,
            Self::ARROW_FUNCTIONS => LangVersion::ES2015,
            Self::BINARY_LITERALS => LangVersion::ES2015,
            Self::BLOCK_SCOPED_FUNCTION_DECLARATION => LangVersion::ES2015,
            Self::CLASSES => LangVersion::ES2015,
            Self::CLASS_GETTER_SETTER => LangVersion::ES2015,
            Self::COMPUTED_PROPERTIES => LangVersion::ES2015,
            Self::CONST_DECLARATIONS => LangVersion::ES2015,
            Self::DEFAULT_PARAMETERS => LangVersion::ES2015,
            Self::FOR_OF => LangVersion::ES2015,
            Self::GENERATORS => LangVersion::ES2015,
            Self::LET_DECLARATIONS => LangVersion::ES2015,
            Self::MEMBER_DECLARATIONS => LangVersion::ES2015,
            Self::NEW_TARGET => LangVersion::ES2015,
            Self::OBJECT_DESTRUCTURING => LangVersion::ES2015,
            Self::OCTAL_LITERALS => LangVersion::ES2015,
            Self::REGEXP_FLAG_U => LangVersion::ES2015,
            Self::REGEXP_FLAG_Y => LangVersion::ES2015,
            Self::REST_PARAMETERS => LangVersion::ES2015,
            Self::SHORTHAND_OBJECT_PROPERTIES => LangVersion::ES2015,
            Self::SPREAD_EXPRESSIONS => LangVersion::ES2015,
            Self::SUPER => LangVersion::ES2015,
            Self::TEMPLATE_LITERALS => LangVersion::ES2015,
            Self::MODULES => LangVersion::ES2015,
            Self::EXPONENT_OP => LangVersion::ES2016,
            Self::ASYNC_FUNCTIONS => LangVersion::ES2017,
            Self::OBJECT_LITERALS_WITH_SPREAD => LangVersion::ES2018,
            Self::OBJECT_PATTERN_REST => LangVersion::ES2018,
            Self::ASYNC_GENERATORS => LangVersion::ES2018,
            Self::FOR_AWAIT_OF => LangVersion::ES2018,
            Self::REGEXP_FLAG_S => LangVersion::ES2018,
            Self::REGEXP_NAMED_GROUPS => LangVersion::ES2018,
            Self::REGEXP_UNICODE_PROPERTY_ESCAPE => LangVersion::ES2018,
            Self::REGEXP_LOOKBEHIND => LangVersion::ES2018,
            Self::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP => LangVersion::ES2019,
            Self::OPTIONAL_CATCH_BINDING => LangVersion::ES2019,
            Self::DYNAMIC_IMPORT => LangVersion::ES2020,
            Self::BIGINT => LangVersion::ES2020,
            Self::IMPORT_META => LangVersion::ES2020,
            Self::NULL_COALESCE_OP => LangVersion::ES2020,
            Self::OPTIONAL_CHAINING => LangVersion::ES2020,
            Self::NUMERIC_SEPARATOR => LangVersion::ES2021,
            Self::LOGICAL_ASSIGNMENT => LangVersion::ES2021,
            Self::PUBLIC_CLASS_FIELDS => LangVersion::ES2022,
            Self::CLASS_STATIC_BLOCK => LangVersion::ES2022,
            Self::REGEXP_FLAG_D => LangVersion::ES2022,
            Self::TOP_LEVEL_AWAIT => LangVersion::ES_UNSUPPORTED,
            Self::ES_NEXT_RUNTIME => LangVersion::ES_NEXT,
            Self::ES_UNSTABLE_RUNTIME => LangVersion::ES_UNSTABLE,
            Self::PRIVATE_ELEMENTS => LangVersion::ES_UNSUPPORTED,
            Self::TYPE_ANNOTATION => LangVersion::TYPESCRIPT,
        };
        FeatureData::new(name, version)
    }
    pub const fn name(self) -> &'static str {
        self.data().name
    }
    pub const fn version(self) -> LangVersion {
        self.data().version
    }
}
impl std::fmt::Display for Feature {
    // port: FeatureSet.Feature#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Represents various aspects of language version and support.
///
/// <p>This is somewhat redundant with LanguageMode, but is separate for two reasons: (1) it's used
/// for parsing, which cannot depend on LanguageMode, and (2) it's concerned with slightly different
/// nuances: implemented features and modules rather than strictness.
///
/// <p>In the long term, it would be good to disentangle all these concerns and pull out a single
/// LanguageSyntax enum with a separate strict mode flag, and then these could possibly be unified.
///
/// <p>Instances of this class are immutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureSet {
    features: u128,
}
impl FeatureSet {
    // The bare minimum set of features.
    pub const BARE_MINIMUM: Self = Self::empty_enum_set();
    // Features from ES3.
    pub const ES3: Self = Self::BARE_MINIMUM.union(LangVersion::ES3.features());
    // Features from ES5 only.
    pub const ES5: Self = Self::ES3.union(LangVersion::ES5.features());
    // All ES2015 features, including modules.
    pub const ES2015_MODULES: Self = Self::ES5.union(LangVersion::ES2015.features());
    // The full set of ES2015 features, not including modules.
    pub const ES2015: Self =
        Self::ES2015_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2016_MODULES: Self = Self::ES2015_MODULES.union(LangVersion::ES2016.features());
    pub const ES2016: Self =
        Self::ES2016_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2017_MODULES: Self = Self::ES2016_MODULES.union(LangVersion::ES2017.features());
    pub const ES2017: Self =
        Self::ES2017_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2018_MODULES: Self = Self::ES2017_MODULES.union(LangVersion::ES2018.features());
    pub const ES2018: Self =
        Self::ES2018_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2019_MODULES: Self = Self::ES2018_MODULES.union(LangVersion::ES2019.features());
    pub const ES2019: Self =
        Self::ES2019_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2020_MODULES: Self = Self::ES2019_MODULES.union(LangVersion::ES2020.features());
    pub const ES2020: Self =
        Self::ES2020_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2021_MODULES: Self = Self::ES2020_MODULES.union(LangVersion::ES2021.features());
    pub const ES2021: Self =
        Self::ES2021_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    pub const ES2022_MODULES: Self = Self::ES2021_MODULES.union(LangVersion::ES2022.features());
    pub const ES2022: Self =
        Self::ES2022_MODULES.difference(Self::from_features(&[Feature::MODULES]));
    // NOTE: when ES2023 is added, the BROWSER_2024 and BROWSER_2025 FeatureSets defined below should
    // be updated to include it.

    // Set of all fully supported features, even those part of language versions not fully supported
    pub const ES_NEXT: Self = Self::ES2022_MODULES.union(LangVersion::ES_NEXT.features());
    // Set of features fully supported in checks, even those not fully supported in optimizations
    pub const ES_UNSTABLE: Self = Self::ES_NEXT.union(LangVersion::ES_UNSTABLE.features());
    // Set of all features that can be parsed, even those not yet fully supported in checks.
    pub const ES_UNSUPPORTED: Self =
        Self::ES_UNSTABLE.union(LangVersion::ES_UNSUPPORTED.features());
    // NOTE: The BROWSER_20XX FeatureSets are the set of features supported by major browsers as of
    // January 1st of the given year. So, typically, take the year of the BFSY, subtract 1 to get
    // the correct ES20XX_MODULES FeatureSet as the base, then exclude any features as needed.
    //
    // Features and their supported browser versions can be found at
    // https://compat-table.github.io/compat-table/es2016plus/ or https://caniuse.com/
    //
    // The format for excluded features is: [Browser name] [version] (version release year)
    // Or, if a browser has no support yet: [Browser name] (unsupported)
    //
    // All excluded features should have a (year) this is greater than or equal to the BFSY.
    pub const BROWSER_2019: Self = Self::ES2018_MODULES.difference(Self::from_features(&[
        // NOTE: These 4 are excluded because, historically, we defined BROWSER_2019 as ES2017.
        // See cl/266708942 and cl/328623467 for context.
        // Chrome 60 (2017). Firefox 55 (2017). Safari 11.1 (2018).
        Feature::OBJECT_LITERALS_WITH_SPREAD,
        // Chrome 60 (2017). Firefox 55 (2017). Safari 11.1 (2018).
        Feature::OBJECT_PATTERN_REST,
        // Chrome 63 (2017). Firefox 57 (2017). Safari 12 (2018).
        Feature::ASYNC_GENERATORS,
        // Chrome 63 (2017). Firefox 57 (2017). Safari 12 (2018).
        Feature::FOR_AWAIT_OF,
        // Chrome 62 (2017). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_FLAG_S,
        // Chrome 62 (2017). Firefox 78 (2020). Safari 16.4 (2023).
        Feature::REGEXP_LOOKBEHIND,
        // Chrome 64 (2018). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_NAMED_GROUPS,
        // Chrome 64 (2018). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
    ]));
    pub const BROWSER_2020: Self = Self::ES2019_MODULES.difference(Self::from_features(&[
        // Chrome 62 (2017). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_FLAG_S,
        // Chrome 64 (2018). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_NAMED_GROUPS,
        // Chrome 64 (2018). Firefox 78 (2020). Safari 11.1 (2018).
        Feature::REGEXP_UNICODE_PROPERTY_ESCAPE,
        // Chrome 62 (2017). Firefox 78 (2020). Safari 16.4 (2023).
        Feature::REGEXP_LOOKBEHIND,
    ]));
    pub const BROWSER_2021: Self = Self::ES2020_MODULES.difference(Self::from_features(&[
        // Chrome 62 (2017). Firefox 78 (2020). Safari 16.4 (2023).
        Feature::REGEXP_LOOKBEHIND,
    ]));
    pub const BROWSER_2022: Self = Self::ES2021_MODULES.difference(Self::from_features(&[
        // Chrome 62 (2017). Firefox 78 (2020). Safari 16.4 (2023).
        Feature::REGEXP_LOOKBEHIND,
    ]));
    pub const BROWSER_2023: Self = Self::ES2022_MODULES.difference(Self::from_features(&[
        // Chrome 62 (2017). Firefox 78 (2020). Safari 16.4 (2023).
        Feature::REGEXP_LOOKBEHIND,
        // Chrome 94 (2021). Firefox 93 (2021). Safari 16.4 (2023).
        Feature::CLASS_STATIC_BLOCK,
    ]));
    // Note: Only "Hashbang Grammar" is an ES2023 syntax feature. Its versions are:
    // Chrome 74 (2019). Firefox 67 (2019). Safari 13.1 (2020).
    pub const BROWSER_2024: Self = Self::ES2022_MODULES;
    // Note: Only "RegExp `v` flag" is an ES2024 syntax feature. Its versions are:
    // Chrome 112 (2023). Firefox 116 (2023). Safari 17 (2023).
    pub const BROWSER_2025: Self = Self::ES2022_MODULES;
    // Note: Only "RegExp syntax features again for ES2025. The 'i', 'm', and 's' flags, as well
    // as support for duplicate named capture groups. As of Jan 5 2026, Safari hasn't yet
    // implemented the new flags.
    pub const BROWSER_2026: Self = Self::ES2022_MODULES;
    pub const ALL: Self = Self::ES_UNSUPPORTED.union(LangVersion::TYPESCRIPT.features());

    // port: FeatureSet#<init>
    pub const fn new(features: u128) -> Self {
        // ImmutableSet will only use an EnumSet if the set starts as an EnumSet.
        Self { features }
    }
    /// Returns a string representation suitable for use in polyfill definition files and encoding in
    /// depgraph and deps.js files.
    // port: FeatureSet#version
    pub fn version(self) -> &'static str {
        if Self::ES3.contains(self) {
            return "es3";
        }
        if Self::ES5.contains(self) {
            return "es5";
        }
        if Self::ES2015_MODULES.contains(self) {
            return "es6";
        }
        if Self::ES2016_MODULES.contains(self) {
            return "es7";
        }
        if Self::ES2017_MODULES.contains(self) {
            return "es8";
        }
        if Self::ES2018_MODULES.contains(self) {
            return "es9";
        }
        if Self::ES2019_MODULES.contains(self) {
            return "es_2019";
        }
        if Self::ES2020_MODULES.contains(self) {
            return "es_2020";
        }
        if Self::ES2021_MODULES.contains(self) {
            return "es_2021";
        }
        if Self::ES2022_MODULES.contains(self) {
            return "es_2022";
        }
        if Self::ES_NEXT.contains(self) {
            return "es_next";
        }
        if Self::ES_UNSTABLE.contains(self) {
            return "es_unstable";
        }
        if Self::ES_UNSUPPORTED.contains(self) {
            return "es_unsupported";
        }
        if Self::ALL.contains(self) {
            return "all";
        }
        panic!("{}", self);
    }
    /// Returns a string representation useful for debugging.
    ///
    /// @deprecated Please use {@link #version()} instead.
    // port: FeatureSet#versionForDebugging
    pub fn version_for_debugging(self) -> &'static str {
        self.version()
    }
    // port: FeatureSet#without
    pub fn without(self, feature_to_remove: impl Into<Self>) -> Self {
        self.difference(feature_to_remove.into())
    }
    // port: FeatureSet#without
    pub fn without_set(self, other: Self) -> Self {
        self.difference(other)
    }
    // port: FeatureSet#withoutTypes
    pub const fn without_types(self) -> Self {
        self.difference(LangVersion::TYPESCRIPT.features())
    }
    /// Returns a new {@link FeatureSet} including all features of both {@code this} and {@code other}.
    // port: FeatureSet#union
    pub const fn union(self, other: Self) -> Self {
        Self::union_sets(self, other)
    }
    // Does this {@link FeatureSet} contain at least one of the features of {@code other}?
    // port: FeatureSet#containsAtLeastOneOf
    pub fn contains_at_least_one_of(self, other: Self) -> bool {
        for other_feature in other.get_features() {
            if self.has(other_feature) {
                return true;
            }
        }
        false
    }
    /// Does this {@link FeatureSet} contain all of the features of {@code other}?
    // port: FeatureSet#contains
    pub fn contains(self, other: impl Into<Self>) -> bool {
        let other = other.into();
        (self.features & other.features) == other.features
    }
    // Does this {@link FeatureSet} contain the given feature?
    // port: FeatureSet#contains
    pub const fn contains_feature(self, feature: Feature) -> bool {
        self.has(feature)
    }
    // port: FeatureSet#emptyEnumSet
    pub const fn empty_enum_set() -> Self {
        Self::new(0)
    }
    // port: FeatureSet#enumSetOf
    pub fn enum_set_of(set: &[Feature]) -> Self {
        Self::from_features(set)
    }
    // port: FeatureSet#add
    pub const fn add(features: Self, feature: Feature) -> Self {
        Self::new(features.features | (1u128 << feature as u32))
    }
    // port: FeatureSet#union
    pub const fn union_sets(features: Self, new_features: Self) -> Self {
        Self::new(features.features | new_features.features)
    }
    // port: FeatureSet#difference
    pub const fn difference(self, removed_features: Self) -> Self {
        Self::new(self.features & !removed_features.features)
    }
    // Returns a feature set combining all the features from {@code this} and {@code feature}.
    // port: FeatureSet#with
    pub fn with(self, feature: Feature) -> Self {
        if self.has(feature) {
            return self;
        }
        Self::add(self, feature)
    }
    /// Returns a feature set combining all the features from {@code this} and {@code newFeatures}.
    // port: FeatureSet#with
    pub fn with_features(self, new_features: &[Feature]) -> Self {
        self.union(Self::from_features(new_features))
    }
    /// Returns a feature set combining all the features from {@code this} and {@code newFeatures}.
    // port: FeatureSet#with
    pub fn with_set(self, new_features: &[Feature]) -> Self {
        self.union(Self::from_features(new_features))
    }
    /// Returns a feature set combining all the features from {@code this} and {@code newFeatures}.
    // port: FeatureSet#with
    pub const fn with_feature_set(self, new_features: Self) -> Self {
        self.union(new_features)
    }
    // Does this {@link FeatureSet} include {@code feature}?
    // port: FeatureSet#has
    pub const fn has(self, feature: Feature) -> bool {
        self.features & (1u128 << feature as u32) != 0
    }
    // port: FeatureSet#getFeatures
    pub fn get_features(self) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .copied()
            .filter(|&feature| self.has(feature))
            .collect()
    }
    // port: FeatureSet#equals
    pub fn equals(self, other: Self) -> bool {
        self.features == other.features
    }
    /// Parses known strings into feature sets.
    // port: FeatureSet#valueOf
    pub fn value_of(name: &str) -> Result<Self, String> {
        if name == "es3" {
            return Ok(Self::ES3);
        }
        if name == "es5" {
            return Ok(Self::ES5);
        }
        if name == "es_2015" {
            return Ok(Self::ES2015);
        }
        if name == "es_2016" {
            return Ok(Self::ES2016);
        }
        if name == "es_2017" {
            return Ok(Self::ES2017);
        }
        if name == "es_2018" {
            return Ok(Self::ES2018);
        }
        if name == "es_2019" {
            return Ok(Self::ES2019);
        }
        if name == "es_2020" {
            return Ok(Self::ES2020);
        }
        if name == "es_2021" {
            return Ok(Self::ES2021);
        }
        if name == "es_2022" {
            return Ok(Self::ES2022);
        }
        if name == "es6" {
            return Ok(Self::ES2015);
        }
        if name == "es7" {
            return Ok(Self::ES2016);
        }
        if name == "es8" {
            return Ok(Self::ES2017);
        }
        if name == "es9" {
            return Ok(Self::ES2018);
        }
        if name == "es_next" {
            return Ok(Self::ES_NEXT);
        }
        if name == "es_unstable" {
            return Ok(Self::ES_UNSTABLE);
        }
        if name == "es_unsupported" {
            return Ok(Self::ES_UNSUPPORTED);
        }
        if name == "all" {
            return Ok(Self::ALL);
        }
        Err(format!("No such FeatureSet: {name}"))
    }
    /// Variant of {@link #valueOf} that outputs a {@code BROWSER_20XX} feature set equivalent. This
    /// will generally exclude some not-yet-implemented features from a given {@code ES20XX} while also
    /// including {@link Feature#MODULES}.
    ///
    /// <p>Note: The returned feature set will include the {@link Feature#MODULES} feature which you
    /// may want to exclude manually.
    // port: FeatureSet#browserFeatureSetValueOf
    pub fn browser_feature_set_value_of(name: &str) -> Result<Self, String> {
        let feature_set = Self::value_of(name)?;
        if feature_set.contains(Self::ES_NEXT) {
            // For ES_NEXT and ES_UNSTABLE, we need to return the raw featureSet to retain the RUNTIME
            // features which pin polyfills to future versions.
            return Ok(feature_set);
        }
        if feature_set.contains(Self::ES2022) {
            return Ok(Self::BROWSER_2023);
        }
        if feature_set.contains(Self::ES2021) {
            return Ok(Self::BROWSER_2022);
        }
        if feature_set.contains(Self::ES2020) {
            return Ok(Self::BROWSER_2021);
        }
        if feature_set.contains(Self::ES2019) {
            return Ok(Self::BROWSER_2020);
        }
        if feature_set.contains(Self::ES2018) {
            return Ok(Self::BROWSER_2019);
        }
        Ok(feature_set)
    }
    /// Returns a {@code FeatureSet} containing all known features.
    ///
    /// <p>NOTE: {@code PassFactory} classes that claim to support {@code FeatureSet.all()} should be
    /// only those that cannot be broken by new features being added to the language. Mainly these are
    /// passes that don't have to actually look at the AST at all, like empty marker passes.
    // port: FeatureSet#all
    pub const fn all() -> Self {
        Self::ALL
    }
    // port: FeatureSet#latest
    pub const fn latest() -> Self {
        Self::ES_UNSUPPORTED
    }
    pub const fn from_features(features: &[Feature]) -> Self {
        let mut result = Self::empty_enum_set();
        let mut i = 0;
        while i < features.len() {
            result = Self::add(result, features[i]);
            i += 1;
        }
        result
    }
}
impl std::hash::Hash for FeatureSet {
    // port: FeatureSet#hashCode
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_features().hash(state);
    }
}
impl From<Feature> for FeatureSet {
    fn from(feature: Feature) -> Self {
        Self::add(Self::BARE_MINIMUM, feature)
    }
}
impl From<&[Feature]> for FeatureSet {
    fn from(features: &[Feature]) -> Self {
        Self::from_features(features)
    }
}
impl<const N: usize> From<[Feature; N]> for FeatureSet {
    fn from(features: [Feature; N]) -> Self {
        Self::from_features(&features)
    }
}
impl std::fmt::Display for FeatureSet {
    // port: FeatureSet#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for (index, feature) in self.get_features().iter().enumerate() {
            if index != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{feature}")?;
        }
        f.write_str("]")
    }
}

#[cfg(test)]
mod tests;
