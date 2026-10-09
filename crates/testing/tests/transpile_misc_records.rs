/*
 * Copyright 2014 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
 * Copyright 2020 The Closure Compiler Authors.
 * Copyright 2021 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
 * Copyright 2026 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/Es6ForOfConverterTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteBlockScopedDeclarationTest.java,
//   test/com/google/javascript/jscomp/Es6RewriteBlockScopedFunctionDeclarationTest.java,
//   test/com/google/javascript/jscomp/Es6TemplateLiteralsTest.java,
//   test/com/google/javascript/jscomp/Es6TranspilationIntegrationTest.java,
//   test/com/google/javascript/jscomp/Es7RewriteExponentialOperatorTest.java,
//   test/com/google/javascript/jscomp/InstrumentAsyncContextTest.java,
//   test/com/google/javascript/jscomp/LateEs6ToEs3ConverterTest.java,
//   test/com/google/javascript/jscomp/RewriteLogicalAssignmentOperatorsTest.java,
//   test/com/google/javascript/jscomp/RewriteNullishCoalesceOperatorTest.java,
//   test/com/google/javascript/jscomp/RewriteObjectSpreadTest.java,
//   test/com/google/javascript/jscomp/RewriteOptionalChainingOperatorTest.java,
//   test/com/google/javascript/jscomp/integration/ES2021IntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/ES2022IntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/OptionalChainingIntegrationTest.java,
//   test/com/google/javascript/jscomp/integration/TranspileOnlyIntegrationTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! The Java tests of the remaining transpilation passes (block scoping, for-of, late ES3 conversion, template
//! literals and the ES2016-2021 operator lowerings, InstrumentAsyncContext, and the transpilation
//! integration tests) are CompilerTestCase / IntegrationTestCase tests. Every
//! test/testSame/testError/testWarning call they make is a corpus unit record; these tests replay
//! each class's records through the Rust CompilerTestCase port (the unit_replay runner) and
//! require the Java outcome.
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

/// Asserts that `class` has `expected` records and that every one of them passes.
fn assert_records(class: &str, expected: usize) {
    let records = replay(class);
    assert_eq!(records.len(), expected, "{class} record count");
    let bad: Vec<_> = records
        .iter()
        .filter(|r| r.status != "pass")
        .map(|r| {
            format!(
                "{}[{}] {}: {} {:?} {:?}",
                r.class, r.index, r.method, r.status, r.why, r.unported_by
            )
        })
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// port: Es6RewriteBlockScopedDeclarationTest (37 @Test methods, 96 recorded calls)
#[test]
fn es6_rewrite_block_scoped_declaration_test() {
    assert_records("Es6RewriteBlockScopedDeclarationTest", 96);
}

// port: Es6RewriteBlockScopedFunctionDeclarationTest (7 @Test methods, 7 recorded calls)
#[test]
fn es6_rewrite_block_scoped_function_declaration_test() {
    assert_records("Es6RewriteBlockScopedFunctionDeclarationTest", 7);
}

// port: Es6ForOfConverterTest (12 @Test methods, 21 recorded calls)
#[test]
fn es6_for_of_converter_test() {
    assert_records("Es6ForOfConverterTest", 21);
}

// port: LateEs6ToEs3ConverterTest (15 @Test methods, 73 recorded calls)
#[test]
fn late_es6_to_es3_converter_test() {
    assert_records("LateEs6ToEs3ConverterTest", 73);
}

// port: Es6TemplateLiteralsTest (3 @Test methods, 3 recorded calls)
#[test]
fn es6_template_literals_test() {
    assert_records("Es6TemplateLiteralsTest", 3);
}

// port: Es7RewriteExponentialOperatorTest (9 @Test methods, 9 recorded calls)
#[test]
fn es7_rewrite_exponential_operator_test() {
    assert_records("Es7RewriteExponentialOperatorTest", 9);
}

// port: RewriteObjectSpreadTest (2 @Test methods, 9 recorded calls)
#[test]
fn rewrite_object_spread_test() {
    assert_records("RewriteObjectSpreadTest", 9);
}

// port: RewriteOptionalChainingOperatorTest (4 @Test methods, one of them the parameterized
// doTest; 28 recorded calls)
#[test]
fn rewrite_optional_chaining_operator_test() {
    assert_records("RewriteOptionalChainingOperatorTest", 28);
}

// port: RewriteNullishCoalesceOperatorTest (4 @Test methods, 4 recorded calls)
#[test]
fn rewrite_nullish_coalesce_operator_test() {
    assert_records("RewriteNullishCoalesceOperatorTest", 4);
}

// port: RewriteLogicalAssignmentOperatorsTest (6 @Test methods, 21 recorded calls)
#[test]
fn rewrite_logical_assignment_operators_test() {
    assert_records("RewriteLogicalAssignmentOperatorsTest", 21);
}

// port: InstrumentAsyncContextTest (50 @Test methods, 49 recorded calls; testTopLevelAwait
// returns before its test call, see below)
#[test]
fn instrument_async_context_test() {
    assert_records("InstrumentAsyncContextTest", 49);
}

// port: InstrumentAsyncContextTest#SUPPORT_TOP_LEVEL_AWAIT
const SUPPORT_TOP_LEVEL_AWAIT: bool = false;

// NOTE: This test is disabled because we do not yet support top-level await.  But if it becomes
// supported, this is how we should handle it.
// port: InstrumentAsyncContextTest#testTopLevelAwait
#[test]
fn instrument_async_context_test_top_level_await() {
    if !SUPPORT_TOP_LEVEL_AWAIT {
        return;
    }
    unreachable!("InstrumentAsyncContextTest#testTopLevelAwait made no recorded call");
}

// port: Es6TranspilationIntegrationTest (73 @Test methods, 185 recorded calls)
#[test]
fn es6_transpilation_integration_test() {
    assert_records("Es6TranspilationIntegrationTest", 185);
}

// port: ES2021IntegrationTest (19 @Test methods, 19 recorded calls)
#[test]
fn es2021_integration_test() {
    assert_records("ES2021IntegrationTest", 19);
}

// port: ES2022IntegrationTest (24 @Test methods, 26 recorded calls)
#[test]
fn es2022_integration_test() {
    assert_records("ES2022IntegrationTest", 26);
}

// port: OptionalChainingIntegrationTest (5 @Test methods, 6 recorded calls)
#[test]
fn optional_chaining_integration_test() {
    assert_records("OptionalChainingIntegrationTest", 6);
}

// port: TranspileOnlyIntegrationTest (4 @Test methods, 4 recorded calls)
#[test]
fn transpile_only_integration_test() {
    assert_records("TranspileOnlyIntegrationTest", 4);
}
