/*
 * Copyright 2013 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/RecentChange.java.

use crate::code_change_handler::CodeChangeHandler;
use std::sync::atomic::{AtomicBool, Ordering};

/// Rust-only: the flag is a relaxed atomic so that the handler is shared without a mutex (see
/// `CodeChangeHandler`).
#[derive(Debug, Default)]
pub struct RecentChange {
    has_changed: AtomicBool,
}

impl CodeChangeHandler for RecentChange {
    // port: RecentChange#reportChange
    fn report_change(&self) {
        self.has_changed.store(true, Ordering::Relaxed);
    }
}

impl RecentChange {
    // port: RecentChange#hasCodeChanged
    pub fn has_code_changed(&self) -> bool {
        self.has_changed.load(Ordering::Relaxed)
    }

    // port: RecentChange#reset
    pub fn reset(&self) {
        self.has_changed.store(false, Ordering::Relaxed);
    }
}
