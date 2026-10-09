/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/ijs/ConvertToTypedInterfaceTest.java.

//! The Rust port of ConvertToTypedInterfaceTest, ExtraRequireRemoverTest and
//! SuggestConstTypesTest: every one of their @Test methods extends CompilerTestCase and has corpus
//! records (docs/PORTING.md §4.2), so the port is the record replay. SuggestConstTypesTest
//! enables type checking (ChangeVerifier prints typed nodes through the compiler's registry). No record may
//! fail.
use closure_testing::{corpus, replay::replay_main::Runner};

// port: ConvertToTypedInterfaceTest, ExtraRequireRemoverTest, SuggestConstTypesTest
#[test]
fn ijs_test_classes_replay() {
    let report = Runner {
        corpus: corpus::corpus_unit_dir(),
        classes: Some(vec![
            "ConvertToTypedInterfaceTest".to_string(),
            "ExtraRequireRemoverTest".to_string(),
            "SuggestConstTypesTest".to_string(),
        ]),
        sample: None,
    }
    .run(
        |r| {
            assert_ne!(
                r.status, "fail",
                "{}#{} call {}: {:?}",
                r.class, r.method, r.call, r.why
            );
            Ok(())
        },
        |_, _| {},
    )
    .unwrap();
    let c = &report.classes["ConvertToTypedInterfaceTest"];
    assert_eq!((c.records, c.pass), (299, 299));
    let c = &report.classes["ExtraRequireRemoverTest"];
    assert_eq!((c.records, c.pass), (14, 14));
    let c = &report.classes["SuggestConstTypesTest"];
    assert_eq!((c.records, c.pass), (2, 2));
    assert_eq!(report.harness_errors, 0);
}
