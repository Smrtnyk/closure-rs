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
//   src/com/google/javascript/jscomp/ShowByPathWarningsGuard.java.

use crate::{
    by_path_warnings_guard::ByPathWarningsGuard, check_level::CheckLevel, js_error::JSError,
    warnings_guard::WarningsGuard,
};
use std::{any::Any, fmt};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowType {
    INCLUDE,
    EXCLUDE,
}
#[derive(Debug)]
pub struct ShowByPathWarningsGuard {
    warnings_guard: ByPathWarningsGuard,
}
impl ShowByPathWarningsGuard {
    // port: ShowByPathWarningsGuard#ShowByPathWarningsGuard(String)
    pub fn new(path: &str) -> Self {
        Self::new_with_show_type(path, ShowType::INCLUDE)
    }
    // port: ShowByPathWarningsGuard#ShowByPathWarningsGuard(String[])
    pub fn new_with_paths(paths: &[&str]) -> Self {
        Self::new_with_paths_and_show_type(paths, ShowType::INCLUDE)
    }
    // port: ShowByPathWarningsGuard#ShowByPathWarningsGuard(String,ShowType)
    pub fn new_with_show_type(path: &str, show_type: ShowType) -> Self {
        Self::new_with_paths_and_show_type(&[path], show_type)
    }
    // port: ShowByPathWarningsGuard#ShowByPathWarningsGuard(String[],ShowType)
    pub fn new_with_paths_and_show_type(paths: &[&str], show_type: ShowType) -> Self {
        let paths = paths.iter().map(|s| (*s).into()).collect();
        Self {
            warnings_guard: if show_type == ShowType::INCLUDE {
                ByPathWarningsGuard::except_path(paths, CheckLevel::OFF)
            } else {
                ByPathWarningsGuard::for_path(paths, CheckLevel::OFF)
            },
        }
    }
}
impl WarningsGuard for ShowByPathWarningsGuard {
    // port: ShowByPathWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        self.warnings_guard.level(error)
    }
    // port: ShowByPathWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        self.warnings_guard.get_priority()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl fmt::Display for ShowByPathWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.ShowByPathWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}

// Typed borrowing views for java.lang.reflect.Field replay; the instance fields remain private.
macro_rules! replay_show_by_path_warnings_guard_fields {
    ($($name:ident: $ty:ty),* $(,)?) => {
        pub struct ShowByPathWarningsGuardReplayFields<'a> { $(pub $name: &'a $ty,)* }
        pub struct ShowByPathWarningsGuardReplayFieldsMut<'a> { $(pub $name: &'a mut $ty,)* }
        impl ShowByPathWarningsGuard {
            // port: java.lang.reflect.Field#get (native replay access)
            pub fn replay_fields(&self) -> ShowByPathWarningsGuardReplayFields<'_> {
                ShowByPathWarningsGuardReplayFields { $($name: &self.$name,)* }
            }
            // port: java.lang.reflect.Field#set (native replay access)
            pub fn replay_fields_mut(&mut self) -> ShowByPathWarningsGuardReplayFieldsMut<'_> {
                ShowByPathWarningsGuardReplayFieldsMut { $($name: &mut self.$name,)* }
            }
        }
    };
}
replay_show_by_path_warnings_guard_fields!(
    warnings_guard: ByPathWarningsGuard,
);
