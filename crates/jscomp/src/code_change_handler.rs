/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CodeChangeHandler.java.

/// Rust-only (performance divergence): `report_change` takes `&self` and the handlers are shared
/// as `Arc<dyn CodeChangeHandler>`, not behind a mutex: every reported change calls every handler,
/// and a handler only records the change in a relaxed atomic (a plain store; Java writes a plain
/// field), so that a change report costs no locked instruction. Only the compiler thread reports
/// changes.
pub trait CodeChangeHandler: Send + Sync {
    // port: CodeChangeHandler#reportChange
    fn report_change(&self);
}
