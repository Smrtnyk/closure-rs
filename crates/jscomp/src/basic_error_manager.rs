/*
 * Copyright 2007 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/BasicErrorManager.java,
//   src/com/google/javascript/jscomp/SortingErrorManager.java.

use crate::{
    check_level::CheckLevel, error_handler::ErrorHandler, error_manager::ErrorManager,
    js_error::JSError, sorting_error_manager::SortingErrorManager,
};
use closure_rhino::node::Ast;
// Java's abstract base class is a trait; subclasses own its SortingErrorManager
// storage and inherit the forwarding ErrorHandler/ErrorManager implementations.
pub trait BasicErrorManager: Send {
    fn sorting_manager(&self) -> &SortingErrorManager;
    fn sorting_manager_mut(&mut self) -> &mut SortingErrorManager;
    // port: BasicErrorManager#BasicErrorManager
    fn new_base() -> SortingErrorManager
    where
        Self: Sized,
    {
        SortingErrorManager::new(Vec::new())
    }
    // port: BasicErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        for message in self.sorting_manager().get_sorted_diagnostics() {
            self.println(ast, message.level, &message.error);
        }
        self.print_summary();
    }
    // port: BasicErrorManager#println
    fn println(&mut self, ast: &Ast, level: CheckLevel, error: &JSError);
    // port: BasicErrorManager#printSummary
    fn print_summary(&mut self);
}
impl<T: BasicErrorManager> ErrorHandler for T {
    // port: SortingErrorManager#report (inherited by BasicErrorManager)
    fn report(&mut self, level: CheckLevel, error: JSError) {
        self.sorting_manager_mut().report(level, error);
    }
}
impl<T: BasicErrorManager> ErrorManager for T {
    // port: BasicErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        BasicErrorManager::generate_report(self, ast);
    }
    // port: SortingErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.sorting_manager().get_error_count()
    }
    // port: SortingErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.sorting_manager().get_warning_count()
    }
    // port: SortingErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError> {
        self.sorting_manager().get_errors()
    }
    // port: SortingErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError> {
        self.sorting_manager().get_warnings()
    }
    // port: SortingErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, value: f64) {
        self.sorting_manager_mut().set_typed_percent(value);
    }
    // port: SortingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.sorting_manager().get_typed_percent()
    }
    // port: SortingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.sorting_manager().has_halting_errors()
    }
}
