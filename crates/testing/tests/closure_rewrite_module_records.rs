/*
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/CheckGoogJsImportTest.java,
//   test/com/google/javascript/jscomp/CheckMissingRequiresTest.java,
//   test/com/google/javascript/jscomp/ClosureRewriteModuleTest.java,
//   test/com/google/javascript/jscomp/MissingProvideTest.java,
//   test/com/google/javascript/jscomp/RewriteGoogJsImportsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of ClosureRewriteModule and related passes (ClosureRewriteModuleTest,
//! RewriteGoogJsImportsTest, CheckGoogJsImportTest, MissingProvideTest, CheckMissingRequiresTest)
//! are CompilerTestCase tests. Every test/testSame/testError/testWarning call they make is a
//! corpus unit record; these tests replay each class's records through the Rust CompilerTestCase
//! port (the unit_replay runner) and require the Java outcome. CheckMissingRequiresTest is also
//! ported test by test in crates/jscomp/tests/check_missing_requires_test.rs.
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

// port: CheckGoogJsImportTest (9 @Test methods, 21 recorded calls)
#[test]
fn check_goog_js_import_test() {
    assert_all_pass("CheckGoogJsImportTest", 21);
}

// port: RewriteGoogJsImportsTest (7 @Test methods, 13 recorded calls)
#[test]
fn rewrite_goog_js_imports_test() {
    assert_all_pass("RewriteGoogJsImportsTest", 13);
}

// port: MissingProvideTest (18 @Test methods, 18 recorded calls)
#[test]
fn missing_provide_test() {
    assert_all_pass("MissingProvideTest", 18);
}

// port: CheckMissingRequiresTest (120 @Test methods, 120 recorded calls)
#[test]
fn check_missing_requires_test() {
    assert_all_pass("CheckMissingRequiresTest", 120);
}

/// ClosureRewriteModuleTest#getProcessor runs `TypeCheck#processForTesting` before the pass
/// (`new SemanticReverseAbstractInterpreter(registry)` first). Every record passes (ChangeVerifier
/// and the pass print typed nodes through the compiler's registry).
// port: ClosureRewriteModuleTest (163 @Test methods, 210 recorded calls)
#[test]
fn closure_rewrite_module_test() {
    assert_all_pass("ClosureRewriteModuleTest", 210);
}
