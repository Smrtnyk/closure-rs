/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2013 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ClosureUnawarePhaseOptimizerTest.java,
//   test/com/google/javascript/jscomp/MultiPassTest.java,
//   test/com/google/javascript/jscomp/ProcessClosureProvidesAndRequiresTest.java,
//   test/com/google/javascript/jscomp/integration/ClosurePrimitivesIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/CommonJSIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/IntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! Record tests of alpha-integration: CompilerTestCase classes whose replay helpers this task
//! registered (ClosureUnawarePhaseOptimizerTest_Helpers, ProcessClosureProvidesAndRequiresTest_Helpers).
//! Every test/testSame/testError/testWarning call they make is a corpus unit record; these tests
//! replay each class's records through the Rust CompilerTestCase port (the unit_replay runner)
//! and require the Java outcome. The integration-test classes: every record of the D-014 gate
//! (tests/data/alpha_integration_gate_records.txt) must pass; their other 508 records also run
//! passes of non-alpha tasks (phase2_plan.json alpha_integration_records.non_gating), all merged,
//! and pass too. Only the 3 IntegrationTest records of out-of-scope passes (ChromePass, J2clPass;
//! docs/PORTING.md §2) stay unported.
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

// port: ClosureUnawarePhaseOptimizerTest (2 @Test methods, 2 recorded calls)
#[test]
fn closure_unaware_phase_optimizer_test() {
    assert_all_pass("ClosureUnawarePhaseOptimizerTest", 2);
}

// port: ProcessClosureProvidesAndRequiresTest (97 recorded calls; the 5 tests that make no
// recorded call are in process_closure_provides_and_requires_test.rs)
#[test]
fn process_closure_provides_and_requires_test() {
    assert_all_pass("ProcessClosureProvidesAndRequiresTest", 97);
}

// port: ClosurePrimitivesIntegrationTest (4 recorded calls)
#[test]
fn closure_primitives_integration_test() {
    assert_all_pass("ClosurePrimitivesIntegrationTest", 4);
}

// port: MultiPassTest (19 recorded calls)
#[test]
fn multi_pass_test() {
    assert_all_pass("MultiPassTest", 19);
}

/// The 9 integration classes of the alpha-integration gate.
const GATE_CLASSES: [&str; 9] = [
    "IntegrationTest",
    "AdvancedOptimizationsIntegrationTest",
    "ClosureIntegrationTest",
    "GetterAndSetterIntegrationTest",
    "ClosurePrimitivesIntegrationTest",
    "MultiPassTest",
    "TranspileAndOptimizeClosureUnawareTest",
    "ClosureUnawareCodeIntegrationTest",
    "ClosureUnawarePhaseOptimizerTest",
];

// port: IntegrationTest, AdvancedOptimizationsIntegrationTest, ClosureIntegrationTest,
// GetterAndSetterIntegrationTest, ClosurePrimitivesIntegrationTest, MultiPassTest,
// TranspileAndOptimizeClosureUnawareTest, ClosureUnawareCodeIntegrationTest,
// ClosureUnawarePhaseOptimizerTest (the 166 gate records)
#[test]
fn alpha_integration_gate_records() {
    let gate: Vec<&str> = include_str!("data/alpha_integration_gate_records.txt")
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(gate.len(), 166);
    let mut statuses = indexmap::IndexMap::new();
    for class in GATE_CLASSES {
        for r in replay(class) {
            statuses.insert(
                format!("{}#{}#{}", r.class, r.method, r.call),
                format!("{} {:?} {:?}", r.status, r.why, r.unported_by),
            );
        }
    }
    let failing: Vec<String> = gate
        .iter()
        .filter_map(|k| match statuses.get(*k) {
            Some(s) if s.starts_with("pass ") => None,
            Some(s) => Some(format!("{k}: {s}")),
            None => Some(format!("{k}: no such record")),
        })
        .collect();
    assert!(failing.is_empty(), "{}", failing.join("\n"));
}

/// Records of the 9 classes excluded from D1 (out of scope, docs/PORTING.md §2): they reach an unported
/// out-of-scope pass.
const OUT_OF_SCOPE: [(&str, &str); 3] = [
    (
        "IntegrationTest#testChromePass_noTranspile#0",
        "com.google.javascript.jscomp.ChromePass",
    ),
    (
        "IntegrationTest#testChromePass_transpile#0",
        "com.google.javascript.jscomp.ChromePass",
    ),
    (
        "IntegrationTest#testPreservesCastInformation#0",
        "com.google.javascript.jscomp.J2clPass",
    ),
];

// port: IntegrationTest, AdvancedOptimizationsIntegrationTest, ClosureIntegrationTest,
// GetterAndSetterIntegrationTest, ClosurePrimitivesIntegrationTest, MultiPassTest,
// TranspileAndOptimizeClosureUnawareTest, ClosureUnawareCodeIntegrationTest,
// ClosureUnawarePhaseOptimizerTest (all 677 records: 166 gate, 508 non-gating, 3 out of scope)
#[test]
fn alpha_integration_all_records() {
    let expected = [
        ("IntegrationTest", 340),
        ("AdvancedOptimizationsIntegrationTest", 134),
        ("ClosureIntegrationTest", 90),
        ("GetterAndSetterIntegrationTest", 31),
        ("ClosurePrimitivesIntegrationTest", 4),
        ("MultiPassTest", 19),
        ("TranspileAndOptimizeClosureUnawareTest", 33),
        ("ClosureUnawareCodeIntegrationTest", 24),
        ("ClosureUnawarePhaseOptimizerTest", 2),
    ];
    let mut failing = Vec::new();
    let mut out_of_scope = Vec::new();
    for (class, count) in expected {
        assert!(GATE_CLASSES.contains(&class), "{class}");
        let records = replay(class);
        assert_eq!(records.len(), count, "{class} record count");
        for r in records {
            let key = format!("{}#{}#{}", r.class, r.method, r.call);
            if let Some((_, pass)) = OUT_OF_SCOPE.iter().find(|(k, _)| *k == key) {
                assert_eq!(r.status, "unported", "{key}");
                assert_eq!(r.unported_by.as_deref(), Some(*pass), "{key}");
                out_of_scope.push(key);
            } else if r.status != "pass" {
                failing.push(format!(
                    "{key}: {} {:?} {:?}",
                    r.status, r.why, r.unported_by
                ));
            }
        }
    }
    assert!(failing.is_empty(), "{}", failing.join("\n"));
    assert_eq!(out_of_scope.len(), OUT_OF_SCOPE.len());
}

// port: CommonJSIntegrationTest (16 recorded calls of the 16 active @Test methods; they compile
// the whole ADVANCED pipeline with ES5 output)
#[test]
fn common_js_integration_test() {
    assert_all_pass("CommonJSIntegrationTest", 16);
}
