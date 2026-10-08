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
//   src/com/google/javascript/jscomp/ByPathWarningsGuard.java.

// Preserve the Java guard condition and null-path branch.
#![allow(clippy::collapsible_if)]
use crate::{
    check_level::CheckLevel,
    js_error::JSError,
    warnings_guard::{Priority, WarningsGuard},
};
use closure_rhino::check_argument;
use std::{any::Any, fmt};
#[derive(Debug)]
pub struct ByPathWarningsGuard {
    paths: Vec<String>,
    include: bool,
    priority: i32,
    level: CheckLevel,
}
impl ByPathWarningsGuard {
    // port: ByPathWarningsGuard#ByPathWarningsGuard
    fn new(paths: Vec<String>, include: bool, level: CheckLevel) -> Self {
        check_argument!(level == CheckLevel::OFF || level == CheckLevel::ERROR);
        Self {
            paths,
            include,
            level,
            priority: if level == CheckLevel::ERROR {
                Priority::STRICT.value()
            } else {
                Priority::FILTER_BY_PATH.value()
            },
        }
    }
    // port: ByPathWarningsGuard#forPath
    pub fn for_path(paths: Vec<String>, level: CheckLevel) -> Self {
        Self::new(paths, true, level)
    }
    // port: ByPathWarningsGuard#exceptPath
    pub fn except_path(paths: Vec<String>, level: CheckLevel) -> Self {
        Self::new(paths, false, level)
    }
}
impl WarningsGuard for ByPathWarningsGuard {
    // port: ByPathWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if error.default_level() != CheckLevel::ERROR {
            if let Some(error_path) = error.source_name() {
                let mut in_path = false;
                for path in &self.paths {
                    in_path |= error_path.contains(path);
                }
                if in_path == self.include {
                    return Some(self.level);
                }
            }
        }
        None
    }
    // port: ByPathWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        self.priority
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for ByPathWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.ByPathWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_by_path_warnings_guard_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct ByPathWarningsGuardReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct ByPathWarningsGuardReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl ByPathWarningsGuard {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> ByPathWarningsGuardReplayFields<'_> {
                ByPathWarningsGuardReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> ByPathWarningsGuardReplayFieldsMut<'_> {
                ByPathWarningsGuardReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_by_path_warnings_guard_fields!(
    paths: Vec<String>,
    include: bool,
    priority: i32,
    level: CheckLevel,
);
