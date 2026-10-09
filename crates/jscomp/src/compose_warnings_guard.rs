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
//   src/com/google/javascript/jscomp/ComposeWarningsGuard.java.

use crate::{
    check_level::CheckLevel, diagnostic_group::DiagnosticGroup, js_error::JSError,
    warnings_guard::WarningsGuard,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_state, jscomp_base::Tri};
use std::{
    any::Any,
    fmt,
    sync::{Arc, Mutex},
};
#[derive(Debug)]
struct State {
    order_of_addition: IndexMap<usize, i32>,
    number_of_adds: i32,
    demote_errors: bool,
    guards: Vec<Arc<dyn WarningsGuard>>,
}
#[derive(Debug)]
pub struct ComposeWarningsGuard {
    state: Mutex<State>,
}
impl ComposeWarningsGuard {
    // port: ComposeWarningsGuard#ComposeWarningsGuard(List<WarningsGuard>)
    // port: ComposeWarningsGuard#ComposeWarningsGuard(WarningsGuard...)
    pub fn new(guards: Vec<Arc<dyn WarningsGuard>>) -> Self {
        let result = Self {
            state: Mutex::new(State {
                order_of_addition: IndexMap::<_, _>::default(),
                number_of_adds: 0,
                demote_errors: false,
                guards: Vec::new(),
            }),
        };
        result.add_guards(guards);
        result
    }
    // port: ComposeWarningsGuard#addGuard
    pub fn add_guard(&self, guard: Arc<dyn WarningsGuard>) {
        if let Some(compose_guard) = guard.as_any().downcast_ref::<Self>() {
            // Snapshot before taking our lock also preserves addGuard(this).
            let (demote, guards) = {
                let state = compose_guard.state.lock().unwrap();
                (
                    state.demote_errors,
                    state.guards.iter().rev().cloned().collect::<Vec<_>>(),
                )
            };
            if demote {
                self.state.lock().unwrap().demote_errors = true;
            }
            self.add_guards(guards);
        } else {
            let mut state = self.state.lock().unwrap();
            state.number_of_adds = state.number_of_adds.wrapping_add(1);
            let number = state.number_of_adds;
            state.order_of_addition.insert(guard_id(&guard), number);
            let comparator = GuardComparator::new(&state.order_of_addition);
            if let Some(index) = state
                .guards
                .iter()
                .position(|g| comparator.compare(g, &guard) == 0)
            {
                state.guards.remove(index);
            }
            let comparator = GuardComparator::new(&state.order_of_addition);
            let index = state
                .guards
                .iter()
                .position(|g| comparator.compare(g, &guard) > 0)
                .unwrap_or(state.guards.len());
            state.guards.insert(index, guard);
        }
    }
    // port: ComposeWarningsGuard#addGuards
    fn add_guards(&self, guards: impl IntoIterator<Item = Arc<dyn WarningsGuard>>) {
        for guard in guards {
            self.add_guard(guard);
        }
    }
    // port: ComposeWarningsGuard#disables
    fn disables(&self, group: &DiagnosticGroup) -> bool {
        'next_singleton: for type_ in group.get_types() {
            let singleton = DiagnosticGroup::for_type(type_);
            for guard in self.get_guards() {
                match guard.must_run_checks(&singleton) {
                    Tri::TRUE => return false,
                    Tri::FALSE => continue 'next_singleton,
                    Tri::UNKNOWN => {}
                }
            }
            return false;
        }
        true
    }
    // port: ComposeWarningsGuard#enables
    fn enables(&self, group: &DiagnosticGroup) -> bool {
        for guard in self.get_guards() {
            match guard.must_run_checks(group) {
                Tri::TRUE => return true,
                Tri::FALSE => return false,
                Tri::UNKNOWN => {}
            }
        }
        false
    }
    // port: ComposeWarningsGuard#getGuards
    pub fn get_guards(&self) -> Vec<Arc<dyn WarningsGuard>> {
        self.state.lock().unwrap().guards.clone()
    }
}
impl WarningsGuard for ComposeWarningsGuard {
    // port: ComposeWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        let demote = self.state.lock().unwrap().demote_errors;
        for guard in self.get_guards() {
            if let Some(new_level) = guard.level(error) {
                return Some(if demote && new_level == CheckLevel::ERROR {
                    CheckLevel::WARNING
                } else {
                    new_level
                });
            }
        }
        None
    }
    // port: ComposeWarningsGuard#mustRunChecks
    fn must_run_checks(&self, group: &DiagnosticGroup) -> Tri {
        let enable = self.enables(group);
        let disable = self.disables(group);
        check_state!(!enable || !disable, "%s applied to %s", self, group);
        if enable {
            Tri::TRUE
        } else if disable {
            Tri::FALSE
        } else {
            Tri::UNKNOWN
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for ComposeWarningsGuard {
    // port: ComposeWarningsGuard#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &self
                .get_guards()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}
// port: Object#equals (guard identity)
fn guard_id(guard: &Arc<dyn WarningsGuard>) -> usize {
    Arc::as_ptr(guard) as *const () as usize
}
pub struct GuardComparator<'a> {
    order_of_addition: &'a IndexMap<usize, i32>,
}
impl<'a> GuardComparator<'a> {
    // port: ComposeWarningsGuard.GuardComparator#GuardComparator
    pub fn new(order_of_addition: &'a IndexMap<usize, i32>) -> Self {
        Self { order_of_addition }
    }
    // port: ComposeWarningsGuard.GuardComparator#compare
    pub fn compare(&self, a: &Arc<dyn WarningsGuard>, b: &Arc<dyn WarningsGuard>) -> i32 {
        let priority_diff = a.get_priority().wrapping_sub(b.get_priority());
        if priority_diff != 0 {
            priority_diff
        } else {
            self.order_of_addition[&guard_id(b)].wrapping_sub(self.order_of_addition[&guard_id(a)])
        }
    }
}
