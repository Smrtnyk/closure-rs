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
use std::{
    any::Any,
    fmt,
    sync::{Arc, Mutex},
};
/// Contextual guards take the compiler at evaluation time, avoiding a self-reference.
pub struct CompilerWarningsGuard {
    options_guard: Arc<ComposeWarningsGuard>,
    j2cl: Arc<J2clSuppressWarningsGuard>,
    closure_unaware: ClosureUnawareCodeWarningsGuard,
    suppress_doc: SuppressDocWarningsGuard,
    /// Rust-only (performance divergence): the order of the composed guards, for the version of
    /// `options_guard` it was made with (`level`).
    order: Mutex<Option<(u64, Arc<GuardOrder>)>>,
}
/// The guards of the composition in the order `ComposeWarningsGuard` asks them.
struct GuardOrder {
    slots: Vec<Slot>,
    demote_errors: bool,
}
enum Slot {
    Guard(Arc<dyn WarningsGuard>),
    ClosureUnaware,
    SuppressDoc,
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
            order: Mutex::new(None),
        }
    }
    /// Java's `ComposeWarningsGuard` of the J2CL, closure-unaware-code, `@suppress` and options
    /// guards (`Compiler#initWarningsGuard`).
    ///
    /// Rust-only (performance divergence): the contextual guards need the compiler, so the
    /// composition was made again for every diagnostic, with their levels computed first. Its
    /// order depends only on the guards' priorities and the order they are added in, so it is
    /// kept while the options guard is unchanged, and the guards are asked in that order until
    /// one answers, the contextual ones included (as Java asks them; their levels only read the
    /// tree). `CLOSURE_RS_WARNINGS_GUARD_ORDER=off` composes for every diagnostic.
    pub fn level(&self, compiler: &AbstractCompiler, error: &JSError) -> Option<CheckLevel> {
        static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
            std::env::var("CLOSURE_RS_WARNINGS_GUARD_ORDER").as_deref() != Ok("off")
        });
        if !*ENABLED {
            return self.composed_level(compiler, error);
        }
        let order = self.order();
        for slot in &order.slots {
            let level = match slot {
                Slot::Guard(guard) => guard.level(error),
                Slot::ClosureUnaware => self.closure_unaware.level(compiler, error),
                Slot::SuppressDoc => self.suppress_doc.level(compiler, error),
            };
            if let Some(level) = level {
                // As `ComposeWarningsGuard#level` answers.
                return Some(if order.demote_errors && level == CheckLevel::ERROR {
                    CheckLevel::WARNING
                } else {
                    level
                });
            }
        }
        None
    }
    fn composed_level(&self, compiler: &AbstractCompiler, error: &JSError) -> Option<CheckLevel> {
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
    fn order(&self) -> Arc<GuardOrder> {
        let version = self.options_guard.version();
        let mut cached = self.order.lock().unwrap();
        if let Some((cached_version, order)) = cached.as_ref()
            && *cached_version == version
        {
            return Arc::clone(order);
        }
        // The composition `composed_level` makes, with stand-ins for the contextual guards.
        let closure_unaware: Arc<dyn WarningsGuard> = Arc::new(ComputedGuard {
            level: None,
            priority: self.closure_unaware.get_priority(),
        });
        let suppress_doc: Arc<dyn WarningsGuard> = Arc::new(ComputedGuard {
            level: None,
            priority: self.suppress_doc.get_priority(),
        });
        let composed = ComposeWarningsGuard::new(vec![
            self.j2cl.clone(),
            closure_unaware.clone(),
            suppress_doc.clone(),
            self.options_guard.clone(),
        ]);
        let same = |a: &Arc<dyn WarningsGuard>, b: &Arc<dyn WarningsGuard>| {
            std::ptr::eq(
                Arc::as_ptr(a) as *const () as *const u8,
                Arc::as_ptr(b) as *const () as *const u8,
            )
        };
        let slots = composed
            .get_guards()
            .into_iter()
            .map(|guard| {
                if same(&guard, &closure_unaware) {
                    Slot::ClosureUnaware
                } else if same(&guard, &suppress_doc) {
                    Slot::SuppressDoc
                } else {
                    Slot::Guard(guard)
                }
            })
            .collect();
        let order = Arc::new(GuardOrder {
            slots,
            demote_errors: composed.demotes_errors(),
        });
        *cached = Some((version, Arc::clone(&order)));
        order
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
