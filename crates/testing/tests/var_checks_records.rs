/*
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/ConstCheckTest.java,
//   test/com/google/javascript/jscomp/DeclaredGlobalExternsOnWindowTest.java,
//   test/com/google/javascript/jscomp/RemoveUnnecessarySyntheticExternsTest.java,
//   test/com/google/javascript/jscomp/StrictModeCheckTest.java,
//   test/com/google/javascript/jscomp/UnusedLocalsCheckTest.java,
//   test/com/google/javascript/jscomp/ValidityCheckTest.java,
//   test/com/google/javascript/jscomp/VarCheckTest.java,
//   test/com/google/javascript/jscomp/VariableReferenceCheckTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of the variable checks (VarCheckTest, VariableReferenceCheckTest,
//! UnusedLocalsCheckTest, ConstCheckTest, ValidityCheckTest, StrictModeCheckTest,
//! DeclaredGlobalExternsOnWindowTest, RemoveUnnecessarySyntheticExternsTest) are CompilerTestCase
//! tests. Every test/testSame/testError/testWarning call they make is a corpus unit record; these
//! tests replay each class's records through the Rust CompilerTestCase port (the unit_replay
//! runner) and require the Java outcome.
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
fn assert_records(class: &str, expected: usize) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    for r in &records {
        assert!(
            r.status == "pass",
            "{}[{}] {}: {} {:?} {:?}",
            r.class,
            r.index,
            r.method,
            r.status,
            r.why,
            r.unported_by
        );
    }
}

// port: VarCheckTest (128 @Test methods, 225 recorded calls)
#[test]
fn var_check_test() {
    assert_records("VarCheckTest", 225);
}

// port: VariableReferenceCheckTest (122 @Test methods, 311 recorded calls)
#[test]
fn variable_reference_check_test() {
    assert_records("VariableReferenceCheckTest", 311);
}

// port: UnusedLocalsCheckTest (47 @Test methods, 80 recorded calls)
#[test]
fn unused_locals_check_test() {
    assert_records("UnusedLocalsCheckTest", 80);
}

// port: ConstCheckTest (47 @Test methods, 54 recorded calls)
#[test]
fn const_check_test() {
    assert_records("ConstCheckTest", 54);
}

// port: ValidityCheckTest (3 @Test methods, 3 recorded calls)
#[test]
fn validity_check_test() {
    assert_records("ValidityCheckTest", 3);
}

// port: StrictModeCheckTest (46 @Test methods, 100 recorded calls)
#[test]
fn strict_mode_check_test() {
    assert_records("StrictModeCheckTest", 100);
}

// port: DeclaredGlobalExternsOnWindowTest (18 @Test methods, 20 recorded calls)
#[test]
fn declared_global_externs_on_window_test() {
    assert_records("DeclaredGlobalExternsOnWindowTest", 20);
}

// port: RemoveUnnecessarySyntheticExternsTest (12 @Test methods, 27 recorded calls)
#[test]
fn remove_unnecessary_synthetic_externs_test() {
    assert_records("RemoveUnnecessarySyntheticExternsTest", 27);
}
