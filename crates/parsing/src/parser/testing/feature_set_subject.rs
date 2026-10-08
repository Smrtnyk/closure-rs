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
//   src/com/google/javascript/jscomp/parsing/parser/testing/FeatureSetSubject.java.

use crate::parser::feature_set::{Feature, FeatureSet};

// port: FeatureSetSubject#assertFS
pub fn assert_fs(fs: FeatureSet) -> FeatureSetSubject {
    FeatureSetSubject::new(fs)
}
// A Truth Subject for FeatureSet. Usage:
//
// <pre>
//   import static com.google.javascript.jscomp.parsing.parser.testing.FeatureSetSubject.assertFS;
//   ...
//   assertFS(features).contains(otherFeatures);
//   assertFS(features).containsNoneOf(otherFeatures);
// </pre>
pub struct FeatureSetSubject {
    actual: FeatureSet,
}
impl FeatureSetSubject {
    // port: FeatureSetSubject#<init>
    pub fn new(feature_set: FeatureSet) -> Self {
        Self {
            actual: feature_set,
        }
    }
    // port: FeatureSetSubject#contains
    pub fn contains(&self, other: FeatureSet) {
        assert!(
            self.actual.contains(other),
            "Expected a FeatureSet containing: {}\nBut got: {}",
            other,
            self.actual
        );
    }
    // port: FeatureSetSubject#containsNoneOf
    #[allow(dead_code)]
    pub fn contains_none_of(&self, other: FeatureSet) {
        assert!(
            other.without(self.actual).equals(other),
            "Expected a FeatureSet containing none of: {}\nBut got: {}",
            other,
            self.actual
        );
    }
    // port: FeatureSetSubject#has
    pub fn has(&self, feature: Feature) {
        assert!(
            self.actual.has(feature),
            "Expected a FeatureSet that has: {}\nBut got: {}",
            feature,
            self.actual
        );
    }
    // port: FeatureSetSubject#doesNotHave
    pub fn does_not_have(&self, feature: Feature) {
        assert!(
            !self.actual.has(feature),
            "Expected a FeatureSet that doesn't have: {}\nBut got: {}",
            feature,
            self.actual
        );
    }
    // port: FeatureSetSubject#equals
    pub fn equals(&self, other: FeatureSet) {
        assert!(
            self.actual.equals(other),
            "Expected a FeatureSet equal to: {}\nBut got: {}",
            other,
            self.actual
        );
    }
}
