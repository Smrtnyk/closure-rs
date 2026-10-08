/*
 * Copyright 2008 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   test/com/google/javascript/jscomp/disambiguate/AmbiguatePropertiesTest.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorFindPropertyReferencesTest.java,
//   test/com/google/javascript/jscomp/disambiguate/ColorGraphBuilderTest.java,
//   test/com/google/javascript/jscomp/disambiguate/DisambiguatePropertiesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The CompilerTestCase tests of the property (dis)ambiguation passes (AmbiguatePropertiesTest,
//! DisambiguatePropertiesTest, ColorFindPropertyReferencesTest, ColorGraphBuilderTest). Every
//! test/testSame call they make is a corpus unit record; these tests replay each class's records
//! through the Rust harness ports (the unit_replay runner) and require the Java outcome, including
//! the `testFieldsAfter` holder fields the descriptors opt in to. All four classes enable type
//! checking (TypeCheck). Pass counts are floors and no record may fail.
use closure_testing::{
    corpus,
    replay::replay_main::{RecordResult, Runner},
};

/// Replays every record of `class` and returns them.
// port: ReplayMain#main (--classes <class>)
fn replay(class: &str) -> Vec<RecordResult> {
    let mut records = Vec::new();
    let report = Runner {
        corpus: corpus::corpus_unit_dir(),
        classes: Some(vec![class.to_string()]),
        sample: None,
    }
    .run(
        |r| {
            records.push(r.clone());
            Ok(())
        },
        |_, _| {},
    )
    .unwrap();
    assert_eq!(report.harness_errors, 0);
    records
}

/// Asserts that `class` has `expected` records, that at least `passing` of them pass and that
/// none fails.
fn assert_records(class: &str, expected: usize, passing: usize) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let mut pass = 0;
    for r in &records {
        match r.status.as_str() {
            "pass" => pass += 1,
            "unported" => {}
            _ => panic!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            ),
        }
    }
    assert!(
        pass >= passing,
        "{class} passing records: {pass} < {passing}"
    );
}

// port: AmbiguatePropertiesTest (139 recorded calls)
#[test]
fn ambiguate_properties_test() {
    assert_records("AmbiguatePropertiesTest", 139, 139);
}

// port: DisambiguatePropertiesTest (39 recorded calls)
#[test]
fn disambiguate_properties_test() {
    assert_records("DisambiguatePropertiesTest", 39, 39);
}

/// The holder ColorFindPropertyReferencesTest_Helpers; testFieldsAfter compares
/// labeledStatementMap, finder (its propIndex) and flattener.
// port: ColorFindPropertyReferencesTest (18 recorded calls)
#[test]
fn color_find_property_references_test() {
    assert_records("ColorFindPropertyReferencesTest", 18, 18);
}

/// The holder ColorGraphBuilderTest_Helpers; testFieldsAfter compares graphNodeFactory and
/// labelToId. `disambiguationSupertypes_createConnection` (no record) is ported natively in
/// crates/jscomp/tests/disambiguate_test.rs.
// port: ColorGraphBuilderTest (12 recorded calls)
#[test]
fn color_graph_builder_test() {
    assert_records("ColorGraphBuilderTest", 12, 12);
}
