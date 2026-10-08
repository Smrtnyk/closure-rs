/*
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/Es6RelativizeImportPathsTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteModulesBeforeTypeCheckingTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteModulesTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteModulesToCommonJsModulesTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteModulesWithGoogInteropTest.java,
//   test/com/google/javascript/jscomp/ForbidDynamicImportUsageTest.java,
//   test/com/google/javascript/jscomp/RewriteDynamicImportsTest.java,
//   test/com/google/javascript/jscomp/integration/EsModuleIntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of the ES module passes (Es6RewriteModulesTest,
//! Es6RewriteModulesBeforeTypeCheckingTest, Es6RewriteModulesWithGoogInteropTest,
//! RewriteDynamicImportsTest, Es6RelativizeImportPathsTest,
//! Es6RewriteModulesToCommonJsModulesTest, ForbidDynamicImportUsageTest and
//! integration/EsModuleIntegrationTest) are CompilerTestCase / IntegrationTestCase tests. Every
//! test/testSame/testError/testWarning call they make is a corpus unit record; these tests replay
//! each class's records through the Rust harness ports (the unit_replay runner) and require the
//! Java outcome. Records whose processor needs a class that is not ported report that class as
//! unported; none may fail. Pass counts are floors, so more passing records keep the tests
//! green.
use closure_testing::{
    corpus,
    replay::replay_main::{RecordResult, Runner},
};

const TYPE_CHECK: &str = "com.google.javascript.jscomp.TypeCheck";
const SEMANTIC_RAI: &str = "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter";
const SEMANTIC_RAI_INIT: &str = "com.google.javascript.jscomp.type.SemanticReverseAbstractInterpreter#<init>(com.google.javascript.rhino.jstype.JSTypeRegistry)";

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
/// more records pass as other classes are ported) and that none fails; a record that does not
/// pass must be unported, by one of `unported_by` (a class that is not ported).
fn assert_records(class: &str, expected: usize, passing: usize, unported_by: &[&str]) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let mut pass = 0;
    let mut unported_by_named = 0;
    for r in &records {
        match r.status.as_str() {
            "pass" => pass += 1,
            "unported" => {
                if unported_by.contains(&r.unported_by.as_deref().unwrap_or("")) {
                    unported_by_named += 1;
                }
            }
            _ => panic!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            ),
        }
    }
    assert!(
        pass >= passing,
        "{class} passing records: {pass} < {passing}"
    );
    assert!(
        unported_by_named <= expected - passing,
        "{class} unported records"
    );
}

// port: Es6RelativizeImportPathsTest (26 recorded calls)
#[test]
fn es6_relativize_import_paths_test() {
    assert_records("Es6RelativizeImportPathsTest", 26, 26, &[]);
}

// port: Es6RewriteModulesToCommonJsModulesTest (29 recorded calls)
#[test]
fn es6_rewrite_modules_to_common_js_modules_test() {
    assert_records("Es6RewriteModulesToCommonJsModulesTest", 29, 29, &[]);
}

/// setUp enables type checking (CompilerTestCase#createTypeCheck runs TypeCheck before the pass).
// port: Es6RewriteModulesBeforeTypeCheckingTest (82 recorded calls)
#[test]
fn es6_rewrite_modules_before_type_checking_test() {
    assert_records(
        "Es6RewriteModulesBeforeTypeCheckingTest",
        82,
        82,
        &[TYPE_CHECK],
    );
}

/// getProcessor runs `TypeCheck#processForTesting` for the global TypedScope before the pass;
/// One record stops at ConvertChunksToESModules (chunk-output).
// port: Es6RewriteModulesTest (86 recorded calls)
#[test]
fn es6_rewrite_modules_test() {
    assert_records("Es6RewriteModulesTest", 86, 86, &[]);
}

// port: Es6RewriteModulesWithGoogInteropTest (79 recorded calls)
#[test]
fn es6_rewrite_modules_with_goog_interop_test() {
    assert_records(
        "Es6RewriteModulesWithGoogInteropTest",
        79,
        79,
        &[TYPE_CHECK, SEMANTIC_RAI_INIT],
    );
}

/// getProcessor is the corpus helper RewriteDynamicImportsTest_Helpers.GetProcessor, which runs
/// `TypeCheck#processForTesting` before Es6RewriteModules and RewriteDynamicImports.
// port: RewriteDynamicImportsTest (25 recorded calls)
#[test]
fn rewrite_dynamic_imports_test() {
    assert_records(
        "RewriteDynamicImportsTest",
        25,
        25,
        &[SEMANTIC_RAI, TYPE_CHECK],
    );
}

// port: ForbidDynamicImportUsageTest (1 recorded call)
#[test]
fn forbid_dynamic_import_usage_test() {
    assert_records("ForbidDynamicImportUsageTest", 1, 1, &[TYPE_CHECK]);
}

// port: EsModuleIntegrationTest (4 recorded calls)
#[test]
fn es_module_integration_test() {
    assert_records("EsModuleIntegrationTest", 4, 4, &[]);
}
