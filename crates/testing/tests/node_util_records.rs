/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/NodeUtilTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! NodeUtilTest's nested CompilerTestCase class CreateSynthesizedExternsSymbolTests makes one
//! recorded call (a testExternChanges whose processor calls
//! NodeUtil.createSynthesizedExternsSymbol). This test replays the class's record through the Rust
//! CompilerTestCase port (the unit_replay runner) with the NodeUtilTest_Helpers port and requires
//! the Java outcome.
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

// port: NodeUtilTest.CreateSynthesizedExternsSymbolTests#test
#[test]
fn create_synthesized_externs_symbol_tests() {
    let records = replay("NodeUtilTest");
    assert_eq!(records.len(), 1);
    for r in &records {
        assert_eq!(
            r.status, "pass",
            "{}[{}] {}: {:?} {:?}",
            r.class, r.index, r.method, r.why, r.unported_by
        );
    }
}
