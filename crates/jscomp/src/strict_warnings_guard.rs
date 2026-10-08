/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/StrictWarningsGuard.java.

use crate::{
    check_level::CheckLevel,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    warnings_guard::{Priority, WarningsGuard},
};
use std::{any::Any, fmt};
pub static UNRAISABLE_WARNING: DiagnosticType =
    DiagnosticType::warning("JSC_UNRAISABLE_WARNING", "{0}");
#[derive(Debug, Default)]
pub struct StrictWarningsGuard;
impl WarningsGuard for StrictWarningsGuard {
    // port: StrictWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if std::ptr::eq(error.get_type(), &UNRAISABLE_WARNING) {
            None
        } else if error.default_level().is_on() {
            Some(CheckLevel::ERROR)
        } else {
            None
        }
    }
    // port: StrictWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::STRICT.value()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for StrictWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.StrictWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}
