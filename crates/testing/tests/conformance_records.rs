/*
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CheckConformanceTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! CheckConformanceTest is a CompilerTestCase test: every test/testSame/testError/testWarning call
//! it makes is a corpus unit record. This test replays the class's records through the Rust
//! CompilerTestCase port (the unit_replay runner) and requires the Java outcome.
//!
//! The five records of testCustom3..testCustom7 configure CUSTOM requirements whose `java_class`
//! is one of the Java test's own classes (CheckConformanceTest, CheckConformanceTest$CustomRule,
//! $CustomRuleMissingPublicConstructor, $CustomRuleReport). ConformanceRules.CustomRuleProxy loads
//! them by reflection, which the port cannot do: those records are Unported, by exactly that.
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

// port: CheckConformanceTest (177 @Test methods, 410 recorded calls)
#[test]
fn check_conformance_test() {
    let records = replay("CheckConformanceTest");
    assert_eq!(records.len(), 410);
    let custom: &[(&str, &str)] = &[
        (
            "testCustom3",
            "com.google.javascript.jscomp.CheckConformanceTest",
        ),
        (
            "testCustom4",
            "com.google.javascript.jscomp.CheckConformanceTest$CustomRuleMissingPublicConstructor",
        ),
        (
            "testCustom5",
            "com.google.javascript.jscomp.CheckConformanceTest$CustomRule",
        ),
        (
            "testCustom6",
            "com.google.javascript.jscomp.CheckConformanceTest$CustomRule",
        ),
        (
            "testCustom7",
            "com.google.javascript.jscomp.CheckConformanceTest$CustomRuleReport",
        ),
    ];
    let mut unported = 0;
    for r in &records {
        match r.status.as_str() {
            "pass" => {}
            "unported" => {
                let java_class = custom
                    .iter()
                    .find(|(method, _)| *method == r.method)
                    .map(|(_, class)| *class)
                    .unwrap_or_else(|| {
                        panic!(
                            "{}[{}] {}: unported {:?}",
                            r.class, r.index, r.method, r.unported_by
                        )
                    });
                assert_eq!(
                    r.unported_by.as_deref(),
                    Some(
                        format!("ConformanceRules.CustomRuleProxy java_class {java_class}")
                            .as_str()
                    ),
                    "{}[{}] {}",
                    r.class,
                    r.index,
                    r.method
                );
                unported += 1;
            }
            _ => panic!(
                "{}[{}] {}: {} {:?}",
                r.class, r.index, r.method, r.status, r.why
            ),
        }
    }
    assert_eq!(unported, 5);
}
