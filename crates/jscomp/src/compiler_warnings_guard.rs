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

use crate::{
    abstract_compiler::AbstractCompiler, check_level::CheckLevel,
    closure_unaware_code_warnings_guard::ClosureUnawareCodeWarningsGuard,
    compose_warnings_guard::ComposeWarningsGuard, diagnostic_groups as groups,
    j2cl_suppress_warnings_guard::J2clSuppressWarningsGuard, js_error::JSError,
    suppress_doc_warnings_guard::SuppressDocWarningsGuard, warnings_guard::WarningsGuard,
};
use std::{any::Any, fmt, sync::Arc};
/// Contextual guards take the compiler at evaluation time, avoiding a self-reference.
pub struct CompilerWarningsGuard {
    options_guard: Arc<ComposeWarningsGuard>,
    j2cl: Arc<J2clSuppressWarningsGuard>,
    closure_unaware: ClosureUnawareCodeWarningsGuard,
    suppress_doc: SuppressDocWarningsGuard,
}
impl CompilerWarningsGuard {
    pub fn new(guard: Arc<ComposeWarningsGuard>) -> Self {
        Self {
            options_guard: guard,
            j2cl: Arc::new(J2clSuppressWarningsGuard::new()),
            closure_unaware: ClosureUnawareCodeWarningsGuard::new(),
            suppress_doc: SuppressDocWarningsGuard::new(
                groups::DiagnosticGroups::get_registered_groups(),
            ),
        }
    }
    pub fn level(&self, compiler: &AbstractCompiler, error: &JSError) -> Option<CheckLevel> {
        let guards: Vec<Arc<dyn WarningsGuard>> = vec![
            self.j2cl.clone(),
            Arc::new(ComputedGuard {
                level: self.closure_unaware.level(compiler, error),
                priority: self.closure_unaware.get_priority(),
            }),
            Arc::new(ComputedGuard {
                level: self.suppress_doc.level(compiler, error),
                priority: self.suppress_doc.get_priority(),
            }),
            self.options_guard.clone(),
        ];
        ComposeWarningsGuard::new(guards).level(error)
    }
}
#[derive(Debug)]
struct ComputedGuard {
    level: Option<CheckLevel>,
    priority: i32,
}
impl WarningsGuard for ComputedGuard {
    fn level(&self, _error: &JSError) -> Option<CheckLevel> {
        self.level
    }
    fn get_priority(&self) -> i32 {
        self.priority
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for ComputedGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContextualWarningsGuard({:?})", self.level)
    }
}
