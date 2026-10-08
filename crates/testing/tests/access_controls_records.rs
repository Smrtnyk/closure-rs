/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CheckAccessControlsOldSyntaxTest.java,
//   test/com/google/javascript/jscomp/CheckAccessControlsTest.java,
//   test/com/google/javascript/jscomp/ImplicitNullabilityCheckTest.java,
//   test/com/google/javascript/jscomp/lint/CheckArrayWithGoogObjectTest.java,
//   test/com/google/javascript/jscomp/lint/CheckNestedNamesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of CheckAccessControls (CheckAccessControlsTest,
//! CheckAccessControlsOldSyntaxTest, ImplicitNullabilityCheckTest, CheckArrayWithGoogObjectTest,
//! CheckNestedNamesTest) are CompilerTestCase tests. Every test/testSame/testError/testWarning call
//! they make is a corpus unit record; these tests replay each class's records through the Rust
//! CompilerTestCase port (the unit_replay runner) and require the Java outcome.
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

/// Asserts that all `expected` records of `class` pass.
fn assert_all_pass(class: &str, expected: usize) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let failing: Vec<_> = records
        .iter()
        .filter(|r| r.status != "pass")
        .map(|r| {
            format!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            )
        })
        .collect();
    assert!(failing.is_empty(), "{}", failing.join("\n"));
}

// port: CheckAccessControlsTest (223 @Test methods, 232 recorded calls)
#[test]
fn check_access_controls_test() {
    assert_all_pass("CheckAccessControlsTest", 232);
}

// port: CheckAccessControlsOldSyntaxTest (210 @Test methods, 215 recorded calls)
#[test]
fn check_access_controls_old_syntax_test() {
    assert_all_pass("CheckAccessControlsOldSyntaxTest", 215);
}

// port: ImplicitNullabilityCheckTest (15 @Test methods, 46 recorded calls)
#[test]
fn implicit_nullability_check_test() {
    assert_all_pass("ImplicitNullabilityCheckTest", 46);
}

// port: CheckNestedNamesTest (13 @Test methods, 45 recorded calls)
#[test]
fn check_nested_names_test() {
    assert_all_pass("CheckNestedNamesTest", 45);
}

/// CheckArrayWithGoogObjectTest#setUp calls enableTranspile(), so the harness runs
/// TranspilationPasses#addTranspilationPasses (all transpile passes ported since transpile-misc).
// port: CheckArrayWithGoogObjectTest (4 @Test methods, 4 recorded calls)
#[test]
fn check_array_with_goog_object_test() {
    assert_all_pass("CheckArrayWithGoogObjectTest", 4);
}
