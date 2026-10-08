/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/BlackHoleErrorManager.java,
//   src/com/google/javascript/jscomp/SortingErrorManager.java.

use crate::{
    check_level::CheckLevel, error_handler::ErrorHandler, error_manager::ErrorManager,
    js_error::JSError, sorting_error_manager::SortingErrorManager,
};
use closure_rhino::node::Ast;
pub struct BlackHoleErrorManager {
    manager: SortingErrorManager,
}
impl BlackHoleErrorManager {
    // port: BlackHoleErrorManager#BlackHoleErrorManager
    pub fn new() -> Self {
        Self {
            manager: SortingErrorManager::new(Vec::new()),
        }
    }
}
impl Default for BlackHoleErrorManager {
    // port: BlackHoleErrorManager#BlackHoleErrorManager
    fn default() -> Self {
        Self::new()
    }
}
impl ErrorHandler for BlackHoleErrorManager {
    // port: SortingErrorManager#report
    fn report(&mut self, level: CheckLevel, error: JSError) {
        self.manager.report(level, error);
    }
}
impl ErrorManager for BlackHoleErrorManager {
    // port: SortingErrorManager#generateReport
    fn generate_report(&mut self, ast: &Ast) {
        self.manager.generate_report(ast)
    }
    // port: SortingErrorManager#getErrorCount
    fn get_error_count(&self) -> i32 {
        self.manager.get_error_count()
    }
    // port: SortingErrorManager#getWarningCount
    fn get_warning_count(&self) -> i32 {
        self.manager.get_warning_count()
    }
    // port: SortingErrorManager#getErrors
    fn get_errors(&self) -> Vec<JSError> {
        self.manager.get_errors()
    }
    // port: SortingErrorManager#getWarnings
    fn get_warnings(&self) -> Vec<JSError> {
        self.manager.get_warnings()
    }
    // port: SortingErrorManager#setTypedPercent
    fn set_typed_percent(&mut self, value: f64) {
        self.manager.set_typed_percent(value)
    }
    // port: SortingErrorManager#getTypedPercent
    fn get_typed_percent(&self) -> f64 {
        self.manager.get_typed_percent()
    }
    // port: SortingErrorManager#hasHaltingErrors
    fn has_halting_errors(&self) -> bool {
        self.manager.has_halting_errors()
    }
}
