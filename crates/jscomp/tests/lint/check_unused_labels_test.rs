/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/lint/CheckUnusedLabelsTest.java.

use super::support::LintTestCase;
use closure_jscomp::lint::check_unused_labels::{CheckUnusedLabels, UNUSED_LABEL};

// port: CheckUnusedLabelsTest#getProcessor
// port: CheckUnusedLabelsTest#getOptions
fn case() -> LintTestCase {
    LintTestCase::new(|c| Box::new(CheckUnusedLabels::new(c))).with_lint_checks_warning()
}

// port: CheckUnusedLabelsTest#testCheckUnusedLabels_noWarning
#[test]
fn test_check_unused_labels_no_warning() {
    let t = case();
    t.test_same("L: if (true) { break L; }");
    t.test_same("L: for (;;) { if (true) { break L; } }");
    t.test_same("L1: L2: if (true) { if (true) { break L1; } break L2; }");
}

// port: CheckUnusedLabelsTest#testCheckUnusedLabels_warning
#[test]
fn test_check_unused_labels_warning() {
    let t = case();
    t.test_warning("L: var x = 0;", &UNUSED_LABEL);
    t.test_warning("L: { f(); }", &UNUSED_LABEL);
    t.test_warning("L: for (;;) {}", &UNUSED_LABEL);
    t.test_warning(
        "L1: for (;;) { L2: if (true) { break L2; } }",
        &UNUSED_LABEL,
    );
    t.test_warning("() => {a: 2}", &UNUSED_LABEL);
}
