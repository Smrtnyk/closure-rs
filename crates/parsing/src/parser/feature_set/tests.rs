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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/parsing/parser/FeatureSetTest.java.

use super::*;
use crate::parser::testing::feature_set_subject::assert_fs;

// port: FeatureSetTest#testContains
#[test]
fn test_contains() {
    assert_fs(FeatureSet::ALL).has(Feature::TYPE_ANNOTATION);
    assert_fs(FeatureSet::ALL).has(Feature::MODULES);
}

// port: FeatureSetTest#testWithoutModules
#[test]
fn test_without_modules() {
    assert_fs(FeatureSet::ALL.without(Feature::MODULES)).has(Feature::TYPE_ANNOTATION);
    assert_fs(FeatureSet::ALL.without(Feature::MODULES)).does_not_have(Feature::MODULES);
}

// port: FeatureSetTest#testWithoutTypes
#[test]
fn test_without_types() {
    assert_fs(FeatureSet::ALL.without_types()).does_not_have(Feature::TYPE_ANNOTATION);
    assert_fs(FeatureSet::ALL.without_types()).has(Feature::MODULES);
}

// port: FeatureSetTest#testEsOrdering
#[test]
fn test_es_ordering() {
    assert_fs(FeatureSet::ALL).contains(FeatureSet::ES_UNSUPPORTED);
    assert_fs(FeatureSet::ES_UNSUPPORTED).contains(FeatureSet::ES_UNSTABLE);
    assert_fs(FeatureSet::ES_UNSTABLE).contains(FeatureSet::ES_NEXT);
    assert_fs(FeatureSet::ES_NEXT).contains(FeatureSet::ES2022);
    assert_fs(FeatureSet::ES2022).contains(FeatureSet::ES2021);
    assert_fs(FeatureSet::ES2021).contains(FeatureSet::ES2020);
    assert_fs(FeatureSet::ES2020).contains(FeatureSet::ES2019);
    assert_fs(FeatureSet::ES2019).contains(FeatureSet::ES2018);
    assert_fs(FeatureSet::ES2018).contains(FeatureSet::ES2017);
    assert_fs(FeatureSet::ES2017).contains(FeatureSet::ES2016);
    assert_fs(FeatureSet::ES2016).contains(FeatureSet::ES2015);
    assert_fs(FeatureSet::ES2015).contains(FeatureSet::ES5);
    assert_fs(FeatureSet::ES5).contains(FeatureSet::ES3);
    assert_fs(FeatureSet::ES3).contains(FeatureSet::BARE_MINIMUM);
}

// port: FeatureSetTest#testEsModuleOrdering
#[test]
fn test_es_module_ordering() {
    assert_fs(FeatureSet::ES2022_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2022);
    assert_fs(FeatureSet::ES2021_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2021);
    assert_fs(FeatureSet::ES2020_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2020);
    assert_fs(FeatureSet::ES2019_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2019);
    assert_fs(FeatureSet::ES2018_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2018);
    assert_fs(FeatureSet::ES2017_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2017);
    assert_fs(FeatureSet::ES2016_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2016);
    assert_fs(FeatureSet::ES2015_MODULES.without(Feature::MODULES)).equals(FeatureSet::ES2015);
}

// port: FeatureSetTest#testVersionForDebugging
#[test]
fn test_version_for_debugging() {
    // ES_NEXT, ES_UNSUPPORTED are tested separately - see below
    assert_eq!(FeatureSet::ES3.version(), "es3");
    assert_eq!(FeatureSet::ES5.version(), "es5");
    assert_eq!(FeatureSet::ES2015.version(), "es6");
    assert_eq!(FeatureSet::ES2015_MODULES.version(), "es6");
    assert_eq!(FeatureSet::ES2016.version(), "es7");
    assert_eq!(FeatureSet::ES2016_MODULES.version(), "es7");
    assert_eq!(FeatureSet::ES2017.version(), "es8");
    assert_eq!(FeatureSet::ES2017_MODULES.version(), "es8");
    assert_eq!(FeatureSet::ES2018.version(), "es9");
    assert_eq!(FeatureSet::ES2018_MODULES.version(), "es9");
    assert_eq!(FeatureSet::ES2019.version(), "es_2019");
    assert_eq!(FeatureSet::ES2019_MODULES.version(), "es_2019");
    assert_eq!(FeatureSet::ES2020.version(), "es_2020");
    assert_eq!(FeatureSet::ES2020_MODULES.version(), "es_2020");
    assert_eq!(FeatureSet::ES2021.version(), "es_2021");
    assert_eq!(FeatureSet::ES2021_MODULES.version(), "es_2021");
    assert_eq!(FeatureSet::ES2022.version(), "es_2022");
    assert_eq!(FeatureSet::ES2022_MODULES.version(), "es_2022");
    assert_eq!(FeatureSet::ALL.version(), "all");
}

// port: FeatureSetTest#testEsNextAndNewer
#[test]
fn test_es_next_and_newer() {
    // ES_NEXT and ES_UNSTABLE contain *_RUNTIME features to make these feature sets NOT moving
    // targets because polyfills can use "es_next" or "es_unstable" as their fromLang and we don't
    // want to incorrectly prune the polyfills in the case that no features remain in the set.
    assert_eq!(FeatureSet::ES_NEXT.version(), "es_next");
    assert_eq!(FeatureSet::ES_UNSTABLE.version(), "es_unstable");

    // ES_UNSUPPORTED is a moving target that may not have any unique features in which case it will
    // resolve to "es_unstable". This will change as new features are added and removed.
    assert_eq!(FeatureSet::ES_UNSUPPORTED.version(), "es_unsupported");
}

// port: FeatureSetTest#testValueOf
#[test]
fn test_value_of() {
    assert_fs(FeatureSet::value_of("es3").unwrap()).equals(FeatureSet::ES3);
    assert_fs(FeatureSet::value_of("es5").unwrap()).equals(FeatureSet::ES5);
    assert_fs(FeatureSet::value_of("es6").unwrap()).equals(FeatureSet::ES2015);
    assert_fs(FeatureSet::value_of("es7").unwrap()).equals(FeatureSet::ES2016);
    assert_fs(FeatureSet::value_of("es8").unwrap()).equals(FeatureSet::ES2017);
    assert_fs(FeatureSet::value_of("es_2018").unwrap()).equals(FeatureSet::ES2018);
    assert_fs(FeatureSet::value_of("es9").unwrap()).equals(FeatureSet::ES2018);
    assert_fs(FeatureSet::value_of("es_2019").unwrap()).equals(FeatureSet::ES2019);
    assert_fs(FeatureSet::value_of("es_2020").unwrap()).equals(FeatureSet::ES2020);
    assert_fs(FeatureSet::value_of("es_2021").unwrap()).equals(FeatureSet::ES2021);
    assert_fs(FeatureSet::value_of("es_2022").unwrap()).equals(FeatureSet::ES2022);
    assert_fs(FeatureSet::value_of("es_next").unwrap()).equals(FeatureSet::ES_NEXT);
    assert_fs(FeatureSet::value_of("es_unstable").unwrap()).equals(FeatureSet::ES_UNSTABLE);
    assert_fs(FeatureSet::value_of("es_unsupported").unwrap()).equals(FeatureSet::ES_UNSUPPORTED);
    assert_fs(FeatureSet::value_of("all").unwrap()).equals(FeatureSet::ALL);
    assert!(FeatureSet::value_of("bad").is_err());
}
