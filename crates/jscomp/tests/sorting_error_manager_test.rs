/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JSError.java,
//   test/com/google/javascript/jscomp/SortingErrorManagerTest.java.

use closure_jscomp::{
    basic_error_manager::BasicErrorManager,
    check_level::CheckLevel,
    diagnostic_type::DiagnosticType,
    error_handler::ErrorHandler,
    error_manager::ErrorManager,
    js_error::JSError,
    sorting_error_manager::{ErrorWithLevel, LeveledJSErrorComparator, SortingErrorManager},
};
use closure_rhino::node::Ast;
static FOO_TYPE: DiagnosticType = DiagnosticType::error("TEST_FOO", "Foo");
static JOO_TYPE: DiagnosticType = DiagnosticType::error("TEST_JOO", "Joo");
// port: SortingErrorManagerTest#error
fn error(e: JSError) -> ErrorWithLevel {
    ErrorWithLevel::new(e, CheckLevel::ERROR)
}
// port: SortingErrorManagerTest#warning
fn warning(e: JSError) -> ErrorWithLevel {
    ErrorWithLevel::new(e, CheckLevel::WARNING)
}
// port: SortingErrorManagerTest#assertSmaller
fn assert_smaller(p1: ErrorWithLevel, p2: ErrorWithLevel) {
    let c = LeveledJSErrorComparator;
    let p1p2 = c.compare(Some(&p1), Some(&p2));
    assert!(p1p2 < 0, "{p1p2}");
    let p2p1 = c.compare(Some(&p2), Some(&p1));
    assert!(p2p1 > 0, "{p2p1}");
}
// port: JSError#make(String,int,int,DiagnosticType,String...) (nullable source-name adaptation)
fn make(source: Option<&str>, line: i32, col: i32, type_: &'static DiagnosticType) -> JSError {
    let mut e = JSError::make_without_location(type_, &[]);
    e.source_name = source.map(str::to_owned);
    e.lineno = line;
    e.charno = col;
    e
}
// port: SortingErrorManagerTest#testOrderingBothNull
#[test]
fn test_ordering_both_null() {
    assert_eq!(LeveledJSErrorComparator.compare(None, None), 0);
}
// port: SortingErrorManagerTest#testOrderingSourceName1
#[test]
fn test_ordering_source_name1() {
    assert_smaller(
        error(make(None, -1, -1, &FOO_TYPE)),
        error(make(Some("a"), -1, -1, &FOO_TYPE)),
    );
}
// port: SortingErrorManagerTest#testOrderingSourceName2
#[test]
fn test_ordering_source_name2() {
    assert_smaller(
        error(make(Some("a"), -1, -1, &FOO_TYPE)),
        error(make(Some("b"), -1, -1, &FOO_TYPE)),
    );
}
// port: SortingErrorManagerTest#testOrderingLineno1
#[test]
fn test_ordering_lineno1() {
    assert_smaller(
        error(make(None, -1, -1, &FOO_TYPE)),
        error(make(None, 2, -1, &FOO_TYPE)),
    );
}
// port: SortingErrorManagerTest#testOrderingLineno2
#[test]
fn test_ordering_lineno2() {
    assert_smaller(
        error(make(None, 8, -1, &FOO_TYPE)),
        error(make(None, 56, -1, &FOO_TYPE)),
    );
}
// port: SortingErrorManagerTest#testOrderingCheckLevel
#[test]
fn test_ordering_check_level() {
    assert_smaller(
        warning(make(None, -1, -1, &FOO_TYPE)),
        error(make(None, -1, -1, &FOO_TYPE)),
    );
}
// port: SortingErrorManagerTest#testOrderingCharno1
#[test]
fn test_ordering_charno1() {
    let e1 = make(None, 5, -1, &FOO_TYPE);
    let e2 = make(None, 5, 2, &FOO_TYPE);
    assert_smaller(error(e1.clone()), error(e2.clone()));
    assert_smaller(warning(e1), error(e2));
}
// port: SortingErrorManagerTest#testOrderingCharno2
#[test]
fn test_ordering_charno2() {
    let e1 = make(None, 8, 7, &FOO_TYPE);
    let e2 = make(None, 8, 5, &FOO_TYPE);
    assert_smaller(error(e2.clone()), error(e1.clone()));
    assert_smaller(warning(e2), error(e1));
}
// port: SortingErrorManagerTest#testOrderingDescription
#[test]
fn test_ordering_description() {
    assert_smaller(
        error(make(None, -1, -1, &FOO_TYPE)),
        error(make(None, -1, -1, &JOO_TYPE)),
    );
}
struct RecordingManager {
    manager: SortingErrorManager,
    printed_errors: Vec<JSError>,
    add_during_report: bool,
}
impl BasicErrorManager for RecordingManager {
    fn sorting_manager(&self) -> &SortingErrorManager {
        &self.manager
    }
    fn sorting_manager_mut(&mut self) -> &mut SortingErrorManager {
        &mut self.manager
    }
    // port: SortingErrorManagerTest.BasicErrorManager#println
    fn println(&mut self, _ast: &Ast, _level: CheckLevel, error: &JSError) {
        if self.add_during_report && error.get_type() == &FOO_TYPE {
            self.report(CheckLevel::ERROR, make(None, -1, -1, &JOO_TYPE));
        }
        self.printed_errors.push(error.clone());
    }
    // port: SortingErrorManagerTest.BasicErrorManager#printSummary
    fn print_summary(&mut self) {
        if self.add_during_report {
            assert_eq!(self.printed_errors.len(), 1);
        }
    }
}
// port: SortingErrorManagerTest#testDeduplicatedErrors
#[test]
fn test_deduplicated_errors() {
    let mut manager = RecordingManager {
        manager: SortingErrorManager::new(vec![]),
        printed_errors: Vec::new(),
        add_during_report: false,
    };
    manager.report(CheckLevel::ERROR, make(None, -1, -1, &FOO_TYPE));
    manager.report(CheckLevel::ERROR, make(None, -1, -1, &FOO_TYPE));
    ErrorManager::generate_report(&mut manager, &Ast::new());
    assert_eq!(manager.printed_errors.len(), 1);
}
// port: SortingErrorManagerTest#testGenerateReportCausesMoreWarnings
#[test]
fn test_generate_report_causes_more_warnings() {
    let mut manager = RecordingManager {
        manager: SortingErrorManager::new(vec![]),
        printed_errors: Vec::new(),
        add_during_report: true,
    };
    manager.report(CheckLevel::ERROR, make(None, -1, -1, &FOO_TYPE));
    ErrorManager::generate_report(&mut manager, &Ast::new());
}
