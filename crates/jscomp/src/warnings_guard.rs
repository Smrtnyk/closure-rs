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
//   src/com/google/javascript/jscomp/WarningsGuard.java.

use crate::{check_level::CheckLevel, diagnostic_group::DiagnosticGroup, js_error::JSError};
use closure_rhino::jscomp_base::Tri;
use std::{
    any::Any,
    fmt::{Debug, Display},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    MAX,
    MIN,
    STRICT,
    DEFAULT,
    SUPPRESS_BY_ALLOWLIST,
    SUPPRESS_DOC,
    FILTER_BY_PATH,
}
impl Priority {
    // port: WarningsGuard.Priority#Priority
    pub const fn value(self) -> i32 {
        match self {
            Self::MAX | Self::FILTER_BY_PATH => 1,
            Self::MIN | Self::STRICT => 100,
            Self::DEFAULT => 50,
            Self::SUPPRESS_BY_ALLOWLIST => 40,
            Self::SUPPRESS_DOC => 20,
        }
    }
    // port: WarningsGuard.Priority#getValue
    pub const fn get_value(self) -> i32 {
        self.value()
    }
}
pub trait WarningsGuard: Send + Sync + Display + Debug {
    // port: WarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel>;
    // port: WarningsGuard#mustRunChecks
    fn must_run_checks(&self, _group: &DiagnosticGroup) -> Tri {
        Tri::UNKNOWN
    }
    // port: WarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::DEFAULT.value()
    }
    fn as_any(&self) -> &dyn Any;
}
