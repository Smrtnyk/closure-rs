/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ReplaceMessagesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! ReplaceMessagesTest is a CompilerTestCase test: every test/testSame/testError/testWarning call
//! it makes is a corpus unit record. Its setUp enables type checking (`enableTypeCheck()`,
//! `replaceTypesWithColors()`, `enableTypeInfoValidation()`), and getProcessor builds
//! ReplaceMessages through the corpus helper ReplaceMessagesTest_Helpers
//! (`crates/testing/src/replay/replace_messages_helpers.rs`). This test replays every record of
//! the class through the Rust harness ports (the unit_replay runner) and requires the Java
//! outcome for each of them.
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
    // On 4 threads; the sinks still see the records in order.
    .run_on(
        4,
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

// port: ReplaceMessagesTest (259 recorded calls of its 87 tests)
#[test]
fn replace_messages_test() {
    let records = replay("ReplaceMessagesTest");
    assert_eq!(records.len(), 259, "ReplaceMessagesTest record count");
    for r in &records {
        assert_eq!(
            r.status, "pass",
            "{}[{}] {}: {} {:?} {:?}",
            r.class, r.index, r.method, r.status, r.why, r.unported_by
        );
    }
}
