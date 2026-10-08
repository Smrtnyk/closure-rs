/*
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
 * Copyright 2012 The Closure Compiler Authors.
 * Copyright 2023 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/AngularPassTest.java,
//   test/com/google/javascript/jscomp/ClosureCodeRemovalTest.java,
//   test/com/google/javascript/jscomp/ClosureOptimizePrimitivesTest.java,
//   test/com/google/javascript/jscomp/GenerateExportsTest.java,
//   test/com/google/javascript/jscomp/ProcessDefinesTest.java,
//   test/com/google/javascript/jscomp/ReplaceCssNamesTest.java,
//   test/com/google/javascript/jscomp/ReplaceIdGeneratorsTest.java,
//   test/com/google/javascript/jscomp/ReplaceTogglesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of the Closure optimization passes (ProcessDefinesTest, ClosureCodeRemovalTest,
//! ClosureOptimizePrimitivesTest, ReplaceIdGeneratorsTest, ReplaceCssNamesTest,
//! ReplaceTogglesTest, GenerateExportsTest, AngularPassTest) are CompilerTestCase tests. Every
//! test/testSame/testError/testWarning call they make is a corpus unit record; these tests replay
//! each class's records through the Rust CompilerTestCase port (the unit_replay runner) and
//! require the Java outcome. IdMappingUtilTest is ported test by test in
//! crates/jscomp/tests/id_mapping_util_test.rs.
//!
//! Records that need a class that is not ported are "unported" by exactly that class (listed per
//! test below); no record may fail.
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

/// Asserts that `class` has `expected` records, that at least `passing` of them pass (a floor:
/// more may pass as other classes are ported) and that every other one
/// is unported by one of `allowed_unported_by` (a class that is not ported).
fn assert_records(class: &str, expected: usize, passing: usize, allowed_unported_by: &[&str]) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let bad: Vec<_> = records
        .iter()
        .filter(|r| match r.status.as_str() {
            "pass" => false,
            "unported" => !r
                .unported_by
                .as_deref()
                .is_some_and(|by| allowed_unported_by.contains(&by)),
            _ => true,
        })
        .map(|r| {
            format!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            )
        })
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let passed = records.iter().filter(|r| r.status == "pass").count();
    assert!(
        passed >= passing,
        "{class} passing records: {passed} < {passing}"
    );
}

// port: ReplaceTogglesTest (13 @Test methods, 25 recorded calls)
#[test]
fn replace_toggles_test() {
    assert_records("ReplaceTogglesTest", 25, 25, &[]);
}

// port: ClosureOptimizePrimitivesTest (36 @Test methods, 42 recorded calls)
#[test]
fn closure_optimize_primitives_test() {
    assert_records("ClosureOptimizePrimitivesTest", 42, 42, &[]);
}

// port: AngularPassTest (33 @Test methods, 57 recorded calls)
#[test]
fn angular_pass_test() {
    assert_records("AngularPassTest", 57, 57, &[]);
}

// port: GenerateExportsTest (52 @Test methods, 79 recorded calls)
#[test]
fn generate_exports_test() {
    assert_records("GenerateExportsTest", 79, 79, &[]);
}

// port: ReplaceIdGeneratorsTest (42 @Test methods, 102 recorded calls)
#[test]
fn replace_id_generators_test() {
    assert_records("ReplaceIdGeneratorsTest", 102, 102, &[]);
}

/// TypeCheck runs first; ChangeVerifier prints typed nodes through the compiler's registry.
// port: ClosureCodeRemovalTest (22 @Test methods, 22 recorded calls)
#[test]
fn closure_code_removal_test() {
    assert_records("ClosureCodeRemovalTest", 22, 22, &[]);
}

/// TypeCheck runs first; ChangeVerifier prints typed nodes through the compiler's registry.
// port: ReplaceCssNamesTest (32 @Test methods, 71 recorded calls)
#[test]
fn replace_css_names_test() {
    assert_records("ReplaceCssNamesTest", 71, 71, &[]);
}

/// Every record also compares the host field `namespace`, a depth-2 dump of the GlobalNamespace
/// the pass built (GlobalNamespace#unit_dump_fields).
// port: ProcessDefinesTest (98 @Test methods, 139 recorded calls)
#[test]
fn process_defines_test() {
    assert_records("ProcessDefinesTest", 139, 139, &[]);
}
