/*
 * Copyright 2005 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/DefaultNameGeneratorTest.java.

use closure_jscomp::{
    default_name_generator::DefaultNameGenerator,
    name_generator::{NameGenerator, ReservedNames},
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::js_string::JsString;
use std::sync::{Arc, RwLock};

// port: DefaultNameGeneratorTest#RESERVED_NAMES
fn reserved_names() -> ReservedNames {
    Arc::new(RwLock::new(
        [JsString::from("ba"), JsString::from("xba")]
            .into_iter()
            .collect(),
    ))
}
// port: DefaultNameGeneratorTest#generate
fn generate(ng: &mut DefaultNameGenerator, prefix: &str, num: i32) -> Vec<JsString> {
    (0..num)
        .map(|_| {
            let result = ng.generate_next_name();
            assert!(
                result.starts_with(JsString::from(prefix)),
                "Error: {result}"
            );
            result
        })
        .collect()
}
// port: DefaultNameGeneratorTest#testNameGeneratorInvalidPrefixes
#[test]
fn test_name_generator_invalid_prefixes() {
    for (prefix, message) in [
        ("123abc", "W, X, Y, Z, $]"),
        ("abc%", "W, X, Y, Z, _, 0, 1"),
    ] {
        let ex = std::panic::catch_unwind(|| {
            DefaultNameGenerator::with_reserved_characters(
                reserved_names(),
                prefix.into(),
                &IndexSet::<_>::default(),
            )
        })
        .err()
        .expect("Constructor should throw exception when prefix is invalid");
        let text = ex
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| ex.downcast_ref::<&str>().copied())
            .unwrap();
        assert!(text.contains(message));
    }
}
// port: DefaultNameGeneratorTest#testGenerate
#[test]
fn test_generate() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "".into(),
        &IndexSet::<_>::default(),
    );
    let result = generate(&mut ng, "", 106);
    for (i, s) in [
        (0, "a"),
        (25, "z"),
        (26, "A"),
        (51, "Z"),
        (52, "$"),
        (53, "aa"),
        (54, "ca"),
        (104, "$a"),
    ] {
        assert_eq!(result[i], s);
    }
    ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "x".into(),
        &IndexSet::<_>::default(),
    );
    let result = generate(&mut ng, "x", 132);
    for (i, s) in [(0, "x"), (1, "xa"), (64, "x$"), (65, "xaa"), (66, "xca")] {
        assert_eq!(result[i], s);
    }
}
// port: DefaultNameGeneratorTest#testReserve
#[test]
fn test_reserve() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "".into(),
        &IndexSet::<_>::from_iter([b'$' as u16]),
    );
    let result = generate(&mut ng, "", 106);
    for (i, s) in [
        (0, "a"),
        (25, "z"),
        (26, "A"),
        (51, "Z"),
        (52, "aa"),
        (53, "ca"),
        (103, "ab"),
    ] {
        assert_eq!(result[i], s);
    }
}
// port: DefaultNameGeneratorTest#testES6KeywordsNotGenerated
#[test]
fn test_es6_keywords_not_generated() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "le".into(),
        &IndexSet::<_>::from_iter([b'$' as u16]),
    );
    let result = generate(&mut ng, "le", 106);
    for (i, s) in [(19, "les"), (20, "leu"), (45, "leT")] {
        assert_eq!(result[i], s);
    }
    ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "awai".into(),
        &IndexSet::<_>::from_iter([b'$' as u16]),
    );
    let result = generate(&mut ng, "awai", 106);
    for (i, s) in [(19, "awais"), (20, "awaiu"), (45, "awaiT")] {
        assert_eq!(result[i], s);
    }
}
// port: DefaultNameGeneratorTest#testGenerateWithPriority1
#[test]
fn test_generate_with_priority1() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "".into(),
        &IndexSet::<_>::default(),
    );
    let result = generate(&mut ng, "", 106);
    for (i, s) in [
        (0, "a"),
        (25, "z"),
        (26, "A"),
        (51, "Z"),
        (52, "$"),
        (53, "aa"),
    ] {
        assert_eq!(result[i], s);
    }
    ng.favors(&"b".into());
    ng.reset(reserved_names(), "".into(), &IndexSet::<_>::default());
    let result = generate(&mut ng, "", 106);
    for (i, s) in [(0, "b"), (1, "a"), (2, "c"), (3, "d")] {
        assert_eq!(result[i], s);
    }
    ng.favors(&"cc".into());
    ng.reset(reserved_names(), "".into(), &IndexSet::<_>::default());
    let result = generate(&mut ng, "", 106);
    for (i, s) in [(0, "c"), (1, "b"), (2, "a"), (3, "d")] {
        assert_eq!(result[i], s);
    }
}
// port: DefaultNameGeneratorTest#testGenerateWithPriority2
#[test]
fn test_generate_with_priority2() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "".into(),
        &IndexSet::<_>::default(),
    );
    let result = generate(&mut ng, "", 106);
    for (i, s) in [
        (0, "a"),
        (25, "z"),
        (26, "A"),
        (51, "Z"),
        (52, "$"),
        (53, "aa"),
    ] {
        assert_eq!(result[i], s);
    }
    ng.favors(&"function".into());
    ng.favors(&"function".into());
    ng.favors(&"function".into());
    ng.reset(reserved_names(), "".into(), &IndexSet::<_>::default());
    let result = generate(&mut ng, "", 106);
    for (i, s) in [
        (0, "n"),
        (1, "c"),
        (2, "f"),
        (3, "i"),
        (4, "o"),
        (5, "t"),
        (6, "u"),
        (7, "a"),
        (8, "b"),
        (9, "d"),
        (10, "e"),
        (53, "nn"),
        (54, "cn"),
    ] {
        assert_eq!(result[i], s);
    }
}
// port: DefaultNameGeneratorTest#testGenerateWithPriority3
#[test]
fn test_generate_with_priority3() {
    let mut ng = DefaultNameGenerator::with_reserved_characters(
        reserved_names(),
        "".into(),
        &IndexSet::<_>::default(),
    );
    let _result = generate(&mut ng, "", 106);
    ng.favors(&"???".into());
    ng.reset(reserved_names(), "".into(), &IndexSet::<_>::default());
    let result = generate(&mut ng, "", 106);
    assert_eq!(result[0], "a");
}
