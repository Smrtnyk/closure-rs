/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2016 The Closure Compiler Authors.
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DiagnosticGroupPathSuppressingWarningsGuard.java,
//   src/com/google/javascript/jscomp/DiagnosticType.java,
//   src/com/google/javascript/jscomp/JSError.java,
//   src/com/google/javascript/jscomp/LightweightMessageFormatter.java,
//   src/com/google/javascript/jscomp/LoggerErrorManager.java,
//   src/com/google/javascript/jscomp/SourceFile.java,
//   src/com/google/javascript/jscomp/StrictWarningsGuard.java,
//   src/com/google/javascript/jscomp/ThreadSafeDelegatingErrorManager.java.

use closure_jscomp::{
    black_hole_error_manager::BlackHoleErrorManager,
    check_level::CheckLevel::{ERROR, OFF, WARNING},
    diagnostic_group::DiagnosticGroup,
    diagnostic_group_path_suppressing_warnings_guard::DiagnosticGroupPathSuppressingWarningsGuard,
    diagnostic_type::DiagnosticType,
    error_handler::ErrorHandler,
    error_manager::ErrorManager,
    js_error::JSError,
    lightweight_message_formatter::LightweightMessageFormatter,
    logger_error_manager::{Level, Logger, LoggerErrorManager},
    message_formatter::MessageFormatter,
    source_file::SourceFile,
    strict_warnings_guard::{StrictWarningsGuard, UNRAISABLE_WARNING},
    thread_safe_delegating_error_manager::ThreadSafeDelegatingErrorManager,
    warnings_guard::WarningsGuard,
};
use closure_rhino::{
    js_string::JsString, node::Ast, static_source_file::StaticSourceFile, token::Token,
};
use std::sync::{Arc, Mutex};
static TYPE: DiagnosticType = DiagnosticType::error("TEST_ERROR", "{0}");
static OTHER_UNRAISABLE: DiagnosticType = DiagnosticType::warning("JSC_UNRAISABLE_WARNING", "{0}");
// port: StrictWarningsGuard#level / DiagnosticType#equals (identity contract)
#[test]
fn diagnostic_type_equality_and_guard_identity() {
    assert_eq!(OTHER_UNRAISABLE, UNRAISABLE_WARNING);
    let g = StrictWarningsGuard;
    assert_eq!(
        g.level(&JSError::make_without_location(&UNRAISABLE_WARNING, &["x"])),
        None
    );
    assert_eq!(
        g.level(&JSError::make_without_location(&OTHER_UNRAISABLE, &["x"])),
        Some(ERROR)
    );
    let group1 = DiagnosticGroup::for_type(&UNRAISABLE_WARNING);
    let group2 = DiagnosticGroup::for_type(&OTHER_UNRAISABLE);
    assert!(Arc::ptr_eq(&group1, &group2));
}
// port: JSError.Builder#setNode / JSError.Builder#setSourceLocation
#[test]
fn builder_location_preconditions() {
    let mut ast = Ast::new();
    let node = ast.new_node(Token::EMPTY);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        JSError::builder(&TYPE, &["x"])
            .set_source_location("input.js", 1, 0)
            .set_node(&ast, node)
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<String>().unwrap(),
        "Cannot provide a Node when there's already a source name"
    );
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        JSError::builder(&TYPE, &["x"])
            .set_node(&ast, node)
            .set_source_location("input.js", 1, 0)
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<String>().unwrap(),
        "Cannot provide a source location when there is already a Node"
    );
}
// port: JSError#format / LightweightMessageFormatter#format
#[test]
fn missing_source_name_formatter() {
    let error = JSError::make_without_location(&TYPE, &["a'b{c}λ"]);
    let ast = Ast::new();
    let formatter = LightweightMessageFormatter::without_source();
    assert_eq!(
        formatter.format_error(&ast, &error),
        "ERROR - [TEST_ERROR] a'b{c}λ\n"
    );
    assert_eq!(error.format(&ast, OFF, &formatter), None);
    assert_eq!(
        error.to_string(),
        "TEST_ERROR. a'b{c}λ at (unknown source) line (unknown line) : (unknown column)"
    );
}
// port: DiagnosticGroupPathSuppressingWarningsGuard#level
#[test]
fn diagnostic_group_path_suppression() {
    let guard = DiagnosticGroupPathSuppressingWarningsGuard::new(
        DiagnosticGroup::for_type(&TYPE),
        "generated/",
    );
    assert_eq!(
        guard.level(&JSError::make_with_source_location(
            "generated/file.js",
            1,
            0,
            &TYPE,
            &["x"]
        )),
        Some(OFF)
    );
    assert_eq!(
        guard.level(&JSError::make_with_source_location(
            "file.js",
            1,
            0,
            &TYPE,
            &["x"]
        )),
        None
    );
    assert_eq!(
        guard.level(&JSError::make_without_location(&TYPE, &["x"])),
        None
    );
    assert_eq!(
        guard.level(&JSError::make_with_source_location(
            "generated/file.js",
            1,
            0,
            &OTHER_UNRAISABLE,
            &["x"]
        )),
        None
    );
}
#[derive(Clone)]
struct RecordingLogger(Arc<Mutex<Vec<(Level, String)>>>);
impl Logger for RecordingLogger {
    // port: Logger#severe
    fn severe(&mut self, message: &str) {
        self.log(Level::SEVERE, message);
    }
    // port: Logger#warning
    fn warning(&mut self, message: &str) {
        self.log(Level::WARNING, message);
    }
    // port: Logger#log
    fn log(&mut self, level: Level, message: &str) {
        self.0.lock().unwrap().push((level, message.into()));
    }
}
// port: LoggerErrorManager#println / LoggerErrorManager#printSummary
#[test]
fn logger_messages_and_summary() {
    let logger = RecordingLogger(Arc::new(Mutex::new(vec![])));
    let mut manager = LoggerErrorManager::new_without_source(Box::new(logger.clone()));
    manager.report(ERROR, JSError::make_without_location(&TYPE, &["error"]));
    manager.report(WARNING, JSError::make_without_location(&TYPE, &["warning"]));
    manager.set_typed_percent(99.95);
    ErrorManager::generate_report(&mut manager, &Ast::new());
    assert_eq!(
        *logger.0.lock().unwrap(),
        vec![
            (Level::WARNING, "WARNING - [TEST_ERROR] warning\n".into()),
            (Level::SEVERE, "ERROR - [TEST_ERROR] error\n".into()),
            (
                Level::WARNING,
                "1 error(s), 1 warning(s), 100.0% typed".into()
            )
        ]
    );
    logger.0.lock().unwrap().clear();
    let mut manager = LoggerErrorManager::new_without_source(Box::new(logger.clone()));
    manager.set_typed_percent(0.15);
    ErrorManager::generate_report(&mut manager, &Ast::new());
    assert_eq!(
        *logger.0.lock().unwrap(),
        vec![(Level::INFO, "0 error(s), 0 warning(s), 0.2% typed".into())]
    );
}
// port: ThreadSafeDelegatingErrorManager#report
#[test]
fn concurrent_delegation() {
    let manager = Arc::new(ThreadSafeDelegatingErrorManager::new(Box::new(
        BlackHoleErrorManager::new(),
    )));
    std::thread::scope(|scope| {
        for i in 0..4 {
            let manager = manager.clone();
            scope.spawn(move || {
                for j in 0..100 {
                    manager.report(
                        ERROR,
                        JSError::make_without_location(&TYPE, &[&format!("{i}/{j}")]),
                    );
                }
            });
        }
    });
    assert_eq!(manager.get_error_count(), 400);
    assert!(manager.has_halting_errors());
    manager.generate_report(&Ast::new());
    assert_eq!(manager.get_errors().len(), 400);
}
// port: SourceFile#setCodeAndDoBookkeeping / SourceFile#getLine
#[test]
fn source_offsets_are_wtf16() {
    let source = SourceFile::from_code(
        "unicode.js",
        JsString::from_units(vec![0xfeff, 0xd83d, 0xde00, 10, 0xd800, 10]),
    );
    assert_eq!(source.get_num_bytes(), 5);
    assert_eq!(source.get_num_lines(), 3);
    assert_eq!(source.get_line_offset(2), 3);
    // Java's UTF-8 PrintStream writes the unpaired surrogate as '?' (SurrogateExcerpt.java).
    assert_eq!(source.get_line(2).as_deref(), Some("?"));
    assert_eq!(
        source.get_code().unwrap().as_units(),
        &[0xd83d, 0xde00, 10, 0xd800, 10]
    );
}
// port: JSError.Builder#setNodeRange / JSError deprecated record accessors
#[test]
fn node_range_and_record_accessors() {
    let mut ast = Ast::new();
    let start = ast.new_node(Token::EMPTY);
    let end = ast.new_node(Token::EMPTY);
    let file: Arc<dyn StaticSourceFile> = Arc::new(SourceFile::from_code("file.js", "abc\nsecond"));
    start.set_static_source_file(&mut ast, Some(file.clone()));
    end.set_static_source_file(&mut ast, Some(file));
    start.set_lineno_charno(&mut ast, 1, 2);
    start.set_length(&mut ast, 1);
    end.set_lineno_charno(&mut ast, 2, 3);
    end.set_length(&mut ast, 2);
    let error = JSError::make_with_node_range(&ast, start, end, &TYPE, &["x"]);
    assert_eq!(error.get_type(), &TYPE);
    assert_eq!(error.get_description(), error.description());
    assert_eq!(error.get_source_name(), Some("file.js"));
    assert_eq!(error.get_lineno(), 1);
    assert_eq!(error.get_line_number(), 1);
    assert_eq!(error.get_charno(), 2);
    assert_eq!(error.get_length(), 7);
    assert_eq!(error.get_node(), Some(start));
    assert_eq!(error.get_default_level(), ERROR);
    assert_eq!(error.get_requirement(), None);
    assert_eq!(error.get_node_source_offset(&ast), 2);
    assert_eq!(error, error.clone());
    let unknown = ast.new_node(Token::EMPTY);
    assert_eq!(
        JSError::make_with_node_range(&ast, start, unknown, &TYPE, &["x"]).length(),
        0
    );
    let before = JSError::make(&ast, start, &TYPE, &["x"]);
    start.set_length(&mut ast, 9);
    assert_eq!(before.length(), 1);
    assert_eq!(before.node().unwrap().get_length(&ast), 9);
}
