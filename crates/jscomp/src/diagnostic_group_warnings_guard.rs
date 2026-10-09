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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/DiagnosticGroupWarningsGuard.java.

use crate::{
    check_level::CheckLevel, diagnostic_group::DiagnosticGroup, js_error::JSError,
    warnings_guard::WarningsGuard,
};
use closure_rhino::jscomp_base::Tri;
use std::{any::Any, fmt, sync::Arc};
#[derive(Debug)]
pub struct DiagnosticGroupWarningsGuard {
    group: Arc<DiagnosticGroup>,
    level: CheckLevel,
}
impl DiagnosticGroupWarningsGuard {
    // port: DiagnosticGroupWarningsGuard#DiagnosticGroupWarningsGuard
    pub fn new(group: Arc<DiagnosticGroup>, level: CheckLevel) -> Self {
        Self { group, level }
    }
}
impl WarningsGuard for DiagnosticGroupWarningsGuard {
    // port: DiagnosticGroupWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        self.group.matches(error).then_some(self.level)
    }
    // port: DiagnosticGroupWarningsGuard#mustRunChecks
    fn must_run_checks(&self, other_group: &DiagnosticGroup) -> Tri {
        if self.level.is_on() {
            if other_group
                .get_types()
                .iter()
                .any(|t| self.group.matches_type(t))
            {
                Tri::TRUE
            } else {
                Tri::UNKNOWN
            }
        } else if other_group
            .get_types()
            .iter()
            .all(|t| self.group.matches_type(t))
        {
            Tri::FALSE
        } else {
            Tri::UNKNOWN
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for DiagnosticGroupWarningsGuard {
    // port: DiagnosticGroupWarningsGuard#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.group, self.level)
    }
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_diagnostic_group_warnings_guard_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct DiagnosticGroupWarningsGuardReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct DiagnosticGroupWarningsGuardReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl DiagnosticGroupWarningsGuard {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> DiagnosticGroupWarningsGuardReplayFields<'_> {
                DiagnosticGroupWarningsGuardReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> DiagnosticGroupWarningsGuardReplayFieldsMut<'_> {
                DiagnosticGroupWarningsGuardReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_diagnostic_group_warnings_guard_fields!(
    group: Arc<DiagnosticGroup>,
    level: CheckLevel,
);
