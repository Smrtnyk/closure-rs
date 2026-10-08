/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SortingErrorManager.java.

use crate::{
    check_level::CheckLevel, error_handler::ErrorHandler, error_manager::ErrorManager,
    js_error::JSError,
};
use closure_rhino::node::Ast;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
/// Rust adapter: reports that Java's message formatters make straight into the ErrorManager
/// while a report generator runs (`SourceMapInput#getSourceMap` called through
/// `Compiler#getSourceMapping`). Rust formatters cannot reach the (locked) manager, so they
/// queue the reports here and the generator replays them with
/// [`SortingErrorManager::report_deferred`].
pub type DeferredReports = Arc<Mutex<Vec<(CheckLevel, JSError)>>>;
pub struct SortingErrorManager {
    messages: Vec<ErrorWithLevel>,
    original_error_count: i32,
    promoted_error_count: i32,
    warning_count: i32,
    typed_percent: f64,
    pub error_report_generators: Vec<Box<dyn ErrorReportGenerator>>,
    deferred_reports: Option<DeferredReports>,
}
impl SortingErrorManager {
    // port: SortingErrorManager#SortingErrorManager
    pub fn new(error_report_generators: Vec<Box<dyn ErrorReportGenerator>>) -> Self {
        Self {
            messages: Vec::new(),
            original_error_count: 0,
            promoted_error_count: 0,
            warning_count: 0,
            typed_percent: 0.0,
            error_report_generators,
            deferred_reports: None,
        }
    }
    /// Rust adapter: the queue formatters report into while a generator runs (see
    /// [`DeferredReports`]).
    pub fn set_deferred_reports(&mut self, deferred_reports: DeferredReports) {
        self.deferred_reports = Some(deferred_reports);
    }
    /// Rust adapter: report what the formatters queued while the generator printed the copy
    /// `getSortedDiagnostics` returned. In Java those reports reach `report` during the loop,
    /// so they are counted in the summary (and seen by later generators) but not printed by
    /// the generator that triggered them.
    pub fn report_deferred(&mut self) {
        let Some(queue) = self.deferred_reports.clone() else {
            return;
        };
        let reports = std::mem::take(&mut *queue.lock().unwrap());
        for (level, error) in reports {
            self.report(level, error);
        }
    }
    // port: SortingErrorManager#getSortedDiagnostics
    pub fn get_sorted_diagnostics(&self) -> Vec<ErrorWithLevel> {
        self.messages.clone()
    }
    // port: SortingErrorManager#filterToListOf
    fn filter_to_list_of(&self, level: CheckLevel) -> Vec<JSError> {
        self.messages
            .iter()
            .filter(|e| e.level == level)
            .map(|e| e.error.clone())
            .collect()
    }
}
impl ErrorHandler for SortingErrorManager {
    // port: SortingErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        let e = ErrorWithLevel::new(error, level);
        if let Err(index) = self
            .messages
            .binary_search_by(|p| LeveledJSErrorComparator.compare(Some(p), Some(&e)).cmp(&0))
        {
            if level == CheckLevel::ERROR {
                if e.error.get_type().level == CheckLevel::ERROR {
                    self.original_error_count += 1;
                } else {
                    self.promoted_error_count += 1;
                }
            } else if level == CheckLevel::WARNING {
                self.warning_count += 1;
            }
            self.messages.insert(index, e);
        }
    }
}
impl ErrorManager for SortingErrorManager {
    // port: SortingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.original_error_count != 0
    }
    // port: SortingErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.original_error_count + self.promoted_error_count
    }
    // port: SortingErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.warning_count
    }
    // port: SortingErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError> {
        self.filter_to_list_of(CheckLevel::ERROR)
    }
    // port: SortingErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError> {
        self.filter_to_list_of(CheckLevel::WARNING)
    }
    // port: SortingErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, typed_percent: f64) {
        self.typed_percent = typed_percent;
    }
    // port: SortingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.typed_percent
    }
    // port: SortingErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        // Move generator ownership temporarily so strategies can read the manager.
        let mut generators = std::mem::take(&mut self.error_report_generators);
        for generator in &mut generators {
            generator.generate_report(self, ast);
        }
        self.error_report_generators = generators;
    }
}
pub trait ErrorReportGenerator: Send {
    // port: SortingErrorManager.ErrorReportGenerator#generateReport
    fn generate_report(&mut self, manager: &mut SortingErrorManager, ast: &Ast);
}
pub struct LeveledJSErrorComparator;
const P1_LT_P2: i32 = -1;
const P1_GT_P2: i32 = 1;
impl LeveledJSErrorComparator {
    // port: SortingErrorManager.LeveledJSErrorComparator#compare
    pub fn compare(&self, p1: Option<&ErrorWithLevel>, p2: Option<&ErrorWithLevel>) -> i32 {
        let Some(p2) = p2 else {
            return if p1.is_none() { 0 } else { P1_GT_P2 };
        };
        let p1 = p1.expect("");
        if p1.level != p2.level {
            return p2.level as i32 - p1.level as i32;
        }
        match (p1.error.source_name(), p2.error.source_name()) {
            (Some(a), Some(b)) => {
                let c = closure_rhino::java_lang::string_compare_to(a, b);
                if c != 0 {
                    return c;
                }
            }
            (None, Some(_)) => return P1_LT_P2,
            (Some(_), None) => return P1_GT_P2,
            _ => {}
        }
        let lineno1 = p1.error.get_line_number();
        let lineno2 = p2.error.get_line_number();
        if lineno1 != lineno2 {
            return lineno1.wrapping_sub(lineno2);
        } else if lineno1 < 0 && 0 <= lineno2 {
            return P1_LT_P2;
        } else if 0 <= lineno1 && lineno2 < 0 {
            return P1_GT_P2;
        }
        let charno1 = p1.error.charno();
        let charno2 = p2.error.charno();
        if charno1 != charno2 {
            return charno1.wrapping_sub(charno2);
        } else if charno1 < 0 && 0 <= charno2 {
            return P1_LT_P2;
        } else if 0 <= charno1 && charno2 < 0 {
            return P1_GT_P2;
        }
        closure_rhino::java_lang::string_compare_to(p1.error.description(), p2.error.description())
    }
}
#[derive(Clone, Debug)]
pub struct ErrorWithLevel {
    pub error: JSError,
    pub level: CheckLevel,
}
impl ErrorWithLevel {
    // port: SortingErrorManager.ErrorWithLevel#ErrorWithLevel
    pub fn new(error: JSError, level: CheckLevel) -> Self {
        Self { error, level }
    }
}
impl PartialEq for ErrorWithLevel {
    // port: SortingErrorManager.ErrorWithLevel#equals
    fn eq(&self, other: &Self) -> bool {
        self.level == other.level
            && self.error.description() == other.error.description()
            && self.error.source_name() == other.error.source_name()
            && self.error.get_line_number() == other.error.get_line_number()
            && self.error.charno() == other.error.charno()
    }
}
impl Eq for ErrorWithLevel {}
impl Hash for ErrorWithLevel {
    // port: SortingErrorManager.ErrorWithLevel#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.level.hash(state);
        self.error.description().hash(state);
        self.error.source_name().hash(state);
        self.error.get_line_number().hash(state);
        self.error.charno().hash(state);
    }
}
