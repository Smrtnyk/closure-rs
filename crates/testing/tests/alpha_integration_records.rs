/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2013 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
 * Copyright 2025 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/TranspileAndOptimizeClosureUnawareTest.java,
//   test/com/google/javascript/jscomp/integration/AdvancedOptimizationsIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/ClosureIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/ClosurePrimitivesIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/ClosureUnawareCodeIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/CommonJSIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/GetterAndSetterIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/IntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! Record tests of the integration test classes: every test/testSame/testError/testWarning call
//! these Java classes make is a corpus unit record; each test below replays one class's records
//! through the Rust CompilerTestCase port (the unit_replay runner) and requires the Java outcome
//! for every record. One test per class, so that the test runner replays the classes in
//! parallel. Only the 3 IntegrationTest records that reach out-of-scope passes (ChromePass,
//! J2clPass) stay unported, and they must stay exactly that.
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

/// Records that reach an unported out-of-scope pass, with that pass.
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

/// Asserts that `class` has `expected` records and that every one passes, except the
/// out-of-scope records of `OUT_OF_SCOPE`, which must be unported by exactly their pass.
fn assert_all_pass(class: &str, expected: usize) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let mut failing = Vec::new();
    let mut out_of_scope = 0;
    for r in &records {
        let key = format!("{}#{}#{}", r.class, r.method, r.call);
        if let Some((_, pass)) = OUT_OF_SCOPE.iter().find(|(k, _)| *k == key) {
            assert_eq!(r.status, "unported", "{key}");
            assert_eq!(r.unported_by.as_deref(), Some(*pass), "{key}");
            out_of_scope += 1;
        } else if r.status != "pass" {
            failing.push(format!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            ));
        }
    }
    assert!(failing.is_empty(), "{}", failing.join("\n"));
    let expected_out_of_scope = OUT_OF_SCOPE
        .iter()
        .filter(|(k, _)| k.starts_with(&format!("{class}#")))
        .count();
    assert_eq!(
        out_of_scope, expected_out_of_scope,
        "{class} out-of-scope records"
    );
}

// port: IntegrationTest (340 recorded calls; 3 reach out-of-scope passes)
#[test]
fn integration_test() {
    assert_all_pass("IntegrationTest", 340);
}

// port: AdvancedOptimizationsIntegrationTest (134 recorded calls)
#[test]
fn advanced_optimizations_integration_test() {
    assert_all_pass("AdvancedOptimizationsIntegrationTest", 134);
}

// port: ClosureIntegrationTest (90 recorded calls)
#[test]
fn closure_integration_test() {
    assert_all_pass("ClosureIntegrationTest", 90);
}

// port: GetterAndSetterIntegrationTest (31 recorded calls)
#[test]
fn getter_and_setter_integration_test() {
    assert_all_pass("GetterAndSetterIntegrationTest", 31);
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

// port: TranspileAndOptimizeClosureUnawareTest (33 recorded calls)
#[test]
fn transpile_and_optimize_closure_unaware_test() {
    assert_all_pass("TranspileAndOptimizeClosureUnawareTest", 33);
}

// port: ClosureUnawareCodeIntegrationTest (24 recorded calls)
#[test]
fn closure_unaware_code_integration_test() {
    assert_all_pass("ClosureUnawareCodeIntegrationTest", 24);
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

// port: CommonJSIntegrationTest (16 recorded calls of the 16 active @Test methods; they compile
// the whole ADVANCED pipeline with ES5 output)
#[test]
fn common_js_integration_test() {
    assert_all_pass("CommonJSIntegrationTest", 16);
}
